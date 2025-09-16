use color_eyre::eyre::OptionExt;
use futures::{FutureExt, StreamExt};
use minesweeper_client::{GameEvent, GameParams, Pos};
use ratatui::crossterm::event::Event as CrosstermEvent;
use tokio::sync::mpsc;

/// Representation of all possible events.
#[derive(Clone, Debug)]
pub enum Event {
    /// Crossterm events.
    ///
    /// These events are emitted by the terminal.
    Crossterm(CrosstermEvent),
    /// Application events.
    ///
    /// Use this event to emit custom events that are specific to your application.
    App(AppEvent),
    /// Game events.
    ///
    /// Events received from the minesweeper server.
    Game(GameEvent),
}

/// Application events.
#[derive(Clone, Debug)]
pub enum AppEvent {
    Start(GameParams),
    Join(String),
    Reveal(Pos),
    Flag(Pos),
    Restart(GameParams),
    KeyAction(KeyAction),
    /// Quit the application.
    Quit,
}

#[derive(Clone, Debug)]
pub enum KeyAction {
    Left,
    Right,
    Up,
    Down,
    Accept,
    Cancel,
    Space,
    Backspace,
    Digit(char),
    Input(char),
    Settings,
    JoinMenu,
}

/// Terminal event handler.
pub struct EventHandler {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
    /// Event receiver channel.
    receiver: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`] and spawns a new thread to handle events.
    pub fn new(game_events: mpsc::UnboundedReceiver<GameEvent>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let actor = EventTask::new(game_events, sender.clone());
        tokio::spawn(async { actor.run().await });
        Self { sender, receiver }
    }

    /// Receives an event from the sender.
    ///
    /// This function blocks until an event is received.
    ///
    /// # Errors
    ///
    /// This function returns an error if the sender channel is disconnected. This can happen if an
    /// error occurs in the event thread. In practice, this should not happen unless there is a
    /// problem with the underlying terminal.
    pub async fn next(&mut self) -> color_eyre::Result<Event> {
        self.receiver
            .recv()
            .await
            .ok_or_eyre("Failed to receive event")
    }

    /// Queue an app event to be sent to the event receiver.
    ///
    /// This is useful for sending events to the event handler which will be processed by the next
    /// iteration of the application's event loop.
    pub fn send(&mut self, app_event: AppEvent) {
        // Ignore the result as the reciever cannot be dropped while this struct still has a
        // reference to it
        let _ = self.sender.send(Event::App(app_event));
    }
}

/// A thread that handles reading crossterm events and emitting tick events on a regular schedule.
struct EventTask {
    /// Game event receive channel.
    game_events: mpsc::UnboundedReceiver<GameEvent>,
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
}

impl EventTask {
    /// Constructs a new instance of [`EventThread`].
    fn new(
        game_events: mpsc::UnboundedReceiver<GameEvent>,
        sender: mpsc::UnboundedSender<Event>,
    ) -> Self {
        Self {
            game_events,
            sender,
        }
    }

    /// Runs the event thread.
    ///
    /// This function emits tick events at a fixed rate and polls for crossterm events in between.
    async fn run(mut self) -> color_eyre::Result<()> {
        let mut reader = crossterm::event::EventStream::new();
        loop {
            let crossterm_event = reader.next().fuse();
            let game_event = self.game_events.recv().fuse();
            tokio::select! {
                _ = self.sender.closed() => {
                    break;
                }
                Some(Ok(evt)) = crossterm_event => {
                    self.send(Event::Crossterm(evt));
                }
                Some(evt) = game_event => {
                    self.send(Event::Game(evt));
                }
            };
        }
        Ok(())
    }

    /// Sends an event to the receiver.
    fn send(&self, event: Event) {
        // Ignores the result because shutting down the app drops the receiver, which causes the send
        // operation to fail. This is expected behavior and should not panic.
        let _ = self.sender.send(event);
    }
}

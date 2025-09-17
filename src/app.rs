use std::vec;

use crate::event::{AppEvent, Event, EventHandler, KeyAction};
use color_eyre::eyre::eyre;
use minesweeper_client::{Cell, GameParams, MinesweeperGame, Pos};
use ratatui::{
    DefaultTerminal,
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Clear, List, ListItem, Paragraph, Widget},
};

/// Application.
pub struct App {
    /// Is the application running?
    pub running: bool,
    pub menu_state: MenuState,
    pub input_buffer: String,
    pub params: GameParams,
    /// Game instance.
    pub game: MinesweeperGame,
    pub selected: Pos,
    pub events: EventHandler,
}

#[derive(PartialEq, Eq)]
pub enum MenuState {
    Closed,
    Parameters {
        modify: bool,
        selection: ParameterSelections,
    },
    Join,
    InfoMessage(Line<'static>),
}

#[derive(PartialEq, Eq)]
pub enum ParameterSelections {
    Width,
    Height,
    Bombs,
    Apply,
    Cancel,
}

impl App {
    /// Constructs a new instance of [`App`].
    pub async fn new() -> color_eyre::Result<Self> {
        let game = MinesweeperGame::new("https://api.minesweeper.mineplay.link")
            .map_err(|_err| eyre!("Invalid server url"))?;

        game.start_game(GameParams::default())
            .await
            .map_err(|err| eyre!("Error while connecting to server: {}", err))?;

        let game_events = game.subscribe_to_events().await;
        Ok(Self {
            running: true,
            menu_state: MenuState::Closed,
            input_buffer: "".to_string(),
            params: GameParams::default(),
            game,
            selected: Pos { x: 0, y: 0 },
            events: EventHandler::new(game_events),
        })
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        while self.running {
            self.render(&mut terminal).await?;
            match self.events.next().await? {
                Event::Crossterm(event) => {
                    if let crossterm::event::Event::Key(key_event) = event {
                        self.handle_key_events(key_event)?
                    }
                }
                Event::App(app_event) => match app_event {
                    AppEvent::Start(params) => {
                        self.game
                            .start_game(params)
                            .await
                            .map_err(|err| eyre!("Failed to send request: {}", err))?;
                    }
                    AppEvent::Join(id) => {
                        self.game
                            .join_game(id)
                            .await
                            .map_err(|err| eyre!("Failed to send request: {}", err))?;
                    }
                    AppEvent::Reveal(pos) => {
                        self.game
                            .reveal(pos)
                            .await
                            .map_err(|err| eyre!("Failed to send request: {}", err))?;
                    }
                    AppEvent::Flag(pos) => {
                        self.game
                            .flag(pos)
                            .await
                            .map_err(|err| eyre!("Failed to send request: {}", err))?;
                    }
                    AppEvent::Restart(params) => {
                        self.game
                            .restart(params)
                            .await
                            .map_err(|err| eyre!("Failed to send request: {}", err))?;
                    }
                    AppEvent::KeyAction(action) => match action {
                        KeyAction::Left => match &mut self.menu_state {
                            MenuState::Closed => self.modify_selection(-1, 0).await,
                            MenuState::Parameters { selection, modify } => match selection {
                                _ if *modify => {}
                                ParameterSelections::Width => {
                                    self.params.width = self.params.width.saturating_sub(1)
                                }
                                ParameterSelections::Height => {
                                    self.params.height = self.params.height.saturating_sub(1)
                                }
                                ParameterSelections::Bombs => {
                                    self.params.bombs = self.params.bombs.saturating_sub(1)
                                }
                                ParameterSelections::Cancel => {
                                    *selection = ParameterSelections::Apply
                                }
                                _ => {}
                            },
                            _ => {}
                        },
                        KeyAction::Right => match &mut self.menu_state {
                            MenuState::Closed => self.modify_selection(1, 0).await,
                            MenuState::Parameters { selection, modify } => match selection {
                                _ if *modify => {}
                                ParameterSelections::Width => {
                                    self.params.width = self.params.width.saturating_add(1)
                                }
                                ParameterSelections::Height => {
                                    self.params.height = self.params.height.saturating_add(1)
                                }
                                ParameterSelections::Bombs => {
                                    self.params.bombs = self.params.bombs.saturating_add(1)
                                }
                                ParameterSelections::Apply => {
                                    *selection = ParameterSelections::Cancel
                                }
                                _ => {}
                            },
                            _ => {}
                        },
                        KeyAction::Up => match &mut self.menu_state {
                            MenuState::Closed => self.modify_selection(0, -1).await,
                            MenuState::Parameters { selection, modify } => match selection {
                                _ if *modify => {}
                                ParameterSelections::Height => {
                                    *selection = ParameterSelections::Width
                                }
                                ParameterSelections::Bombs => {
                                    *selection = ParameterSelections::Height
                                }
                                ParameterSelections::Apply => {
                                    *selection = ParameterSelections::Bombs
                                }
                                ParameterSelections::Cancel => {
                                    *selection = ParameterSelections::Bombs
                                }
                                _ => {}
                            },
                            _ => {}
                        },
                        KeyAction::Down => match &mut self.menu_state {
                            MenuState::Closed => self.modify_selection(0, 1).await,
                            MenuState::Parameters { selection, modify } => match selection {
                                _ if *modify => {}
                                ParameterSelections::Width => {
                                    *selection = ParameterSelections::Height
                                }
                                ParameterSelections::Height => {
                                    *selection = ParameterSelections::Bombs
                                }
                                ParameterSelections::Bombs => {
                                    *selection = ParameterSelections::Apply
                                }
                                _ => {}
                            },
                            _ => {}
                        },
                        KeyAction::Accept => match &mut self.menu_state {
                            MenuState::Closed => {
                                if self.game.is_connected().await {
                                    self.events.send(AppEvent::Reveal(self.selected))
                                }
                            }
                            MenuState::Parameters { modify, selection } => match selection {
                                ParameterSelections::Width => {
                                    if *modify {
                                        if let Ok(value) = self.input_buffer.parse() {
                                            self.params.width = value
                                        }
                                    } else {
                                        self.input_buffer = self.params.width.to_string()
                                    }
                                    *modify = !*modify
                                }
                                ParameterSelections::Height => {
                                    if *modify {
                                        if let Ok(value) = self.input_buffer.parse() {
                                            self.params.height = value
                                        }
                                    } else {
                                        self.input_buffer = self.params.height.to_string()
                                    }
                                    *modify = !*modify
                                }
                                ParameterSelections::Bombs => {
                                    if *modify {
                                        if let Ok(value) = self.input_buffer.parse() {
                                            self.params.bombs = value
                                        }
                                    } else {
                                        self.input_buffer = self.params.bombs.to_string()
                                    }
                                    *modify = !*modify
                                }
                                ParameterSelections::Apply => {
                                    if self.game.is_connected().await {
                                        self.events.send(AppEvent::Restart(self.params))
                                    } else {
                                        self.events.send(AppEvent::Start(self.params))
                                    }
                                    self.menu_state = MenuState::Closed
                                }
                                ParameterSelections::Cancel => self.menu_state = MenuState::Closed,
                            },
                            MenuState::Join => {
                                match &self.game.join_game(self.input_buffer.clone()).await {
                                    Ok(_) => self.menu_state = MenuState::Closed,
                                    Err(_) => {
                                        self.menu_state = MenuState::InfoMessage(
                                            Span::raw("Invalide ID").fg(Color::Red).into(),
                                        )
                                    }
                                }
                            }
                            MenuState::InfoMessage(_) => self.menu_state = MenuState::Closed,
                        },
                        KeyAction::Cancel => match &mut self.menu_state {
                            MenuState::Closed => self.events.send(AppEvent::Quit),
                            MenuState::Parameters { modify, .. } if *modify => *modify = false,
                            _ => self.menu_state = MenuState::Closed,
                        },
                        KeyAction::Space => {
                            if self.menu_state == MenuState::Closed {
                                self.events.send(AppEvent::Flag(self.selected))
                            }
                        }
                        KeyAction::Backspace => match &self.menu_state {
                            MenuState::Parameters { modify: true, .. } | MenuState::Join => {
                                self.input_buffer.pop();
                            }
                            _ => {}
                        },
                        KeyAction::Digit(d) => {
                            if let MenuState::Parameters { modify: true, .. } = self.menu_state {
                                self.input_buffer.push(d);
                            }
                        }
                        KeyAction::Input(c) => {
                            if self.menu_state == MenuState::Join {
                                self.input_buffer.push(c);
                            }
                        }
                        KeyAction::Parameters => match &self.menu_state {
                            MenuState::Closed => {
                                self.menu_state = MenuState::Parameters {
                                    modify: false,
                                    selection: ParameterSelections::Width,
                                }
                            }
                            MenuState::Parameters {
                                modify: false,
                                selection: _,
                            } => self.menu_state = MenuState::Closed,
                            _ => {}
                        },
                        KeyAction::JoinMenu => match &self.menu_state {
                            MenuState::Closed => {
                                self.menu_state = MenuState::Join;
                                self.input_buffer = "".to_string()
                            }
                            MenuState::Join => self.menu_state = MenuState::Closed,
                            _ => {}
                        },
                        KeyAction::Restart => match &self.menu_state {
                            MenuState::Closed | MenuState::InfoMessage(_) => {
                                if self.game.is_connected().await {
                                    self.events.send(AppEvent::Restart(self.params))
                                } else {
                                    self.events.send(AppEvent::Start(self.params))
                                }
                                self.menu_state = MenuState::Closed
                            }
                            _ => (),
                        },
                    },
                    AppEvent::Quit => self.quit(),
                },
                Event::Game(game_event) => match game_event {
                    minesweeper_client::GameEvent::BoardUpdated { .. } => {
                        // do nothing, just redraw
                    }
                    minesweeper_client::GameEvent::GameStatusChanged { won, lost } => {
                        if won {
                            let span = Span::raw("You won").fg(Color::Green).bg(Color::Black);
                            self.menu_state = MenuState::InfoMessage(span.into())
                        } else if lost {
                            let span = Span::raw("You lost").fg(Color::Red).bg(Color::Black);
                            self.menu_state = MenuState::InfoMessage(span.into())
                        }
                    }
                    minesweeper_client::GameEvent::GameInitialized {
                        width,
                        height,
                        bombs,
                    } => {
                        self.params = GameParams {
                            width,
                            height,
                            bombs,
                        }
                    }
                    minesweeper_client::GameEvent::ConnectionLost => {
                        self.quit();
                    }
                },
            }
        }
        Ok(())
    }

    /// Handles the key events and updates the state of [`App`].
    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        match key_event.code {
            KeyCode::Char('c' | 'C') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(AppEvent::Quit)
            }
            KeyCode::Char(c) if self.menu_state == MenuState::Join => {
                self.events.send(AppEvent::KeyAction(KeyAction::Input(c)));
            }
            KeyCode::Left | KeyCode::Char('a' | 'h') => {
                self.events.send(AppEvent::KeyAction(KeyAction::Left))
            }
            KeyCode::Right | KeyCode::Char('d' | 'l') => {
                self.events.send(AppEvent::KeyAction(KeyAction::Right))
            }
            KeyCode::Up | KeyCode::Char('w' | 'k') => {
                self.events.send(AppEvent::KeyAction(KeyAction::Up))
            }
            KeyCode::Down | KeyCode::Char('s' | 'j') => {
                self.events.send(AppEvent::KeyAction(KeyAction::Down))
            }
            KeyCode::Enter => self.events.send(AppEvent::KeyAction(KeyAction::Accept)),
            KeyCode::Esc | KeyCode::Char('q') => {
                self.events.send(AppEvent::KeyAction(KeyAction::Cancel))
            }
            KeyCode::Char(' ') => self.events.send(AppEvent::KeyAction(KeyAction::Space)),
            KeyCode::Backspace => self.events.send(AppEvent::KeyAction(KeyAction::Backspace)),
            KeyCode::Char('p') => self.events.send(AppEvent::KeyAction(KeyAction::Parameters)),
            KeyCode::Char('g') => self.events.send(AppEvent::KeyAction(KeyAction::JoinMenu)),
            KeyCode::Char('r') => self.events.send(AppEvent::KeyAction(KeyAction::Restart)),
            KeyCode::Char(c) => {
                if c.is_ascii_digit() {
                    self.events.send(AppEvent::KeyAction(KeyAction::Digit(c)))
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn modify_selection(&mut self, dx: isize, dy: isize) {
        let x = self.selected.x as isize + dx;
        let y = self.selected.y as isize + dy;
        let state = self.game.get_state().await;
        let max_x = state.clone().map_or_else(|| 0, |s| s.width - 1) as isize;
        let max_y = state.map_or_else(|| 0, |s| s.height - 1) as isize;
        self.selected = Pos {
            x: x.clamp(0, max_x) as usize,
            y: y.clamp(0, max_y) as usize,
        }
    }

    async fn render(&self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        let game = self.render_game().await;
        let game_id = self.game.get_game_id().await;

        terminal.draw(|frame| {
            let block = Block::bordered()
                .title("Minesweeper")
                .fg(Color::White)
                .bg(Color::Black)
                .title_alignment(Alignment::Center)
                .border_type(BorderType::Rounded);

            let [_, game_area, _, info_area] = Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(game.lines.len() as u16),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .areas(block.inner(frame.area()));

            block.render(frame.area(), frame.buffer_mut());

            let paragraph = Paragraph::new(game)
                .fg(Color::White)
                .bg(Color::Black)
                .centered();

            paragraph.render(game_area, frame.buffer_mut());

            let mut spans = vec![
                Span::raw("game id: ").fg(Color::Gray),
                Span::raw(game_id.unwrap_or("".to_string())).fg(Color::Cyan),
            ];

            self.add_keybinding_hints(&mut spans);

            let line = Line::from(spans).centered();
            line.render(info_area, frame.buffer_mut());

            match &self.menu_state {
                MenuState::Parameters { modify, selection } => {
                    self.render_params_popup(
                        popup_area(frame.area(), 20, 9),
                        frame.buffer_mut(),
                        *modify,
                        selection,
                    );
                }
                MenuState::Join => {
                    self.render_join_popup(popup_area(frame.area(), 20, 3), frame.buffer_mut())
                }
                MenuState::InfoMessage(msg) => self.render_message_popup(
                    popup_area(frame.area(), 40, 3),
                    frame.buffer_mut(),
                    msg,
                ),
                _ => (),
            }
        })?;
        Ok(())
    }

    fn add_keybinding_hints(&self, spans: &mut Vec<Span<'static>>) {
        spans.extend_from_slice(&[]);
        match &self.menu_state {
            MenuState::Closed => {
                Self::add_keybinding_hint(spans, "←↓↑→", "move selection");
                Self::add_keybinding_hint(spans, "↵", "reveal");
                Self::add_keybinding_hint(spans, "space", "mark");
                Self::add_keybinding_hint(spans, "p", "parameters");
                Self::add_keybinding_hint(spans, "g", "join game");
                Self::add_keybinding_hint(spans, "r", "restart");
                Self::add_keybinding_hint(spans, "q", "quit");
            }
            MenuState::Parameters { modify, selection } => match selection {
                ParameterSelections::Apply => {
                    Self::add_keybinding_hint(spans, "←↓↑→", "move selection");
                    Self::add_keybinding_hint(spans, "↵", "apply");
                }
                ParameterSelections::Cancel => {
                    Self::add_keybinding_hint(spans, "←↓↑→", "move selection");
                    Self::add_keybinding_hint(spans, "↵", "cancel");
                }
                _ if *modify => {
                    Self::add_keybinding_hint(spans, "esc", "cancel");
                    Self::add_keybinding_hint(spans, "↵", "accept value");
                }
                _ => {
                    Self::add_keybinding_hint(spans, "↓↑", "move selection");
                    Self::add_keybinding_hint(spans, "←", "decrease value");
                    Self::add_keybinding_hint(spans, "↵", "modify value");
                    Self::add_keybinding_hint(spans, "→", "increase value");
                }
            },
            MenuState::Join => {
                Self::add_keybinding_hint(spans, "esc", "cancel");
                Self::add_keybinding_hint(spans, "↵", "join game");
            }
            MenuState::InfoMessage(_) => {
                Self::add_keybinding_hint(spans, "r", "restart");
                Self::add_keybinding_hint(spans, "↵", "ok");
            }
        }
    }

    fn add_keybinding_hint(
        spans: &mut Vec<Span<'static>>,
        key: &'static str,
        action: &'static str,
    ) {
        spans.extend_from_slice(&[
            Span::raw(" | ").fg(Color::Gray),
            Span::raw(key).fg(Color::Cyan),
            Span::raw(" ").fg(Color::Gray),
            Span::raw(action).fg(Color::Gray),
        ]);
    }

    async fn render_game(&self) -> Text<'_> {
        match self.game.get_state().await {
            Some(state) => {
                let rows = state.board.iter().enumerate().map(|(y, row)| {
                    let cells = row.iter().enumerate().flat_map(|(x, cell)| {
                        let mut cell = match *cell {
                            Cell::Hidden => Span::raw("·").fg(Color::Reset),
                            Cell::Marked => Span::raw("M").fg(Color::Yellow),
                            Cell::Flagged => Span::raw("F").fg(Color::Red),
                            Cell::Revealed { adjacent } => {
                                Span::raw(adjacent.to_string()).fg(Color::Reset)
                            }
                            Cell::Bomb => Span::raw("X").fg(Color::Red),
                        };
                        if self.selected.x == x && self.selected.y == y {
                            cell = cell.fg(Color::Black).bg(Color::Cyan)
                        }
                        if x == 0 {
                            vec![cell]
                        } else {
                            vec![Span::raw(" "), cell]
                        }
                    });
                    Line::from(cells.collect::<Vec<_>>())
                });
                Text::from(rows.collect::<Vec<_>>())
            }
            None => "".into(),
        }
    }

    fn render_params_popup(
        &self,
        area: Rect,
        buf: &mut Buffer,
        modify: bool,
        selection: &ParameterSelections,
    ) {
        Clear.render(area, buf);

        let block = Block::bordered()
            .title("Parameters")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .fg(Color::Cyan)
            .bg(Color::Black);

        let inner_area = block.inner(area);
        block.render(area, buf);

        let [list_area, buttons_area] =
            Layout::vertical([Constraint::Length(6), Constraint::Length(1)]).areas(inner_area);

        let list = List::new(vec![
            self.get_params_popup_item(
                "Width",
                self.params.width,
                modify,
                *selection == ParameterSelections::Width,
                list_area.width,
            ),
            self.get_params_popup_item(
                "Height",
                self.params.height,
                modify,
                *selection == ParameterSelections::Height,
                list_area.width,
            ),
            self.get_params_popup_item(
                "Bombs",
                self.params.bombs,
                modify,
                *selection == ParameterSelections::Bombs,
                list_area.width,
            ),
        ])
        .fg(Color::Reset);

        list.render(list_area, buf);

        let [button_left, button_right] =
            Layout::horizontal([Constraint::Fill(1); 2]).areas(buttons_area);

        let mut apply = Paragraph::new("Apply").centered().fg(Color::Reset);
        if *selection == ParameterSelections::Apply {
            apply = apply.fg(Color::Black).bg(Color::Cyan)
        };

        apply.render(button_left, buf);

        let mut cancel = Paragraph::new("Cancel").centered().fg(Color::Reset);
        if *selection == ParameterSelections::Cancel {
            cancel = cancel.fg(Color::Black).bg(Color::Cyan)
        };

        cancel.render(button_right, buf);
    }

    fn get_params_popup_item(
        &self,
        name: &'static str,
        value: usize,
        modify: bool,
        selected: bool,
        width: u16,
    ) -> ListItem<'static> {
        let modify = selected && modify;

        let header_spacing = width as usize - name.len();
        let header = format!(
            "{}{}{}",
            " ".repeat(header_spacing - header_spacing / 2),
            name,
            " ".repeat(header_spacing / 2)
        );
        let header = Span::raw(header).bold();
        let value = if modify {
            format!("{}_", self.input_buffer)
        } else {
            format!("{}", value)
        };
        let value_spacing = width as usize - value.len() - 2;
        let value = if modify {
            format!(
                " {}{}{}↵",
                " ".repeat(value_spacing - value_spacing / 2),
                value,
                " ".repeat(value_spacing / 2)
            )
        } else if selected {
            format!(
                "←{}{}{}↵ →",
                " ".repeat(value_spacing - value_spacing / 2),
                value,
                " ".repeat(value_spacing / 2 - 2)
            )
        } else {
            format!(
                " {}{}{} ",
                " ".repeat(value_spacing - value_spacing / 2),
                value,
                " ".repeat(value_spacing / 2)
            )
        };
        let value = Span::raw(value);
        let mut text = Text::default();
        text.push_line(header);
        text.push_line(value);
        if selected {
            text = text.fg(Color::Black).bg(Color::Cyan)
        };
        ListItem::new(text)
    }

    fn render_join_popup(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let block = Block::bordered()
            .title("Join Game")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .fg(Color::Cyan)
            .bg(Color::Black);

        let inner_area = block.inner(area);
        block.render(area, buf);

        let prefix = Span::raw("ID: ").fg(Color::Gray);
        let spacing = inner_area.width as usize - self.input_buffer.len() - 6;
        let value = format!("{}_{}↵", self.input_buffer, " ".repeat(spacing));
        let value = Span::raw(value).fg(Color::Reset);
        let line = Line::from(vec![prefix, value]);

        line.render(inner_area, buf);
    }

    fn render_message_popup(&self, area: Rect, buf: &mut Buffer, msg: &Line) {
        Clear.render(area, buf);

        let block = Block::bordered()
            .title("Join Game")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .fg(Color::Cyan)
            .bg(Color::Black);

        let inner_area = block.inner(area);
        block.render(area, buf);

        msg.render(inner_area, buf);
    }

    /// Set running to false to quit the application.
    pub fn quit(&mut self) {
        self.running = false;
    }
}

fn popup_area(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Max(height)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Max(width)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

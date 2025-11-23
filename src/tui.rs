use crate::game::{Game, GameState};
use anyhow::Result;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap, List, ListItem, ListState},
    Terminal,
};
use crossterm::event::{Event, KeyCode, KeyEventKind};

// Trait for getting events (Native vs WASM)
#[async_trait::async_trait]
pub trait EventSource {
    async fn next_event(&mut self) -> Result<Option<Event>>;
}

pub struct Tui<B: Backend, E: EventSource> {
    terminal: Terminal<B>,
    event_source: E,
    input_buffer: String,
}

impl<B: Backend, E: EventSource> Tui<B, E> {
    pub fn new(terminal: Terminal<B>, event_source: E) -> Self {
        Self {
            terminal,
            event_source,
            input_buffer: String::new(),
        }
    }

    pub async fn run(&mut self, game: &mut Game) -> Result<()> {
        loop {
            let input_buffer = self.input_buffer.clone();
            self.terminal.draw(|frame| {
                match game.state {
                    GameState::SplashScreen => Self::render_splash_screen(frame, game),
                    _ => Self::render_main_game(frame, game, &input_buffer),
                }
            })?;

            // Wait for next event
            if let Some(event) = self.event_source.next_event().await? {
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Enter => {
                                if game.state == GameState::SplashScreen {
                                    // Handle selection
                                    if game.selected_save_index < game.save_list.len() {
                                        game.process_input("load").await?;
                                    } else {
                                        // "New Game" is the last item
                                        game.process_input("new").await?;
                                    }
                                } else if !self.input_buffer.is_empty() {
                                    let input = self.input_buffer.clone();
                                    self.input_buffer.clear();
                                    game.process_input(&input).await?;
                                }
                            }
                            KeyCode::Char(c) => {
                                if game.state != GameState::SplashScreen {
                                    self.input_buffer.push(c);
                                }
                            }
                            KeyCode::Backspace => {
                                if game.state != GameState::SplashScreen {
                                    self.input_buffer.pop();
                                }
                            }
                            KeyCode::Up => {
                                if game.state == GameState::SplashScreen {
                                    game.process_input("up").await.unwrap();
                                }
                            }
                            KeyCode::Down => {
                                if game.state == GameState::SplashScreen {
                                    game.process_input("down").await.unwrap();
                                }
                            }
                            KeyCode::Esc => {
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn render_splash_screen(frame: &mut Frame, game: &Game) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ])
            .split(frame.area());

        let title = Paragraph::new("INFINITE TEXT ADVENTURE")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, chunks[0]);

        let mut items: Vec<ListItem> = game.save_list.iter()
            .map(|save| ListItem::new(format!("Load: {} ({})", save.filename, save.modified.format("%Y-%m-%d %H:%M"))))
            .collect();
        items.push(ListItem::new("Start New Game"));

        let list = List::new(items)
            .block(Block::default().title("Select Game").borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">> ");

        let mut state = ListState::default();
        state.select(Some(game.selected_save_index));

        frame.render_stateful_widget(list, chunks[1], &mut state);
    }

    fn render_main_game(frame: &mut Frame, game: &Game, input_buffer: &str) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1), // Main content
                Constraint::Length(3), // Input bar
                Constraint::Length(1), // Status bar
            ])
            .split(frame.area());

        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Image
                Constraint::Percentage(50), // Narrative
            ])
            .split(chunks[0]);

        // Image Area
        let image_block = Block::default().borders(Borders::ALL).title("Visuals");
        let image_text = match &game.world.locations.get(&game.world.current_location_id) {
            Some(loc) => format!("Image for: {}\nPrompt: {}", loc.name, loc.image_prompt),
            None => "No location".to_string(),
        };
        frame.render_widget(Paragraph::new(image_text).block(image_block), top_chunks[0]);

        // Narrative Area
        let narrative_block = Block::default().borders(Borders::ALL).title("Narrative");
        let narrative_text = &game.last_narrative;
        frame.render_widget(
            Paragraph::new(narrative_text.as_str())
                .block(narrative_block)
                .wrap(Wrap { trim: true }),
            top_chunks[1],
        );

        // Input Area
        let input_block = Block::default().borders(Borders::ALL).title("Input");
        let input_text = match game.state {
            GameState::Processing | GameState::UpdatingWorld => "Thinking...".to_string(),
            _ => input_buffer.to_string(),
        };
        frame.render_widget(Paragraph::new(input_text).block(input_block), chunks[1]);

        // Status Bar
        let status_text = format!(
            "Save: {} | Status: {:?} | Money: {}",
            game.current_save_path.as_deref().unwrap_or("Unsaved"),
            match game.state {
                GameState::Processing => "Processing",
                GameState::UpdatingWorld => "Updating",
                _ => "Idle",
            },
            game.world.player.money
        );
        frame.render_widget(Paragraph::new(status_text).style(Style::default().bg(Color::Blue).fg(Color::White)), chunks[2]);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct CrosstermEventSource;

#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl EventSource for CrosstermEventSource {
    async fn next_event(&mut self) -> Result<Option<Event>> {
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            Ok(Some(crossterm::event::read()?))
        } else {
            Ok(None)
        }
    }
}

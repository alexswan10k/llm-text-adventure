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
                            },
                            KeyCode::Char(c) => {
                                if game.state != GameState::SplashScreen {
                                    self.input_buffer.push(c);
                                }
                            },
                            KeyCode::Backspace => {
                                if game.state != GameState::SplashScreen {
                                    self.input_buffer.pop();
                                }
                            },
                            KeyCode::Up => {
                                if game.state == GameState::SplashScreen {
                                    game.process_input("up").await?;
                                } else {
                                    let current_id = game.world.current_location_id.clone();
                                    if let Some(loc) = game.world.locations.get(&current_id) {
                                        if let Some(next_option) = loc.exits.get("north") {
                                            if let Some(next_id) = next_option {
                                                game.world.current_location_id = next_id.clone();
                                                let next_name = game.world.locations.get(&game.world.current_location_id).map_or_else(|| "unknown place".to_string(), |l| l.name.clone());
                                                game.last_narrative = format!("You go north to {}.", next_name);
                                                game.log("Quick move north");
                                            } else {
                                                game.last_narrative = "Path north is blocked.".to_string();
                                                game.log("North exit blocked");
                                            }
                                        } else {
                                            game.last_narrative = "No path north from here.".to_string();
                                            game.log("No north exit");
                                        }
                                    }
                                }
                            },
                            KeyCode::Down => {
                                if game.state == GameState::SplashScreen {
                                    game.process_input("down").await?;
                                } else {
                                    let current_id = game.world.current_location_id.clone();
                                    if let Some(loc) = game.world.locations.get(&current_id) {
                                        if let Some(next_option) = loc.exits.get("south") {
                                            if let Some(next_id) = next_option {
                                                game.world.current_location_id = next_id.clone();
                                                let next_name = game.world.locations.get(&game.world.current_location_id).map_or_else(|| "unknown place".to_string(), |l| l.name.clone());
                                                game.last_narrative = format!("You go south to {}.", next_name);
                                                game.log("Quick move south");
                                            } else {
                                                game.last_narrative = "Path south is blocked.".to_string();
                                                game.log("South exit blocked");
                                            }
                                        } else {
                                            game.last_narrative = "No path south from here.".to_string();
                                            game.log("No south exit");
                                        }
                                    }
                                }
                            },
                            KeyCode::Left => {
                                let current_id = game.world.current_location_id.clone();
                                if let Some(loc) = game.world.locations.get(&current_id) {
                                    if let Some(next_option) = loc.exits.get("west") {
                                        if let Some(next_id) = next_option {
                                            game.world.current_location_id = next_id.clone();
                                            let next_name = game.world.locations.get(&game.world.current_location_id).map_or_else(|| "unknown place".to_string(), |l| l.name.clone());
                                            game.last_narrative = format!("You go west to {}.", next_name);
                                            game.log("Quick move west");
                                        } else {
                                            game.last_narrative = "Path west is blocked.".to_string();
                                            game.log("West exit blocked");
                                        }
                                    } else {
                                        game.last_narrative = "No path west from here.".to_string();
                                        game.log("No west exit");
                                    }
                                }
                            },
                            KeyCode::Right => {
                                let current_id = game.world.current_location_id.clone();
                                if let Some(loc) = game.world.locations.get(&current_id) {
                                    if let Some(next_option) = loc.exits.get("east") {
                                        if let Some(next_id) = next_option {
                                            game.world.current_location_id = next_id.clone();
                                            let next_name = game.world.locations.get(&game.world.current_location_id).map_or_else(|| "unknown place".to_string(), |l| l.name.clone());
                                            game.last_narrative = format!("You go east to {}.", next_name);
                                            game.log("Quick move east");
                                        } else {
                                            game.last_narrative = "Path east is blocked.".to_string();
                                            game.log("East exit blocked");
                                        }
                                    } else {
                                        game.last_narrative = "No path east from here.".to_string();
                                        game.log("No east exit");
                                    }
                                }
                            },
                            KeyCode::Esc => {
                                return Ok(());
                            },
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
                Constraint::Length(10), // Debug Log (New)
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
        let mut narrative_text = game.last_narrative.clone();
        
        // Append options
        if !game.current_options.is_empty() {
            narrative_text.push_str("\n\nSuggested Actions:\n");
            for (i, option) in game.current_options.iter().enumerate() {
                narrative_text.push_str(&format!("{}. {}\n", i + 1, option));
            }
        }

        frame.render_widget(
            Paragraph::new(narrative_text)
                .block(narrative_block)
                .wrap(Wrap { trim: true }),
            top_chunks[1],
        );

        // Debug Log Area
        let debug_block = Block::default().borders(Borders::ALL).title("Debug Log");
        let debug_text = game.debug_log.iter().rev().take(8).rev().cloned().collect::<Vec<_>>().join("\n");
        frame.render_widget(Paragraph::new(debug_text).block(debug_block), chunks[1]);

        // Input Area
        let input_block = Block::default().borders(Borders::ALL).title("Input");
        let input_text = match game.state {
            GameState::Processing | GameState::UpdatingWorld => "Thinking...".to_string(),
            _ => input_buffer.to_string(),
        };
        frame.render_widget(Paragraph::new(input_text).block(input_block), chunks[2]);

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
        frame.render_widget(Paragraph::new(status_text).style(Style::default().bg(Color::Blue).fg(Color::White)), chunks[3]);
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

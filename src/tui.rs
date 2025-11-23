use crate::game::{Game, GameState};
use anyhow::Result;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
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
            self.terminal.draw(|frame| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(1), // Main content
                        Constraint::Length(3), // Input bar
                    ])
                    .split(frame.size());

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
                    _ => self.input_buffer.clone(),
                };
                frame.render_widget(Paragraph::new(input_text).block(input_block), chunks[1]);
            })?;

            // Wait for next event
            if let Some(event) = self.event_source.next_event().await? {
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Enter => {
                                if !self.input_buffer.is_empty() {
                                    let input = self.input_buffer.clone();
                                    self.input_buffer.clear();
                                    game.process_input(&input).await?;
                                }
                            }
                            KeyCode::Char(c) => {
                                self.input_buffer.push(c);
                            }
                            KeyCode::Backspace => {
                                self.input_buffer.pop();
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

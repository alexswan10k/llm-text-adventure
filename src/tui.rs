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
    spinner_frame: usize,
}

impl<B: Backend, E: EventSource> Tui<B, E> {
    pub fn new(terminal: Terminal<B>, event_source: E) -> Self {
        Self {
            terminal,
            event_source,
            input_buffer: String::new(),
            spinner_frame: 0,
        }
    }

    pub async fn run(&mut self, game: &mut Game) -> Result<()> {
        let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        loop {
            let command_buffer = self.input_buffer.clone();

            // Update spinner frame when processing
            if game.state == GameState::Processing || game.state == GameState::UpdatingWorld {
                self.spinner_frame = (self.spinner_frame + 1) % spinner_chars.len();
            }

            self.terminal.draw(|frame| {
                match game.state {
                    GameState::SplashScreen => Self::render_splash_screen(frame, game),
                    GameState::NamingWorld => Self::render_naming_screen(frame, game, &game.new_world_name),
                    _ => Self::render_main_game(frame, game, &command_buffer, spinner_chars[self.spinner_frame]),
                }
            })?;

            // Wait for next event
            if let Some(event) = self.event_source.next_event().await? {
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        // Handle quit key first, before any state checks
                        if key.code == KeyCode::Esc {
                            if game.state == GameState::NamingWorld {
                                game.process_input("back").await?;
                                self.input_buffer.clear();
                            } else {
                                return Ok(());
                            }
                        }
                        
                        match key.code {
                            KeyCode::Enter => {
                                if game.state == GameState::SplashScreen {
                                    if game.selected_save_index < game.save_list.len() {
                                        game.process_input("load").await?;
                                    } else {
                                        game.process_input("new").await?;
                                    }
                                } else if game.state == GameState::NamingWorld {
                                    game.process_input("enter").await?;
                                } else if !self.input_buffer.is_empty() {
                                    let input = self.input_buffer.clone();
                                    game.log(&format!("Enter pressed: '{}' (len: {}) state: {:?}", input, input.len(), game.state));
                                    self.input_buffer.clear();
                                    game.process_input(&input).await?;
                                }
                            },
                            KeyCode::Char('c') => {
                                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                                    game.log("Ctrl+C pressed, quitting...");
                                    return Ok(());
                                }
                                if game.state == GameState::SplashScreen {
                                    // Allow navigation in splash screen
                                } else if game.state == GameState::NamingWorld {
                                    game.process_input("c").await?;
                                } else if game.state == GameState::WaitingForInput {
                                    self.input_buffer.push('c');
                                } else {
                                    game.log(&format!("Ignored char 'c' - state is {:?}", game.state));
                                }
                            },
                            KeyCode::Char(c) => {
                                if game.state == GameState::SplashScreen {
                                    // Allow navigation in splash screen
                                } else if game.state == GameState::NamingWorld {
                                    game.process_input(&c.to_string()).await?;
                                } else if game.state == GameState::WaitingForInput {
                                    self.input_buffer.push(c);
                                } else {
                                    game.log(&format!("Ignored char '{}' - state is {:?}", c, game.state));
                                }
                            },
                            KeyCode::Backspace => {
                                if game.state == GameState::SplashScreen {
                                    // Allow navigation in splash screen
                                } else if game.state == GameState::NamingWorld {
                                    game.process_input("backspace").await?;
                                } else if game.state == GameState::WaitingForInput {
                                    self.input_buffer.pop();
                                } else {
                                    game.log(&format!("Ignored backspace - state is {:?}", game.state));
                                }
                            },
                            KeyCode::Up => {
                                if game.state == GameState::SplashScreen {
                                    game.process_input("up").await?;
                                } else if game.state == GameState::WaitingForInput {
                                    let (x, y) = game.world.current_pos;
                                    let target_pos = (x, y + 1);

                                    if let Some(target_loc) = game.world.locations.get(&target_pos).cloned() {
                                        game.world.current_pos = target_pos;
                                        if let Some(loc) = game.world.locations.get_mut(&target_pos) {
                                            loc.visited = true;
                                        }
                                        game.last_narrative = format!("You move north to {}.\n{}", target_loc.name, target_loc.description);
                                        game.log("Quick move north");
                                        if let Some(path) = &game.current_save_path {
                                            let _ = game.save_manager.save_game(path, &game.world);
                                        }
                                    } else {
                                        game.log("Cannot move north - area unexplored");
                                        game.last_narrative = format!("The path north leads to unexplored territory. Type your action to explore.");
                                    }
                                }
                            },
                            KeyCode::Down => {
                                if game.state == GameState::SplashScreen {
                                    game.process_input("down").await?;
                                } else if game.state == GameState::WaitingForInput {
                                    let (x, y) = game.world.current_pos;
                                    let target_pos = (x, y - 1);

                                    if let Some(target_loc) = game.world.locations.get(&target_pos).cloned() {
                                        game.world.current_pos = target_pos;
                                        if let Some(loc) = game.world.locations.get_mut(&target_pos) {
                                            loc.visited = true;
                                        }
                                        game.last_narrative = format!("You move south to {}.\n{}", target_loc.name, target_loc.description);
                                        game.log("Quick move south");
                                        if let Some(path) = &game.current_save_path {
                                            let _ = game.save_manager.save_game(path, &game.world);
                                        }
                                    } else {
                                        game.log("Cannot move south - area unexplored");
                                        game.last_narrative = format!("The path south leads to unexplored territory. Type your action to explore.");
                                    }
                                }
                            },
                            KeyCode::Left => {
                                if game.state == GameState::WaitingForInput {
                                    let (x, y) = game.world.current_pos;
                                    let target_pos = (x - 1, y);

                                    if let Some(target_loc) = game.world.locations.get(&target_pos).cloned() {
                                        game.world.current_pos = target_pos;
                                        if let Some(loc) = game.world.locations.get_mut(&target_pos) {
                                            loc.visited = true;
                                        }
                                        game.last_narrative = format!("You move west to {}.\n{}", target_loc.name, target_loc.description);
                                        game.log("Quick move west");
                                        if let Some(path) = &game.current_save_path {
                                            let _ = game.save_manager.save_game(path, &game.world);
                                        }
                                    } else {
                                        game.log("Cannot move west - area unexplored");
                                        game.last_narrative = format!("The path west leads to unexplored territory. Type your action to explore.");
                                    }
                                }
                            },
                            KeyCode::Right => {
                                if game.state == GameState::WaitingForInput {
                                    let (x, y) = game.world.current_pos;
                                    let target_pos = (x + 1, y);

                                    if let Some(target_loc) = game.world.locations.get(&target_pos).cloned() {
                                        game.world.current_pos = target_pos;
                                        if let Some(loc) = game.world.locations.get_mut(&target_pos) {
                                            loc.visited = true;
                                        }
                                        game.last_narrative = format!("You move east to {}.\n{}", target_loc.name, target_loc.description);
                                        game.log("Quick move east");
                                        if let Some(path) = &game.current_save_path {
                                            let _ = game.save_manager.save_game(path, &game.world);
                                        }
                                    } else {
                                        game.log("Cannot move east - area unexplored");
                                        game.last_narrative = format!("The path east leads to unexplored territory. Type your action to explore.");
                                    }
                                }
                            },
                            KeyCode::Esc => {
                                return Ok(());
                            },
                            KeyCode::Delete => {
                                if game.state == GameState::SplashScreen && !game.save_list.is_empty() {
                                    let save = &game.save_list[game.selected_save_index];
                                    if let Err(e) = game.save_manager.delete_save(&save.filename) {
                                        game.log(&format!("Failed to delete save: {}", e));
                                    } else {
                                        game.log(&format!("Deleted save: {}", save.filename));
                                        // Refresh save list
                                        game.save_list = game.save_manager.list_saves().unwrap_or_default();
                                        if game.selected_save_index >= game.save_list.len() && game.selected_save_index > 0 {
                                            game.selected_save_index = game.save_list.len() - 1;
                                        }
                                    }
                                }
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

        let title = Paragraph::new("INFINITE TEXT ADVENTURE\n(↑↓ to select, Enter to load, Delete to remove)")
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

    fn render_naming_screen(frame: &mut Frame, game: &Game, _input_buffer: &str) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ])
            .split(frame.area());

        let title = Paragraph::new("NAME YOUR NEW WORLD")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, chunks[0]);

        let input_display = if game.new_world_name.is_empty() {
            "_\n".to_string()
        } else {
            format!("{}_\n", game.new_world_name)
        };
        let input_block = Paragraph::new(input_display)
            .alignment(Alignment::Center)
            .block(Block::default().title("World Name").borders(Borders::ALL));
        frame.render_widget(input_block, chunks[1]);

        let help = Paragraph::new("Type a name and press Enter\nPress Backspace to delete\nPress Esc to go back")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(help, chunks[2]);
    }

    fn render_map(game: &Game) -> String {
        if game.world.locations.is_empty() {
            return "No locations".to_string();
        }

        // Get visible locations (visited + adjacent to current_pos for fog-of-war)
        let (current_x, current_y) = game.world.current_pos;
        let mut visible_coords = Vec::new();
        
        for (&(x, y), loc) in &game.world.locations {
            if loc.visited || 
               (x.abs_diff(current_x) <= 1 && y.abs_diff(current_y) <= 1) {
                visible_coords.push((x, y));
            }
        }

        if visible_coords.is_empty() {
            return "No visible locations".to_string();
        }

        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_y = i32::MAX;
        let mut max_y = i32::MIN;

        for &(x, y) in &visible_coords {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }

        let width = (max_x - min_x + 1) as usize;
        let height = (max_y - min_y + 1) as usize;

        let mut grid = vec![vec!['.'; width]; height];

        // Render locations and paths
        for &(x, y) in &visible_coords {
            let gx = (x - min_x) as usize;
            let gy = (max_y - y) as usize; // Y reversed (north at top)
            
            if gx < width && gy < height {
                if (x, y) == game.world.current_pos {
                    grid[gy][gx] = '@';
                } else if let Some(loc) = game.world.locations.get(&(x, y)) {
                    grid[gy][gx] = if loc.visited { '#' } else { '?' };
                }
                
                // Draw paths to adjacent visible locations
                if let Some(current_loc) = game.world.locations.get(&(x, y)) {
                    // North path
                    if let Some(Some((nx, ny))) = current_loc.exits.get("north") {
                        if visible_coords.contains(&(*nx, *ny)) {
                            let ngx = (*nx - min_x) as usize;
                            let ngy = (max_y - *ny) as usize;
                            if ngx < width && ngy < height && ngy < gy {
                                grid[ngy][ngx] = '|';
                            }
                        }
                    }
                    // East path
                    if let Some(Some((ex, ey))) = current_loc.exits.get("east") {
                        if visible_coords.contains(&(*ex, *ey)) {
                            let egx = (*ex - min_x) as usize;
                            let egy = (max_y - *ey) as usize;
                            if egx < width && egy < height && egx > gx {
                                grid[egy][gx] = '-';
                            }
                        }
                    }
                }
            }
        }

        let mut map_str = String::new();
        for row in grid {
            map_str.push_str(&row.iter().collect::<String>());
            map_str.push('\n');
        }
        map_str.trim_end().to_string()
    }

    fn render_main_game(frame: &mut Frame, game: &Game, input_buffer: &str, spinner_char: char) {
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
        let image_text = match &game.world.locations.get(&game.world.current_pos) {
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

        // Debug and Map Area
        let debug_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Map
                Constraint::Percentage(50), // Debug Log
            ])
            .split(chunks[1]);

        // Map Area
        let map_block = Block::default().borders(Borders::ALL).title("Map");
        let map_text = Self::render_map(game);
        frame.render_widget(Paragraph::new(map_text).block(map_block), debug_chunks[0]);

        // Debug Log Area
        let debug_block = Block::default().borders(Borders::ALL).title("Debug Log");
        let debug_text = game.debug_log.iter().rev().take(8).rev().cloned().collect::<Vec<_>>().join("\n");
        frame.render_widget(Paragraph::new(debug_text).block(debug_block), debug_chunks[1]);

        // Input Area
        let input_block = Block::default().borders(Borders::ALL).title("Input");
        let input_text = match game.state {
            GameState::Processing | GameState::UpdatingWorld => {
                if game.status_message.is_empty() {
                    format!("{} Thinking...", spinner_char)
                } else {
                    format!("{} {}", spinner_char, game.status_message)
                }
            },
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
        if crossterm::event::poll(std::time::Duration::from_millis(10))? {  // Reduced from 100ms to 10ms
            Ok(Some(crossterm::event::read()?))
        } else {
            Ok(None)
        }
    }
}

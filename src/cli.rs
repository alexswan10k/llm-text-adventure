use crate::game::Game;
use crate::model::ItemState;
use anyhow::Result;
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};

pub struct Cli;

impl Cli {
    pub fn new() -> Self {
        Self
    }

    /// Run the CLI debug mode.
    ///
    /// This mode is designed for LLM agents to test the game logic interactively.
    ///
    /// ## Protocol
    /// 1. Game prints complete world state to stdout
    /// 2. Agent reads input from stdin
    /// 3. Game processes input and prints new state
    /// 4. Loop until `/exit` command
    ///
    /// ## Commands
    /// - `/north`, `/south`, `/east`, `/west` - Quick movement (instant if location exists)
    /// - `/exit` - Terminate cleanly
    /// - `1`, `2`, `3`... - Select from suggested_actions list
    /// - Any text - Pass to game.process_input() for LLM interpretation
    ///
    /// ## State Output Format
    /// ```text
    /// ========================================
    /// WORLD STATE
    /// ========================================
    ///
    /// --- Location ---
    /// Name: ...
    /// Position: (x, y)
    /// Description: ...
    /// Visited: true/false
    ///
    /// --- Items Here --- (if any)
    ///   - ItemName (Type) [state]
    ///
    /// --- Actors Here --- (if any)
    ///   - ActorName
    ///
    /// --- Exits --- (if any)
    ///   - direction: (x, y) - Name
    ///
    /// --- Player Inventory --- (if any)
    ///   - ItemName (Type) [state]
    ///
    /// --- Player Stats ---
    /// Money: N
    ///
    /// --- Narrative ---
    /// The story text...
    ///
    /// --- Suggested Actions --- (if any)
    ///   1. Action one
    ///   2. Action two
    ///
    /// --- Debug Log (Last 5) ---
    ///   [timestamp] log entry
    ///
    /// --- Game State ---
    /// State: SplashScreen|NamingWorld|WaitingForInput|Processing|UpdatingWorld|Rendering
    /// Save Path: Some("savefile.json") or None
    ///
    /// > [prompt for input]
    /// ```

    pub async fn run(&mut self, game: &mut Game) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        println!("=== LLM Debug Mode ===");
        println!("Special commands: /north, /south, /east, /west, /exit");
        println!("Type any text to interact with the game.\n");

        loop {
            self.print_state(game);

            print!("\n> ");
            io::stdout().flush()?;

            line.clear();
            reader.read_line(&mut line).await?;
            let input = line.trim();

            if input.is_empty() {
                continue;
            }

            if input == "/exit" {
                println!("Exiting debug mode.");
                break;
            }

            use crate::game::GameState;

            match game.state {
                GameState::SplashScreen => {
                    match input {
                        "new" | "n" => {
                            game.process_input("new").await?;
                        }
                        "load" | "l" => {
                            game.process_input("load").await?;
                        }
                        num if num.parse::<usize>().is_ok() => {
                            let idx: usize = num.parse()?;
                            if idx < game.save_list.len() {
                                game.selected_save_index = idx;
                                game.process_input("load").await?;
                            }
                        }
                        _ => {
                            println!("Use 'new', 'load', or a number to select a save.");
                        }
                    }
                }
                GameState::NamingWorld => {
                    match input {
                        "enter" | "done" => {
                            game.process_input("enter").await?;
                        }
                        "back" | "cancel" => {
                            game.process_input("back").await?;
                        }
                        name => {
                            game.new_world_name = name.to_string();
                            println!("World name set to: '{}'. Type 'enter' to confirm or 'back' to cancel.", name);
                        }
                    }
                }
                GameState::WaitingForInput => {
                    let processed_input = match input {
                        "/north" => {
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
                                None
                            } else {
                                Some("go north".to_string())
                            }
                        }
                        "/south" => {
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
                                None
                            } else {
                                Some("go south".to_string())
                            }
                        }
                        "/east" => {
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
                                None
                            } else {
                                Some("go east".to_string())
                            }
                        }
                        "/west" => {
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
                                None
                            } else {
                                Some("go west".to_string())
                            }
                        }
                        num if num.parse::<usize>().is_ok() => {
                            let idx: usize = num.parse()?;
                            if idx > 0 && idx <= game.current_options.len() {
                                Some(game.current_options[idx - 1].clone())
                            } else {
                                Some(input.to_string())
                            }
                        }
                        _ => Some(input.to_string()),
                    };

                    if let Some(cmd) = processed_input {
                        if let Err(e) = game.process_input(&cmd).await {
                            println!("Error processing input: {}", e);
                        }
                    }
                }
                _ => {
                    println!("Game is processing, please wait...");
                }
            }
        }

        Ok(())
    }

    fn print_state(&self, game: &Game) {
        println!("\n========================================");
        println!("WORLD STATE");
        println!("========================================");

        let (x, y) = game.world.current_pos;

        if let Some(loc) = game.world.locations.get(&(x, y)) {
            println!("\n--- Location ---");
            println!("Name: {}", loc.name);
            println!("Position: ({}, {})", x, y);
            println!("Description: {}", loc.description);
            println!("Visited: {}", loc.visited);

            if !loc.items.is_empty() {
                println!("\n--- Items Here ---");
                for item_id in &loc.items {
                    if let Some(item) = game.world.items.get(item_id) {
                        let state_str = format_item_state(item);
                        println!("  - {} ({}) [{}]", item.name, item.item_type, state_str);
                    }
                }
            }

            if !loc.actors.is_empty() {
                println!("\n--- Actors Here ---");
                for actor_id in &loc.actors {
                    if let Some(actor) = game.world.actors.get(actor_id) {
                        println!("  - {}", actor.name);
                    }
                }
            }

            if !loc.exits.is_empty() {
                println!("\n--- Exits ---");
                for (dir, target) in &loc.exits {
                    match target {
                        Some((tx, ty)) => {
                            let name = game.world.locations.get(&(*tx, *ty))
                                .map(|l| l.name.as_str())
                                .unwrap_or("Unknown");
                            println!("  - {}: ({}, {}) - {}", dir, tx, ty, name);
                        }
                        None => println!("  - {}: blocked", dir),
                    }
                }
            }
        }

        if !game.world.player.inventory.is_empty() {
            println!("\n--- Player Inventory ---");
            for item_id in &game.world.player.inventory {
                if let Some(item) = game.world.items.get(item_id) {
                    let state_str = format_item_state(item);
                    println!("  - {} ({}) [{}]", item.name, item.item_type, state_str);
                }
            }
        }

        println!("\n--- Player Stats ---");
        println!("Money: {}", game.world.player.money);

        println!("\n--- Narrative ---");
        println!("{}", game.last_narrative);

        if !game.current_options.is_empty() {
            println!("\n--- Suggested Actions ---");
            for (i, option) in game.current_options.iter().enumerate() {
                println!("  {}. {}", i + 1, option);
            }
        }

        println!("\n--- Debug Log (Last 5) ---");
        for log in game.debug_log.iter().rev().take(5) {
            println!("  {}", log);
        }

        println!("\n--- Game State ---");
        println!("State: {:?}", game.state);
        println!("Save Path: {:?}", game.current_save_path);
    }
}

fn format_item_state(item: &crate::model::Item) -> String {
    match &item.state {
        ItemState::Normal => "normal".to_string(),
        ItemState::Equipped => "equipped".to_string(),
        ItemState::Damaged { durability, max_durability } => {
            format!("damaged: {}/{}", durability, max_durability)
        }
        ItemState::Consumed { charges, max_charges } => {
            format!("charges: {}/{}", charges, max_charges)
        }
        ItemState::Locked { key_id } => {
            format!("locked by: {:?}", key_id)
        }
        ItemState::Open { contents } => {
            format!("open: {} items", contents.len())
        }
    }
}

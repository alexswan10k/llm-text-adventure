use crate::model::{WorldState, WorldUpdate, Location};
use crate::llm::LlmClient;
use crate::save::{SaveManager, SaveInfo};
use anyhow::{Result, Context};
use std::collections::HashMap;

use std::time::Duration;
use tokio::time::sleep;
use chrono::prelude::*;
#[derive(PartialEq, Debug)]
pub enum GameState {
    SplashScreen,
    NamingWorld,
    WaitingForInput,
    Processing,
    UpdatingWorld,
    Rendering,
}

pub struct Game {
    pub world: WorldState,
    pub llm_client: LlmClient,
    pub save_manager: SaveManager,
    pub last_narrative: String,
    pub state: GameState,
    pub current_save_path: Option<String>,
    pub save_list: Vec<SaveInfo>,
    pub selected_save_index: usize,
    pub debug_log: Vec<String>,
    pub current_options: Vec<String>,
    pub status_message: String,
    pub new_world_name: String,
}

impl Game {
    pub fn new(llm_client: LlmClient) -> Self {
        let save_manager = SaveManager::new();
        let save_list = save_manager.list_saves().unwrap_or_default();

        Self {
            world: WorldState::new(),
            llm_client,
            save_manager,
            last_narrative: "Welcome to the Infinite Text Adventure.".to_string(),
            state: GameState::SplashScreen,
            current_save_path: None,
            save_list,
            selected_save_index: 0,
            debug_log: vec!["Game initialized.".to_string()],
            current_options: Vec::new(),
            status_message: "".to_string(),
            new_world_name: String::new(),
        }
    }

    pub fn log(&mut self, message: &str) {
        self.debug_log.push(format!("[{}] {}", Local::now().format("%H:%M:%S"), message));
        if self.debug_log.len() > 100 {
            self.debug_log.remove(0);
        }
    }

    pub async fn process_input(&mut self, input: &str) -> Result<()> {
        match self.state {
            GameState::SplashScreen => self.handle_splash_input(input).await,
            GameState::NamingWorld => self.handle_naming_input(input).await,
            GameState::WaitingForInput => {
                if let Ok(idx) = input.parse::<usize>() {
                    if idx > 0 && idx <= self.current_options.len() {
                        let selected_action = self.current_options[idx - 1].clone();
                        self.log(&format!("User selected option {}: {}", idx, selected_action));
                        return self.handle_game_input(&selected_action).await;
                    }
                }
                self.handle_game_input(input).await
            },
            _ => Ok(()),
        }
    }

    pub async fn generate_and_move_to(&mut self, target_pos: (i32, i32), direction: &str) -> Result<()> {
        let (x, y) = self.world.current_pos;
        let (target_x, target_y) = target_pos;

        self.log(&format!("Generating location at ({}, {}) heading {}", target_x, target_y, direction));

        let current_loc = self.world.locations.get(&self.world.current_pos)
            .context("Current location not found")?;

        let prompt = format!(
            r#"Current Location: {} at ({}, {})
Description: {}

The player is heading {} toward coordinates ({}, {}).
This grid cell is currently EMPTY and needs to be generated.

Create a new location at ({}, {}) that fits thematically with the current location.
Return ONLY a JSON object:
{{
  "name": "Location name",
  "description": "Description of what the player sees",
  "image_prompt": "Visual description for generating an image",
  "exits": {{"north": null, "south": null, "east": null, "west": null}},
  "items": [],
  "actors": []
}}

Do NOT include any narrative text or MoveTo actions. Just the location JSON."#,
            current_loc.name, x, y,
            current_loc.description,
            direction, target_x, target_y,
            target_x, target_y
        );

        let system_prompt = "You are a world generator for a text adventure game. Create interesting, thematically consistent locations.";

        self.state = GameState::Processing;
        self.status_message = format!("Exploring {}...", direction);

        match self.llm_client.generate_location(system_prompt, &prompt).await {
            Ok(mut location) => {
                location.visited = true;
                self.world.locations.insert(target_pos, location);
                self.world.current_pos = target_pos;

                let loc = self.world.locations.get(&target_pos).unwrap();
                self.last_narrative = format!("You travel {} to {}.\n{}", direction, loc.name, loc.description);
                self.log(&format!("Created and moved to ({}, {})", target_x, target_y));

                if let Some(path) = &self.current_save_path {
                    let _ = self.save_manager.save_game(path, &self.world);
                }
            }
            Err(e) => {
                self.log(&format!("Failed to generate location: {}", e));

                let fallback_loc = Location {
                    name: format!("Mysterious area ({}, {})", target_x, target_y),
                    description: "A mysterious place that appeared suddenly.".to_string(),
                    items: vec![],
                    actors: vec![],
                    exits: HashMap::new(),
                    cached_image_path: None,
                    image_prompt: "A mysterious location with undefined characteristics.".to_string(),
                    visited: true,
                };

                self.world.locations.insert(target_pos, fallback_loc);
                self.world.current_pos = target_pos;

                let loc = self.world.locations.get(&target_pos).unwrap();
                self.last_narrative = format!("You travel {} into the unknown.\n{}", direction, loc.description);
                self.log(&format!("Used fallback location at ({}, {})", target_x, target_y));

                if let Some(path) = &self.current_save_path {
                    let _ = self.save_manager.save_game(path, &self.world);
                }
            }
        }

        self.state = GameState::WaitingForInput;
        self.status_message = "".to_string();
        Ok(())
    }

    async fn handle_splash_input(&mut self, input: &str) -> Result<()> {
        match input {
            "new" => {
                self.new_world_name.clear();
                self.state = GameState::NamingWorld;
            }
            "load" => {
                if !self.save_list.is_empty() {
                    let save = &self.save_list[self.selected_save_index];
                    self.world = self.save_manager.load_save(&save.filename)?;
                    self.current_save_path = Some(save.filename.clone());
                    self.state = GameState::WaitingForInput;
                    self.last_narrative = format!("Loaded world: {}. What do you want to do?", save.filename);
                }
            }
            "up" => {
                if self.selected_save_index > 0 {
                    self.selected_save_index -= 1;
                }
            }
            "down" => {
                if self.selected_save_index < self.save_list.len() {
                    self.selected_save_index += 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_naming_input(&mut self, input: &str) -> Result<()> {
        match input {
            "enter" => {
                if !self.new_world_name.trim().is_empty() {
                    self.world = WorldState::new();
                    let start_loc = Location {
                        name: "The Beginning".to_string(),
                        description: "You stand in a void of potential. Anything can happen here.".to_string(),
                        items: vec![],
                        actors: vec![],
                        exits: HashMap::new(),
                        cached_image_path: None,
                        image_prompt: "A swirling void of colors and shapes, representing potential.".to_string(),
                        visited: true,
                    };
                    self.world.locations.insert((0, 0), start_loc);
                    let save_name = self.new_world_name.trim();
                    self.current_save_path = Some(self.save_manager.create_new_save(save_name, &self.world)?);
                    self.state = GameState::WaitingForInput;
                    self.last_narrative = format!("Created new world: '{}'. What do you want to do?", save_name);
                    self.log(&format!("Created new world: {}", save_name));
                }
            }
            "back" => {
                self.state = GameState::SplashScreen;
                self.new_world_name.clear();
            }
            "backspace" => {
                self.new_world_name.pop();
            }
            _ => {
                self.new_world_name.push_str(input);
            }
        }
        Ok(())
    }

    async fn handle_game_input(&mut self, input: &str) -> Result<()> {
        self.state = GameState::Processing;
        self.status_message = "Thinking...".to_string();

        // 1. Construct Context
        let current_loc = self.world.locations.get(&self.world.current_pos)
            .context("Current location not found in world map")?;

        // Resolve items and actors names for context
        let visible_items: Vec<String> = current_loc.items.iter()
            .filter_map(|id| self.world.items.get(id).map(|i| i.name.clone()))
            .collect();

        let visible_actors: Vec<String> = current_loc.actors.iter()
            .filter_map(|id| self.world.actors.get(id).map(|a| a.name.clone()))
            .collect();

        let player_inventory: Vec<String> = self.world.player.inventory.iter()
            .filter_map(|id| self.world.items.get(id).map(|i| i.name.clone()))
            .collect();

        let exits_str = current_loc.exits.iter()
            .map(|(dir, _)| dir.clone())
            .collect::<Vec<_>>()
            .join(", ");

        // Add adjacent cell information for spatial context
        let (x, y) = self.world.current_pos;
        let adjacent_info = format!(
            "Adjacent cells:\nNorth at ({}, {}): {}\nSouth at ({}, {}): {}\nEast at ({}, {}): {}\nWest at ({}, {}): {}",
            x, y + 1,
            self.world.locations.get(&(x, y + 1)).map_or("UNKNOWN (not yet explored)".to_string(), |l| format!("{} - {}", l.name, l.description)),
            x, y - 1,
            self.world.locations.get(&(x, y - 1)).map_or("UNKNOWN (not yet explored)".to_string(), |l| format!("{} - {}", l.name, l.description)),
            x + 1, y,
            self.world.locations.get(&(x + 1, y)).map_or("UNKNOWN (not yet explored)".to_string(), |l| format!("{} - {}", l.name, l.description)),
            x - 1, y,
            self.world.locations.get(&(x - 1, y)).map_or("UNKNOWN (not yet explored)".to_string(), |l| format!("{} - {}", l.name, l.description))
        );

        let context_str = format!(
            "Current Location: {} ({}, {})\nDescription: {}\nExits: {}\nItems here: {:?}\nActors here: {:?}\nPlayer Inventory: {:?}\nPlayer Money: {}\n\n{}\n\nLast Narrative: {}\n\nUser Action: {}",
            current_loc.name,
            x, y,
            current_loc.description,
            exits_str,
            visible_items,
            visible_actors,
            player_inventory,
            self.world.player.money,
            adjacent_info,
            self.last_narrative,
            input
        );

        let system_prompt = r#"
You are Dungeon Master for a text adventure game.
Your goal is to update the game world based on the user's action.
You MUST return a JSON object representing the world update, followed by a narrative description.

IMPORTANT: You can and should use MULTIPLE actions to accomplish complex tasks. For example, to create a new room and move there: ["CreateLocation(x, y, {...})", "MoveTo(x, y)"]

The JSON structure is:
{
  "narrative": "The story text describing what happens.",
  "actions": [ ... list of action strings ... ],
  "suggested_actions": ["Option 1", "Option 2", "Option 3"] // 3-5 short, relevant follow-up actions for the user
}

Actions are strings in the format:
- MoveTo(x, y)
- CreateLocation(x, y, {location JSON object})
- UpdateLocation(x, y, {location JSON object})
- AddItemToInventory("item_id")
- RemoveItemFromInventory("item_id")
- CreateItem({item JSON object})
- AddItemToLocation(x, y, "item_id")
- RemoveItemFromLocation(x, y, "item_id")

Location object:
{
  "name": "Name",
  "description": "Description",
  "items": [],
  "actors": [],
  "exits": { "north": [x, y] or null },
  "cached_image_path": null,
  "image_prompt": "Visual description",
  "visited": true/false
}

Item object:
{
  "id": "unique_id",
  "name": "Name",
  "description": "Description"
}

Rules:
1. Use grid coordinates: north +y, south -y, east +x, west -x.
2. ONLY CreateLocation(x,y,...) if NO location exists at (x,y).
3. For movement: If user provides target coordinates like "go east to coordinates (x, y)", use MoveTo(x,y). If target exists, use MoveTo. If not, CreateLocation first then MoveTo.
4. Assign exits with exact coords: e.g., "north": [x, y+1] or null for blocked.
5. Maintain spatial coherence: no teleporting, adjacent only unless specified.
6. Use `AddItemToInventory` / `RemoveItemFromInventory` for picking up/dropping items.
7. Use `CreateItem` before adding a NEW item to inventory or location.
8. Provide `suggested_actions` that are relevant to the current situation (e.g., "go north", "take sword", "examine chest").
"#;

        self.log(&format!("Processing input: '{}'", input));
        self.log(&format!("Current player position: {:?}", self.world.current_pos));

        // 2. Call LLM with Retry Loop
        let max_attempts = 20;
        let mut attempts = 0;

        loop {
            attempts += 1;
            self.status_message = format!("Attempt {}/{} - Asking the spirits...", attempts, max_attempts);
            self.log(&self.status_message.clone());
            
            match self.llm_client.generate_update(system_prompt, &context_str).await {
                Ok(update) => {
                    self.log(&format!("Received update: {} actions, {} suggestions.", update.actions.len(), update.suggested_actions.len()));
                    for (i, action) in update.actions.iter().enumerate() {
                        self.log(&format!("Action {}: {}", i, action));
                    }
                    self.state = GameState::UpdatingWorld;
                    self.status_message = "Processing response...".to_string();

                    // Update options
                    self.current_options = update.suggested_actions.clone();

                    if let Err(e) = self.apply_update(update) {
                        let err_msg = e.to_string();
                        let summary = if err_msg.len() > 50 { format!("{}...", &err_msg[..47]) } else { err_msg };
                        self.last_narrative = format!("Error applying update: {}", summary);
                        self.log(&format!("Error applying update: {}", summary));
                    }
                    // Auto-save
                    if let Some(path) = &self.current_save_path {
                        let _ = self.save_manager.save_game(path, &self.world);
                    }
                    break; // Success, exit loop
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    let summary = if err_msg.len() > 50 { format!("{}...", &err_msg[..47]) } else { err_msg };
                    self.log(&format!("LLM Error (Attempt {}): {}", attempts, summary));
                    self.status_message = format!("Error, retrying in 30s... ({}/{})", attempts, max_attempts);
                    self.log("Retrying in 30 seconds. Model may still be loading.");
                    sleep(Duration::from_secs(30)).await;
                    
                    if attempts >= max_attempts {
                        self.last_narrative = format!("The spirits are silent. (Failed after {} attempts)", max_attempts);
                        self.status_message = "Failed.".to_string();
                        break;
                    }
                    // Optional: Add a small delay here if needed, but for now just retry
                }
            }
        }

        self.state = GameState::WaitingForInput;
        self.status_message = "".to_string();
        Ok(())
    }

    fn apply_update(&mut self, update: WorldUpdate) -> Result<()> {
        self.last_narrative = update.narrative.clone();

        // First pass: Create all locations
        for action_str in &update.actions {
            let action_str = action_str.trim();
            if action_str.starts_with("CreateLocation(") {
                self.parse_and_apply_action(action_str)?;
            }
        }

        // Second pass: Handle all other actions (MoveTo, items, etc.)
        for action_str in &update.actions {
            let action_str = action_str.trim();
            if !action_str.starts_with("CreateLocation(") {
                self.parse_and_apply_action(action_str)?;
            }
        }
        Ok(())
    }

    fn parse_and_apply_action(&mut self, action_str: &str) -> Result<()> {
        let action_str = action_str.trim();

        self.log(&format!("Parsing action: '{}'", action_str));

        if action_str.starts_with("MoveTo(") {
            // Parse MoveTo(x, y) - handle various formats
            let coords_str = action_str.strip_prefix("MoveTo(")
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or("");

            self.log(&format!("MoveTo coords_str: '{}'", coords_str));

            if let Some((x_str, y_str)) = coords_str.split_once(',') {
                self.log(&format!("Parsing x='{}' y='{}'", x_str.trim(), y_str.trim()));
                let x: i32 = x_str.trim().parse().context("Invalid x coordinate")?;
                let y: i32 = y_str.trim().parse().context("Invalid y coordinate")?;
                let pos = (x, y);
                self.log(&format!("Parsed MoveTo to ({}, {})", x, y));
                
                if self.world.locations.contains_key(&pos) {
                    self.world.current_pos = pos;
                    if let Some(loc) = self.world.locations.get_mut(&pos) {
                        loc.visited = true;
                    }
                } else {
                    // Try to create a default location instead of failing
                    self.log(&format!("Creating default location at {:?} for MoveTo", pos));
                    let default_loc = crate::model::Location {
                        name: format!("Location ({}, {})", x, y),
                        description: "A mysterious place that appeared suddenly.".to_string(),
                        items: vec![],
                        actors: vec![],
                        exits: HashMap::new(),
                        cached_image_path: None,
                        image_prompt: "A mysterious location with undefined characteristics.".to_string(),
                        visited: true,
                    };
                    self.world.locations.insert(pos, default_loc);
                    self.world.current_pos = pos;
                }
            }
        } else if action_str.starts_with("CreateLocation(") {
            // Parse CreateLocation(x, y, {location JSON})
            let after_paren = &action_str[14..]; // Remove "CreateLocation("
            
            // Find the position of the SECOND comma to separate coordinates from JSON
            let mut comma_count = 0;
            let coords_end_pos = after_paren.find(|c| {
                if c == ',' {
                    comma_count += 1;
                    comma_count == 2
                } else {
                    false
                }
            });
            
            if let Some(coords_end_pos) = coords_end_pos {
                let coords_part = &after_paren[..coords_end_pos];
                let json_part = &after_paren[coords_end_pos+1..].trim();
                
                if let Some((x_str, y_str)) = coords_part.split_once(',') {
                    let x: i32 = x_str.trim().parse().context("Invalid x coordinate")?;
                    let y: i32 = y_str.trim().parse().context("Invalid y coordinate")?;
                    let pos = (x, y);
                    
                    if !self.world.locations.contains_key(&pos) {
                        let json_str = json_part.trim_end_matches(')');
                        let mut loc: crate::model::Location = serde_json::from_str(json_str)
                            .context(format!("Failed to parse CreateLocation: {}", json_str))?;
                        
                        // Validate exits point to valid coordinates or null
                        for (direction, exit_coord) in &loc.exits {
                            if let Some((exit_x, exit_y)) = exit_coord {
                                // Check if exit coordinates are reasonable (adjacent unless specified)
                                let dx = (*exit_x - x).abs();
                                let dy = (*exit_y - y).abs();
                                if dx > 1 || dy > 1 {
                                    self.log(&format!("Warning: Exit '{}' from ({},{}) to ({},{}) is not adjacent", direction, x, y, exit_x, exit_y));
                                }
                            }
                        }
                        
                        loc.visited = true;  // Created for player
                        self.world.locations.insert(pos, loc);
                        self.log(&format!("SUCCESS: Created location at ({}, {})", x, y));
                    } else {
                        self.log(&format!("Location at ({}, {}) already exists, skipping CreateLocation", x, y));
                    }
                } else {
                    self.log(&format!("ERROR: Failed to parse coordinates from: '{}'", coords_part));
                }
            } else {
                self.log(&format!("ERROR: Could not find two commas in: '{}'", after_paren));
            }
        } else if action_str.starts_with("UpdateLocation(") {
            // Parse UpdateLocation(x, y, {location JSON})
            let after_paren = &action_str[15..]; // Remove "UpdateLocation("
            if let Some(comma_pos) = after_paren.find(',') {
                let coords_part = &after_paren[..comma_pos];
                let json_part = &after_paren[comma_pos+1..].trim();
                
                if let Some((x_str, y_str)) = coords_part.split_once(',') {
                    let x: i32 = x_str.trim().parse().context("Invalid x coordinate")?;
                    let y: i32 = y_str.trim().parse().context("Invalid y coordinate")?;
                    let pos = (x, y);
                    
                    let json_str = json_part.trim_end_matches(')');
                    let loc: crate::model::Location = serde_json::from_str(json_str)
                        .context(format!("Failed to parse UpdateLocation: {}", json_str))?;
                    self.world.locations.insert(pos, loc);
                }
            }
        } else if action_str.starts_with("CreateItem(") && action_str.ends_with(")") {
            let json_str = &action_str[11..action_str.len()-1];
            let item: crate::model::Item = serde_json::from_str(json_str)
                .context(format!("Failed to parse CreateItem: {}", json_str))?;
            self.world.items.insert(item.id.clone(), item);
        } else if action_str.starts_with("AddItemToInventory(") && action_str.ends_with(")") {
            let item_id = &action_str[20..action_str.len()-1];
            let item_id = item_id.trim_matches('"');
            if !self.world.player.inventory.contains(&item_id.to_string()) {
                self.world.player.inventory.push(item_id.to_string());
            }
        } else if action_str.starts_with("RemoveItemFromInventory(") && action_str.ends_with(")") {
            let item_id = &action_str[24..action_str.len()-1];
            let item_id = item_id.trim_matches('"');
            self.world.player.inventory.retain(|id| id != item_id);
        } else if action_str.starts_with("AddItemToLocation(") {
            let params_str = &action_str[18..action_str.len()-1];
            // Parse "x, y, item_id"
            if let Some(first_comma) = params_str.find(',') {
                let coords_part = &params_str[..first_comma];
                let rest = &params_str[first_comma+1..].trim();
                
                if let Some((x_str, y_str)) = coords_part.split_once(',') {
                    let x: i32 = x_str.trim().parse().context("Invalid x coordinate")?;
                    let y: i32 = y_str.trim().parse().context("Invalid y coordinate")?;
                    let pos = (x, y);
                    let item_id = rest.trim().trim_matches('"');
                    
                    if let Some(loc) = self.world.locations.get_mut(&pos) {
                        if !loc.items.contains(&item_id.to_string()) {
                            loc.items.push(item_id.to_string());
                        }
                    }
                }
            }
        } else if action_str.starts_with("RemoveItemFromLocation(") && action_str.ends_with(")") {
            let params_str = &action_str[22..action_str.len()-1];
            // Parse "x, y, item_id"
            if let Some(first_comma) = params_str.find(',') {
                let coords_part = &params_str[..first_comma];
                let rest = &params_str[first_comma+1..].trim();
                
                if let Some((x_str, y_str)) = coords_part.split_once(',') {
                    let x: i32 = x_str.trim().parse().context("Invalid x coordinate")?;
                    let y: i32 = y_str.trim().parse().context("Invalid y coordinate")?;
                    let pos = (x, y);
                    let item_id = rest.trim().trim_matches('"');
                    
                    if let Some(loc) = self.world.locations.get_mut(&pos) {
                        loc.items.retain(|id| id != &item_id.to_string());
                    }
                }
            }
        } else {
            eprintln!("Unknown action: {}", action_str);
        }
        Ok(())
    }
}

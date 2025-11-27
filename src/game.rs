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
            GameState::WaitingForInput => {
                // Check if input is a number selection
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

    async fn handle_splash_input(&mut self, input: &str) -> Result<()> {
        match input {
            "new" => {
                self.world = WorldState::new();
                // Initialize default location if needed
                if self.world.locations.is_empty() {
                    let start_loc = Location {
                        id: "start".to_string(),
                        name: "The Beginning".to_string(),
                        description: "You stand in a void of potential. Anything can happen here.".to_string(),
                        items: vec![],
                        actors: vec![],
                        exits: HashMap::new(),
                        cached_image_path: None,
                        image_prompt: "A swirling void of colors and shapes, representing potential.".to_string(),
                        x: 0,
                        y: 0,
                    };
                    self.world.locations.insert("start".to_string(), start_loc);
                }
                self.current_save_path = Some(self.save_manager.create_new_save("new_world", &self.world)?);
                self.state = GameState::WaitingForInput;
                self.last_narrative = "You have created a new world. What do you want to do?".to_string();
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

    async fn handle_game_input(&mut self, input: &str) -> Result<()> {
        self.state = GameState::Processing;
        self.status_message = "Thinking...".to_string();

        // 1. Construct Context
        let current_loc = self.world.locations.get(&self.world.current_location_id)
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

        let context_str = format!(
            "Current Location: {}\nDescription: {}\nExits: {}\nItems here: {:?}\nActors here: {:?}\nPlayer Inventory: {:?}\nPlayer Money: {}\n\nLast Narrative: {}\n\nUser Action: {}",
            current_loc.name,
            current_loc.description,
            exits_str,
            visible_items,
            visible_actors,
            player_inventory,
            self.world.player.money,
            self.last_narrative,
            input
        );

        let system_prompt = r#"
You are the Dungeon Master for a text adventure game.
Your goal is to update the game world based on the user's action.
You MUST return a JSON object representing the world update, followed by a narrative description.

The JSON structure is:
{
  "narrative": "The story text describing what happens.",
  "actions": [ ... list of action strings ... ],
  "suggested_actions": ["Option 1", "Option 2", "Option 3"] // 3-5 short, relevant follow-up actions for the user
}

Actions are strings in the format:
- MoveTo("location_id")
- CreateLocation({location JSON object})
- UpdateLocation({location JSON object})
- AddItemToInventory("item_id")
- RemoveItemFromInventory("item_id")
- CreateItem({item JSON object})
- AddItemToLocation("loc_id", "item_id")
- RemoveItemFromLocation("loc_id", "item_id")

Location object:
{
  "id": "unique_id",
  "name": "Name",
  "description": "Description",
  "items": [],
  "actors": [],
  "exits": { "north": "loc_id" },
  "cached_image_path": null,
  "image_prompt": "Visual description",
  "x": 0,
  "y": 0
}

Item object:
{
  "id": "unique_id",
  "name": "Name",
  "description": "Description"
}

Rules:
1. If the user moves to an EXISTING exit, use `MoveTo`.
2. If the user moves to a NEW direction, use `CreateLocation` for the new room, `UpdateLocation` to link the current room to it, and `MoveTo` to go there.
3. Maintain spatial coherence. Assign grid coordinates (x, y) relative to the current location: north increases y by 1, south decreases y by 1, east increases x by 1, west decreases x by 1.
4. Use `AddItemToInventory` / `RemoveItemFromInventory` for picking up/dropping items.
5. Use `CreateItem` before adding a NEW item to inventory or location.
6. Provide `suggested_actions` that are relevant to the current situation (e.g., "go north", "take sword", "examine chest").
"#;

        self.log(&format!("Processing input: '{}'", input));

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

        for action_str in update.actions {
            self.parse_and_apply_action(&action_str)?;
        }
        Ok(())
    }

    fn parse_and_apply_action(&mut self, action_str: &str) -> Result<()> {
        let action_str = action_str.trim();
        if action_str.starts_with("MoveTo(") && action_str.ends_with(")") {
            let loc_id = &action_str[7..action_str.len()-1];
            let loc_id = loc_id.trim_matches('"');
            if self.world.locations.contains_key(loc_id) {
                self.world.current_location_id = loc_id.to_string();
            } else {
                eprintln!("Warning: LLM tried to move to non-existent location {}", loc_id);
            }
        } else if action_str.starts_with("CreateLocation(") && action_str.ends_with(")") {
            let json_str = &action_str[15..action_str.len()-1];
            let loc: crate::model::Location = serde_json::from_str(json_str)
                .context(format!("Failed to parse CreateLocation: {}", json_str))?;
            self.world.locations.insert(loc.id.clone(), loc);
        } else if action_str.starts_with("UpdateLocation(") && action_str.ends_with(")") {
            let json_str = &action_str[15..action_str.len()-1];
            let loc: crate::model::Location = serde_json::from_str(json_str)
                .context(format!("Failed to parse UpdateLocation: {}", json_str))?;
            self.world.locations.insert(loc.id.clone(), loc);
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
        } else if action_str.starts_with("AddItemToLocation(") && action_str.ends_with(")") {
            let params_str = &action_str[18..action_str.len()-1];
            // Parse "loc_id, item_id"
            if let Some(comma_pos) = params_str.find(',') {
                let location_id = params_str[..comma_pos].trim().trim_matches('"');
                let item_id = params_str[comma_pos+1..].trim().trim_matches('"');
                if let Some(loc) = self.world.locations.get_mut(location_id) {
                    if !loc.items.contains(&item_id.to_string()) {
                        loc.items.push(item_id.to_string());
                    }
                }
            }
        } else if action_str.starts_with("RemoveItemFromLocation(") && action_str.ends_with(")") {
            let params_str = &action_str[22..action_str.len()-1];
            // Parse "loc_id, item_id"
            if let Some(comma_pos) = params_str.find(',') {
                let location_id = params_str[..comma_pos].trim().trim_matches('"');
                let item_id = params_str[comma_pos+1..].trim().trim_matches('"');
                if let Some(loc) = self.world.locations.get_mut(location_id) {
                    loc.items.retain(|id| id != &item_id.to_string());
                }
            }
        } else {
            eprintln!("Unknown action: {}", action_str);
        }
        Ok(())
    }
}

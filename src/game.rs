use crate::model::{WorldState, Location};
use crate::llm::LlmClient;
use crate::agent::Agent;
use crate::save::{SaveManager, SaveInfo};
use crate::commands::Command;
use anyhow::Result;
use std::collections::HashMap;

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
        let command = Command::from_str(input);
        self.process_command(command).await
    }

    pub async fn process_command(&mut self, command: Command) -> Result<()> {
        match self.state {
            GameState::SplashScreen => self.handle_splash_command(command).await,
            GameState::NamingWorld => self.handle_naming_command(command).await,
            GameState::WaitingForInput => self.handle_game_command(command).await,
            _ => Ok(()),
        }
    }

    pub async fn generate_and_move_to(&mut self, target_pos: (i32, i32), direction: &str) -> Result<()> {
        let (target_x, target_y) = target_pos;

        self.log(&format!("Generating location at ({}, {}) heading {}", target_x, target_y, direction));

        let current_loc = self.world.locations.get(&self.world.current_pos)
            .ok_or_else(|| anyhow::anyhow!("Current location not found"))?;

        let prompt = format!(
            r#"Current Location: {} at ({}, {})
Description: {}

The player is heading {} toward coordinates ({}, {}).
This grid cell is currently EMPTY and needs to be generated.

Create a new location at ({}, {}) that fits thematically with the current location.
IMPORTANT: All exits must be null (blocked). The game will create actual exit connections automatically.

Return ONLY a valid JSON object:
{{
  "name": "Location name",
  "description": "Description of what the player sees",
  "image_prompt": "Visual description for generating an image",
  "exits": {{"north": null, "south": null, "east": null, "west": null}},
  "items": [],
  "actors": []
}}

CRITICAL: 
- exits MUST be null objects (blocked), NOT strings or booleans
- items MUST be an empty array []
- actors MUST be an empty array []
- NO narrative text, NO extra commentary

Just the JSON. Nothing else."#,
            current_loc.name,
            self.world.current_pos.0,
            self.world.current_pos.1,
            current_loc.description,
            direction, target_x, target_y,
            target_x, target_y
        );

        let system_prompt = "You are a world generator for a text adventure game. Create interesting, thematically consistent locations. You MUST output valid JSON only.";

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

    async fn handle_splash_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::New => {
                self.new_world_name.clear();
                self.state = GameState::NamingWorld;
            }
            Command::Load => {
                if !self.save_list.is_empty() {
                    let save = &self.save_list[self.selected_save_index];
                    self.world = self.save_manager.load_save(&save.filename)?;
                    self.current_save_path = Some(save.filename.clone());
                    self.state = GameState::WaitingForInput;
                    self.last_narrative = format!("Loaded world: {}. What do you want to do?", save.filename);
                }
            }
            Command::Up => {
                if self.selected_save_index > 0 {
                    self.selected_save_index -= 1;
                }
            }
            Command::Down => {
                if self.selected_save_index < self.save_list.len() {
                    self.selected_save_index += 1;
                }
            }
            Command::Delete => {
                if !self.save_list.is_empty() {
                    let save = &self.save_list[self.selected_save_index];
                    if let Err(e) = self.save_manager.delete_save(&save.filename) {
                        self.log(&format!("Failed to delete save: {}", e));
                    } else {
                        self.log(&format!("Deleted save: {}", save.filename));
                        self.save_list = self.save_manager.list_saves().unwrap_or_default();
                        if self.selected_save_index >= self.save_list.len() && self.selected_save_index > 0 {
                            self.selected_save_index = self.save_list.len() - 1;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_naming_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::Enter => {
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
            Command::Back => {
                self.state = GameState::SplashScreen;
                self.new_world_name.clear();
            }
            Command::Backspace => {
                self.new_world_name.pop();
            }
            Command::TextInput(text) => {
                self.new_world_name.push_str(&text);
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_game_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::MoveNorth => {
                self.handle_quick_movement("north").await?;
            }
            Command::MoveSouth => {
                self.handle_quick_movement("south").await?;
            }
            Command::MoveEast => {
                self.handle_quick_movement("east").await?;
            }
            Command::MoveWest => {
                self.handle_quick_movement("west").await?;
            }
            Command::SelectOption(idx) => {
                if idx > 0 && idx <= self.current_options.len() {
                    let selected_action = self.current_options[idx - 1].clone();
                    self.log(&format!("User selected option {}: {}", idx, selected_action));
                    self.handle_agent_action(&selected_action).await?;
                }
            }
            Command::TextInput(text) => {
                self.handle_agent_action(&text).await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_agent_action(&mut self, action: &str) -> Result<()> {
        self.state = GameState::Processing;
        self.status_message = "Thinking...".to_string();

        self.log(&format!("Processing action: '{}'", action));
        self.log(&format!("Current player position: {:?}", self.world.current_pos));

        let mut agent = Agent::new(self.llm_client.clone(), self.world.clone());
        let max_attempts = 3;
        let mut attempts = 0;

        loop {
            attempts += 1;
            self.status_message = format!("Attempt {}/{} - Processing...", attempts, max_attempts);
            self.log(&self.status_message.clone());

            match agent.process_action(action).await {
                Ok(response) => {
                    if response.narrative.contains("Failed after") ||
                       response.narrative.contains("Agent stopped") ||
                       response.narrative.contains("Timeout") {
                        self.last_narrative = response.narrative;
                    } else {
                        self.world = agent.take_world();
                        if let Some(path) = &self.current_save_path {
                            let _ = self.save_manager.save_game(path, &self.world);
                        }
                        self.last_narrative = response.narrative;
                    }
                    self.current_options = response.suggested_actions;
                    self.state = GameState::WaitingForInput;
                    self.status_message = "".to_string();
                    break;
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    let summary = if err_msg.len() > 50 { format!("{}...", &err_msg[..47]) } else { err_msg };
                    self.log(&format!("Agent Error (Attempt {}): {}", attempts, summary));

                    if attempts >= max_attempts {
                        self.last_narrative = format!("The spirits are confused. (Failed after {} attempts)\nError: {}", max_attempts, summary);
                        self.status_message = "Failed.".to_string();
                        self.state = GameState::WaitingForInput;
                        break;
                    }

                    #[cfg(not(target_arch = "wasm32"))]
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    #[cfg(target_arch = "wasm32")]
                    {
                         let promise = js_sys::Promise::new(&mut |resolve, _| {
                             web_sys::window().unwrap().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 500).unwrap();
                         });
                         wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_quick_movement(&mut self, direction: &str) -> Result<()> {
        let (x, y) = self.world.current_pos;
        let target_pos = match direction {
            "north" => (x, y + 1),
            "south" => (x, y - 1),
            "east" => (x + 1, y),
            "west" => (x - 1, y),
            _ => return Ok(()),
        };

        // If location exists, quick move (no LLM)
        if let Some(target_loc) = self.world.locations.get(&target_pos).cloned() {
            self.world.current_pos = target_pos;
            if let Some(loc) = self.world.locations.get_mut(&target_pos) {
                loc.visited = true;
            }
            self.last_narrative = format!("You move {} to {}.\n{}", direction, target_loc.name, target_loc.description);
            self.log(&format!("Quick move {} to existing location ({}, {})", direction, target_pos.0, target_pos.1));
            if let Some(path) = &self.current_save_path {
                let _ = self.save_manager.save_game(path, &self.world);
            }
        } else {
            // New location - must use LLM
            self.generate_and_move_to(target_pos, direction).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_creation() {
        let llm_client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let game = Game::new(llm_client);
        assert_eq!(game.state, GameState::SplashScreen);
        assert_eq!(game.debug_log.len(), 1);
    }

    #[test]
    fn test_log_functionality() {
        let llm_client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let mut game = Game::new(llm_client);

        game.log("Test log message");
        assert_eq!(game.debug_log.len(), 2);
        assert!(game.debug_log[1].contains("Test log message"));
    }

    #[test]
    fn test_log_truncation() {
        let llm_client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let mut game = Game::new(llm_client);

        for i in 0..105 {
            game.log(&format!("Message {}", i));
        }

        assert_eq!(game.debug_log.len(), 100);
        assert!(!game.debug_log.iter().any(|msg| msg.contains("Message 0")));
        assert!(game.debug_log.iter().any(|msg| msg.contains("Message 104")));
    }
}

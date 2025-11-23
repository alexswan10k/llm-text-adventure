use crate::model::{WorldState, WorldUpdate, Location, Item};
use crate::llm::LlmClient;
use anyhow::{Result, Context};
use std::collections::HashMap;

pub enum GameState {
    WaitingForInput,
    Processing,
    UpdatingWorld,
    Rendering,
}

pub struct Game {
    pub world: WorldState,
    pub llm_client: LlmClient,
    pub last_narrative: String,
    pub state: GameState,
}

impl Game {
    pub fn new(llm_client: LlmClient) -> Self {
        let mut world = WorldState::new();
        // Initialize with a default location if empty
        if world.locations.is_empty() {
            let start_loc = Location {
                id: "start".to_string(),
                name: "The Beginning".to_string(),
                description: "You stand in a void of potential. Anything can happen here.".to_string(),
                items: vec![],
                cached_image_path: None,
                image_prompt: "A swirling void of colors and shapes, representing potential.".to_string(),
            };
            world.locations.insert("start".to_string(), start_loc);
        }

        Self {
            world,
            llm_client,
            last_narrative: "Welcome to the Infinite Text Adventure. What do you want to do?".to_string(),
            state: GameState::WaitingForInput,
        }
    }

    pub async fn process_input(&mut self, input: &str) -> Result<()> {
        self.state = GameState::Processing;

        // 1. Construct Context
        let current_loc = self.world.locations.get(&self.world.current_location_id)
            .context("Current location not found in world map")?;
        
        let context_str = format!(
            "Current Location: {}\nDescription: {}\nItems here: {:?}\nInventory: {:?}\n\nLast Narrative: {}\n\nUser Action: {}",
            current_loc.name,
            current_loc.description,
            current_loc.items,
            self.world.global_inventory,
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
  "new_location": null, // Or a Location object if a NEW location is created
  "updated_location": null, // Or a Location object if the CURRENT location changes
  "inventory_add": [], // List of Item objects to add to inventory
  "inventory_remove": [], // List of Item IDs to remove from inventory
  "move_to_location_id": null // String ID if the user moves to a DIFFERENT existing location
}

Location object:
{
  "id": "unique_id",
  "name": "Name",
  "description": "Description",
  "items": [],
  "cached_image_path": null,
  "image_prompt": "Visual description for image generation"
}

Item object:
{
  "id": "unique_id",
  "name": "Name",
  "description": "Description"
}

If the user moves to a new area that doesn't exist, create it in `new_location` and set `move_to_location_id` to its ID.
If the user stays, you can update the current location in `updated_location`.
Ensure IDs are unique and consistent.
"#;

        // 2. Call LLM
        let update = self.llm_client.generate_update(system_prompt, &context_str).await?;

        // 3. Apply Update
        self.state = GameState::UpdatingWorld;
        self.apply_update(update)?;

        self.state = GameState::WaitingForInput;
        Ok(())
    }

    fn apply_update(&mut self, update: WorldUpdate) -> Result<()> {
        self.last_narrative = update.narrative.clone();

        // Handle new location
        if let Some(new_loc) = update.new_location {
            self.world.locations.insert(new_loc.id.clone(), new_loc);
        }

        // Handle updated location
        if let Some(updated_loc) = update.updated_location {
            self.world.locations.insert(updated_loc.id.clone(), updated_loc);
        }

        // Handle inventory add
        if let Some(items) = update.inventory_add {
            self.world.global_inventory.extend(items);
        }

        // Handle inventory remove
        if let Some(item_ids) = update.inventory_remove {
            self.world.global_inventory.retain(|item| !item_ids.contains(&item.id));
        }

        // Handle movement
        if let Some(loc_id) = update.move_to_location_id {
            if self.world.locations.contains_key(&loc_id) {
                self.world.current_location_id = loc_id;
            } else {
                // Fallback or error if LLM hallucinates a non-existent ID without creating it
                // For now, just log or ignore, maybe stay put.
                eprintln!("Warning: LLM tried to move to non-existent location {}", loc_id);
            }
        }

        Ok(())
    }
}

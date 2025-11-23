use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct WorldState {
    pub current_location_id: String,
    pub locations: HashMap<String, Location>,
    pub actors: Vec<Actor>,
    pub global_inventory: Vec<Item>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Location {
    pub id: String,
    pub name: String,
    pub description: String,
    pub items: Vec<Item>,
    pub cached_image_path: Option<String>,
    pub image_prompt: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Actor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub current_location_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub description: String,
}

// The structure returned by the LLM
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorldUpdate {
    pub narrative: String,
    pub new_location: Option<Location>, // If we moved to a new place or it changed significantly
    pub updated_location: Option<Location>, // Updates to current location
    pub inventory_add: Option<Vec<Item>>,
    pub inventory_remove: Option<Vec<String>>, // Item IDs
    pub move_to_location_id: Option<String>,
}

impl WorldState {
    pub fn new() -> Self {
        Self {
            current_location_id: "start".to_string(),
            locations: HashMap::new(),
            actors: Vec::new(),
            global_inventory: Vec::new(),
        }
    }
}

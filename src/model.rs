use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct WorldState {
    pub current_location_id: String,
    pub locations: HashMap<String, Location>,
    pub actors: HashMap<String, Actor>, // Changed to HashMap for easier lookup
    pub items: HashMap<String, Item>,   // Global registry of all items
    pub player: Player,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Player {
    pub inventory: Vec<String>, // List of Item IDs
    pub money: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Location {
    pub id: String,
    pub name: String,
    pub description: String,
    pub items: Vec<String>, // List of Item IDs currently here
    pub actors: Vec<String>, // List of Actor IDs currently here
    pub exits: HashMap<String, Option<String>>, // Direction -> Location ID (Option for explicit null/blocked)
    pub cached_image_path: Option<String>,
    pub image_prompt: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Actor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub current_location_id: String,
    pub inventory: Vec<String>, // List of Item IDs
    pub money: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub description: String,
    // potentially other properties like "is_carryable", "value", etc.
}

// Atomic actions the LLM can take
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum GameAction {
    CreateLocation(Location),
    UpdateLocation(Location),
    CreateItem(Item),
    AddItemToInventory(String), // item_id
    RemoveItemFromInventory(String), // item_id
    MoveTo(String), // location_id
    // Add more as needed, e.g., AddItemToLocation, RemoveItemFromLocation
    AddItemToLocation { location_id: String, item_id: String },
    RemoveItemFromLocation { location_id: String, item_id: String },
}

// The structure returned by the LLM
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorldUpdate {
    pub narrative: String,
    pub actions: Vec<GameAction>,
}

impl WorldState {
    pub fn new() -> Self {
        Self {
            current_location_id: "start".to_string(),
            locations: HashMap::new(),
            actors: HashMap::new(),
            items: HashMap::new(),
            player: Player::default(),
        }
    }
}

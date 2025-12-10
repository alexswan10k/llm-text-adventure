use serde::{Deserialize, Serialize, Deserializer, Serializer};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct WorldState {
    pub current_pos: (i32, i32),  // Replaces current_location_id: String
    #[serde(serialize_with = "serialize_coords", deserialize_with = "deserialize_coords")]
    pub locations: HashMap<(i32, i32), Location>,  // Coord -> Location (primary key)
    pub actors: HashMap<String, Actor>, // Changed to HashMap for easier lookup
    pub items: HashMap<String, Item>,   // Global registry of all items
    pub player: Player,
}

// Helper functions for serializing coordinate HashMaps
fn serialize_coords<S, T>(map: &HashMap<(i32, i32), T>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    use serde::ser::SerializeMap;
    let mut seq = serializer.serialize_map(Some(map.len()))?;
    for ((x, y), value) in map {
        let key = format!("{},{}", x, y);
        seq.serialize_entry(&key, value)?;
    }
    seq.end()
}

fn deserialize_coords<'de, D, T>(deserializer: D) -> Result<HashMap<(i32, i32), T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    use std::collections::BTreeMap;
    
    let string_map: BTreeMap<String, T> = Deserialize::deserialize(deserializer)?;
    let mut coord_map = HashMap::new();
    
    for (key_str, value) in string_map {
        if let Some((x_str, y_str)) = key_str.split_once(',') {
            if let (Ok(x), Ok(y)) = (x_str.parse::<i32>(), y_str.parse::<i32>()) {
                coord_map.insert((x, y), value);
            }
        }
    }
    
    Ok(coord_map)
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Player {
    pub inventory: Vec<String>, // List of Item IDs
    pub money: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Location {
    pub name: String,
    pub description: String,
    pub items: Vec<String>, // List of Item IDs currently here
    pub actors: Vec<String>, // List of Actor IDs currently here
    pub exits: HashMap<String, Option<(i32, i32)>>, // Dir -> Coord or None (blocked)
    pub cached_image_path: Option<String>,
    pub image_prompt: String,
    pub visited: bool,  // New: Track if player has been here (for fog-of-war)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Actor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub current_pos: (i32, i32),  // Replaces current_location_id: String
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
    CreateLocation((i32, i32), Location),  // coord, location
    UpdateLocation((i32, i32), Location),  // coord, location
    CreateItem(Item),
    AddItemToInventory(String), // item_id
    RemoveItemFromInventory(String), // item_id
    MoveTo((i32, i32)), // coord
    // Add more as needed, e.g., AddItemToLocation, RemoveItemFromLocation
    AddItemToLocation { pos: (i32, i32), item_id: String },
    RemoveItemFromLocation { pos: (i32, i32), item_id: String },
}

// The structure returned by the LLM
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorldUpdate {
    pub narrative: String,
    pub actions: Vec<String>,
    pub suggested_actions: Vec<String>,
}

impl WorldState {
    pub fn new() -> Self {
        Self {
            current_pos: (0, 0),  // Starting at origin
            locations: HashMap::new(),
            actors: HashMap::new(),
            items: HashMap::new(),
            player: Player::default(),
        }
    }
}

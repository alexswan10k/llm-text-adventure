use serde::{Deserialize, Serialize, Deserializer, Serializer};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorldState {
    pub current_pos: (i32, i32),  // Replaces current_location_id: String
    #[serde(serialize_with = "serialize_coords", deserialize_with = "deserialize_coords")]
    pub locations: HashMap<(i32, i32), Location>,  // Coord -> Location (primary key)
    pub actors: HashMap<String, Actor>, // Changed to HashMap for easier lookup
    pub items: HashMap<String, Item>,   // Global registry of all items
    pub player: Player,
    pub combat: CombatState,
    pub max_items: u32,
    pub max_combatants: u32,
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            current_pos: (0, 0),
            locations: HashMap::new(),
            actors: HashMap::new(),
            items: HashMap::new(),
            player: Player::default(),
            combat: CombatState::default(),
            max_items: 20,
            max_combatants: 4,
        }
    }
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
    #[serde(default = "default_location_name")]
    pub name: String,
    #[serde(default = "default_location_description")]
    pub description: String,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub actors: Vec<String>,
    #[serde(default)]
    pub exits: HashMap<String, Option<(i32, i32)>>,
    #[serde(default)]
    pub cached_image_path: Option<String>,
    #[serde(default = "default_image_prompt")]
    pub image_prompt: String,
    #[serde(default)]
    pub visited: bool,
}

fn default_location_name() -> String {
    "Unknown Location".to_string()
}

fn default_location_description() -> String {
    "An unknown place.".to_string()
}

fn default_image_prompt() -> String {
    "A mysterious location".to_string()
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ItemType {
    Weapon,
    Armor,
    Consumable,
    Tool,
    Key,
    Container,
    QuestItem,
    Material,
}

impl std::fmt::Display for ItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemType::Weapon => write!(f, "Weapon"),
            ItemType::Armor => write!(f, "Armor"),
            ItemType::Consumable => write!(f, "Consumable"),
            ItemType::Tool => write!(f, "Tool"),
            ItemType::Key => write!(f, "Key"),
            ItemType::Container => write!(f, "Container"),
            ItemType::QuestItem => write!(f, "QuestItem"),
            ItemType::Material => write!(f, "Material"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ItemState {
    Normal,
    Equipped,
    Damaged { durability: u32, max_durability: u32 },
    Consumed { charges: u32, max_charges: u32 },
    Locked { key_id: Option<String> },
    Open { contents: Vec<String> },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ItemProperties {
    pub damage: Option<u32>,
    pub defense: Option<u32>,
    pub value: Option<u32>,
    pub weight: Option<u32>,
    pub carryable: bool,
    pub usable: bool,
    pub equip_slot: Option<String>,
    pub status_effects: Vec<String>,
}

impl Default for ItemProperties {
    fn default() -> Self {
        Self {
            damage: None,
            defense: None,
            value: None,
            weight: None,
            carryable: true,
            usable: false,
            equip_slot: None,
            status_effects: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum StatusType {
    Poison,
    Stunned,
    Burning,
    Frozen,
    Bleeding,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatusEffect {
    pub effect_type: StatusType,
    pub duration: u32,
    pub severity: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Combatant {
    pub id: String,
    pub is_player: bool,
    pub hp: u32,
    pub max_hp: u32,
    pub weapon_id: Option<String>,
    pub armor_id: Option<String>,
    pub initiative: u32,
    pub status_effects: Vec<StatusEffect>,
    pub temp_defense: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CombatState {
    pub active: bool,
    pub combatants: Vec<Combatant>,
    pub current_turn_index: usize,
    pub round_number: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub description: String,
    pub item_type: ItemType,
    pub state: ItemState,
    pub properties: ItemProperties,
}

// Atomic actions the LLM can take
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum GameAction {
    CreateLocation((i32, i32), Location),
    UpdateLocation((i32, i32), Location),
    CreateItem(Item),
    AddItemToInventory(String),
    RemoveItemFromInventory(String),
    MoveTo((i32, i32)),
    AddItemToLocation { pos: (i32, i32), item_id: String },
    RemoveItemFromLocation { pos: (i32, i32), item_id: String },

    // Item Actions
    UseItem(String),
    EquipItem(String),
    UnequipItem(String),
    CombineItems { item1_id: String, item2_id: String, result_id: String },
    SetItemState { item_id: String, state: ItemState },
    BreakItem(String),
    AddItemToContainer { container_id: String, item_id: String },
    RemoveItemFromContainer { container_id: String, item_id: String },

    StartCombat { enemy_ids: Vec<String> },
    AttackActor { attacker_id: String, target_id: String, weapon_id: Option<String> },
    Defend { actor_id: String },
    Flee { actor_id: String },
    UseItemInCombat { user_id: String, item_id: String, target_id: Option<String> },
    EndTurn { actor_id: String },
    EndCombat { victor_id: String },
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
            combat: CombatState::default(),
            max_items: 20,
            max_combatants: 4,
        }
    }
}

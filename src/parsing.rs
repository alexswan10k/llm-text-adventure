use anyhow::{anyhow, Result};
use crate::model::{Item, Location};

#[derive(Debug)]
pub enum ParsedAction {
    MoveTo(i32, i32),
    CreateLocation((i32, i32), Location),
    UpdateLocation((i32, i32), Location),
    CreateItem(Item),
    AddItemToInventory(String),
    RemoveItemFromInventory(String),
    AddItemToLocation { pos: (i32, i32), item_id: String },
    RemoveItemFromLocation { pos: (i32, i32), item_id: String },
    UseItem(String),
    EquipItem(String),
    UnequipItem(String),
    CombineItems { item1_id: String, item2_id: String, result_id: String },
    SetItemState { item_id: String, state: serde_json::Value },
    BreakItem(String),
    AddItemToContainer { container_id: String, item_id: String },
    RemoveItemFromContainer { container_id: String, item_id: String },
}

pub struct ActionParser {
    debug_log: Vec<String>,
}

impl ActionParser {
    pub fn new() -> Self {
        Self {
            debug_log: Vec::new(),
        }
    }

    pub fn log(&mut self, message: &str) {
        self.debug_log.push(format!("[{}] {}", chrono::Local::now().format("%H:%M:%S"), message));
        if self.debug_log.len() > 100 {
            self.debug_log.remove(0);
        }
    }

    pub fn get_debug_log(&self) -> &[String] {
        &self.debug_log
    }

    pub fn parse_action(&mut self, action_str: &str) -> Result<ParsedAction> {
        let action_str = action_str.trim();
        self.log(&format!("Parsing action: '{}'", action_str));

        // Check for obviously truncated actions
        if action_str.len() < 10 {
            return Err(anyhow!("Action too short: '{}'", action_str));
        }

        // CreateItem({item JSON object})
        if action_str.starts_with("CreateItem(") && action_str.ends_with(")") {
            return self.parse_create_item(action_str);
        }

        // Add more action parsers here as we refactor them
        Err(anyhow!("Unknown action format: '{}'", action_str))
    }

    fn parse_create_item(&mut self, action_str: &str) -> Result<ParsedAction> {
        let json_str = &action_str[11..action_str.len()-1].trim();
        self.log(&format!("CreateItem JSON string: '{}'", json_str));
        
        // Validate JSON structure before parsing
        if !json_str.starts_with('{') || !json_str.ends_with('}') {
            return Err(anyhow!("CreateItem JSON must be wrapped in braces: {}", json_str));
        }
        
        // Additional validation for common truncation patterns
        if !json_str.contains("\"id\"") {
            return Err(anyhow!("CreateItem JSON missing required 'id' field: {}", json_str));
        }
        
        if !json_str.contains("\"name\"") {
            return Err(anyhow!("CreateItem JSON missing required 'name' field: {}", json_str));
        }
        
        if !json_str.contains("\"item_type\"") {
            return Err(anyhow!("CreateItem JSON missing required 'item_type' field: {}", json_str));
        }

        let item: Item = serde_json::from_str(json_str)
            .map_err(|e| anyhow!("Failed to parse CreateItem JSON: {}. JSON was: {}", e, json_str))?;
        
        self.log(&format!("Successfully parsed CreateItem: {}", item.id));
        Ok(ParsedAction::CreateItem(item))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ItemType, ItemState, ItemProperties};

    fn create_test_item() -> Item {
        Item {
            id: "test_item".to_string(),
            name: "Test Item".to_string(),
            description: "A test item".to_string(),
            item_type: ItemType::Tool,
            state: ItemState::Normal,
            properties: ItemProperties {
                damage: None,
                defense: None,
                value: Some(10),
                weight: Some(1),
                carryable: true,
                usable: true,
                equip_slot: None,
                status_effects: vec![],
            },
        }
    }

    #[test]
    fn test_parse_create_item_success() {
        let mut parser = ActionParser::new();
        let item = create_test_item();
        let json = serde_json::to_string(&item).unwrap();
        let action = format!("CreateItem({})", json);
        
        let result = parser.parse_action(&action);
        assert!(result.is_ok());
        
        match result.unwrap() {
            ParsedAction::CreateItem(parsed_item) => {
                assert_eq!(parsed_item.id, "test_item");
                assert_eq!(parsed_item.name, "Test Item");
                assert_eq!(parsed_item.item_type, ItemType::Tool);
            }
            _ => panic!("Expected CreateItem action"),
        }
    }

    #[test]
    fn test_parse_create_item_truncated_json() {
        let mut parser = ActionParser::new();
        let truncated_json = r#"{"id": "test_item", "name": "Test""#;
        let action = format!("CreateItem({})", truncated_json);
        
        let result = parser.parse_action(&action);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Failed to parse CreateItem JSON") || error_msg.contains("JSON must be wrapped in braces"));
    }

    #[test]
    fn test_parse_create_item_missing_id() {
        let mut parser = ActionParser::new();
        let invalid_json = r#"{"name": "Test Item", "item_type": "Tool"}"#;
        let action = format!("CreateItem({})", invalid_json);
        
        let result = parser.parse_action(&action);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing required 'id' field"));
    }

    #[test]
    fn test_parse_create_item_missing_name() {
        let mut parser = ActionParser::new();
        let invalid_json = r#"{"id": "test_item", "item_type": "Tool"}"#;
        let action = format!("CreateItem({})", invalid_json);
        
        let result = parser.parse_action(&action);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing required 'name' field"));
    }

    #[test]
    fn test_parse_create_item_missing_item_type() {
        let mut parser = ActionParser::new();
        let invalid_json = r#"{"id": "test_item", "name": "Test Item"}"#;
        let action = format!("CreateItem({})", invalid_json);
        
        let result = parser.parse_action(&action);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing required 'item_type' field"));
    }

    #[test]
    fn test_parse_create_item_invalid_format() {
        let mut parser = ActionParser::new();
        let result = parser.parse_action("CreateItem(invalid)");
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("JSON must be wrapped in braces"));
    }

    #[test]
    fn test_parse_action_too_short() {
        let mut parser = ActionParser::new();
        let result = parser.parse_action("CreateIte");
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Action too short"));
    }

    #[test]
    fn test_parse_create_item_with_complex_state() {
        let mut parser = ActionParser::new();
        let item = Item {
            id: "damaged_sword".to_string(),
            name: "Damaged Sword".to_string(),
            description: "A sword that has seen better days".to_string(),
            item_type: ItemType::Weapon,
            state: ItemState::Damaged { durability: 5, max_durability: 10 },
            properties: ItemProperties {
                damage: Some(15),
                defense: None,
                value: Some(25),
                weight: Some(3),
                carryable: true,
                usable: true,
                equip_slot: Some("weapon".to_string()),
                status_effects: vec![],
            },
        };

        let json = serde_json::to_string(&item).unwrap();
        let action = format!("CreateItem({})", json);
        
        let result = parser.parse_action(&action);
        assert!(result.is_ok());
        
        match result.unwrap() {
            ParsedAction::CreateItem(parsed_item) => {
                assert_eq!(parsed_item.id, "damaged_sword");
                match parsed_item.state {
                    ItemState::Damaged { durability, max_durability } => {
                        assert_eq!(durability, 5);
                        assert_eq!(max_durability, 10);
                    }
                    _ => panic!("Expected Damaged state"),
                }
            }
            _ => panic!("Expected CreateItem action"),
        }
    }

    #[test]
    fn test_debug_logging() {
        let mut parser = ActionParser::new();
        let item = create_test_item();
        let json = serde_json::to_string(&item).unwrap();
        let action = format!("CreateItem({})", json);
        
        parser.parse_action(&action).unwrap();
        let log = parser.get_debug_log();
        
        assert!(!log.is_empty());
        assert!(log.iter().any(|entry| entry.contains("Parsing action")));
        assert!(log.iter().any(|entry| entry.contains("Successfully parsed CreateItem")));
    }
}
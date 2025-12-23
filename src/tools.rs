use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "move_to",
            description: "Move the player to a specific coordinate on the map",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer", "description": "X coordinate on the grid"},
                    "y": {"type": "integer", "description": "Y coordinate on the grid"}
                },
                "required": ["x", "y"]
            }),
        },
        ToolDefinition {
            name: "create_location",
            description: "Create a new location at specific coordinates",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer"},
                    "y": {"type": "integer"},
                    "name": {"type": "string"},
                    "description": {"type": "string"},
                    "image_prompt": {"type": "string"},
                    "exits": {
                        "type": "object",
                        "description": "Map of direction to [x, y] coordinates or null",
                        "properties": {
                            "north": {"anyOf": [{"type": "array", "items": {"type": "integer"}}, {"type": "null"}]},
                            "south": {"anyOf": [{"type": "array", "items": {"type": "integer"}}, {"type": "null"}]},
                            "east": {"anyOf": [{"type": "array", "items": {"type": "integer"}}, {"type": "null"}]},
                            "west": {"anyOf": [{"type": "array", "items": {"type": "integer"}}, {"type": "null"}]}
                        }
                    },
                    "items": {"type": "array", "items": {"type": "string"}, "default": []},
                    "actors": {"type": "array", "items": {"type": "string"}, "default": []}
                },
                "required": ["x", "y", "name", "description", "image_prompt"]
            }),
        },
        ToolDefinition {
            name: "create_item",
            description: "Create a new item in the world",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Unique identifier for the item"},
                    "name": {"type": "string"},
                    "description": {"type": "string"},
                    "item_type": {
                        "type": "string",
                        "enum": ["Weapon", "Armor", "Consumable", "Tool", "Key", "Container", "QuestItem", "Material"]
                    },
                    "state": {
                        "oneOf": [
                            {"type": "string", "enum": ["Normal", "Equipped"]},
                            {
                                "type": "object",
                                "properties": {
                                    "Damaged": {
                                        "type": "object",
                                        "properties": {
                                            "durability": {"type": "integer"},
                                            "max_durability": {"type": "integer"}
                                        },
                                        "required": ["durability", "max_durability"]
                                    }
                                }
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "Consumed": {
                                        "type": "object",
                                        "properties": {
                                            "charges": {"type": "integer"},
                                            "max_charges": {"type": "integer"}
                                        },
                                        "required": ["charges", "max_charges"]
                                    }
                                }
                            }
                        ]
                    },
                    "properties": {
                        "type": "object",
                        "properties": {
                            "damage": {"type": "integer"},
                            "defense": {"type": "integer"},
                            "value": {"type": "integer"},
                            "weight": {"type": "integer"},
                            "carryable": {"type": "boolean"},
                            "usable": {"type": "boolean"},
                            "equip_slot": {"type": "string", "enum": ["weapon", "armor", null]},
                            "status_effects": {"type": "array", "items": {"type": "string"}}
                        }
                    }
                },
                "required": ["id", "name", "description", "item_type"]
            }),
        },
        ToolDefinition {
            name: "add_item_to_inventory",
            description: "Add an existing item to the player's inventory",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "item_id": {"type": "string"}
                },
                "required": ["item_id"]
            }),
        },
        ToolDefinition {
            name: "remove_item_from_inventory",
            description: "Remove an item from the player's inventory",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "item_id": {"type": "string"}
                },
                "required": ["item_id"]
            }),
        },
        ToolDefinition {
            name: "add_item_to_location",
            description: "Add an item to a specific location",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer"},
                    "y": {"type": "integer"},
                    "item_id": {"type": "string"}
                },
                "required": ["x", "y", "item_id"]
            }),
        },
        ToolDefinition {
            name: "remove_item_from_location",
            description: "Remove an item from a specific location",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer"},
                    "y": {"type": "integer"},
                    "item_id": {"type": "string"}
                },
                "required": ["x", "y", "item_id"]
            }),
        },
        ToolDefinition {
            name: "use_item",
            description: "Use an item (activates consumables or tools)",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "item_id": {"type": "string"}
                },
                "required": ["item_id"]
            }),
        },
        ToolDefinition {
            name: "equip_item",
            description: "Equip an item to its slot",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "item_id": {"type": "string"}
                },
                "required": ["item_id"]
            }),
        },
        ToolDefinition {
            name: "unequip_item",
            description: "Unequip an item",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "item_id": {"type": "string"}
                },
                "required": ["item_id"]
            }),
        },
        ToolDefinition {
            name: "combine_items",
            description: "Combine two items into a new item",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "item1_id": {"type": "string"},
                    "item2_id": {"type": "string"},
                    "result_id": {"type": "string"}
                },
                "required": ["item1_id", "item2_id", "result_id"]
            }),
        },
        ToolDefinition {
            name: "break_item",
            description: "Break and remove an item from the world",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "item_id": {"type": "string"}
                },
                "required": ["item_id"]
            }),
        },
        ToolDefinition {
            name: "add_item_to_container",
            description: "Add an item to a container",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "container_id": {"type": "string"},
                    "item_id": {"type": "string"}
                },
                "required": ["container_id", "item_id"]
            }),
        },
        ToolDefinition {
            name: "remove_item_from_container",
            description: "Remove an item from a container",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "container_id": {"type": "string"},
                    "item_id": {"type": "string"}
                },
                "required": ["container_id", "item_id"]
            }),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions_exist() {
        let tools = get_tool_definitions();
        assert!(!tools.is_empty());
        assert_eq!(tools.len(), 14);
    }

    #[test]
    fn test_move_to_tool_schema() {
        let tools = get_tool_definitions();
        let move_to = tools.iter().find(|t| t.name == "move_to").unwrap();
        assert_eq!(move_to.name, "move_to");
        let params = &move_to.parameters;
        assert_eq!(params["type"], "object");
        assert!(params["properties"]["x"].is_object());
        assert!(params["properties"]["y"].is_object());
        assert!(params["required"].is_array());
    }

    #[test]
    fn test_create_item_tool_schema() {
        let tools = get_tool_definitions();
        let create_item = tools.iter().find(|t| t.name == "create_item").unwrap();
        assert_eq!(create_item.name, "create_item");
        let params = &create_item.parameters;
        assert!(params["properties"]["id"].is_object());
        assert!(params["properties"]["item_type"]["enum"].is_array());
    }

    #[test]
    fn test_tool_call_serialization() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            call_type: "function".to_string(),
            function: ToolFunction {
                name: "create_item".to_string(),
                arguments: r#"{"id":"test","name":"Test Item"}"#.to_string(),
            },
        };

        let json = serde_json::to_string(&tool_call).unwrap();
        assert!(json.contains("call_123"));
        assert!(json.contains("create_item"));

        let deserialized: ToolCall = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "call_123");
        assert_eq!(deserialized.function.name, "create_item");
    }

    #[test]
    fn test_tool_result_serialization() {
        let result = ToolResult {
            tool_call_id: "call_123".to_string(),
            content: "Item created successfully".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Item created successfully"));

        let deserialized: ToolResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tool_call_id, "call_123");
        assert_eq!(deserialized.content, "Item created successfully");
    }
}

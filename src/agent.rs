use crate::model::{WorldState, Item, Location, ItemState, ItemProperties, ItemType};
use crate::tools::{ToolCall, ToolResult, ToolFunction, get_tool_definitions};
use crate::llm::LlmClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// No observation tools needed - all tools are batch executed
const OBSERVATION_TOOLS: &[&str] = &[];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<LlmMessage>,
    pub tools: Option<Vec<serde_json::Value>>,
    pub tool_choice: Option<serde_json::Value>,
    pub temperature: f32,
    pub max_tokens: i32,
}

#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub narrative: String,
    pub suggested_actions: Vec<String>,
}

pub struct Agent {
    llm_client: LlmClient,
    world: WorldState,
    max_iterations: usize,
    overall_timeout_seconds: u64,
    debug_log: Vec<String>,
}

impl Agent {
    pub fn new(llm_client: LlmClient, world: WorldState) -> Self {
        Self {
            llm_client,
            world,
            max_iterations: 3,
            overall_timeout_seconds: 60,
            debug_log: Vec::new(),
        }
    }

    pub fn log(&mut self, message: &str) {
        self.debug_log.push(format!("[Agent] {}", message));
        if self.debug_log.len() > 100 {
            self.debug_log.remove(0);
        }
    }

    pub fn get_debug_log(&self) -> &[String] {
        &self.debug_log
    }

    pub fn take_world(self) -> WorldState {
        self.world
    }

    pub async fn process_action(&mut self, user_input: &str) -> Result<AgentResponse> {
        self.log(&format!("Processing user action: {}", user_input));

        let start_time = std::time::Instant::now();
        let overall_timeout = std::time::Duration::from_secs(self.overall_timeout_seconds);

        let mut messages = vec![
            self.build_system_message(),
            self.build_user_message(user_input),
        ];

        let mut narrative = String::new();
        let mut iteration = 0;
        let mut last_tool_name: Option<String> = None;

        loop {
            iteration += 1;

            if start_time.elapsed() > overall_timeout {
                self.log(&format!("Overall timeout reached ({}s)", self.overall_timeout_seconds));
                return Ok(AgentResponse {
                    narrative: format!("{} [Timeout: The game took too long to respond]", narrative),
                    suggested_actions: self.extract_suggested_actions(&narrative),
                });
            }

            self.log(&format!("Agent iteration {}", iteration));

            if iteration > self.max_iterations {
                self.log("Max iterations reached, breaking");
                break;
            }

            let tools = get_tool_definitions();
            let tool_schemas: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect();

            let request = LlmRequest {
                model: self.llm_client.model_name.clone(),
                messages: messages.clone(),
                tools: Some(tool_schemas),
                tool_choice: None,
                temperature: 0.7,
                max_tokens: 4096,
            };

            let response = self.llm_client.send_chat_request(&request).await?;
            let response_content = response.get("content").and_then(|c| c.as_str());
            let response_tool_calls: Option<Vec<ToolCall>> = response
                .get("tool_calls")
                .and_then(|tc| serde_json::from_value(tc.clone()).ok());

            if let Some(content) = response_content {
                if !content.is_empty() {
                    self.log(&format!("Received narrative content ({} chars)", content.len()));
                    if narrative.is_empty() {
                        narrative = content.to_string();
                    } else {
                        narrative.push_str("\n\n");
                        narrative.push_str(content);
                    }
                }
            }

            let (tool_calls_needs_llm, _has_tools) = match (&response_tool_calls, response_content) {
                (None, None) => {
                    self.log("No content, no tool calls - LLM didn't understand");
                    (false, false)
                },
                (None, Some(_)) => {
                    self.log("No tool calls but we have content - done");
                    (false, false)
                },
                (Some(tool_calls), content) => {
                    let has_observation_tool = tool_calls.iter()
                        .any(|tc| OBSERVATION_TOOLS.contains(&tc.function.name.as_str()));

                    let first_tool_name = tool_calls.get(0).map(|tc| tc.function.name.clone());

                    if let (Some(last), Some(current)) = (&last_tool_name, &first_tool_name) {
                        if last == current && has_observation_tool {
                            self.log(&format!("Detected repeated observation tool call '{}'. Breaking to prevent loop.", last));
                            return Ok(AgentResponse {
                                narrative: format!("{} [Agent stopped due to repeated tool calls]", narrative),
                                suggested_actions: self.extract_suggested_actions(&narrative),
                            });
                        }
                    }

                    if has_observation_tool {
                        self.log(&format!("Got {} tool call(s) with observation tool - will loop", tool_calls.len()));
                    } else {
                        self.log(&format!("Got {} batch tool call(s) - will not loop", tool_calls.len()));
                    }

                    for tool_call in tool_calls {
                        let result = self.execute_tool_call(tool_call)?;
                        let tool_result_msg = LlmMessage {
                            role: "tool".to_string(),
                            content: Some(result.content.clone()),
                            tool_calls: None,
                            tool_call_id: Some(tool_call.id.clone()),
                        };
                        messages.push(tool_result_msg);
                    }

                    let assistant_msg = LlmMessage {
                        role: "assistant".to_string(),
                        content: content.map(|c| c.to_string()),
                        tool_calls: Some(tool_calls.clone()),
                        tool_call_id: None,
                    };
                    messages.push(assistant_msg);

                    last_tool_name = first_tool_name;
                    (has_observation_tool, true)
                },
            };

            if !tool_calls_needs_llm {
                self.log("No more LLM calls needed, ending agent loop");
                break;
            }
        }

        let suggested_actions = self.extract_suggested_actions(&narrative);
        Ok(AgentResponse {
            narrative,
            suggested_actions,
        })
    }

    fn build_system_message(&self) -> LlmMessage {
        let default_loc = Location {
            name: "Unknown".to_string(),
            description: "You are nowhere.".to_string(),
            items: vec![],
            actors: vec![],
            exits: std::collections::HashMap::new(),
            cached_image_path: None,
            image_prompt: String::new(),
            visited: false,
        };
        let current_loc = self.world.locations.get(&self.world.current_pos)
            .unwrap_or(&default_loc);

        let visible_items: Vec<String> = current_loc.items.iter()
            .filter_map(|id| self.world.items.get(id).map(|i| i.name.clone()))
            .collect();

        let player_inventory: Vec<String> = self.world.player.inventory.iter()
            .filter_map(|id| self.world.items.get(id).map(|i| i.name.clone()))
            .collect();

        let (x, y) = self.world.current_pos;
        let context = format!(
            r#"You are Dungeon Master for a text adventure game.
Current Location: {} at ({}, {})
Description: {}
Items here: {:?}
Player Inventory: {:?}
Player Money: {}

RULES:
1. Use tools to modify world state based on user actions.
2. You can call MULTIPLE tools in ONE response.
3. Batch up related actions (create + add) in a single response.
4. For movement: Use create_location THEN move_to in the same response.
5. Provide natural, engaging narrative descriptions.
6. End your response with 3-5 suggested actions for player (as narrative text).
7. NEVER generate JSON text - use tool calls instead.

Available tools: move_to, create_location, create_item, add_item_to_inventory, remove_item_from_inventory, add_item_to_location, remove_item_from_location, use_item, equip_item, unequip_item, combine_items, break_item, add_item_to_container, remove_item_from_container"#,
            current_loc.name, x, y,
            current_loc.description,
            visible_items,
            player_inventory,
            self.world.player.money
        );

        LlmMessage {
            role: "system".to_string(),
            content: Some(context),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    fn build_user_message(&self, user_input: &str) -> LlmMessage {
        let (x, y) = self.world.current_pos;
        let adjacent_info = self.get_adjacent_info(x, y);

        let content = format!(
            "User Action: {}\n\n{}\n\nLast Narrative:\n(See system message for current context)",
            user_input, adjacent_info
        );

        LlmMessage {
            role: "user".to_string(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    fn get_adjacent_info(&self, x: i32, y: i32) -> String {
        let directions = [
            ("North", x, y + 1),
            ("South", x, y - 1),
            ("East", x + 1, y),
            ("West", x - 1, y),
        ];

        directions.iter()
            .map(|(dir, dx, dy)| {
                let status = self.world.locations.get(&(*dx, *dy))
                    .map(|l| format!("{} - {}", l.name, l.description))
                    .unwrap_or_else(|| "UNKNOWN (not yet explored)".to_string());
                format!("{} at ({}, {}): {}", dir, dx, dy, status)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn execute_tool_call(&mut self, tool_call: &ToolCall) -> Result<ToolResult> {
        let ToolFunction { name, arguments } = &tool_call.function;

        self.log(&format!("Executing tool: {} with args: {}", name, arguments));

        let result = match name.as_str() {
            "move_to" => self.execute_move_to(arguments)?,
            "create_location" => self.execute_create_location(arguments)?,
            "create_item" => self.execute_create_item(arguments)?,
            "add_item_to_inventory" => self.execute_add_item_to_inventory(arguments)?,
            "remove_item_from_inventory" => self.execute_remove_item_from_inventory(arguments)?,
            "add_item_to_location" => self.execute_add_item_to_location(arguments)?,
            "remove_item_from_location" => self.execute_remove_item_from_location(arguments)?,
            "use_item" => self.execute_use_item(arguments)?,
            "equip_item" => self.execute_equip_item(arguments)?,
            "unequip_item" => self.execute_unequip_item(arguments)?,
            "combine_items" => self.execute_combine_items(arguments)?,
            "break_item" => self.execute_break_item(arguments)?,
            "add_item_to_container" => self.execute_add_item_to_container(arguments)?,
            "remove_item_from_container" => self.execute_remove_item_from_container(arguments)?,
            _ => return Err(anyhow::anyhow!("Unknown tool: {}", name)),
        };

        Ok(ToolResult {
            tool_call_id: tool_call.id.clone(),
            content: result,
        })
    }

    fn execute_move_to(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let x: i32 = args["x"].as_i64().unwrap() as i32;
        let y: i32 = args["y"].as_i64().unwrap() as i32;
        let pos = (x, y);

        if self.world.locations.contains_key(&pos) {
            self.world.current_pos = pos;
            if let Some(loc) = self.world.locations.get_mut(&pos) {
                loc.visited = true;
            }
            Ok(format!("Moved to ({}, {})", x, y))
        } else {
            Err(anyhow::anyhow!("Location ({}, {}) does not exist. Create it first.", x, y))
        }
    }

    fn execute_create_location(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let x: i32 = args["x"].as_i64().unwrap() as i32;
        let y: i32 = args["y"].as_i64().unwrap() as i32;
        let pos = (x, y);

        if self.world.locations.contains_key(&pos) {
            return Err(anyhow::anyhow!("Location ({}, {}) already exists", x, y));
        }

        let exits = args["exits"].as_object()
            .and_then(|e| {
                let mut map = std::collections::HashMap::new();
                for (dir, val) in e {
                    if let Some(arr) = val.as_array() {
                        if arr.len() == 2 {
                            if let (Some(x_val), Some(y_val)) = (arr[0].as_i64(), arr[1].as_i64()) {
                                map.insert(dir.clone(), Some((x_val as i32, y_val as i32)));
                            }
                        }
                    } else if val.is_null() {
                        map.insert(dir.clone(), None);
                    }
                }
                Some(map)
            })
            .unwrap_or_default();

        let items: Vec<String> = args["items"].as_array()
            .and_then(|a| a.iter().map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        let actors: Vec<String> = args["actors"].as_array()
            .and_then(|a| a.iter().map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        let loc = Location {
            name: args["name"].as_str().unwrap_or("Unknown").to_string(),
            description: args["description"].as_str().unwrap_or("").to_string(),
            image_prompt: args["image_prompt"].as_str().unwrap_or("").to_string(),
            items,
            actors,
            exits,
            cached_image_path: None,
            visited: true,
        };

        self.world.locations.insert(pos, loc);
        Ok(format!("Created location at ({}, {})", x, y))
    }

    fn execute_create_item(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let id = args["id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing id"))?;

        if self.world.items.contains_key(id) {
            return Err(anyhow::anyhow!("Item {} already exists", id));
        }

        let item_type_str = args["item_type"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_type"))?;
        let item_type = match item_type_str {
            "Weapon" => ItemType::Weapon,
            "Armor" => ItemType::Armor,
            "Consumable" => ItemType::Consumable,
            "Tool" => ItemType::Tool,
            "Key" => ItemType::Key,
            "Container" => ItemType::Container,
            "QuestItem" => ItemType::QuestItem,
            "Material" => ItemType::Material,
            _ => return Err(anyhow::anyhow!("Unknown item_type: {}", item_type_str)),
        };

        let state = if let Some(state_obj) = args.get("state") {
            if let Some(s) = state_obj.as_str() {
                match s {
                    "Normal" => ItemState::Normal,
                    "Equipped" => ItemState::Equipped,
                    _ => ItemState::Normal,
                }
            } else if let Some(damaged) = state_obj.get("Damaged") {
                ItemState::Damaged {
                    durability: damaged["durability"].as_u64().unwrap_or(10) as u32,
                    max_durability: damaged["max_durability"].as_u64().unwrap_or(10) as u32,
                }
            } else if let Some(consumed) = state_obj.get("Consumed") {
                ItemState::Consumed {
                    charges: consumed["charges"].as_u64().unwrap_or(1) as u32,
                    max_charges: consumed["max_charges"].as_u64().unwrap_or(1) as u32,
                }
            } else {
                ItemState::Normal
            }
        } else {
            ItemState::Normal
        };

        let props = args.get("properties").and_then(|p| {
            Some(ItemProperties {
                damage: p["damage"].as_u64().map(|d| d as u32),
                defense: p["defense"].as_u64().map(|d| d as u32),
                value: p["value"].as_u64().map(|v| v as u32),
                weight: p["weight"].as_u64().map(|w| w as u32),
                carryable: p["carryable"].as_bool().unwrap_or(true),
                usable: p["usable"].as_bool().unwrap_or(false),
                equip_slot: p["equip_slot"].as_str().map(|s| s.to_string()),
                status_effects: p["status_effects"].as_array()
                    .and_then(|a| a.iter().map(|v| v.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default(),
            })
        }).unwrap_or_default();

        let item = Item {
            id: id.to_string(),
            name: args["name"].as_str().unwrap_or(id).to_string(),
            description: args["description"].as_str().unwrap_or("").to_string(),
            item_type,
            state,
            properties: props,
        };

        self.world.items.insert(id.to_string(), item);
        Ok(format!("Created item: {}", id))
    }

    fn execute_add_item_to_inventory(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        if !self.world.player.inventory.contains(&item_id.to_string()) {
            self.world.player.inventory.push(item_id.to_string());
        }
        Ok(format!("Added {} to inventory", item_id))
    }

    fn execute_remove_item_from_inventory(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;
        self.world.player.inventory.retain(|id| id != item_id);
        Ok(format!("Removed {} from inventory", item_id))
    }

    fn execute_add_item_to_location(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let x: i32 = args["x"].as_i64().unwrap() as i32;
        let y: i32 = args["y"].as_i64().unwrap() as i32;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        if let Some(loc) = self.world.locations.get_mut(&(x, y)) {
            if !loc.items.contains(&item_id.to_string()) {
                loc.items.push(item_id.to_string());
            }
        }
        Ok(format!("Added {} to location ({}, {})", item_id, x, y))
    }

    fn execute_remove_item_from_location(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let x: i32 = args["x"].as_i64().unwrap() as i32;
        let y: i32 = args["y"].as_i64().unwrap() as i32;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        if let Some(loc) = self.world.locations.get_mut(&(x, y)) {
            loc.items.retain(|id| id != item_id);
        }
        Ok(format!("Removed {} from location ({}, {})", item_id, x, y))
    }

    fn execute_use_item(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        if let Some(item) = self.world.items.get_mut(item_id) {
            if item.properties.usable {
                match &item.state {
                    ItemState::Consumed { charges, max_charges } if *charges > 1 => {
                        item.state = ItemState::Consumed { charges: charges - 1, max_charges: *max_charges };
                    }
                    ItemState::Consumed { .. } => {
                        self.world.player.inventory.retain(|id| id != item_id);
                    }
                    _ => {}
                }
            }
        }
        Ok(format!("Used item: {}", item_id))
    }

    fn execute_equip_item(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        if let Some(item) = self.world.items.get_mut(item_id) {
            if item.properties.equip_slot.is_some() {
                item.state = ItemState::Equipped;
            }
        }
        Ok(format!("Equipped item: {}", item_id))
    }

    fn execute_unequip_item(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        if let Some(item) = self.world.items.get_mut(item_id) {
            if matches!(item.state, ItemState::Equipped) {
                item.state = ItemState::Normal;
            }
        }
        Ok(format!("Unequipped item: {}", item_id))
    }

    fn execute_combine_items(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let item1_id = args["item1_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item1_id"))?;
        let item2_id = args["item2_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item2_id"))?;
        let result_id = args["result_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing result_id"))?;

        self.world.player.inventory.retain(|id| id != item1_id && id != item2_id);
        for loc in self.world.locations.values_mut() {
            loc.items.retain(|id| id != item1_id && id != item2_id);
        }

        if let Some(result_item) = self.world.items.get(result_id) {
            self.world.items.insert(result_id.to_string(), result_item.clone());
            self.world.player.inventory.push(result_id.to_string());
        }
        Ok(format!("Combined {} and {} into {}", item1_id, item2_id, result_id))
    }

    fn execute_break_item(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        self.world.player.inventory.retain(|id| id != item_id);
        for loc in self.world.locations.values_mut() {
            loc.items.retain(|id| id != item_id);
        }
        self.world.items.remove(item_id);
        Ok(format!("Broke item: {}", item_id))
    }

    fn execute_add_item_to_container(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let container_id = args["container_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing container_id"))?;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        if let Some(container) = self.world.items.get_mut(container_id) {
            if let ItemState::Open { contents } = &mut container.state {
                contents.push(item_id.to_string());
            }
        }
        Ok(format!("Added {} to container {}", item_id, container_id))
    }

    fn execute_remove_item_from_container(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let container_id = args["container_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing container_id"))?;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        if let Some(container) = self.world.items.get_mut(container_id) {
            if let ItemState::Open { contents } = &mut container.state {
                contents.retain(|id| id != item_id);
            }
        }
        Ok(format!("Removed {} from container {}", item_id, container_id))
    }

    fn extract_suggested_actions(&self, narrative: &str) -> Vec<String> {
        let mut actions = Vec::new();
        for line in narrative.lines() {
            let line = line.trim();
            if line.starts_with('-') || line.starts_with('*') || line.starts_with('•') {
                let action = line.trim_start_matches(&['-', '*', '•', ' ']).trim();
                if !action.is_empty() && action.len() < 100 {
                    actions.push(action.to_string());
                }
            }
        }

        if actions.is_empty() {
            actions.push("look around".to_string());
            actions.push("check inventory".to_string());
        }

        actions.truncate(5);
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_creation() {
        let llm_client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let world = WorldState::new();
        let agent = Agent::new(llm_client, world);
        assert_eq!(agent.max_iterations, 3);
        assert_eq!(agent.overall_timeout_seconds, 60);
    }

    #[test]
    fn test_extract_suggested_actions() {
        let llm_client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let world = WorldState::new();
        let agent = Agent::new(llm_client, world);

        let narrative = "You see a door.\n- Open the door\n- Look around\n- Check your inventory";
        let actions = agent.extract_suggested_actions(narrative);
        assert_eq!(actions.len(), 3);
        assert!(actions.contains(&"Open the door".to_string()));
        assert!(actions.contains(&"Look around".to_string()));
    }

    #[test]
    fn test_extract_suggested_actions_fallback() {
        let llm_client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let world = WorldState::new();
        let agent = Agent::new(llm_client, world);

        let narrative = "Just some text with no suggestions.";
        let actions = agent.extract_suggested_actions(narrative);
        assert!(!actions.is_empty());
        assert!(actions.len() <= 5);
    }

    #[test]
    fn test_batch_vs_observation_tools() {
        assert!(BATCH_TOOLS.contains(&"create_location"));
        assert!(BATCH_TOOLS.contains(&"create_item"));
        assert!(BATCH_TOOLS.contains(&"move_to"));

        assert!(!OBSERVATION_TOOLS.contains(&"move_to"));
        assert!(!OBSERVATION_TOOLS.contains(&"create_item"));
    }
}

use crate::model::{WorldState, Item, Location, ItemState, ItemProperties, ItemType, Combatant, StatusType, CombatState, StatusEffect};
use crate::tools::{ToolCall, ToolResult, ToolFunction, get_tool_definitions};
use crate::llm::LlmClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};

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
    overall_timeout_seconds: u64,
    turn_narrative: Option<String>,
    debug_log: Vec<String>,
}

impl Agent {
    pub fn new(llm_client: LlmClient, world: WorldState) -> Self {
        Self {
            llm_client,
            world,
            overall_timeout_seconds: 60,
            turn_narrative: None,
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

        self.turn_narrative = None;

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

        if start_time.elapsed() > overall_timeout {
            self.log(&format!("Timeout reached ({}s)", self.overall_timeout_seconds));
            return Ok(AgentResponse {
                narrative: "[Timeout: The game took too long to respond]".to_string(),
                suggested_actions: vec!["look around".to_string()],
            });
        }

        let response = self.llm_client.send_chat_request(&request).await?;
        let response_content = response.get("content").and_then(|c| c.as_str());
        let response_tool_calls: Option<Vec<ToolCall>> = response
            .get("tool_calls")
            .and_then(|tc| serde_json::from_value(tc.clone()).ok());

        if let Some(ref tool_calls) = response_tool_calls {
            self.log(&format!("Got {} tool call(s)", tool_calls.len()));
            for tool_call in tool_calls {
                self.log(&format!("  - {}", tool_call.function.name));
                let _ = self.execute_tool_call(tool_call).await;
            }
        }

        if let Some(turn_narrative) = &self.turn_narrative {
            let narrative = turn_narrative.clone();
            self.log(&format!("Narrative length: {} chars", narrative.len()));
            let suggested_actions = self.extract_suggested_actions(&narrative);
            return Ok(AgentResponse {
                narrative,
                suggested_actions,
            });
        }

        if response_tool_calls.is_some() && response_content.is_none() {
            messages.push(LlmMessage {
                role: "assistant".to_string(),
                content: None,
                tool_calls: response_tool_calls.clone(),
                tool_call_id: None,
            });

            messages.push(LlmMessage {
                role: "user".to_string(),
                content: Some("Describe what just happened in 2-3 sentences. Do not call any tools, just provide narrative.".to_string()),
                tool_calls: None,
                tool_call_id: None,
            });

            let narrative_request = LlmRequest {
                model: self.llm_client.model_name.clone(),
                messages: messages.clone(),
                tools: None,
                tool_choice: None,
                temperature: 0.7,
                max_tokens: 1000,
            };

            if let Ok(narrative_response) = self.llm_client.send_chat_request(&narrative_request).await {
                if let Some(content) = narrative_response.get("content").and_then(|c| c.as_str()) {
                    let narrative = content.to_string();
                    self.log(&format!("Narrative length: {} chars", narrative.len()));
                    let suggested_actions = self.extract_suggested_actions(&narrative);
                    return Ok(AgentResponse {
                        narrative,
                        suggested_actions,
                    });
                }
            }
        }

        let narrative = response_content.map(|c| c.to_string()).unwrap_or_default();
        self.log(&format!("Narrative length: {} chars", narrative.len()));
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
        let adjacent_info = self.get_adjacent_info(x, y);

        let mut context = format!(
            r#"You are Dungeon Master for a text adventure game.
 Current Location: {} at ({}, {})
 Description: {}
 Items here: {:?}
 Player Inventory: {:?}
 Player Money: {}

 Adjacent Areas: {}"#,
            current_loc.name, x, y,
            current_loc.description,
            visible_items,
            player_inventory,
            self.world.player.money,
            adjacent_info
        );

        if self.world.combat.active {
            let combat_info: Vec<String> = self.world.combat.combatants.iter()
                .map(|c| {
                    let status = c.status_effects.iter()
                        .map(|e| format!("{:?}({}t)", e.effect_type, e.duration))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(
                        "- {} ({}): HP {}/{} | Weapon: {:?} | Armor: {:?} | Temp Def: {} | Status: {}",
                        c.id, if c.is_player { "PLAYER" } else { "ENEMY" },
                        c.hp, c.max_hp, c.weapon_id, c.armor_id, c.temp_defense, status
                    )
                })
                .collect();
            context.push_str(&format!(
                r#"

 COMBAT ACTIVE - Round {} - Turn: {}
 Combatants:
 {}

 Combat Actions: start_combat, attack_actor, defend, flee, use_item_in_combat, end_turn"#,
                self.world.combat.round_number,
                self.world.combat.combatants.get(self.world.combat.current_turn_index)
                    .map(|c| c.id.as_str())
                    .unwrap_or("none"),
                combat_info.join("\n")
            ));
        }

        context.push_str(&format!(
            r#"

 RULES:
 1. You can call MULTIPLE tools in ONE response.
 2. When calling tools: The narrative you generate should describe what happens AFTER tools execute.
 3. For movement: Use move_to(direction). New tiles are auto-generated if needed.
 4. For describing location: Use update_location_description(text) to permanently change location's description.
 5. For responding to player: Use generate_turn_narrative(text) if you want full control, or let the system generate narrative after your tools execute.
 6. If you call tools WITHOUT using generate_turn_narrative or adding narrative content, the system will ask you to describe what happened with the updated world state.
 7. End your response with 3-5 suggested actions (in the LLM content, not as a tool).
 8. NEVER generate JSON text - use tool calls instead.

 Available tools: move_to, update_location_description, generate_turn_narrative, create_item, add_item_to_inventory, remove_item_from_inventory, add_item_to_location, remove_item_from_location, use_item, equip_item, unequip_item, combine_items, break_item, add_item_to_container, remove_item_to_container, start_combat, attack_actor, defend, flee, use_item_in_combat, end_turn"#
        ));

        LlmMessage {
            role: "system".to_string(),
            content: Some(context),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    fn build_user_message(&self, user_input: &str) -> LlmMessage {
        let content = format!(
            "Player Action: {}",
            user_input
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
            ("north", x, y + 1),
            ("south", x, y - 1),
            ("east", x + 1, y),
            ("west", x - 1, y),
        ];

        directions.iter()
            .map(|(dir, dx, dy)| {
                let status = self.world.locations.get(&(*dx, *dy))
                    .map(|l| l.name.as_str())
                    .unwrap_or("unexplored");
                format!("{}: {}", dir, status)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    async fn execute_tool_call(&mut self, tool_call: &ToolCall) -> Result<ToolResult> {
        let ToolFunction { name, arguments } = &tool_call.function;

        self.log(&format!("Executing tool: {} with args: {}", name, arguments));

let result = match name.as_str() {
            "move_to" => self.execute_move_to(arguments).await?,
            "update_location_description" => self.execute_update_location_description(arguments)?,
            "generate_turn_narrative" => self.execute_generate_turn_narrative(arguments)?,
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
            "start_combat" => self.execute_start_combat(arguments)?,
            "attack_actor" => self.execute_attack_actor(arguments)?,
            "defend" => self.execute_defend(arguments)?,
            "flee" => self.execute_flee(arguments)?,
            "use_item_in_combat" => self.execute_use_item_in_combat(arguments)?,
            "end_turn" => self.execute_end_turn(arguments)?,
            "inspect_object" => self.execute_inspect_object(arguments)?,
            _ => return Err(anyhow::anyhow!("Unknown tool: {}", name)),
        };

        Ok(ToolResult {
            tool_call_id: tool_call.id.clone(),
            content: result,
        })
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
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        if let Some(loc) = self.world.locations.get_mut(&self.world.current_pos) {
            if !loc.items.contains(&item_id.to_string()) {
                loc.items.push(item_id.to_string());
            }
            Ok(format!("Added {} to current location", item_id))
        } else {
            Err(anyhow::anyhow!("Current location not found"))
        }
    }

    fn execute_remove_item_from_location(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        if let Some(loc) = self.world.locations.get_mut(&self.world.current_pos) {
            loc.items.retain(|id| id != item_id);
            Ok(format!("Removed {} from current location", item_id))
        } else {
            Err(anyhow::anyhow!("Current location not found"))
        }
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

    async fn execute_move_to(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let direction = args["direction"].as_str().ok_or_else(|| anyhow::anyhow!("Missing direction"))?;

        let (current_x, current_y) = self.world.current_pos;
        let target_pos = match direction {
            "north" => (current_x, current_y + 1),
            "south" => (current_x, current_y - 1),
            "east" => (current_x + 1, current_y),
            "west" => (current_x - 1, current_y),
            _ => return Err(anyhow::anyhow!("Invalid direction")),
        };

        let opposite = get_opposite_direction(direction);

        if !self.world.locations.contains_key(&target_pos) {
            self.log(&format!("Generating new location at ({}, {}) heading {}", target_pos.0, target_pos.1, direction));

            let current_loc = self.world.locations.get(&self.world.current_pos)
                .ok_or_else(|| anyhow::anyhow!("Current location not found"))?;

            let prompt = format!(
                r#"Current Location: {} at ({}, {})
Description: {}

The player is heading {} toward coordinates ({}, {}).
This grid cell is currently EMPTY and needs to be generated.

Create a new location at ({}, {}) that fits thematically with current location.
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
                current_x,
                current_y,
                current_loc.description,
                direction, target_pos.0, target_pos.1,
                target_pos.0, target_pos.1
            );

            let system_prompt = "You are a world generator for a text adventure game. Create interesting, thematically consistent locations. You MUST output valid JSON only.";

            match self.llm_client.generate_location(system_prompt, &prompt).await {
                Ok(mut location) => {
                    location.visited = true;
                    self.world.locations.insert(target_pos, location.clone());
                    self.log(&format!("Created location at ({}, {}): {}", target_pos.0, target_pos.1, location.name));

                    if let Some(current_loc) = self.world.locations.get_mut(&self.world.current_pos) {
                        current_loc.exits.insert(direction.to_string(), Some(target_pos));
                    }
                    if let Some(new_loc) = self.world.locations.get_mut(&target_pos) {
                        new_loc.exits.insert(opposite, Some(self.world.current_pos));
                    }
                }
                Err(e) => {
                    self.log(&format!("Failed to generate location: {}", e));

                    let fallback_loc = Location {
                        name: format!("Mysterious area ({}, {})", target_pos.0, target_pos.1),
                        description: "A mysterious place that appeared suddenly.".to_string(),
                        items: vec![],
                        actors: vec![],
                        exits: std::collections::HashMap::new(),
                        cached_image_path: None,
                        image_prompt: "A mysterious location with undefined characteristics.".to_string(),
                        visited: true,
                    };

                    self.world.locations.insert(target_pos, fallback_loc.clone());
                    self.log(&format!("Used fallback location at ({}, {})", target_pos.0, target_pos.1));

                    if let Some(current_loc) = self.world.locations.get_mut(&self.world.current_pos) {
                        current_loc.exits.insert(direction.to_string(), Some(target_pos));
                    }
                    if let Some(new_loc) = self.world.locations.get_mut(&target_pos) {
                        new_loc.exits.insert(opposite, Some(self.world.current_pos));
                    }
                }
            }
        }

        self.world.current_pos = target_pos;
        if let Some(loc) = self.world.locations.get_mut(&target_pos) {
            loc.visited = true;
        }

        let loc_name = self.world.locations.get(&target_pos)
            .map(|l| l.name.as_str())
            .unwrap_or("Unknown");
        Ok(format!("Moved {} to ({}, {}) - {}", direction, target_pos.0, target_pos.1, loc_name))
    }

    fn execute_update_location_description(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let text = args["text"].as_str().ok_or_else(|| anyhow::anyhow!("Missing text"))?;

        if let Some(loc) = self.world.locations.get_mut(&self.world.current_pos) {
            loc.description = text.to_string();
            Ok("Location description updated".to_string())
        } else {
            Err(anyhow::anyhow!("Current location not found"))
        }
    }

    fn execute_generate_turn_narrative(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let text = args["text"].as_str().ok_or_else(|| anyhow::anyhow!("Missing text"))?;

        self.turn_narrative = Some(text.to_string());
        Ok("Turn narrative generated".to_string())
    }

    fn execute_start_combat(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let enemy_ids_val = args["enemy_ids"].as_array().ok_or_else(|| anyhow::anyhow!("Missing enemy_ids array"))?;

        if self.world.combat.active {
            return Err(anyhow::anyhow!("Combat is already active"));
        }

        let total_combatants = 1 + enemy_ids_val.len();
        if total_combatants > self.world.max_combatants as usize {
            return Err(anyhow::anyhow!("Too many combatants (max {})", self.world.max_combatants));
        }

        let mut combatants = Vec::new();

        combatants.push(Combatant {
            id: "player".to_string(),
            is_player: true,
            hp: 100,
            max_hp: 100,
            weapon_id: None,
            armor_id: None,
            initiative: rand::random::<u32>() % 20 + 1,
            status_effects: Vec::new(),
            temp_defense: 0,
        });

        for enemy_id_val in enemy_ids_val {
            let enemy_id = enemy_id_val.as_str().ok_or_else(|| anyhow::anyhow!("Invalid enemy_id"))?;
            if let Some(actor) = self.world.actors.get(enemy_id) {
                if actor.current_pos != self.world.current_pos {
                    return Err(anyhow::anyhow!("Enemy {} is not at current location", enemy_id));
                }
                combatants.push(Combatant {
                    id: enemy_id.to_string(),
                    is_player: false,
                    hp: 50,
                    max_hp: 50,
                    weapon_id: None,
                    armor_id: None,
                    initiative: rand::random::<u32>() % 20 + 1,
                    status_effects: Vec::new(),
                    temp_defense: 0,
                });
            }
        }

        combatants.sort_by(|a, b| b.initiative.cmp(&a.initiative));

        self.world.combat = CombatState {
            active: true,
            combatants,
            current_turn_index: 0,
            round_number: 1,
        };

        Ok(format!("Started combat with {} enemies", enemy_ids_val.len()))
    }

    fn execute_attack_actor(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let attacker_id = args["attacker_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing attacker_id"))?;
        let target_id = args["target_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing target_id"))?;
        let weapon_id_opt = args["weapon_id"].as_str();

        if !self.world.combat.active {
            return Err(anyhow::anyhow!("Combat is not active"));
        }

        let _attacker_idx = self.world.combat.combatants.iter()
            .position(|c| c.id == attacker_id)
            .ok_or_else(|| anyhow::anyhow!("Attacker not in combat"))?;

        let target_idx = self.world.combat.combatants.iter()
            .position(|c| c.id == target_id)
            .ok_or_else(|| anyhow::anyhow!("Target not in combat"))?;

        let weapon_damage = if let Some(weapon_id) = weapon_id_opt {
            self.world.items.get(weapon_id).and_then(|i| i.properties.damage).unwrap_or(5)
        } else {
            5
        };

        let armor_defense = self.world.combat.combatants[target_idx].armor_id.as_ref()
            .and_then(|id| self.world.items.get(id))
            .and_then(|i| i.properties.defense)
            .unwrap_or(0);

        let temp_defense = self.world.combat.combatants[target_idx].temp_defense;
        let total_defense = armor_defense + temp_defense;

        let damage = weapon_damage.saturating_sub(total_defense);
        let final_damage = if damage == 0 { 1 } else { damage };

        self.world.combat.combatants[target_idx].hp = self.world.combat.combatants[target_idx].hp.saturating_sub(final_damage);

        Ok(format!("{} attacked {} for {} damage", attacker_id, target_id, final_damage))
    }

    fn execute_defend(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let actor_id = args["actor_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing actor_id"))?;

        if !self.world.combat.active {
            return Err(anyhow::anyhow!("Combat is not active"));
        }

        let combatant_idx = self.world.combat.combatants.iter()
            .position(|c| c.id == actor_id)
            .ok_or_else(|| anyhow::anyhow!("Actor not in combat"))?;

        self.world.combat.combatants[combatant_idx].temp_defense += 5;

        Ok(format!("{} is defending (+5 temp defense)", actor_id))
    }

    fn execute_flee(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let actor_id = args["actor_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing actor_id"))?;

        if !self.world.combat.active {
            return Err(anyhow::anyhow!("Combat is not active"));
        }

        let roll = rand::random::<u32>() % 20;
        if roll >= 10 {
            self.world.combat.combatants.retain(|c| c.id != actor_id);

            if !self.world.combat.combatants.iter().any(|c| c.is_player) ||
               !self.world.combat.combatants.iter().any(|c| !c.is_player) {
                self.world.combat.active = false;
            }

            Ok(format!("{} fled successfully!", actor_id))
        } else {
            Ok(format!("{} failed to flee", actor_id))
        }
    }

    fn execute_use_item_in_combat(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let user_id = args["user_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing user_id"))?;
        let item_id = args["item_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing item_id"))?;

        if !self.world.combat.active {
            return Err(anyhow::anyhow!("Combat is not active"));
        }

        let combatant_idx = self.world.combat.combatants.iter()
            .position(|c| c.id == user_id)
            .ok_or_else(|| anyhow::anyhow!("User not in combat"))?;

        if !self.world.player.inventory.contains(&item_id.to_string()) {
            return Err(anyhow::anyhow!("Item {} not in inventory", item_id));
        }

        if let Some(item) = self.world.items.get_mut(item_id) {
            if item.properties.usable {
                match &mut item.state {
                    ItemState::Consumed { charges, max_charges: _ } if *charges > 1 => {
                        *charges -= 1;
                    }
                    ItemState::Consumed { .. } => {
                        self.world.player.inventory.retain(|id| id != item_id);
                    }
                    _ => {}
                }

                let heal_amount = 20;
                self.world.combat.combatants[combatant_idx].hp = (self.world.combat.combatants[combatant_idx].hp + heal_amount)
                    .min(self.world.combat.combatants[combatant_idx].max_hp);

                Ok(format!("{} used {} and healed for {}", user_id, item_id, heal_amount))
            } else {
                Err(anyhow::anyhow!("Item {} is not usable", item_id))
            }
        } else {
            Err(anyhow::anyhow!("Item {} not found", item_id))
        }
    }

    fn execute_end_turn(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let actor_id = args["actor_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing actor_id"))?;

        if !self.world.combat.active {
            return Err(anyhow::anyhow!("Combat is not active"));
        }

        let current_idx = self.world.combat.current_turn_index;
        if self.world.combat.combatants.get(current_idx).map(|c| &c.id) != Some(&actor_id.to_string()) {
            return Err(anyhow::anyhow!("Not {}'s turn", actor_id));
        }

        for combatant in &mut self.world.combat.combatants {
            combatant.temp_defense = 0;
        }

        let mut new_turn_index = current_idx + 1;

        while new_turn_index < self.world.combat.combatants.len() {
            let has_stunned = self.world.combat.combatants[new_turn_index]
                .status_effects
                .iter()
                .any(|e| e.effect_type == StatusType::Stunned);

            if !has_stunned {
                break;
            }

            for effect in &mut self.world.combat.combatants[new_turn_index].status_effects {
                if effect.duration > 0 {
                    effect.duration -= 1;
                }
            }

            new_turn_index += 1;
        }

        if new_turn_index >= self.world.combat.combatants.len() {
            self.world.combat.round_number += 1;

            for combatant in &mut self.world.combat.combatants {
                let mut new_effects = Vec::new();
                for effect in &combatant.status_effects {
                    let remaining = effect.duration - 1;
                    match effect.effect_type {
                        StatusType::Poison | StatusType::Burning => {
                            if combatant.hp > effect.severity {
                                combatant.hp -= effect.severity;
                            } else {
                                combatant.hp = 0;
                            }
                        }
                        _ => {}
                    }

                    if remaining > 0 {
                        new_effects.push(StatusEffect {
                            effect_type: effect.effect_type.clone(),
                            duration: remaining,
                            severity: effect.severity,
                        });
                    }
                }
                combatant.status_effects = new_effects;
            }

            self.world.combat.combatants.retain(|c| c.hp > 0);

            let player_alive = self.world.combat.combatants.iter().any(|c| c.is_player);
            let enemies_alive = self.world.combat.combatants.iter().any(|c| !c.is_player);

            if !player_alive || !enemies_alive {
                self.world.combat.active = false;
                return Ok("Combat ended".to_string());
            }

            new_turn_index = 0;

            while new_turn_index < self.world.combat.combatants.len() &&
                  self.world.combat.combatants[new_turn_index]
                      .status_effects
                      .iter()
                      .any(|e| e.effect_type == StatusType::Stunned) {
                new_turn_index += 1;
            }

            if new_turn_index >= self.world.combat.combatants.len() {
                new_turn_index = 0;
            }
        }

        self.world.combat.current_turn_index = new_turn_index;

        let next_combatant = self.world.combat.combatants.get(new_turn_index)
            .map(|c| c.id.as_str())
            .unwrap_or("none");

        Ok(format!("Turn ended. Next: {}", next_combatant))
    }

    fn execute_inspect_object(&mut self, arguments: &str) -> Result<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)?;
        let id = args["object_id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing object_id"))?;

        if let Some(item) = self.world.items.get(id) {
            return Ok(format!(
                "Item: {}\nDescription: {}\nType: {:?}\nState: {:?}\nProperties: {:?}",
                item.name, item.description, item.item_type, item.state, item.properties
            ));
        }

        if let Some(actor) = self.world.actors.get(id) {
            return Ok(format!(
                "Actor: {}\nDescription: {}\nInventory: {:?}\nMoney: {}",
                actor.name, actor.description, actor.inventory, actor.money
            ));
        }

        Err(anyhow::anyhow!("Object {} not found", id))
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

fn get_opposite_direction(direction: &str) -> String {
    match direction {
        "north" => "south".to_string(),
        "south" => "north".to_string(),
        "east" => "west".to_string(),
        "west" => "east".to_string(),
        _ => direction.to_string(),
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
    fn test_get_opposite_direction() {
        assert_eq!(get_opposite_direction("north"), "south");
        assert_eq!(get_opposite_direction("south"), "north");
        assert_eq!(get_opposite_direction("east"), "west");
        assert_eq!(get_opposite_direction("west"), "east");
        assert_eq!(get_opposite_direction("other"), "other");
    }

    #[tokio::test]
    async fn test_execute_update_location_description() {
        let llm_client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let mut world = WorldState::new();
        world.locations.insert((0, 0), Location {
            name: "Test Location".to_string(),
            description: "Old description".to_string(),
            items: vec![],
            actors: vec![],
            exits: std::collections::HashMap::new(),
            cached_image_path: None,
            image_prompt: "Test".to_string(),
            visited: true,
        });

        let mut agent = Agent::new(llm_client, world);
        let result = agent.execute_update_location_description(r#"{"text":"New description"}"#).unwrap();
        assert!(result.contains("updated"));

        assert_eq!(
            agent.world.locations.get(&(0, 0)).unwrap().description,
            "New description"
        );
    }

    #[tokio::test]
    async fn test_execute_generate_turn_narrative() {
        let llm_client = LlmClient::new("http://localhost:11434".to_string(), "test".to_string());
        let world = WorldState::new();
        let mut agent = Agent::new(llm_client, world);

        let result = agent.execute_generate_turn_narrative(r#"{"text":"You see a treasure chest."}"#).unwrap();
        assert!(result.contains("generated"));
        assert_eq!(agent.turn_narrative, Some("You see a treasure chest.".to_string()));
    }
}

# LLM Text Adventure Roadmap

## Overview

### Vision
A dynamic, LLM-driven text adventure where the AI creates content via defined schemas and uses tool calls to manipulate a strictly tracked world state. The system balances creative freedom with deterministic mechanics.

### Philosophy
- **LLM Role**: Create items, locations, actors based on defined schemas; generate narrative; decide which actions to take
- **System Role**: Execute actions deterministically; enforce rules; maintain state; validate constraints
- **Separation of Concerns**: Creative generation vs. mechanical execution

### Constraints
- Max items in world: **20**
- Max combat participants: **4**
- No backward compatibility (save format breaks expected on each phase)
- All state must be tracked in WorldState (no LLM hallucinations)

---

## Phase 1: Enhanced Item System

### Schema Changes (`src/model.rs`)

```rust
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub description: String,
    pub item_type: ItemType,
    pub state: ItemState,
    pub properties: ItemProperties,
}
```

### New GameActions (`src/model.rs`)

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "payload")]
pub enum GameAction {
    // Existing actions...
    CreateLocation((i32, i32), Location),
    UpdateLocation((i32, i32), Location),
    CreateItem(Item),
    AddItemToInventory(String),
    RemoveItemFromInventory(String),
    MoveTo((i32, i32)),
    AddItemToLocation { pos: (i32, i32), item_id: String },
    RemoveItemFromLocation { pos: (i32, i32), item_id: String },

    // New Item Actions
    UseItem(String),
    EquipItem(String),
    UnequipItem(String),
    CombineItems { item1_id: String, item2_id: String, result_id: String },
    SetItemState { item_id: String, state: ItemState },
    BreakItem(String),
    AddItemToContainer { container_id: String, item_id: String },
    RemoveItemFromContainer { container_id: String, item_id: String },
}
```

### LLM Context Updates (`src/game.rs`)

Add to context string in `handle_game_input`:

```rust
// Item details with states
let item_context: String = self.world.items.iter()
    .map(|(id, item)| {
        let state_desc = match &item.state {
            ItemState::Normal => "normal".to_string(),
            ItemState::Equipped => "equipped".to_string(),
            ItemState::Damaged { durability, max_durability } =>
                format!("damaged ({}/{})", durability, max_durability),
            ItemState::Consumed { charges, max_charges } =>
                format!("consumed ({} charges left of {})", charges, max_charges),
            ItemState::Locked { key_id } =>
                format!("locked (requires key: {:?})", key_id),
            ItemState::Open { contents } =>
                format!("open (contains: {:?})", contents),
        };
        format!("- {}: {} ({}) - state: {}", item.id, item.name, item.item_type, state_desc)
    })
    .collect::<Vec<_>>()
    .join("\n");

let context_str = format!(
    // ... existing context ...
    "\n\nAll Items:\n{}\n\nItem Rules:\n- Max 20 items total in world\n- Items have durability, charges, container contents, etc.\n- CombineItems merges two items into one\n- UseItem activates consumables or tools",
    item_context
);
```

### Parser Updates (`src/game.rs`)

Add to `parse_and_apply_action`:

```rust
} else if action_str.starts_with("UseItem(") && action_str.ends_with(")") {
    let item_id = &action_str[8..action_str.len()-1].trim_matches('"');
    if let Some(item) = self.world.items.get_mut(item_id) {
        if item.properties.usable {
            match &item.state {
                ItemState::Consumed { charges, max_charges } if *charges > 1 => {
                    item.state = ItemState::Consumed { charges: charges - 1, max_charges: *max_charges };
                }
                ItemState::Consumed { .. } => {
                    // Consumed last charge, remove from inventory
                    self.world.player.inventory.retain(|id| id != item_id);
                }
                _ => {}
            }
        }
    }
} else if action_str.starts_with("EquipItem(") && action_str.ends_with(")") {
    let item_id = &action_str[10..action_str.len()-1].trim_matches('"');
    if let Some(item) = self.world.items.get_mut(item_id) {
        if item.properties.equip_slot.is_some() {
            item.state = ItemState::Equipped;
        }
    }
} else if action_str.starts_with("UnequipItem(") && action_str.ends_with(")") {
    let item_id = &action_str[12..action_str.len()-1].trim_matches('"');
    if let Some(item) = self.world.items.get_mut(item_id) {
        if matches!(item.state, ItemState::Equipped) {
            item.state = ItemState::Normal;
        }
    }
} else if action_str.starts_with("CombineItems(") {
    let json_str = &action_str[13..action_str.len()-1];
    let combine: serde_json::Value = serde_json::from_str(json_str)?;
    let item1_id = combine["item1_id"].as_str().ok_or("Missing item1_id")?;
    let item2_id = combine["item2_id"].as_str().ok_or("Missing item2_id")?;
    let result_id = combine["result_id"].as_str().ok_or("Missing result_id")?;

    // Remove source items from wherever they are
    self.world.player.inventory.retain(|id| id != item1_id && id != item2_id);
    for loc in self.world.locations.values_mut() {
        loc.items.retain(|id| id != item1_id && id != item2_id);
    }

    // Add result item
    self.world.items.insert(result_id.to_string(), self.world.items.get(result_id).unwrap().clone());
    self.world.player.inventory.push(result_id.to_string());
}
// ... other new actions
```

---

## Phase 2: Combat Mechanics

### Combat State Schema (`src/model.rs`)

```rust
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
    pub temp_defense: u32, // from Defend action
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CombatState {
    pub active: bool,
    pub combatants: Vec<Combatant>,
    pub current_turn_index: usize,
    pub round_number: u32,
}

// Add to WorldState
pub struct WorldState {
    // ... existing fields ...
    pub combat: CombatState,
    pub max_items: u32, // = 20
    pub max_combatants: u32, // = 4
}
```

### New Combat Actions (`src/model.rs`)

```rust
pub enum GameAction {
    // ... existing actions ...

    // Combat Actions
    StartCombat { enemy_ids: Vec<String> },
    AttackActor { attacker_id: String, target_id: String, weapon_id: Option<String> },
    Defend { actor_id: String },
    Flee { actor_id: String },
    UseItemInCombat { user_id: String, item_id: String, target_id: Option<String> },
    EndTurn { actor_id: String },
    EndCombat { victor_id: String },
}
```

### Combat Flow Logic (`src/game.rs`)

Add combat processing:

```rust
fn process_combat_round(&mut self) -> Result<()> {
    let combat = &mut self.world.combat;
    if !combat.active { return Ok(()); }

    combat.round_number += 1;

    // Apply status effects
    for combatant in &mut combat.combatants {
        let mut new_effects = Vec::new();
        for effect in &combatant.status_effects {
            let remaining = effect.duration - 1;
            match effect.effect_type {
                StatusType::Poison => combatant.hp = combatant.hp.saturating_sub(effect.severity),
                StatusType::Burning => combatant.hp = combatant.hp.saturating_sub(effect.severity),
                StatusType::Stunned => {}, // handled in turn processing
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
        combatant.temp_defense = 0; // reset defend bonus
    }

    // Check for deaths
    combat.combatants.retain(|c| c.hp > 0);

    // Check for victory/defeat
    let player_alive = combat.combatants.iter().any(|c| c.is_player);
    let enemies_alive = combat.combatants.iter().any(|c| !c.is_player);

    if !player_alive || !enemies_alive {
        combat.active = false;
    }

    Ok(())
}
```

### LLM Context for Combat (`src/game.rs`)

```rust
// Add combat context
if self.world.combat.active {
    let combat_info: Vec<String> = self.world.combat.combatants.iter()
        .map(|c| {
            let status = c.status_effects.iter()
                .map(|e| format!("{:?}({}t)", e.effect_type, e.duration))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "- {} ({}): HP {}/{} | Weapon: {:?} | Armor: {:?} | Status: {}",
                c.id, if c.is_player { "PLAYER" } else { "ENEMY" },
                c.hp, c.max_hp, c.weapon_id, c.armor_id, status
            )
        })
        .collect();
    context_str.push_str(&format!("\n\nCOMBAT ACTIVE - Round {} - Turn: {}\nCombatants:\n{}\nActions: AttackActor, Defend, Flee, UseItemInCombat",
        self.world.combat.round_number,
        self.world.combat.combatants.get(self.world.combat.current_turn_index)
            .map(|c| c.id.as_str())
            .unwrap_or("none"),
        combat_info.join("\n")
    ));
}
```

---

## Phase 3: NPC Interactions

### Actor State Schema (`src/model.rs`)

```rust
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ActorState {
    Idle,
    Hostile,
    Friendly,
    Neutral,
    Trading { inventory: Vec<String> },
    Following { follow_target: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Actor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub current_pos: (i32, i32),
    pub inventory: Vec<String>,
    pub money: u32,
    pub state: ActorState,
    pub hp: u32,
    pub max_hp: u32,
}
```

### New NPC Actions (`src/model.rs`)

```rust
pub enum GameAction {
    // ... existing actions ...

    // NPC Actions
    MoveActor { actor_id: String, pos: (i32, i32) },
    GiveItem { from_id: String, to_id: String, item_id: String },
    TakeItem { actor_id: String, target_id: String, item_id: String },
    ActorSay { actor_id: String, message: String },
    ActorAttack { actor_id: String, target_id: String },
    SetActorState { actor_id: String, state: ActorState },
    ActorBuy { actor_id: String, item_id: String, price: u32 },
    ActorSell { actor_id: String, item_id: String, price: u32 },
}
```

### AI Behavior Rules

Implement simple reactive AI in `src/game.rs`:

```rust
fn process_npc_turn(&mut self, actor_id: &str) -> Result<()> {
    if let Some(actor) = self.world.actors.get_mut(actor_id) {
        let player_pos = self.world.current_pos;

        match &actor.state {
            ActorState::Hostile => {
                // Attack player if in same location
                if actor.current_pos == player_pos {
                    // Generate combat action
                    let action = format!("ActorAttack {{ actor_id: \"{}\", target_id: \"player\" }}", actor_id);
                    self.parse_and_apply_action(&action)?;
                } else {
                    // Move toward player
                    let dx = (player_pos.0 - actor.current_pos.0).signum();
                    let dy = (player_pos.1 - actor.current_pos.1).signum();
                    let new_pos = (actor.current_pos.0 + dx, actor.current_pos.1 + dy);
                    let action = format!("MoveActor {{ actor_id: \"{}\", pos: ({}, {}) }}", actor_id, new_pos.0, new_pos.1);
                    self.parse_and_apply_action(&action)?;
                }
            }
            ActorState::Friendly => {
                // Offer help or trade
                if actor.current_pos == player_pos && rand::random::<f32>() > 0.7 {
                    let greetings = vec![
                        "Hello traveler, how can I help?",
                        "Greetings, adventurer!",
                        "Welcome to these lands.",
                    ];
                    let msg = greetings.choose(&mut rand::thread_rng()).unwrap_or(&"Hello.");
                    let action = format!("ActorSay {{ actor_id: \"{}\", message: \"{}\" }}", actor_id, msg);
                    self.parse_and_apply_action(&action)?;
                }
            }
            ActorState::Following { follow_target } => {
                // Move toward follow target
                if let Some(target_actor) = self.world.actors.get(follow_target) {
                    let dx = (target_actor.current_pos.0 - actor.current_pos.0).signum();
                    let dy = (target_actor.current_pos.1 - actor.current_pos.1).signum();
                    let new_pos = (actor.current_pos.0 + dx, actor.current_pos.1 + dy);
                    let action = format!("MoveActor {{ actor_id: \"{}\", pos: ({}, {}) }}", actor_id, new_pos.0, new_pos.1);
                    self.parse_and_apply_action(&action)?;
                }
            }
            _ => {} // Idle or Neutral - do nothing
        }
    }
    Ok(())
}
```

---

## Testing Checklist

### Phase 1: Enhanced Items
- [ ] All item types serialize/deserialize correctly
- [ ] Item properties HashMap stores/retrieves values correctly
- [ ] UseItem decreases charges for consumables
- [ ] UseItem removes item when charges reach 0
- [ ] EquipItem sets state to Equipped
- [ ] UnequipItem sets state back to Normal
- [ ] CombineItems removes source items and creates result
- [ ] Max 20 items enforced when creating new items
- [ ] Items save/load with correct states
- [ ] LLM generates valid item schemas

### Phase 2: Combat
- [ ] CombatState initializes correctly
- [ ] StartCombat creates combatants with correct HP
- [ ] Max 4 combatants enforced
- [ ] AttackActor calculates damage correctly (weapon damage - armor defense)
- [ ] Defend adds temp_defense for one round
- [ ] Flee removes combatant from combat
- [ ] Status effects apply damage correctly
- [ ] Status effects expire after duration
- [ ] Combat ends when all enemies or player dead
- [ ] Turn order follows initiative
- [ ] Stunned combatants skip turn

### Phase 3: NPC Interactions
- [ ] Actors move to new positions
- [ ] Hostile actors attack player on sight
- [ ] Friendly actors speak to player
- [ ] GiveItem moves item between inventories
- [ ] TakeItem removes item from target inventory
- [ ] ActorSay adds dialogue to narrative
- [ ] ActorAttack triggers combat
- [ ] ActorState changes correctly
- [ ] Following actors move toward target
- [ ] NPCs respect max item constraint

---

## Implementation Order

### Step 1: Update Model (All Phases)
1. Add new enums and structs to `src/model.rs`
2. Update `GameAction` with all new variants
3. Update `WorldState` to include `combat`, `max_items`, `max_combatants`
4. Add derives for Serialize/Deserialize

### Step 2: Phase 1 Implementation
1. Add item action parsing in `parse_and_apply_action`
2. Add item validation (max items, usability)
3. Update LLM context to include item details
4. Update system prompt with item usage rules

### Step 3: Phase 2 Implementation
1. Add combat action parsing
2. Implement `process_combat_round`
3. Add combat validation (max combatants)
4. Update LLM context with combat state
5. Add combat-specific system prompt rules

### Step 4: Phase 3 Implementation
1. Add NPC action parsing
2. Implement `process_npc_turn`
3. Add AI behavior logic
4. Update LLM context with actor states
5. Add NPC interaction rules to system prompt

### Step 5: TUI Updates
1. Display item states in inventory view
2. Show combat UI (HP bars, turn order)
3. Display NPC status indicators
4. Add combat log

### Step 6: Integration Testing
1. Run through testing checklist for all phases
2. Verify save/load persistence
3. Test LLM generation of valid actions
4. Stress test with max constraints

---

## Migration Strategy

**No migration strategy.** Breaking changes expected on each phase. Users should start new saves after each phase.

---

## Future Enhancements (Beyond Roadmap)

- Spell system with mana
- Crafting recipes system
- Faction/reputation tracking
- Multi-location events (fires spreading, armies marching)
- Time of day mechanics
- Procedural quest generation
- Persistent world changes (buildings destroyed, lands claimed)

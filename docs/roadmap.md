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

## Phase 1: Enhanced Item System (Completed)

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

### New Tools (`src/agent.rs`)

Implemented tools:
- `create_item`
- `add_item_to_inventory`
- `remove_item_from_inventory`
- `use_item`
- `equip_item`
- `unequip_item`
- `combine_items`
- `break_item`
- `add_item_to_container`
- `remove_item_from_container`

---

## Phase 2: Combat Mechanics (Completed)

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

### New Combat Tools (`src/agent.rs`)

Implemented tools:
- `start_combat`
- `attack_actor`
- `defend`
- `flee`
- `use_item_in_combat`
- `end_turn`

---

## Phase 3: NPC Interactions (Pending)

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
    // New fields
    pub state: ActorState,
    pub hp: u32,
    pub max_hp: u32,
}
```

### New NPC Tools (`src/agent.rs`)

To be implemented:
- `move_actor(actor_id, direction)`
- `give_item(from_actor_id, to_actor_id, item_id)`
- `take_item(actor_id, target_id, item_id)`
- `actor_say(actor_id, message)`
- `actor_attack(actor_id, target_id)` (Triggers combat)
- `set_actor_state(actor_id, state)`
- `actor_buy(actor_id, item_id)`
- `actor_sell(actor_id, item_id)`

### AI Behavior Rules

Implement simple reactive AI in `src/game.rs` or `src/agent.rs`. This may require a new method `process_npc_turn` that is called after the player's turn or Agent's turn.

```rust
fn process_npc_turn(&mut self, actor_id: &str) -> Result<()> {
    // Logic to control NPCs based on their state
    // Hostile -> Attack or Move closer
    // Friendly -> Chat or Trade
    // Following -> Move closer to target
}
```

---

## Testing Checklist

### Phase 1: Enhanced Items (Completed)
- [x] All item types serialize/deserialize correctly
- [x] Item properties HashMap stores/retrieves values correctly
- [x] UseItem decreases charges for consumables
- [x] UseItem removes item when charges reach 0
- [x] EquipItem sets state to Equipped
- [x] UnequipItem sets state back to Normal
- [x] CombineItems removes source items and creates result
- [x] Max 20 items enforced when creating new items
- [x] Items save/load with correct states
- [x] LLM generates valid item schemas

### Phase 2: Combat (Completed)
- [x] CombatState initializes correctly
- [x] StartCombat creates combatants with correct HP
- [x] Max 4 combatants enforced
- [x] AttackActor calculates damage correctly (weapon damage - armor defense)
- [x] Defend adds temp_defense for one round
- [x] Flee removes combatant from combat
- [x] Status effects apply damage correctly
- [x] Status effects expire after duration
- [x] Combat ends when all enemies or player dead
- [x] Turn order follows initiative
- [x] Stunned combatants skip turn

### Phase 3: NPC Interactions (Next)
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
1. [x] Add new enums and structs to `src/model.rs` (Partial for Phase 3)
2. [x] Update `WorldState` to include `combat`, `max_items`, `max_combatants`
3. [x] Add derives for Serialize/Deserialize

### Step 2: Phase 1 Implementation
1. [x] Implement Item Tools in `src/agent.rs`
2. [x] Add item validation (max items, usability)
3. [x] Update LLM context to include item details
4. [x] Update system prompt with item usage rules

### Step 3: Phase 2 Implementation
1. [x] Implement Combat Tools in `src/agent.rs`
2. [x] Implement `end_turn` logic
3. [x] Add combat validation (max combatants)
4. [x] Update LLM context with combat state
5. [x] Add combat-specific system prompt rules

### Step 4: Phase 3 Implementation (Current)
1. [ ] Update `Actor` struct in `src/model.rs` (add `state`, `hp`, `max_hp`)
2. [ ] Implement NPC Tools in `src/agent.rs`
3. [ ] Implement `process_npc_turn` logic
4. [ ] Update LLM context with actor states
5. [ ] Add NPC interaction rules to system prompt

### Step 5: TUI Updates
1. [ ] Display item states in inventory view
2. [ ] Show combat UI (HP bars, turn order)
3. [ ] Display NPC status indicators
4. [ ] Add combat log

### Step 6: Integration Testing
1. [ ] Run through testing checklist for all phases
2. [ ] Verify save/load persistence
3. [ ] Test LLM generation of valid actions
4. [ ] Stress test with max constraints

---

## Future Enhancements (Beyond Roadmap)

- Spell system with mana
- Crafting recipes system
- Faction/reputation tracking
- Multi-location events (fires spreading, armies marching)
- Time of day mechanics
- Procedural quest generation
- Persistent world changes (buildings destroyed, lands claimed)

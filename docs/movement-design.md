# Improved Movement and Map System Design

## Executive Summary

This design addresses key issues in the current movement and map systems:
- **Inconsistent movement**: Quick moves bypass LLM narrative; erratic behavior.
- **Poor spatial coherence**: LLM-assigned arbitrary x/y coords distort map.
- **Limited LLM context**: No adjacent cell summaries.
- **Map visualization**: No paths, fog-of-war, or walls; shows all locations.

**High-level changes**:
- Switch to coordinate-based location keys: `HashMap<(i32, i32), Location>`.
- Primary position tracking via `(i32, i32)` instead of string IDs.
- Quick arrow-key moves for explored adjacent cells.
- Enhanced LLM prompts/context for spatial consistency and conditional generation.
- Improved ASCII map with paths, fog-of-war, and basic styling.

This ensures reliable quick-moves, coherent grid expansion, rich LLM context, and a usable map.

## 1. Data Structure Changes

### Core Changes in [`model.rs`](src/model.rs)
- **`WorldState`**:
  ```rust
  pub struct WorldState {
      locations: HashMap<(i32, i32), Location>,  // Coord -> Location (primary key)
      current_pos: (i32, i32),                  // Replaces current_location_id: String
      // ... other fields unchanged
  }
  ```
- **`Location`**:
  ```rust
  pub struct Location {
      name: String,
      description: String,
      exits: HashMap<String, Option<(i32, i32)>>,  // Dir -> Coord or None (blocked)
      items: Vec<String>,
      actors: Vec<String>,
      image_prompt: Option<String>,
      visited: bool,  // New: Track if player has been here (for fog-of-war)
  }
  ```
  - Remove `id: String` field (coord is now ID).
  - `visited: true` set on first `MoveTo` or creation when player enters.

- **Explored tracking**: Use `locations.contains_key(&(x,y))` for "exists/explored". `visited` for map fog-of-war (show adjacent unvisited?).

- **`WorldUpdate`** (JSON to LLM):
  - Update action strings: `"MoveTo(1, 2)"`, `"CreateLocation(1, 2, {name: \"Cave\", ...})"`.
  - Enforce coord in actions.

- **Backward compatibility**: On load, assign coords to existing locations if missing (e.g., derive from exits or prompt LLM).

## 2. Movement Logic Flows

### Arrow Key Movement in [`tui.rs`](src/tui.rs:99)
Pseudocode:
```
fn handle_arrow_key(dir: Direction) -> Result<(), Error> {
    let target_pos = match dir {
        North => (current_pos.0, current_pos.1 + 1),
        South => (current_pos.0, current_pos.1 - 1),
        // etc.
    };
    if let Some(target_loc) = world.locations.get(&target_pos) {
        // Quick move: explored/exists
        world.current_pos = target_pos;
        world.locations.get_mut(&target_pos).unwrap().visited = true;
        narrative = format!("You move {} to {}.\\n{}", dir, target_loc.name, target_loc.description);
        save_world();  // Quick-save
        render();
    } else {
        // LLM fallback
        game.process_input(format!("go {}", dir.to_string()));
    }
    Ok(())
}
```

### LLM-Driven Movement
- Triggered by text input or failed quick-move.
- LLM outputs `actions: ["CreateLocation(x,y,{...})", "MoveTo(x,y)"]` or just `"MoveTo(x,y)"`.
- Parser enforces: Create only if `!locations.contains_key((x,y))`; MoveTo only if exists.

## 3. LLM Context and Prompt Updates

### Enhanced Context in [`handle_game_input`](src/game.rs:126)
- Append adjacent summaries:
  ```
  Adjacent cells:
  North (x, y+1): {name/desc if exists}
  South (x, y-1): ...
  East/West similarly.
  ```

### System Prompt Updates (lines 165-215 in [`game.rs`](src/game.rs))
```
Rules:
- Use grid coordinates: north +y, south -y, east +x, west -x.
- ONLY CreateLocation(x,y,...) if NO location exists at (x,y).
- For movement: If target (x,y) exists, use MoveTo(x,y). Else CreateLocation first.
- Assign exits with exact coords: e.g., "north": [x, y+1] or null for blocked.
- Maintain spatial coherence: no teleporting, adjacent only unless specified.
```

## 4. LLM Action Parsing and Execution in [`parse_and_apply_action`](src/game.rs:281)

Pseudocode:
```
fn parse_action(action: &str) -> Option<Action> {
    if action.starts_with("MoveTo(") {
        let (x, y) = parse_coords(action);
        if world.locations.contains_key(&(x,y)) {
            Action::MoveTo((x,y))
        } else {
            // Ignore or log error; prompt will handle creation
            None
        }
    } else if action.starts_with("CreateLocation(") {
        let (x, y, loc_json) = parse_create(action);
        if !world.locations.contains_key(&(x,y)) {
            let loc: Location = from_json(loc_json);
            // Validate exits point to valid/existing or None
            world.locations.insert((x,y), loc);
            world.locations.get_mut(&(x,y)).unwrap().visited = true;  // Since created for player
        }
        Action::Created((x,y))
    }
    // Sequential: Create then MoveTo
}
```

- Auto-save after any Create/Update.

## 5. Improved Map Rendering in [`render_map`](src/tui.rs:223)

### Features
- **Fog-of-war**: Only render locations where `visited == true` or adjacent to current_pos (reveal on approach).
- **Paths/Walls**: 
  - `@` = player.
  - `#` = location.
  - `- |` = open exits (draw lines between connected cells).
  - `X` = blocked exit (if dir has None).
- **Grid bounds**: Min/max x/y of visible locations only.
- **Colors** (Ratatui): Green #, Red X, Yellow -, Blue |, Cyan @.
- **Y reversed** (north top).

Pseudocode:
```
fn render_map(world: &WorldState) -> String {
    let visible = get_visible_locations(current_pos);  // visited + 3x3 around current
    let (min_x, max_x, min_y, max_y) = compute_bounds(visible);
    let mut grid = vec![vec!['.'; width]; height];
    for &(x,y) in visible {
        grid[y - min_y][x - min_x] = if (x,y) == current_pos { '@' } else { '#' };
        // Draw exits: if north exit Some((x, y+1)), draw '|' above, etc.
    }
    // Style with spans
}
```

```mermaid
flowchart TD
    A[Arrow Key Pressed] --> B{Target Pos Exists?}
    B -->|Yes| C[Quick Move + Narrative]
    C --> D[Save + Render]
    B -->|No| E[LLM: go dir<br/>Context: adjacents]
    E --> F[LLM JSON: CreateLocation(x,y,...)?]
    F -->|Yes| G[Insert Loc + Mark Visited]
    G --> H[MoveTo(x,y)]
    H --> D
    I[Text Input] --> E
```

## 6. File Changes Needed

| File | Changes |
|------|---------|
| [`src/model.rs`](src/model.rs) | Update structs as above; derive Serialize/Deserialize for coords keys. |
| [`src/game.rs`](src/game.rs) | Context building (+adjs); prompt rules; parse_action for coords; apply_update seq logic; save after create. |
| [`src/tui.rs`](src/tui.rs) | Arrow logic (quick vs LLM); render_map (fog, paths, colors). |
| [`src/llm.rs`](src/llm.rs) | Minor: JSON handling unchanged. |
| Save JSON | Auto-migrate old saves (derive coords from exits/LLM). |

## 7. Migration and Testing

- **Load old saves**: Scan locations, assign incremental coords based on exits graph (BFS from start).
- **Testing**: Unit tests for parse_action; integration: sim movements/LLM mocks.
- **Risks**: Coord collisions (low, prompt enforces); Ratatui color compat.

This design fully meets requirements while minimizing disruptions.
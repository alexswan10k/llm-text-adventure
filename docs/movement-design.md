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

### Arrow Key Movement in [`tui.rs`](src/tui.rs:119-202)

**Up Arrow (North)**:
- Calculates target_pos = (current_x, current_y + 1)
- If location exists: quick move, update position, mark visited, auto-save
- If location doesn't exist: call `generate_and_move_to(target_pos, "north")` for LLM generation

**Down Arrow (South)**:
- Calculates target_pos = (current_x, current_y - 1)
- Same logic as north

**Left Arrow (West)**:
- Calculates target_pos = (current_x - 1, current_y)
- Same logic as north

**Right Arrow (East)**:
- Calculates target_pos = (current_x + 1, current_y)
- Same logic as north

Pseudocode:
```
fn handle_arrow_key(dir: Direction) -> Result<(), Error> {
    let target_pos = calculate_target(dir);
    if let Some(target_loc) = world.locations.get(&target_pos) {
        // Quick move: explored/exists
        world.current_pos = target_pos;
        world.locations.get_mut(&target_pos).unwrap().visited = true;
        narrative = format!("You move {} to {}.\\n{}", dir, target_loc.name, target_loc.description);
        auto_save();  // Quick-save
    } else {
        // LLM generation for new location
        generate_and_move_to(target_pos, direction_str);
    }
    Ok(())
}
```

### LLM-Driven Movement in [`generate_and_move_to`](src/game.rs:81)
- Triggered by arrow key when target location doesn't exist
- Calls LLM with context about current location and direction
- LLM returns new location JSON (name, description, items, actors, exits, image_prompt)
- Parser validates and creates location, then moves player
- Auto-saves after successful creation
- Uses fallback location if LLM fails

## 3. LLM Context and Prompt Updates

### Enhanced Context in [`handle_game_input`](src/game.rs:233)
- Append adjacent cell information with full descriptions:
  ```
  Adjacent cells:
  North at (x, y+1): Location Name - Description (or "UNKNOWN (not yet explored)")
  South at (x, y-1): Location Name - Description (or "UNKNOWN (not yet explored)")
  East at (x+1, y): Location Name - Description (or "UNKNOWN (not yet explored)")
  West at (x-1, y): Location Name - Description (or "UNKNOWN (not yet explored)")
  ```
- **Note**: The actual implementation includes full descriptions, not just existence status. This provides richer context for the LLM to generate coherent narratives.

### System Prompt Updates (lines 179-231 in [`game.rs`](src/game.rs))
**Improved Prompt with Examples:**
- Clear examples of proper `CreateLocation` → `MoveTo` sequences
- Simplified structure with emphasis on the two-step movement pattern
- Specific JSON format examples for new vs existing locations
- Reduced adjacent cell context to existence info only (not full descriptions)

**Key Rules:**
- Use grid coordinates: north +y, south -y, east +x, west -x.
- **CRITICAL**: For movement to NEW locations, ALWAYS use BOTH actions: `CreateLocation(x,y,{...})` then `MoveTo(x,y)`
- For movement to EXISTING locations: use only `MoveTo(x,y)`
- Assign exits with exact coords: e.g., "north": [x, y+1] or null for blocked.
- Maintain spatial coherence: adjacent movement only unless specified.

## 4. LLM Action Parsing and Execution in [`parse_and_apply_action`](src/game.rs:422)

**Key Implementation Changes:**
- **MoveTo with fallback**: `MoveTo(x,y)` creates a default location if it doesn't exist, then moves there
- **Error logging**: Failed actions are logged to debug log but don't stop execution
- **Exit validation**: Validates that exits point to adjacent coordinates and logs warnings for non-adjacent exits
- **Two-pass processing**:
  1. Create all locations first
  2. Handle all other actions (MoveTo, items, etc.)

Pseudocode:
```
fn parse_and_apply_action(action: &str) -> Result<()> {
    if action.starts_with("MoveTo(") {
        let (x, y) = parse_coords(action);
        if world.locations.contains_key(&(x,y)) {
            world.current_pos = (x,y);
            mark_visited((x,y));
        } else {
            // Create default location as fallback
            let default_loc = Location {
                name: format!("Location ({}, {})", x, y),
                description: "A mysterious place that appeared suddenly.".to_string(),
                // ... other defaults
            };
            world.locations.insert((x,y), default_loc);
            world.current_pos = (x,y);
        }
    } else if action.starts_with("CreateLocation(") {
        let (x, y, loc_json) = parse_create(action);
        if !world.locations.contains_key(&(x,y)) {
            let loc: Location = from_json(loc_json);
            validate_exits(&loc); // Check adjacency and coordinate validity
            world.locations.insert((x,y), loc);
            mark_visited((x,y));
        } else {
            log("Location already exists, skipping");
        }
    }
    // ... other actions
}
```

- Auto-save after any action in game loop.
- **Error handling**: Errors are logged to debug log but don't cause retry; failures are displayed in narrative.

## 5. Improved Map Rendering in [`render_map`](src/tui.rs:291)

### Features
- **Fog-of-war**: Only render locations where `visited == true` or adjacent to current_pos (reveal on approach).
- **Paths/Walls**:
  - `@` = player
  - `#` = visited location
  - `?` = adjacent unvisited location
  - `.` = unexplored space
  - `|` = north/south path between locations
  - `-` = east/west path between locations
- **Grid bounds**: Min/max x/y of visible locations only.
- **Y reversed**: Y coordinates reversed (north at top, south at bottom).
- **No colors**: Current implementation uses plain ASCII characters without Ratatui colors.

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
| [`src/model.rs`](src/model.rs) | Update structs with coordinate-based HashMaps; derive Serialize/Deserialize for coords keys; add custom serializers for HashMap<(i32, i32), T>. |
| [`src/game.rs`](src/game.rs) | Context building (full adjacent descriptions); prompt rules; parse_action for coords; apply_update seq logic; auto-save after each action; fallback location creation. |
| [`src/tui.rs`](src/tui.rs) | Arrow key handlers (quick vs LLM generation); render_map with fog-of-war and path drawing; no colors used. |
| [`src/llm.rs`](src/llm.rs) | JSON handling unchanged; uses standard OpenAI-compatible API. |
| [`src/save.rs`](src/save.rs) | Auto-migrate old saves (derive coords from exits using BFS); handle coordinate serialization/deserialization. |

## 7. Migration and Testing

### Save Migration ([`src/save.rs`](src/save.rs:76-303))
- **Load old saves**: Scans old string-based locations, extracts x/y coordinates from old data
- Uses BFS-like approach to assign coordinates based on existing x/y fields
- Converts exits from string IDs to coordinate tuples
- Converts actors' current_location_id to current_pos coordinates
- Migrates items, player inventory, and player money

### Testing
- **Manual testing**: Quick movement to adjacent explored cells works; LLM generation for new cells works; map renders correctly with fog-of-war
- **No unit tests**: Currently no automated tests for parse_action or movement logic
- **Risks**: Low - coordinate collisions prevented by HashMap structure; no Ratatui colors to worry about

### Implementation Notes (Differences from Design)
1. **MoveTo fallback**: Creates default location instead of failing (designed to fail)
2. **Adjacent context**: Uses full descriptions instead of simple existence status
3. **Map rendering**: Plain ASCII without colors (design called for colored output)
4. **Error handling**: Logs to debug log instead of retrying with error context
5. **Exit validation**: Logs warnings but doesn't enforce bidirectional consistency

This design documents the coordinate-based movement system as implemented, with notes where implementation differs from original design decisions.
# Infinite Text Adventure

A Rust-based Text Adventure Engine that runs as a Terminal TUI and compiles to WASM for the web.
The game content is generated at runtime by a Local LLM (e.g., LM Studio), maintaining a persistent world state.

## Features

- **Infinite World**: Locations and narratives generated on the fly by LLM.
- **Coordinate-Based Map**: Grid-based world with `(x, y)` coordinates for spatial consistency.
- **Quick Movement**: Arrow keys for instant movement to explored adjacent cells.
- **Fog-of-War Map**: Only reveals visited and nearby locations on the ASCII map.
- **Enhanced LLM Context**: Includes adjacent cell information for better spatial coherence.
- **Persistent State**: World, actors, and items are saved to JSON with auto-save.
- **Splash Screen**: Manage multiple save files.
- **TUI Interface**: Split layout for visuals, narrative, and input with styled map rendering.
- **WASM Support**: Play in the browser.

## Quick Start

### Prerequisites

1.  **Rust**: Install from [rustup.rs](https://rustup.rs/).
2.  **Local LLM**: Run a server compatible with OpenAI API (e.g., LM Studio).
    *   Base URL: `http://localhost:1234`
    *   Model: `qwen3-coder-30b-a3b-instruct` (or similar)

### Running Native

```bash
# Set environment variables (optional, defaults shown)
export LLM_BASE_URL="http://localhost:1234"
export LLM_MODEL_NAME="qwen3-coder-30b-a3b-instruct"

cargo run
```

### Running WASM

1.  Install `wasm-pack`:
    ```bash
    cargo install wasm-pack
    ```
2.  Build and Serve:
    ```bash
    wasm-pack build --target web
    python3 -m http.server
    ```
3.  Open `http://localhost:8000`.

## Controls

- **Splash Screen**:
    - `Up`/`Down`: Select save file.
    - `Enter`: Load save or Start New Game.
- **In Game**:
    - **Arrow Keys**: Quick move to explored adjacent cells (North/South/East/West).
    - **Text Input**: Type action (e.g., "look around", "go north", "take sword") and press `Enter` for LLM-driven actions.
    - `Esc`: Quit.

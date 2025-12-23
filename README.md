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

## Quick Startwh

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

### CLI Debug Mode (For LLM Testing)

**Purpose**: The CLI debug mode (`--llm-mode`) is designed specifically for LLM agents to test and debug the game logic. It uses stdin/stdout for all interaction, making it fully automatable without TUI overhead.

**How to use**:
```bash
# Run in debug mode
cargo run -- --llm-mode

# Or use the release binary
./target/release/llm-text-adventure --llm-mode
```

**Interaction Protocol**:
1. The game prints world state to stdout after each action
2. Read your command from stdin
3. The game processes and prints new state
4. Repeat until `/exit` command

**Special Commands**:
- `/north`, `/south`, `/east`, `/west` - Quick move in direction
- `/exit` - Exit the game cleanly
- `1`, `2`, `3`, etc. - Select from suggested actions list
- Any other text - Pass to game.process_input() for LLM interpretation

**Automated Testing Example**:
```bash
# Pipe commands for automated testing
echo -e "new\nTestWorld\nenter\nlook around\n/exit" | \
  ./target/release/llm-text-adventure --llm-mode
```

## OpenCode Tool Integration (For LLM Debugging)

**Purpose**: A session server that enables persistent REPL interaction across multiple LLM tool calls. This allows an LLM agent to maintain a continuous game session while testing game logic.

**This is a debugging tool, not part of the core game application.**

### Setup

1. **Build the game binary**:
```bash
cargo build --release
```

2. **Install session server dependencies**:
```bash
cd session-server
npm install
cd ..
```

3. **OpenCode tool is auto-started**: The `.opencode/tools/text-adventure.ts` tool will automatically start the session server when first called.

### OpenCode Tool Usage

The tool provides these actions:

| Action | Args | Description |
|--------|------|-------------|
| `start` | - | Start a new game session, returns `session_id` |
| `input` | `session_id`, `command` | Send command to game, get output |
| `status` | `session_id` | Check if session is active |
| `list` | - | List all active sessions |
| `stop` | `session_id` | Terminate a game session |
| `kill_server` | - | Shutdown session server |

### Example Tool Call Sequence

```
1. Start new session:
   action: "start"
   → Returns: session_id, initial_output

2. Send command:
   action: "input"
   session_id: "uuid-here"
   command: "look around"
   → Returns: complete world state

3. Quick move:
   action: "input"
   session_id: "uuid-here"
   command: "/north"
   → Returns: updated world state

4. Check status:
   action: "status"
   session_id: "uuid-here"
   → Returns: {active, last_activity, is_waiting}

5. Stop session:
   action: "stop"
   session_id: "uuid-here"
   → Session terminated, game process killed
```

### Auto-Features

- **Auto-start**: Session server starts automatically on first tool call
- **Auto-cleanup**: Inactive sessions deleted after 30 minutes
- **Timeout**: Commands that don't respond within 60 seconds return error (session preserved)

### Server Details

- **Port**: 8080
- **Process**: Node.js server in `session-server/`
- **Management**: Spawns game processes with `--llm-mode` flag
- **Documentation**: See `session-server/README.md` for API details

### Manual Server Management

If you need to manually control the server:

```bash
# Start server manually
cd session-server
npm start

# The server will run on port 8080
# Use Ctrl+C to stop, or send DELETE /server via HTTP
```

### Stopping a Forgotten Session

If the LLM forgets to stop a session:

1. Use tool action: `list` - See all active sessions
2. Use tool action: `stop` with the `session_id` - Terminate specific session
3. Or use tool action: `kill_server` - Shutdown entire server (kills all sessions)

Sessions are also auto-deleted after 30 minutes of inactivity.

### Running WASM

```bash
./run-wasm.sh
```

This script installs `wasm-pack` if needed, builds the project, and starts a local server at `http://localhost:8000`.

**Or manually**:

1.  Install `wasm-pack`:
    ```bash
    cargo install wasm-pack
    ```
2.  Build and Serve:
    ```bash
    wasm-pack build --target web
    python3 -m http.server 8000
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

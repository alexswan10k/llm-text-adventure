# Session Server

A Node.js server that manages persistent LLM text adventure game sessions for debugging.

This is a **debugging tool**, not part of the core game application. It enables LLM agents to interact with the game via a true REPL (Read-Eval-Print Loop) across multiple tool calls.

## Why This Exists

The core game has two modes:
1. **TUI mode** - Interactive terminal UI (not automatable by LLMs)
2. **CLI mode (`--llm-mode`)** - Stdin/stdout interaction (single session only)

The session server solves this by:
- Managing multiple concurrent game processes
- Providing HTTP API for session control
- Auto-starting and auto-cleaning up sessions
- Handling timeouts and stuck processes gracefully

## Installation

```bash
cd session-server
npm install
```

## Starting the Server

```bash
# Start server (runs in foreground)
npm start

# Or start in background
npm start &
```

The server will start on port **8080**.

## Configuration

Edit `index.js` to change:

| Setting | Default | Description |
|---------|----------|-------------|
| `PORT` | 8080 | HTTP server port |
| `SESSION_TIMEOUT` | 30 * 60 * 1000 (30 min) | Auto-delete inactive sessions |
| `RESPONSE_TIMEOUT` | 60 * 1000 (60 sec) | Max time to wait for game response |
| `CLEANUP_INTERVAL` | 5 * 60 * 1000 (5 min) | How often to check for stale sessions |

## API Endpoints

### POST `/start`
Start a new game session.

**Response:**
```json
{
  "session_id": "uuid-here",
  "initial_output": "=== LLM Debug Mode ===\n..."
}
```

### POST `/input/:sessionId`
Send a command to a game session.

**Request:**
```json
{
  "command": "look around"
}
```

**Response:**
```json
{
  "output": "...game state output..."
}
```

**Error (timeout):**
```json
{
  "error": "Response timeout",
  "message": "Game did not respond within 60 seconds. Session preserved but may be stuck."
}
```

### GET `/status/:sessionId`
Check if a session is active.

**Response:**
```json
{
  "active": true,
  "last_activity": 1234567890,
  "is_waiting": false
}
```

### GET `/sessions`
List all active sessions.

**Response:**
```json
{
  "sessions": [
    {
      "id": "uuid-1",
      "last_activity": 1234567890,
      "is_waiting": false,
      "active": true
    }
  ]
}
```

### GET `/health`
Check if server is running.

**Response:**
```json
{
  "status": "ok",
  "sessions": 2
}
```

### DELETE `/sessions/:sessionId`
Terminate a game session (cleanly exits game, saves if needed).

**Response:**
```json
{
  "message": "Session uuid-here terminated"
}
```

### DELETE `/server`
Shutdown the session server (terminates all sessions).

**Response:**
```json
{
  "message": "Session server shutdown initiated"
}
```

## Game Commands

Once a session is started, you can send these commands via `/input`:

| Command | Description |
|---------|-------------|
| `/north`, `/south`, `/east`, `/west` | Quick movement (instant if location exists) |
| `/exit` | Exit the game (also terminates session) |
| `1`, `2`, `3`, etc. | Select from suggested actions list |
| Any text | Pass to game for LLM interpretation |

## Output Format

Each command response includes the complete world state:

```
========================================
WORLD STATE
========================================

--- Location ---
Name: The Beginning
Position: (0, 0)
Description: You stand in a void of potential...
Visited: true

--- Player Stats ---
Money: 0

--- Narrative ---
The story text...

--- Suggested Actions ---
  1. go north
  2. go south
  3. examine surroundings

--- Game State ---
State: WaitingForInput
Save Path: Some("savefile.json")

> [prompt ends here]
```

## Auto-Cleanup

The server automatically:
- Checks for inactive sessions every 5 minutes
- Deletes sessions that haven't been used for 30 minutes
- Terminates game processes cleanly (saves if applicable)

## Timeout Handling

If a game process doesn't respond within **60 seconds**:
- The tool returns an error
- The session is **preserved** (not killed)
- You can use `stop` action to terminate the stuck session

## Stopping the Server

To stop the server manually:
1. Use the OpenCode tool with action `kill_server`
2. Or send `DELETE /server` via HTTP
3. Or `Ctrl+C` if running in foreground

## Troubleshooting

### "Failed to start game session"
- Ensure the game binary exists at `../target/release/llm-text-adventure`
- Run `cargo build --release` from the project root

### "Response timeout"
- The game process may be stuck (e.g., waiting on LLM that hasn't loaded)
- Session is preserved - you can try another command or use `stop`

### Server won't start (port already in use)
- Check if another instance is running: `lsof -i :8080`
- Change `PORT` in `index.js` or kill the existing process

### Old processes still running
- Sessions are auto-cleaned after 30 minutes
- Use `GET /sessions` to list active sessions
- Use `DELETE /sessions/:id` to manually terminate specific sessions

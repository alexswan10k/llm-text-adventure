# Session Server & OpenCode Tool - Implementation Complete

## Summary

A complete session server implementation with OpenCode tool integration for LLM text adventure game debugging.

## What Was Built

### 1. Session Server (`session-server/`)
- Node.js Express server on port 8080
- Manages persistent game processes (--llm-mode)
- HTTP API for session control
- Auto-cleanup of inactive sessions (30min timeout)
- Response timeout protection (60s per command)
- Graceful shutdown handling

### 2. OpenCode Tool (`.opencode/tools/text-adventure.ts`)
- TypeScript tool for session management
- Auto-starts session server on first call
- Actions: start, input, status, list, stop, kill_server
- Error handling for all edge cases
- Clear response formatting

### 3. Documentation
- `session-server/README.md` - Server API reference
- `README.md` - Updated with tool usage
- Setup and test scripts

## Validation Results

### API Tests (test-session-server.sh)
✅ Start new session
✅ Send input command
✅ Check session status
✅ List all sessions
✅ Stop session
✅ Verify session cleanup
✅ Health check
✅ Server shutdown

### OpenCode Tool Test (Child Agent)
✅ Session started: `4be0dbe1-3508-4400-8a07-74b23de3bb66`
✅ Multiple commands sent successfully
✅ World state received with all fields
✅ Session status check passed
✅ Session stopped cleanly

## File Structure

```
llm-text-adventure/
├── .opencode/
│   └── tools/
│       └── text-adventure.ts        # OpenCode tool
├── session-server/
│   ├── index.js                    # Server implementation
│   ├── package.json                # Node dependencies
│   └── README.md                  # API documentation
├── .gitignore                    # Added node_modules/
├── setup-session-server.sh         # Quick setup script
├── test-session-server.sh          # Validation tests
└── README.md                     # Updated with tool usage
```

## How LLM Agents Use It

### Step 1: Start Session
```
action: "start"
→ Returns: session_id, initial_output
```

### Step 2: Send Commands (repeat as needed)
```
action: "input"
session_id: "<id>"
command: "look around"
→ Returns: world state, narrative, suggested actions
```

### Step 3: Stop When Done
```
action: "stop"
session_id: "<id>"
→ Session terminated cleanly
```

## Key Features

| Feature | Implementation |
|----------|---------------|
| Auto-start server | Tool detects missing server and spawns it |
| Persistent sessions | Game processes run until stopped |
| Multi-session support | Server manages concurrent game instances |
| Auto-cleanup | Inactive sessions removed after 30min |
| Timeout protection | Commands fail after 60s (session preserved) |
| Graceful shutdown | `/exit` sent before SIGTERM |
| Health monitoring | `/health` endpoint for status checks |

## Edge Cases Handled

- Server not running → Auto-start
- Session already exists → Continue using it
- Timeout waiting for output → Return error, preserve session
- Process crashes → Auto-remove from session map
- Forgotten sessions → List and stop manually, or wait for auto-cleanup
- Port already in use → Documented troubleshooting

## Quick Start

```bash
# 1. Build game
cargo build --release

# 2. Install dependencies (or run setup script)
cd session-server && npm install && cd ..
# OR
./setup-session-server.sh

# 3. Use via OpenCode tool
# The tool auto-starts server on first call
```

## Testing

```bash
# Run API validation tests
./test-session-server.sh

# Or start manually and test with curl
cd session-server && npm start &
curl -X POST http://localhost:8080/start
```

## Future Improvements (Optional)

- Add session restart (keep save, new process)
- Add session pause/resume
- Rate limiting per session
- Session export/import (save to file)
- WebSocket support for real-time updates

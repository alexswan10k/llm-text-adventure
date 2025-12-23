# LLM Text Adventure Debugger

Session server for REPL testing. Port: 8080.

## Tool

**Action: `repl`** - Write command, read response. Auto-starts session if session_id missing.

**Optional:** `stop`, `kill_server`

## Usage

```
action: 'repl', command: 'new'           → Starts session, sends "new"
action: 'repl', command: 'WorldName'    → Sends command to existing session
action: 'repl', command: 'look around'   → Explore
action: 'repl', command: '/north'         → Quick move N
action: 'stop', session_id: '<id>'          → End session
action: 'kill_server'                       → Shutdown server
```

## Returns

```json
{
  "session_id": "uuid",
  "new_session": true/false,
  "location": "...",
  "position": {"x": 0, "y": 0},
  "narrative": "...",
  "suggestedActions": ["..."],
  "gameState": "...",
  "money": 0
}
```

## Setup

```bash
cargo build --release
cd session-server && npm install && cd ..
```

Tool auto-starts server.

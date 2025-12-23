/**
 * Session Server for LLM Text Adventure Debugging
 *
 * Manages persistent game processes (--llm-mode) with REPL interface.
 *
 * Auto-cleanup: Sessions inactive > 30 minutes are terminated.
 * Timeout: Game responses > 60 seconds return error (session preserved).
 */

const express = require('express');
const { spawn } = require('child_process');
const { v4: uuidv4 } = require('uuid');

const app = express();
app.use(express.json());

const PORT = 8080;

const sessions = new Map();
const SESSION_TIMEOUT = 30 * 60 * 1000;
const RESPONSE_TIMEOUT = 60 * 1000;
const CLEANUP_INTERVAL = 5 * 60 * 1000;

setInterval(() => {
  const now = Date.now();
  for (const [id, session] of sessions.entries()) {
    if (now - session.lastActivity > SESSION_TIMEOUT) {
      console.log(`Auto-cleanup: ${id}`);
      killSession(id);
    }
  }
}, CLEANUP_INTERVAL);

/**
 * Start new game session (optional, repl auto-starts if session_id missing)
 * POST /start
 */
app.post('/start', (req, res) => {
  const sessionId = uuidv4();
  console.log(`Start: ${sessionId}`);

  const gameProcess = spawn('../target/release/llm-text-adventure', ['--llm-mode'], { cwd: __dirname });

  const session = {
    process: gameProcess,
    lastActivity: Date.now(),
    outputBuffer: '',
    isWaiting: false,
    resolveOutput: null,
    outputTimeout: null
  };

  gameProcess.stdout.on('data', (data) => {
    const output = data.toString();
    session.outputBuffer += output;

    if (output.includes('> ')) {
      session.isWaiting = false;
      if (session.resolveOutput) {
        clearTimeout(session.outputTimeout);
        session.resolveOutput(session.outputBuffer);
        session.resolveOutput = null;
        session.outputTimeout = null;
      }
    }
  });

  gameProcess.stderr.on('data', (data) => console.error(`Session ${sessionId} stderr:`, data.toString()));

  gameProcess.on('close', (code) => {
    console.log(`Session ${sessionId} exit: ${code}`);
    if (session.resolveOutput) {
      clearTimeout(session.outputTimeout);
      session.resolveOutput(null);
      session.resolveOutput = null;
      session.outputTimeout = null;
    }
    sessions.delete(sessionId);
  });

  sessions.set(sessionId, session);

  waitForOutput(session, RESPONSE_TIMEOUT)
    .then((initialOutput) => {
      session.outputBuffer = '';
      res.json({ session_id: sessionId, initial_output: initialOutput });
    })
    .catch((error) => {
      killSession(sessionId);
      res.status(500).json({ error: 'Failed to start', message: error.message });
    });
});

/**
 * REPL: Write command, read response (auto-start if no session_id)
 * POST /repl (with body: { command: string, session_id?: string })
 * POST /repl/:sessionId
 * Body: { command: string }
 * Returns: { output: string, session_id?: string }
 */
app.post('/repl', async (req, res) => {
  const { command, session_id } = req.body;
  
  if (!session_id) {
    const startRes = await fetch(`http://localhost:${PORT}/start`, { method: 'POST' });
    const startData = await startRes.json();
    if (!startRes.ok) return res.status(startRes.status).json(startData);
    
    const sessionId = startData.session_id;
    
    req.params = { sessionId };
    replHandler(req, res, sessionId, command, true);
  } else {
    const session = sessions.get(session_id);
    if (!session) return res.status(404).json({ error: 'Not found' });
    replHandler(req, res, session_id, command);
  }
});

app.post('/repl/:sessionId', (req, res) => {
  const { sessionId } = req.params;
  const { command } = req.body;
  replHandler(req, res, sessionId, command);
});

function replHandler(req, res, sessionId, command, includeSessionId = false) {
  const session = sessions.get(sessionId);

  if (!session) return res.status(404).json({ error: 'Not found' });
  if (session.process.killed) return res.status(410).json({ error: 'Terminated' });

  console.log(`Session ${sessionId}: ${command}`);

  session.process.stdin.write(command + '\n');
  session.lastActivity = Date.now();
  session.isWaiting = true;

  waitForOutput(session, RESPONSE_TIMEOUT)
    .then((output) => {
      if (output === null) return res.status(410).json({ error: 'Process ended' });
      const result = session.outputBuffer;
      session.outputBuffer = '';
      const response = { output: result };
      if (includeSessionId) response.session_id = sessionId;
      res.json(response);
    })
    .catch((error) => {
      session.isWaiting = false;
      if (error.message === 'TIMEOUT') {
        res.status(504).json({
          error: 'Timeout',
          message: `No response in ${RESPONSE_TIMEOUT/1000}s. Use stop if stuck.`
        });
      } else {
        res.status(500).json({ error: 'Failed', message: error.message });
      }
    });
}

app.get('/health', (req, res) => res.json({ status: 'ok', sessions: sessions.size }));

app.delete('/sessions/:sessionId', (req, res) => {
  const { sessionId } = req.params;
  if (killSession(sessionId)) res.json({ message: 'Stopped' });
  else res.status(404).json({ error: 'Not found' });
});

app.delete('/server', (req, res) => {
  console.log('Shutdown...');
  for (const [id] of sessions.keys()) killSession(id);
  res.json({ message: 'Shutdown' });
  setTimeout(() => process.exit(0), 100);
});

// Helper function to wait for output with timeout
function waitForOutput(session, timeout) {
  return new Promise((resolve, reject) => {
    session.resolveOutput = resolve;

    // Set timeout
    session.outputTimeout = setTimeout(() => {
      if (session.resolveOutput) {
        session.resolveOutput = null;
        session.outputTimeout = null;
        reject(new Error('TIMEOUT'));
      }
    }, timeout);
  });
}

// Helper function to kill a session
function killSession(sessionId) {
  const session = sessions.get(sessionId);
  if (!session) {
    return false;
  }

  console.log(`Killing session: ${sessionId}`);

  // Cancel any pending timeout
  if (session.outputTimeout) {
    clearTimeout(session.outputTimeout);
  }

  // Send /exit command for clean shutdown, then kill
  try {
    session.process.stdin.write('/exit\n');
  } catch (e) {
    // Ignore if stdin already closed
  }

  // Force kill after 1 second
  setTimeout(() => {
    if (!session.process.killed) {
      session.process.kill('SIGTERM');
    }
  }, 1000);

  sessions.delete(sessionId);
  return true;
}

// Start server
app.listen(PORT, () => {
  console.log(`LLM Text Adventure Session Server running on port ${PORT}`);
  console.log(`Auto-cleanup interval: ${CLEANUP_INTERVAL/1000}s, Session timeout: ${SESSION_TIMEOUT/1000}s`);
  console.log(`Response timeout: ${RESPONSE_TIMEOUT/1000}s`);
  console.log('Endpoints:');
  console.log('  POST   /start            - Start new game session');
  console.log('  POST   /input/:id         - Send command to session');
  console.log('  GET    /status/:id        - Get session status');
  console.log('  GET    /sessions          - List all sessions');
  console.log('  GET    /health            - Server health check');
  console.log('  DELETE /sessions/:id      - Terminate session');
  console.log('  DELETE /server           - Shutdown server');
});

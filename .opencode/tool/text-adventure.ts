/**
 * OpenCode Tool: LLM Text Adventure Debugger (REPL)
 *
 * Write command → read response in one operation.
 */

import { tool } from "@opencode-ai/plugin"
import http from "http";

const SERVER_URL = "http://localhost:8080";

function request(method, path, body = null) {
  return new Promise((resolve, reject) => {
    const url = new URL(path, SERVER_URL);
    const options = { method, headers: { 'Content-Type': 'application/json' } };

    const req = http.request(url, options, (res) => {
      let data = '';
      res.on('data', c => data += c);
      res.on('end', () => {
        try {
          const parsed = JSON.parse(data);
          res.statusCode >= 400 ? reject({ statusCode: res.statusCode, error: parsed }) : resolve({ statusCode: res.statusCode, data: parsed });
        } catch (e) {
          resolve({ statusCode: res.statusCode, data });
        }
      });
    });
    req.on('error', reject);
    if (body) req.write(JSON.stringify(body));
    req.end();
  });
}

async function ensureServer() {
  try {
    await request('GET', '/health');
  } catch (e) {
    const { spawn } = require('child_process');
    const path = require('path');
    spawn('node', ['index.js'], {
      cwd: path.join(__dirname, '../../session-server'),
      detached: true,
      stdio: 'ignore'
    }).unref();

    for (let i = 20; i >0; i--) {
      await new Promise(r => setTimeout(r, 100));
      try {
        await request('GET', '/health');
        return;
      } catch (e) {}
    }
    throw new Error("Failed to start session server");
  }
}

function parseOutput(output) {
  const parsed = { location: null, position: null, narrative: '', suggestedActions: [], gameState: '', money: 0 };
  const lines = output.split('\n');
  let section = '';

  for (const line of lines) {
    const t = line.trim();
    if (t.startsWith('--- Location ---')) section = 'loc';
    else if (t.startsWith('--- Player Stats ---')) section = 'stats';
    else if (t.startsWith('--- Narrative ---')) section = 'nar';
    else if (t.startsWith('--- Suggested Actions ---')) section = 'act';
    else if (t.startsWith('--- Game State ---')) section = 'state';
    else if (t.startsWith('---')) section = '';
    else {
      if (section === 'loc' && t.startsWith('Name:')) parsed.location = t.replace('Name:', '').trim();
      else if (section === 'loc' && t.startsWith('Position:')) {
        const m = t.match(/\((\d+),\s*(\d+)\)/);
        if (m) parsed.position = { x: parseInt(m[1]), y: parseInt(m[2]) };
      }
      else if (section === 'stats' && t.startsWith('Money:')) parsed.money = parseInt(t.replace('Money:', '').trim());
      else if (section === 'nar' && t) parsed.narrative += t + '\n';
      else if (section === 'act' && t.match(/^\d+\./)) parsed.suggestedActions.push(t.replace(/^\d+\.\s*/, '').trim());
      else if (section === 'state' && t.startsWith('State:')) parsed.gameState = t.replace('State:', '').trim();
    }
  }
  return parsed;
}

export default tool({
  description: "LLM text adventure REPL. Action: repl (write command, read response). Args: session_id (optional, starts new if missing), command. Optional: stop, kill_server. Auto-starts server (port 8080), auto-cleanup (30min), timeout (60s).",
  args: {
    action: tool.schema.string().describe("Action: 'repl' (write command + read response), 'stop', 'kill_server'"),
    session_id: tool.schema.string().optional().describe("Session ID (if missing, starts new session)"),
    command: tool.schema.string().optional().describe("Command to send (required for repl action)"),
  },
  async execute(args) {
    try {
      await ensureServer();

      switch (args.action) {
        case 'repl': {
          const sessionId = args.session_id || (await request('POST', '/start')).data.session_id;
          const r = await request('POST', `/repl/${sessionId}`, { command: args.command });
          return {
            session_id: sessionId,
            new_session: !args.session_id,
            ...parseOutput(r.data.output)
          };
        }

        case 'stop': {
          await request('DELETE', `/sessions/${args.session_id}`);
          return { done: true };
        }

        case 'kill_server': {
          await request('DELETE', '/server');
          return { done: true };
        }

        default:
          return { error: "Invalid action. Use: repl, stop, kill_server" };
      }
    } catch (error) {
      return {
        error: error.error?.error || error.message || 'Failed',
        session_id: args.session_id
      };
    }
  },
})

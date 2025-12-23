#!/bin/bash
# Quick start script for session server + OpenCode tool testing

set -e

echo "=== LLM Text Adventure Session Server - Quick Start ==="
echo ""

# Check if game binary exists
if [ ! -f "target/release/llm-text-adventure" ]; then
  echo "Game binary not found. Building..."
  cargo build --release
  echo ""
fi

# Check if session-server dependencies are installed
if [ ! -d "session-server/node_modules" ]; then
  echo "Installing session server dependencies..."
  cd session-server
  npm install
  cd ..
  echo ""
fi

echo "Setup complete!"
echo ""
echo "To start the session server manually:"
echo "  cd session-server && npm start"
echo ""
echo "To start a game in CLI mode (single session):"
echo "  ./target/release/llm-text-adventure --llm-mode"
echo ""
echo "For OpenCode tool integration:"
echo "  - Use the text-adventure.ts tool in .opencode/tools/"
echo "  - The tool will auto-start the session server"
echo "  - Server runs on port 8080"
echo ""
echo "See documentation:"
echo "  - session-server/README.md - Server API details"
echo "  - README.md - Complete game documentation"
echo ""

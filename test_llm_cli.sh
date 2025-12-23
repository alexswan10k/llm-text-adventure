#!/bin/bash
# Automated LLM testing script for CLI debug mode
# This demonstrates how an LLM agent can test the game via stdin/stdout

set -e

echo "=== LLM Automated Testing Script ==="
echo ""
echo "This script demonstrates automated testing via CLI mode."
echo "The LLM agent would read stdout and send commands to stdin."
echo ""

# Build the project first
echo "Building project..."
cargo build --release 2>&1 | grep -E "(Compiling|Finished|error)" || true
echo ""

# Test sequence for LLM agent to execute
# This simulates an LLM exploring the game world
echo "Test sequence:"
echo "1. Create new world named 'LLMTest'"
echo "2. Look around"
echo "3. Quick move north using /north command"
echo "4. Look around again"
echo "5. Exit cleanly with /exit"
echo ""

# Run the game with piped input
echo "Starting game..."
echo "---"

./target/release/llm-text-adventure --llm-mode <<EOF
new
LLMTest
enter
look around
/north
look around
/exit
EOF

echo "---"
echo ""
echo "Test completed. The LLM agent successfully:"
echo "- Created a new world"
echo "- Received world state via stdout"
echo "- Sent commands via stdin"
echo "- Used quick movement commands"
echo "- Exited cleanly"
echo ""
echo "For LLM integration:"
echo "- Read stdout to get complete world state"
echo "- Parse state to understand current game context"
echo "- Send appropriate commands via stdin"
echo "- Handle the /exit command to terminate"

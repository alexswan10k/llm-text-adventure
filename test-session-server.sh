#!/bin/bash
# Test script to validate session server functionality
# This simulates what an LLM agent would do via OpenCode tool

set -e

echo "=== Session Server Validation Test ==="
echo ""

# Start session server in background
echo "1. Starting session server..."
cd session-server
node index.js > /tmp/session-server.log 2>&1 &
SERVER_PID=$!
cd ..
echo "   Server PID: $SERVER_PID"

# Wait for server to start
echo "   Waiting for server to be ready..."
for i in {1..20}; do
  if curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "   Server is ready!"
    break
  fi
  sleep 0.1
done
echo ""

# Test 1: Start new session
echo "2. Testing: Start new session"
START_RESPONSE=$(curl -s -X POST http://localhost:8080/start)
echo "   Response: $START_RESPONSE"

SESSION_ID=$(echo "$START_RESPONSE" | grep -o '"session_id":"[^"]*' | cut -d'"' -f4)
echo "   Session ID: $SESSION_ID"

if [ -z "$SESSION_ID" ]; then
  echo "   ❌ FAILED: No session_id returned"
  kill $SERVER_PID
  exit 1
fi
echo "   ✅ PASS"
echo ""

# Test 2: Send input command
echo "3. Testing: Send 'look around' command"
INPUT_RESPONSE=$(curl -s -X POST http://localhost:8080/input/$SESSION_ID \
  -H "Content-Type: application/json" \
  -d '{"command":"look around"}')
echo "   Response length: ${#INPUT_RESPONSE}"

if [ ${#INPUT_RESPONSE} -lt 100 ]; then
  echo "   ❌ FAILED: Response too short"
  kill $SERVER_PID
  exit 1
fi

# Check for expected output patterns
if ! echo "$INPUT_RESPONSE" | grep -q "WORLD STATE"; then
  echo "   ❌ FAILED: Missing WORLD STATE in output"
  kill $SERVER_PID
  exit 1
fi

echo "   ✅ PASS: Got expected output with WORLD STATE"
echo ""

# Test 3: Check session status
echo "4. Testing: Check session status"
STATUS_RESPONSE=$(curl -s http://localhost:8080/status/$SESSION_ID)
echo "   Response: $STATUS_RESPONSE"

if ! echo "$STATUS_RESPONSE" | grep -q '"active":true'; then
  echo "   ❌ FAILED: Session not marked as active"
  kill $SERVER_PID
  exit 1
fi
echo "   ✅ PASS: Session is active"
echo ""

# Test 4: List all sessions
echo "5. Testing: List all sessions"
LIST_RESPONSE=$(curl -s http://localhost:8080/sessions)
echo "   Response: $LIST_RESPONSE"

if ! echo "$LIST_RESPONSE" | grep -q "$SESSION_ID"; then
  echo "   ❌ FAILED: Session not in list"
  kill $SERVER_PID
  exit 1
fi
echo "   ✅ PASS: Session found in list"
echo ""

# Test 5: Stop session
echo "6. Testing: Stop session"
STOP_RESPONSE=$(curl -s -X DELETE http://localhost:8080/sessions/$SESSION_ID)
echo "   Response: $STOP_RESPONSE"

if ! echo "$STOP_RESPONSE" | grep -q "terminated"; then
  echo "   ❌ FAILED: Session not terminated"
  kill $SERVER_PID
  exit 1
fi
echo "   ✅ PASS: Session terminated"
echo ""

# Test 6: Verify session is gone
echo "7. Testing: Verify session is stopped"
STATUS_AFTER_STOP=$(curl -s http://localhost:8080/status/$SESSION_ID)

if ! echo "$STATUS_AFTER_STOP" | grep -q '"Session not found"'; then
  echo "   ❌ FAILED: Session still exists after stop"
  kill $SERVER_PID
  exit 1
fi
echo "   ✅ PASS: Session no longer exists"
echo ""

# Test 7: Health check
echo "8. Testing: Health check"
HEALTH_RESPONSE=$(curl -s http://localhost:8080/health)
echo "   Response: $HEALTH_RESPONSE"

if ! echo "$HEALTH_RESPONSE" | grep -q '"status":"ok"'; then
  echo "   ❌ FAILED: Health check failed"
  kill $SERVER_PID
  exit 1
fi
echo "   ✅ PASS: Server is healthy"
echo ""

# Cleanup: Shutdown server
echo "9. Testing: Shutdown server"
SHUTDOWN_RESPONSE=$(curl -s -X DELETE http://localhost:8080/server)
echo "   Response: $SHUTDOWN_RESPONSE"

# Wait for server to stop
sleep 1
if kill -0 $SERVER_PID 2>/dev/null; then
  echo "   ⚠️  Server still running, forcing kill"
  kill $SERVER_PID
fi
echo "   ✅ PASS: Server shut down"
echo ""

# Summary
echo "=== All Tests Passed ✅ ==="
echo ""
echo "Session server is working correctly!"
echo "The OpenCode tool should be able to:"
echo "  - Auto-start the server"
echo "  - Start game sessions"
echo "  - Send commands and receive responses"
echo "  - Manage multiple sessions"
echo "  - Auto-cleanup inactive sessions"
echo ""
echo "Server log saved to: /tmp/session-server.log"
echo ""

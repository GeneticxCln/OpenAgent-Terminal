#!/bin/bash
# Run both backend and frontend together

set -e

echo "╔════════════════════════════════════════════════════════════╗"
echo "║  OpenAgent Terminal - Run Both Processes                  ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Cleanup function
cleanup() {
    echo ""
    echo "🛑 Shutting down..."
    if [ ! -z "$BACKEND_PID" ]; then
        kill $BACKEND_PID 2>/dev/null || true
        wait $BACKEND_PID 2>/dev/null || true
    fi
    exit 0
}

trap cleanup SIGINT SIGTERM

# Start backend in background
echo "📦 Starting Python backend..."
cd backend
python -m openagent_terminal.bridge --debug &
BACKEND_PID=$!
cd ..

# Wait for socket to be ready
echo "⏳ Waiting for backend socket..."
SOCKET_PATH="/run/user/1000/openagent-terminal-test.sock"
MAX_WAIT=10
COUNT=0

while [ ! -S "$SOCKET_PATH" ] && [ $COUNT -lt $MAX_WAIT ]; do
    sleep 0.5
    COUNT=$((COUNT + 1))
done

if [ ! -S "$SOCKET_PATH" ]; then
    echo "❌ Backend socket not ready after ${MAX_WAIT} seconds"
    kill $BACKEND_PID 2>/dev/null || true
    exit 1
fi

echo "✅ Backend ready!"
echo ""
echo "🚀 Starting Rust frontend..."
echo ""

# Run frontend
cargo run --release

# Cleanup
cleanup

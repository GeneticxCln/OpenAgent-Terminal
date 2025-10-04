#!/bin/bash
# Integration test script for OpenAgent-Terminal Phase 1 IPC

set -e

echo "🧪 OpenAgent-Terminal Phase 1 Integration Test"
echo "=============================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Determine runtime directory
if [ -n "$XDG_RUNTIME_DIR" ]; then
    RUNTIME_DIR="$XDG_RUNTIME_DIR"
else
    RUNTIME_DIR="/tmp"
fi

SOCKET_PATH="$RUNTIME_DIR/openagent-terminal-test.sock"

echo "📁 Runtime directory: $RUNTIME_DIR"
echo "🔌 Socket path: $SOCKET_PATH"
echo ""

# Clean up old socket if exists
if [ -e "$SOCKET_PATH" ]; then
    echo "🧹 Cleaning up old socket..."
    rm "$SOCKET_PATH"
fi

# Build the Rust frontend
echo -e "${BLUE}🔨 Building Rust frontend...${NC}"
cargo build --quiet
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Build successful${NC}"
else
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi
echo ""

# Start Python backend in background
echo -e "${BLUE}🐍 Starting Python backend...${NC}"
cd backend
python -m openagent_terminal.bridge &
BACKEND_PID=$!
cd ..

echo "Backend PID: $BACKEND_PID"
echo ""

# Give backend time to start
echo "⏳ Waiting for backend to start (3 seconds)..."
sleep 3

# Check if socket was created
if [ -e "$SOCKET_PATH" ]; then
    echo -e "${GREEN}✅ Socket created successfully${NC}"
else
    echo -e "${RED}❌ Socket not found - backend may have failed to start${NC}"
    kill $BACKEND_PID 2>/dev/null || true
    exit 1
fi
echo ""

# Run the Rust frontend
echo -e "${BLUE}🦀 Running Rust frontend...${NC}"
echo ""
cargo run --quiet

FRONTEND_EXIT=$?

# Cleanup
echo ""
echo "🧹 Cleaning up..."
kill $BACKEND_PID 2>/dev/null || true
wait $BACKEND_PID 2>/dev/null || true

if [ -e "$SOCKET_PATH" ]; then
    rm "$SOCKET_PATH"
fi

echo ""
if [ $FRONTEND_EXIT -eq 0 ]; then
    echo -e "${GREEN}✅ Phase 1 IPC Test PASSED!${NC}"
    echo ""
    echo "Next steps:"
    echo "  ✅ Unix socket connection working"
    echo "  ✅ Initialize handshake working"
    echo "  🔴 Ready for Phase 2: Agent Integration"
    exit 0
else
    echo -e "${RED}❌ Phase 1 IPC Test FAILED${NC}"
    exit 1
fi

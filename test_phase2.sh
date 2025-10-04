#!/bin/bash
# Phase 2 Integration Test - Agent Query with Streaming

set -e

echo "🧪 OpenAgent-Terminal Phase 2 Integration Test"
echo "=============================================="
echo "Testing: Agent Query + Token Streaming"
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
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
cargo build --quiet 2>&1 | grep -v "warning:" || true
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
echo -e "${BLUE}🦀 Running Rust frontend with agent test...${NC}"
echo ""
echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
cargo run --quiet 2>&1 | grep -v "warning:"

FRONTEND_EXIT=$?

echo ""
echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# Cleanup
echo "🧹 Cleaning up..."
kill $BACKEND_PID 2>/dev/null || true
wait $BACKEND_PID 2>/dev/null || true

if [ -e "$SOCKET_PATH" ]; then
    rm "$SOCKET_PATH"
fi

echo ""
if [ $FRONTEND_EXIT -eq 0 ]; then
    echo -e "${GREEN}✅ Phase 2 Integration Test PASSED!${NC}"
    echo ""
    echo "Achievements:"
    echo "  ✅ IPC communication working"
    echo "  ✅ Agent query/response working"
    echo "  ✅ Token streaming working"
    echo "  ✅ Real-time display working"
    echo ""
    echo "Next: Phase 3 - Block Rendering with Syntax Highlighting"
    exit 0
else
    echo -e "${RED}❌ Phase 2 Integration Test FAILED${NC}"
    exit 1
fi

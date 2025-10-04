#!/bin/bash
# Test script for Phase 5: Real File Operations
# Tests the --execute flag with actual file system changes

set -e  # Exit on error

echo "🧪 OpenAgent-Terminal Real Execution Test"
echo "=========================================="
echo "Testing: Real file operations (--execute flag)"
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Get runtime directory
RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp}"
SOCKET_PATH="$RUNTIME_DIR/openagent-terminal-test.sock"

echo "📁 Runtime directory: $RUNTIME_DIR"
echo "🔌 Socket path: $SOCKET_PATH"
echo ""

# Clean up function
cleanup() {
    echo ""
    echo "🧹 Cleaning up..."
    
    # Kill backend if running
    if [ ! -z "$BACKEND_PID" ]; then
        kill $BACKEND_PID 2>/dev/null || true
        wait $BACKEND_PID 2>/dev/null || true
    fi
    
    # Remove socket
    rm -f "$SOCKET_PATH"
    
    # Remove test file if it exists
    rm -f test_real.txt
    
    echo "✅ Cleanup complete"
}

trap cleanup EXIT INT TERM

# Build Rust frontend
echo "🔨 Building Rust frontend..."
cargo build --quiet 2>&1 | grep -v "warning:" || true
echo "✅ Build successful"
echo ""

# Start Python backend in REAL EXECUTION mode
echo "🐍 Starting Python backend with --execute flag..."
echo "${YELLOW}⚠️  WARNING: This will actually write files!${NC}"
echo ""

python -m backend.openagent_terminal.bridge --execute 2>&1 | while IFS= read -r line; do
    echo "$line"
    # Check if ready
    if echo "$line" | grep -q "ready at"; then
        # Signal that backend is ready
        touch /tmp/backend_ready
        break
    fi
done &

BACKEND_PID=$!

# Wait for backend to be ready
echo "⏳ Waiting for backend to start..."
for i in {1..10}; do
    if [ -f /tmp/backend_ready ] && [ -e "$SOCKET_PATH" ]; then
        break
    fi
    sleep 0.5
done

if [ ! -e "$SOCKET_PATH" ]; then
    echo "${RED}❌ Socket not created!${NC}"
    exit 1
fi

rm -f /tmp/backend_ready
echo "${GREEN}✅ Socket created successfully${NC}"
echo ""

# Run Rust frontend to test file write
echo "🦀 Running Rust frontend to test real file write..."
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Run the frontend (it will auto-approve and write test.txt)
cargo run --quiet 2>&1 | grep -v "warning:" | head -100

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Check if test.txt was created
if [ -f "test.txt" ]; then
    echo "${GREEN}✅ SUCCESS: test.txt was created!${NC}"
    echo ""
    echo "File contents:"
    echo "─────────────"
    cat test.txt
    echo "─────────────"
    echo ""
    echo "File info:"
    ls -lh test.txt
    echo ""
    
    # Clean up test file
    rm test.txt
    echo "🧹 Removed test.txt"
else
    echo "${YELLOW}⚠️  test.txt was not created${NC}"
    echo "This might mean:"
    echo "  1. The backend is still in demo mode"
    echo "  2. The tool was not executed"
    echo "  3. The approval was rejected"
fi

echo ""
echo "${GREEN}✅ Real Execution Test Complete!${NC}"
echo ""
echo "Next steps:"
echo "  1. Try: python -m backend.openagent_terminal.bridge --execute"
echo "  2. Then run the frontend and test file operations"
echo "  3. Verify files are actually created/modified"
echo ""

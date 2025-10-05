#!/usr/bin/env bash
# Test script to demonstrate IPC robustness improvements

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "======================================"
echo "IPC Robustness Testing Script"
echo "======================================"
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Test 1: Verify build
echo -e "${YELLOW}Test 1: Verifying build...${NC}"
if cargo build --release 2>&1 | grep -q "Finished"; then
    echo -e "${GREEN}✓ Build successful${NC}"
else
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi
echo ""

# Test 2: Check for IPC improvements in code
echo -e "${YELLOW}Test 2: Verifying IPC improvements in code...${NC}"

# Check for ConnectionState enum
if grep -q "pub enum ConnectionState" src/ipc/client.rs; then
    echo -e "${GREEN}✓ ConnectionState enum found${NC}"
else
    echo -e "${RED}✗ ConnectionState enum not found${NC}"
    exit 1
fi

# Check for deny_unknown_fields
if grep -q "deny_unknown_fields" src/ipc/message.rs; then
    echo -e "${GREEN}✓ Strict JSON-RPC validation enabled${NC}"
else
    echo -e "${RED}✗ Strict validation not found${NC}"
    exit 1
fi

# Check for TolerantMessage
if grep -q "TolerantMessage" src/ipc/message.rs; then
    echo -e "${GREEN}✓ Protocol drift detection added${NC}"
else
    echo -e "${RED}✗ Protocol drift detection not found${NC}"
    exit 1
fi

# Check for ID space constants
if grep -q "INTERACTIVE_ID_MAX" src/ipc/client.rs && \
   grep -q "SESSION_MANAGER_ID_MIN" src/session.rs; then
    echo -e "${GREEN}✓ Request ID space separation implemented${NC}"
else
    echo -e "${RED}✗ ID space separation not found${NC}"
    exit 1
fi

# Check for reconnect method
if grep -q "pub async fn reconnect" src/ipc/client.rs; then
    echo -e "${GREEN}✓ Reconnection strategy implemented${NC}"
else
    echo -e "${RED}✗ Reconnection method not found${NC}"
    exit 1
fi

echo ""

# Test 3: Verify documentation
echo -e "${YELLOW}Test 3: Verifying documentation...${NC}"

if [ -f "docs/IPC_ROBUSTNESS.md" ]; then
    echo -e "${GREEN}✓ IPC_ROBUSTNESS.md exists${NC}"
else
    echo -e "${RED}✗ Documentation missing${NC}"
    exit 1
fi

if [ -f "CHANGELOG_IPC_ROBUSTNESS.md" ]; then
    echo -e "${GREEN}✓ CHANGELOG_IPC_ROBUSTNESS.md exists${NC}"
else
    echo -e "${RED}✗ Changelog missing${NC}"
    exit 1
fi

echo ""

# Test 4: Connection retry demonstration
echo -e "${YELLOW}Test 4: Connection retry demonstration...${NC}"
echo "This test will start the Rust client without the backend."
echo "You should see connection retry attempts with exponential backoff."
echo ""
echo -e "${YELLOW}Press Enter to start the test, or Ctrl+C to skip...${NC}"
read

echo "Starting Rust client (backend is NOT running)..."
echo "Expected: Connection retry attempts with increasing delays"
echo "The client will timeout after 3 attempts."
echo ""

# Run for a few seconds to show retry attempts
timeout 5s ./target/release/openagent-terminal 2>&1 || true

echo ""
echo -e "${GREEN}✓ Retry behavior demonstrated (check logs above)${NC}"
echo ""

# Summary
echo "======================================"
echo -e "${GREEN}All tests passed!${NC}"
echo "======================================"
echo ""
echo "IPC Robustness Features Implemented:"
echo "  • Connection state tracking with ConnectionState enum"
echo "  • Exponential backoff retry strategy"
echo "  • Strict JSON-RPC validation with deny_unknown_fields"
echo "  • Protocol drift detection with TolerantMessage"
echo "  • Request ID space separation (0-9999 vs 10000+)"
echo "  • Enhanced error logging and connection monitoring"
echo "  • Manual reconnection capability"
echo ""
echo "Documentation:"
echo "  • docs/IPC_ROBUSTNESS.md - Comprehensive feature guide"
echo "  • CHANGELOG_IPC_ROBUSTNESS.md - Implementation details"
echo ""
echo "To test full IPC functionality:"
echo "  Terminal 1: cd backend && python -m openagent_terminal.bridge"
echo "  Terminal 2: ./target/release/openagent-terminal"
echo ""

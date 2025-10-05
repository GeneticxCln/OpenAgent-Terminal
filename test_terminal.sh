#!/bin/bash
# Quick test script for the improved terminal

set -e

echo "╔════════════════════════════════════════════════════════════╗"
echo "║  OpenAgent Terminal - Quick Verification Test             ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Check if backend is running
if pgrep -f "openagent_terminal.bridge" > /dev/null; then
    echo "✅ Backend is already running"
else
    echo "⚠️  Backend is not running. Starting it now..."
    echo ""
    echo "Run in another terminal:"
    echo "  cd backend && python -m openagent_terminal.bridge --debug"
    echo ""
    read -p "Press Enter once the backend is running..."
fi

echo ""
echo "Building terminal (release mode)..."
cargo build --release --quiet

echo ""
echo "✅ Build successful!"
echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║  Testing Instructions                                      ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""
echo "The terminal will now start. Test these features:"
echo ""
echo "1. Type some text and use LEFT/RIGHT arrows to move cursor"
echo "2. Type multiple commands and use UP/DOWN to navigate history"
echo "3. Press Ctrl+K to clear screen"
echo "4. Press Ctrl+L to show recent commands"
echo "5. Send a query (e.g., 'hello') and try typing while it streams"
echo "6. Press Ctrl+C to cancel input or streaming"
echo "7. Press Ctrl+D (on empty line) to exit"
echo ""
read -p "Press Enter to start the terminal..."

echo ""
echo "Starting OpenAgent Terminal..."
echo ""

# Run the terminal
./target/release/openagent-terminal

echo ""
echo "✅ Terminal exited successfully"

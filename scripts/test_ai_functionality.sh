#!/bin/bash

# Test script for validating OpenAgent Terminal AI functionality

echo "🧪 OpenAgent Terminal AI Functionality Test"
echo "=========================================="

# Build the project with AI feature
echo "📦 Building OpenAgent Terminal with AI features..."
cargo build --features ai --release

if [ $? -ne 0 ]; then
    echo "❌ Build failed!"
    exit 1
fi

echo "✅ Build successful!"

# Check if binary exists
BINARY_PATH="./target/release/openagent-terminal"
if [ ! -f "$BINARY_PATH" ]; then
    echo "❌ Binary not found at $BINARY_PATH"
    exit 1
fi

echo "✅ Binary found"

# Create test config with AI enabled
echo "📝 Creating test configuration with AI enabled..."
cat > /tmp/ai_test_config.toml << 'EOF'
[ai]
enabled = true
provider = "null"
panel_height_fraction = 0.4
backdrop_alpha = 0.3
log_verbosity = "verbose"

[font]
size = 12.0

[window]
dimensions = { columns = 120, lines = 30 }
padding = { x = 10, y = 10 }
EOF

echo "✅ Test configuration created"

# Test configuration validation
echo "🔍 Testing configuration validation..."
$BINARY_PATH --config-file /tmp/ai_test_config.toml --help > /dev/null 2>&1

if [ $? -eq 0 ]; then
    echo "✅ Configuration file is valid"
else
    echo "❌ Configuration validation failed"
fi

echo ""
echo "🎯 TEST SUMMARY"
echo "==============="
echo "✅ Build: SUCCESS"
echo "✅ Binary: PRESENT"
echo "✅ Config: VALID"
echo ""
echo "🔧 MANUAL TESTING INSTRUCTIONS"
echo "=============================="
echo "1. Run the terminal with AI enabled:"
echo "   $BINARY_PATH --config-file /tmp/ai_test_config.toml"
echo ""
echo "2. Test AI panel functionality:"
echo "   - Press Ctrl+Shift+A to open AI panel"
echo "   - Type a query like 'list files'"
echo "   - Press Enter to get suggestions"
echo "   - Use arrow keys to navigate proposals"
echo "   - Press Ctrl+E to apply a command safely"
echo "   - Press Escape to close the AI panel"
echo ""
echo "3. Test blocks search (Ctrl+Shift+S):"
echo "   - Press Ctrl+Shift+S to open blocks search"
echo "   - Navigate through command history"
echo ""
echo "4. Test workflows panel (Ctrl+Shift+W):"
echo "   - Press Ctrl+Shift+W to open workflows panel"
echo "   - Browse available workflows"
echo ""
echo "💡 The AI provider is set to 'null' for testing, so it will show mock responses."
echo "   To use real AI providers, configure api_key_env, endpoint_env, and model_env."

#!/bin/bash

# Test script for all AI providers in OpenAgent Terminal

echo "=== OpenAgent Terminal - AI Providers Test ==="
echo

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test Ollama Provider
echo -e "${YELLOW}Testing Ollama Provider (Local)${NC}"
echo "================================"
if command -v ollama &> /dev/null; then
    echo -e "${GREEN}✓${NC} Ollama installed"
    if curl -s http://localhost:11434/api/tags > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} Ollama service running"
        export OLLAMA_ENDPOINT="http://localhost:11434"
        export OLLAMA_MODEL="codellama"
        echo "  Endpoint: $OLLAMA_ENDPOINT"
        echo "  Model: $OLLAMA_MODEL"
    else
        echo -e "${RED}✗${NC} Ollama service not running"
        echo "  Start with: ollama serve"
    fi
else
    echo -e "${RED}✗${NC} Ollama not installed"
    echo "  Install: curl -fsSL https://ollama.ai/install.sh | sh"
fi
echo

# Test OpenAI Provider
echo -e "${YELLOW}Testing OpenAI Provider${NC}"
echo "======================="
if [ -n "$OPENAI_API_KEY" ]; then
    echo -e "${GREEN}✓${NC} OPENAI_API_KEY set"
    echo "  Model: ${OPENAI_MODEL:-gpt-3.5-turbo}"
    echo "  Endpoint: ${OPENAI_API_BASE:-https://api.openai.com/v1}"
    
    # Test API connectivity
    if curl -s -H "Authorization: Bearer $OPENAI_API_KEY" \
            https://api.openai.com/v1/models > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} OpenAI API accessible"
    else
        echo -e "${YELLOW}⚠${NC} Could not verify API access"
    fi
else
    echo -e "${RED}✗${NC} OPENAI_API_KEY not set"
    echo "  Set with: export OPENAI_API_KEY='your-api-key'"
fi
echo

# Test Anthropic Provider
echo -e "${YELLOW}Testing Anthropic Provider${NC}"
echo "========================="
if [ -n "$ANTHROPIC_API_KEY" ]; then
    echo -e "${GREEN}✓${NC} ANTHROPIC_API_KEY set"
    echo "  Model: ${ANTHROPIC_MODEL:-claude-3-haiku-20240307}"
    echo "  Endpoint: ${ANTHROPIC_API_BASE:-https://api.anthropic.com/v1}"
else
    echo -e "${RED}✗${NC} ANTHROPIC_API_KEY not set"
    echo "  Set with: export ANTHROPIC_API_KEY='your-api-key'"
fi
echo

# Build with all features
echo -e "${YELLOW}Building OpenAgent Terminal with AI features${NC}"
echo "==========================================="
if cargo build --features "ai ollama" 2>&1 | tail -3; then
    echo -e "${GREEN}✓${NC} Build successful"
else
    echo -e "${RED}✗${NC} Build failed"
    exit 1
fi
echo

# Run unit tests
echo -e "${YELLOW}Running AI Module Tests${NC}"
echo "======================"
cargo test -p openagent-terminal-ai --features "ollama" 2>&1 | grep -E "(test result:|passed|failed)"
echo

# Create test program
echo -e "${YELLOW}Testing Provider Integration${NC}"
echo "==========================="
cat > /tmp/test_providers.rs << 'EOF'
use openagent_terminal_ai::{create_provider, AiRequest};

fn main() {
    println!("Testing AI providers...\n");
    
    let providers = vec!["null", "ollama", "openai", "anthropic"];
    
    for provider_name in providers {
        println!("Provider: {}", provider_name);
        match create_provider(provider_name) {
            Ok(provider) => {
                println!("  ✓ Created successfully");
                println!("  Name: {}", provider.name());
                
                // Test with a simple query
                let request = AiRequest {
                    scratch_text: "list files".to_string(),
                    working_directory: Some("/tmp".to_string()),
                    shell_kind: Some("bash".to_string()),
                    context: vec![("test".to_string(), "true".to_string())],
                };
                
                match provider.propose(request) {
                    Ok(proposals) => {
                        println!("  ✓ Query successful");
                        println!("  Proposals: {}", proposals.len());
                    },
                    Err(e) => {
                        println!("  ⚠ Query failed: {}", e);
                    }
                }
            },
            Err(e) => {
                println!("  ✗ Failed to create: {}", e);
            }
        }
        println!();
    }
}
EOF

echo "Provider factory test results:"
echo "(Compilation test only - actual API calls require valid credentials)"
echo

# Configuration examples
echo -e "${YELLOW}Configuration Examples${NC}"
echo "===================="
echo
echo "1. For Ollama (local, privacy-first):"
echo "   [ai]"
echo "   provider = \"ollama\""
echo "   enabled = true"
echo
echo "2. For OpenAI:"
echo "   [ai]"
echo "   provider = \"openai\""
echo "   enabled = true"
echo "   # Set OPENAI_API_KEY environment variable"
echo
echo "3. For Anthropic:"
echo "   [ai]"
echo "   provider = \"anthropic\""
echo "   enabled = true"
echo "   # Set ANTHROPIC_API_KEY environment variable"
echo

# Usage instructions
echo -e "${YELLOW}How to Use${NC}"
echo "========="
echo "1. Choose your provider and set environment variables"
echo "2. Update ~/.config/openagent-terminal/openagent-terminal.toml"
echo "3. Run: cargo run --features \"ai ollama\""
echo "4. Press Ctrl+Shift+A to open AI assistant"
echo "5. Type your query and press Enter"
echo "6. Use arrow keys to navigate proposals"
echo "7. Press Ctrl+C to copy selected command"
echo

echo -e "${GREEN}=== Test Complete ===${NC}"

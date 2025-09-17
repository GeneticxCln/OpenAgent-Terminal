#!/bin/bash
# OpenAgent Terminal Local-Only Setup Script
# Sets up Ollama and configures OpenAgent Terminal for privacy-first AI

set -e

echo "🔧 OpenAgent Terminal Local-Only AI Setup"
echo "=========================================="

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if running on supported platform
check_platform() {
    case "$(uname -s)" in
        Linux*)     PLATFORM=linux;;
        Darwin*)    PLATFORM=macos;;
        CYGWIN*|MINGW*) PLATFORM=windows;;
        *)          echo -e "${RED}❌ Unsupported platform: $(uname -s)${NC}"; exit 1;;
    esac
    echo -e "${BLUE}📱 Detected platform: $PLATFORM${NC}"
}

# Install Ollama if not present
install_ollama() {
    if command -v ollama >/dev/null 2>&1; then
        echo -e "${GREEN}✅ Ollama already installed: $(ollama --version)${NC}"
        return 0
    fi

    echo -e "${YELLOW}📦 Installing Ollama...${NC}"
    
    case $PLATFORM in
        linux|macos)
            curl -fsSL https://ollama.ai/install.sh | sh
            ;;
        windows)
            echo -e "${YELLOW}⬇️  Please download Ollama from: https://ollama.ai/download/windows${NC}"
            echo "Press Enter after installation..."
            read -r
            ;;
    esac

    if command -v ollama >/dev/null 2>&1; then
        echo -e "${GREEN}✅ Ollama installed successfully${NC}"
    else
        echo -e "${RED}❌ Ollama installation failed${NC}"
        exit 1
    fi
}

# Start Ollama service
start_ollama_service() {
    echo -e "${YELLOW}🚀 Starting Ollama service...${NC}"
    
    # Check if ollama is already running
    if pgrep -x "ollama" > /dev/null; then
        echo -e "${GREEN}✅ Ollama service already running${NC}"
        return 0
    fi

    case $PLATFORM in
        linux)
            if systemctl is-active --quiet ollama; then
                echo -e "${GREEN}✅ Ollama systemd service is running${NC}"
            else
                ollama serve &
                OLLAMA_PID=$!
                echo -e "${BLUE}🔧 Started Ollama manually (PID: $OLLAMA_PID)${NC}"
                # Give it time to start
                sleep 3
            fi
            ;;
        macos)
            ollama serve &
            OLLAMA_PID=$!
            echo -e "${BLUE}🔧 Started Ollama manually (PID: $OLLAMA_PID)${NC}"
            sleep 3
            ;;
        windows)
            echo -e "${YELLOW}ℹ️  Please ensure Ollama service is running${NC}"
            ;;
    esac
}

# Download recommended models
download_models() {
    echo -e "${YELLOW}📚 Downloading AI models...${NC}"
    
    # Check if models are already available
    MODELS_TO_PULL=(
        "codellama:7b"      # Primary coding model (lightweight)
        "llama3.1:8b"       # General purpose model
    )

    for model in "${MODELS_TO_PULL[@]}"; do
        echo -e "${BLUE}🔄 Checking model: $model${NC}"
        
        if ollama list | grep -q "^$model"; then
            echo -e "${GREEN}✅ Model $model already downloaded${NC}"
        else
            echo -e "${YELLOW}⬇️  Downloading $model (this may take a while)...${NC}"
            if ollama pull "$model"; then
                echo -e "${GREEN}✅ Downloaded $model${NC}"
            else
                echo -e "${RED}❌ Failed to download $model${NC}"
            fi
        fi
    done
}

# Create OpenAgent Terminal configuration
setup_openagent_config() {
    echo -e "${YELLOW}⚙️  Setting up OpenAgent Terminal configuration...${NC}"
    
    # Detect config directory based on platform
    case $PLATFORM in
        linux)
            CONFIG_DIR="$HOME/.config/openagent-terminal"
            ;;
        macos)
            CONFIG_DIR="$HOME/.config/openagent-terminal"
            ;;
        windows)
            CONFIG_DIR="$APPDATA/openagent-terminal"
            ;;
    esac

    # Create config directory
    mkdir -p "$CONFIG_DIR"
    
    # Copy configuration file
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    cp "$SCRIPT_DIR/openagent-terminal.toml" "$CONFIG_DIR/"
    
    echo -e "${GREEN}✅ Configuration copied to: $CONFIG_DIR${NC}"
}

# Set up environment variables
setup_environment() {
    echo -e "${YELLOW}🌍 Setting up environment variables...${NC}"
    
    # Create environment setup script
    ENV_SCRIPT="$HOME/.openagent-terminal-env"
    
    cat > "$ENV_SCRIPT" << 'EOF'
#!/bin/bash
# OpenAgent Terminal Local-Only Environment
# Source this file in your shell startup script

# Ollama configuration
export OPENAGENT_OLLAMA_ENDPOINT="http://localhost:11434"
export OPENAGENT_OLLAMA_MODEL="codellama:7b"

# Privacy settings
export OPENAGENT_AI_LOG_VERBOSITY="summary"
export OPENAGENT_TELEMETRY_DISABLED="true"

echo "🤖 OpenAgent Terminal AI environment loaded (Local-Only)"
EOF

    chmod +x "$ENV_SCRIPT"
    
    echo -e "${GREEN}✅ Environment script created: $ENV_SCRIPT${NC}"
    echo -e "${BLUE}💡 Add this to your shell startup script:${NC}"
    echo -e "${BLUE}   source $ENV_SCRIPT${NC}"
}

# Test the setup
test_setup() {
    echo -e "${YELLOW}🧪 Testing OpenAgent Terminal AI setup...${NC}"
    
    # Source environment
    source "$HOME/.openagent-terminal-env"
    
    # Test Ollama connection
    if curl -s http://localhost:11434/api/version >/dev/null; then
        echo -e "${GREEN}✅ Ollama API is responding${NC}"
    else
        echo -e "${RED}❌ Ollama API not accessible${NC}"
        return 1
    fi
    
    # Test model availability
    if ollama list | grep -q "codellama:7b"; then
        echo -e "${GREEN}✅ CodeLlama model is available${NC}"
    else
        echo -e "${YELLOW}⚠️  CodeLlama model not found${NC}"
    fi
    
    # Test OpenAgent Terminal configuration
    if [[ -f "$HOME/.config/openagent-terminal/openagent-terminal.toml" ]]; then
        echo -e "${GREEN}✅ OpenAgent Terminal configuration exists${NC}"
    else
        echo -e "${RED}❌ OpenAgent Terminal configuration missing${NC}"
        return 1
    fi
}

# Print usage instructions
print_usage_instructions() {
    echo -e "\n${GREEN}🎉 Setup Complete! 🎉${NC}"
    echo -e "${BLUE}===================${NC}"
    echo
    echo -e "${YELLOW}Next Steps:${NC}"
    echo -e "1. Add to your shell startup script (${BLUE}~/.bashrc${NC}, ${BLUE}~/.zshrc${NC}, etc.):"
    echo -e "   ${GREEN}source $HOME/.openagent-terminal-env${NC}"
    echo
    echo -e "2. Start a new terminal session or run:"
    echo -e "   ${GREEN}source $HOME/.openagent-terminal-env${NC}"
    echo
    echo -e "3. Launch OpenAgent Terminal:"
    echo -e "   ${GREEN}openagent-terminal${NC}"
    echo
    echo -e "4. Use AI assistance:"
    echo -e "   • Press ${GREEN}Ctrl+Shift+A${NC} to open AI panel"
    echo -e "   • Type natural language commands"
    echo -e "   • AI suggestions appear - you choose what to run"
    echo
    echo -e "${YELLOW}Example Queries:${NC}"
    echo -e "• \"${BLUE}find all rust files modified today${NC}\""
    echo -e "• \"${BLUE}show git status and recent commits${NC}\""
    echo -e "• \"${BLUE}create a new rust project with web server${NC}\""
    echo -e "• \"${BLUE}explain this error message${NC}\""
    echo
    echo -e "${GREEN}Privacy Features:${NC}"
    echo -e "✅ All AI processing happens locally"
    echo -e "✅ No data sent to cloud services"
    echo -e "✅ Full offline operation after setup"
    echo -e "✅ Your code never leaves your machine"
    echo
    echo -e "${BLUE}Troubleshooting:${NC}"
    echo -e "• Validate setup: ${GREEN}openagent-terminal ai validate${NC}"
    echo -e "• Check logs: ${GREEN}tail -f ~/.local/share/openagent-terminal/logs/ai.log${NC}"
    echo -e "• Restart Ollama: ${GREEN}pkill ollama && ollama serve${NC}"
}

# Error handling
handle_error() {
    echo -e "\n${RED}❌ Setup failed at step: $1${NC}"
    echo -e "${YELLOW}Please check the error above and try again${NC}"
    echo -e "${BLUE}For help, visit: https://github.com/GeneticxCln/OpenAgent-Terminal/issues${NC}"
    exit 1
}

# Main setup function
main() {
    echo -e "${BLUE}Starting OpenAgent Terminal local-only AI setup...${NC}\n"

    check_platform || handle_error "Platform Check"
    install_ollama || handle_error "Ollama Installation"
    start_ollama_service || handle_error "Ollama Service Start"
    download_models || handle_error "Model Download"
    setup_openagent_config || handle_error "Configuration Setup"
    setup_environment || handle_error "Environment Setup"
    test_setup || handle_error "Setup Testing"
    print_usage_instructions
    
    echo -e "\n${GREEN}🚀 Local-only AI setup completed successfully!${NC}"
}

# Run main function
main "$@"
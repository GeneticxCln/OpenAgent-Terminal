#!/usr/bin/env sh

# OpenAgent Terminal Shell Detection and OSC 133 Verification Utility
# This script detects the current shell and provides instructions for OSC 133 setup

# ANSI color codes for output formatting
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Detect current shell
detect_shell() {
    if [ -n "$ZSH_VERSION" ]; then
        echo "zsh"
    elif [ -n "$BASH_VERSION" ]; then
        echo "bash"
    elif [ -n "$FISH_VERSION" ]; then
        echo "fish"
    else
        # Fallback to process name
        basename "$0" 2>/dev/null || echo "unknown"
    fi
}

# Check if terminal supports OSC 133
check_terminal_support() {
    # Check for OpenAgent Terminal
    if [ "$TERM_PROGRAM" = "openagent-terminal" ]; then
        return 0
    fi
    
    # Check for other known compatible terminals
    case "$TERM_PROGRAM" in
        "vscode"|"iTerm.app"|"WezTerm")
            return 0
            ;;
    esac
    
    # Check TERM variable
    case "$TERM" in
        *-256color|xterm-kitty|alacritty|wezterm)
            return 0
            ;;
    esac
    
    # If force flag is set
    if [ -n "$OPENAGENT_FORCE_OSC133" ]; then
        return 0
    fi
    
    return 1
}

# Test OSC 133 functionality
test_osc133() {
    echo "Testing OSC 133 sequences..."
    echo "The following should create visible command blocks in supported terminals:"
    echo
    
    # Emit a test sequence
    printf '\e]133;A\a'
    printf 'test_prompt> '
    printf '\e]133;B\a'
    echo "echo 'Test command output'"
    printf '\e]133;C\a'
    echo "Test command output"
    printf '\e]133;D;0\a'
    echo
    
    echo "If you saw distinct command blocks above, OSC 133 is working!"
    echo "If not, your terminal may not support OSC 133 sequences."
}

# Check if integration is already loaded
check_integration_status() {
    if [ -n "$OPENAGENT_INTEGRATION_LOADED" ]; then
        echo "${GREEN}✓${NC} OpenAgent Terminal integration is loaded"
        return 0
    else
        echo "${RED}✗${NC} OpenAgent Terminal integration is not loaded"
        return 1
    fi
}

# Provide shell-specific setup instructions
show_setup_instructions() {
    local shell_type="$1"
    local script_dir
    script_dir="$(cd "$(dirname "$0")" && pwd)"
    
    echo "${BLUE}=== Setup Instructions for $shell_type ===${NC}"
    echo
    
    case "$shell_type" in
        "bash")
            echo "Add this line to your ~/.bashrc:"
            echo "${YELLOW}source '$script_dir/../bash/openagent_integration.bash'${NC}"
            echo
            echo "Then restart your shell or run:"
            echo "${YELLOW}source ~/.bashrc${NC}"
            ;;
        "zsh")
            echo "Option 1 - Manual setup:"
            echo "Add this line to your ~/.zshrc:"
            echo "${YELLOW}source '$script_dir/../zsh/openagent_integration.zsh'${NC}"
            echo
            echo "Option 2 - Oh-My-Zsh plugin:"
            echo "1. Copy the plugin to oh-my-zsh plugins directory:"
            echo "${YELLOW}cp -r '$script_dir/../zsh' \"\$ZSH/plugins/openagent\"${NC}"
            echo "2. Add 'openagent' to your plugins list in ~/.zshrc:"
            echo "${YELLOW}plugins=(... openagent)${NC}"
            echo
            echo "Then restart your shell or run:"
            echo "${YELLOW}source ~/.zshrc${NC}"
            ;;
        "fish")
            echo "Add this line to your ~/.config/fish/config.fish:"
            echo "${YELLOW}source '$script_dir/../fish/openagent_integration.fish'${NC}"
            echo
            echo "Then restart your shell or run:"
            echo "${YELLOW}source ~/.config/fish/config.fish${NC}"
            ;;
        *)
            echo "${RED}Unknown shell: $shell_type${NC}"
            echo "OSC 133 integration may not be available for this shell."
            echo "Supported shells: bash, zsh, fish"
            ;;
    esac
}

# Main execution
main() {
    echo "${BLUE}OpenAgent Terminal Shell Integration Detector${NC}"
    echo "=================================================="
    echo
    
    # Detect current shell
    current_shell=$(detect_shell)
    echo "${BLUE}Detected shell:${NC} $current_shell"
    
    # Check terminal support
    if check_terminal_support; then
        echo "${GREEN}✓${NC} Terminal supports OSC 133 sequences"
        echo "${BLUE}Terminal:${NC} ${TERM_PROGRAM:-$TERM}"
    else
        echo "${YELLOW}?${NC} Terminal support unknown or unsupported"
        echo "${BLUE}Terminal:${NC} ${TERM_PROGRAM:-$TERM}"
        echo
        echo "You can force enable OSC 133 by setting:"
        echo "${YELLOW}export OPENAGENT_FORCE_OSC133=1${NC}"
    fi
    
    echo
    
    # Check integration status
    check_integration_status
    echo
    
    # Show setup instructions
    show_setup_instructions "$current_shell"
    echo
    
    # Offer to test OSC 133
    echo "${BLUE}Commands available after setup:${NC}"
    case "$current_shell" in
        "bash"|"zsh")
            echo "  openagent_test_osc133     - Test if OSC 133 is working"
            echo "  openagent_disable_osc133  - Disable integration for this session"
            ;;
        "fish")
            echo "  openagent_test_osc133     - Test if OSC 133 is working"
            echo "  openagent_show_status     - Show integration status"
            echo "  openagent_disable_osc133  - Disable integration for this session"
            ;;
    esac
    echo
    
    # Test if we can run a basic test
    if [ "$1" = "--test" ]; then
        echo "${BLUE}Running OSC 133 test...${NC}"
        echo
        test_osc133
    fi
}

# Run main function with all arguments
main "$@"

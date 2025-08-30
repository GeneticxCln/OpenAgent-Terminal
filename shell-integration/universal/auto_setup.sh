#!/usr/bin/env sh

# OpenAgent Terminal Universal Auto-Setup Script
# This script automatically detects the environment and loads the appropriate OSC 133 integration

# Set script directory for relative path resolution
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Check if we're being sourced (not executed)
if [ "${BASH_SOURCE[0]}" = "${0}" ] && [ -n "$BASH_VERSION" ]; then
    # Being executed in bash, not sourced
    echo "This script should be sourced, not executed."
    echo "Usage: source $(basename "$0")"
    exit 1
fi

# Universal function to detect shell type
_openagent_detect_shell() {
    if [ -n "$ZSH_VERSION" ]; then
        echo "zsh"
    elif [ -n "$BASH_VERSION" ]; then
        echo "bash"
    elif [ -n "$FISH_VERSION" ]; then
        echo "fish"
    else
        # Try to detect from $0 or process name
        case "$(basename "$SHELL" 2>/dev/null)" in
            *zsh*) echo "zsh" ;;
            *bash*) echo "bash" ;;
            *fish*) echo "fish" ;;
            *) echo "unknown" ;;
        esac
    fi
}

# Check for framework compatibility
_openagent_check_frameworks() {
    # Oh-My-Zsh detection
    if [ -n "$ZSH" ] && [ -d "$ZSH" ]; then
        export OPENAGENT_OMZ_DETECTED=1
    fi
    
    # Starship detection
    if command -v starship >/dev/null 2>&1; then
        export OPENAGENT_STARSHIP_DETECTED=1
    fi
    
    # Powerlevel10k detection
    if [ -n "$POWERLEVEL9K_VERSION" ]; then
        export OPENAGENT_P10K_DETECTED=1
    fi
    
    # Prezto detection
    if [ -n "$PREZTO" ]; then
        export OPENAGENT_PREZTO_DETECTED=1
    fi
}

# Load appropriate integration based on shell and environment
_openagent_load_integration() {
    local shell_type="$1"
    
    case "$shell_type" in
        "bash")
            # Source bash integration
            if [ -f "$SCRIPT_DIR/../bash/openagent_integration.bash" ]; then
                . "$SCRIPT_DIR/../bash/openagent_integration.bash"
            else
                echo "Warning: Bash integration script not found"
                return 1
            fi
            ;;
        "zsh")
            # Check if Oh-My-Zsh is available and prefer plugin method
            if [ -n "$OPENAGENT_OMZ_DETECTED" ] && [ -f "$SCRIPT_DIR/../zsh/openagent.plugin.zsh" ]; then
                # Use oh-my-zsh plugin
                . "$SCRIPT_DIR/../zsh/openagent.plugin.zsh"
            elif [ -f "$SCRIPT_DIR/../zsh/openagent_integration.zsh" ]; then
                # Use standalone zsh integration
                . "$SCRIPT_DIR/../zsh/openagent_integration.zsh"
            else
                echo "Warning: Zsh integration script not found"
                return 1
            fi
            ;;
        "fish")
            if [ -f "$SCRIPT_DIR/../fish/openagent_integration.fish" ]; then
                # Fish uses different sourcing syntax
                source "$SCRIPT_DIR/../fish/openagent_integration.fish"
            else
                echo "Warning: Fish integration script not found"
                return 1
            fi
            ;;
        *)
            echo "Unsupported shell: $shell_type"
            echo "OpenAgent Terminal OSC 133 integration supports: bash, zsh, fish"
            return 1
            ;;
    esac
}

# Framework compatibility adjustments
_openagent_framework_adjustments() {
    # Starship compatibility
    if [ -n "$OPENAGENT_STARSHIP_DETECTED" ]; then
        # Starship handles prompts differently, minimal interference mode
        export OPENAGENT_MINIMAL_MODE=1
    fi
    
    # Powerlevel10k compatibility
    if [ -n "$OPENAGENT_P10K_DETECTED" ]; then
        # P10K has complex prompt handling, be extra careful
        export OPENAGENT_MINIMAL_MODE=1
    fi
}

# Main auto-setup execution
_openagent_auto_setup() {
    # Only run in interactive shells
    case "$-" in
        *i*) ;;
        *) return 0 ;;
    esac
    
    # Detect shell
    local shell_type
    shell_type="$(_openagent_detect_shell)"
    
    # Check for known frameworks
    _openagent_check_frameworks
    
    # Apply framework-specific adjustments
    _openagent_framework_adjustments
    
    # Load appropriate integration
    if _openagent_load_integration "$shell_type"; then
        # Set up universal aliases available across all shells
        case "$shell_type" in
            "bash"|"zsh")
                alias openagent-setup="$SCRIPT_DIR/detect_shell.sh"
                alias openagent-test="openagent_test_osc133"
                alias openagent-disable="openagent_disable_osc133"
                ;;
            "fish")
                alias openagent-setup="$SCRIPT_DIR/detect_shell.sh"
                alias openagent-test="openagent_test_osc133"
                alias openagent-disable="openagent_disable_osc133"
                ;;
        esac
        
        # Success feedback (only in debug mode)
        if [ -n "$OPENAGENT_DEBUG" ]; then
            echo "OpenAgent Terminal auto-setup completed for $shell_type"
            if [ -n "$OPENAGENT_OMZ_DETECTED" ]; then
                echo "  - Oh-My-Zsh detected and integrated"
            fi
            if [ -n "$OPENAGENT_STARSHIP_DETECTED" ]; then
                echo "  - Starship detected, using minimal mode"
            fi
            if [ -n "$OPENAGENT_P10K_DETECTED" ]; then
                echo "  - Powerlevel10k detected, using minimal mode"
            fi
        fi
    else
        echo "Failed to load OpenAgent Terminal integration for $shell_type"
        return 1
    fi
}

# Provide manual setup function for troubleshooting
openagent_manual_setup() {
    "$SCRIPT_DIR/detect_shell.sh"
}

# Run auto-setup
_openagent_auto_setup

#!/usr/bin/env zsh

# OpenAgent Terminal Oh-My-Zsh Plugin
# This plugin provides OSC 133 integration that works seamlessly with oh-my-zsh

# Plugin metadata
local plugin_name="openagent"
local plugin_version="1.0.0"

# Source the main integration script
source "${0:A:h}/openagent_integration.zsh" 2>/dev/null || {
    # Fallback if the integration script is not found
    echo "Warning: OpenAgent Terminal integration script not found"
    echo "Expected location: ${0:A:h}/openagent_integration.zsh"
    return 1
}

# Oh-My-Zsh specific enhancements
if [[ -n "$ZSH_VERSION" ]] && [[ -n "$ZSH" ]]; then
    # Add compatibility with popular oh-my-zsh themes
    _openagent_omz_theme_compatibility() {
        case "$ZSH_THEME" in
            "agnoster"|"powerlevel9k"|"powerlevel10k")
                # These themes use right prompts heavily, be extra careful
                if [[ -n "$RPS1" ]] || [[ -n "$RPROMPT" ]]; then
                    # Don't interfere with right prompts
                    return
                fi
                ;;
            "robbyrussell"|"simple")
                # These themes are simple and should work fine
                ;;
            *)
                # Unknown theme, proceed with caution
                ;;
        esac
    }
    
    # Override the theme compatibility check
    add-zsh-hook precmd _openagent_omz_theme_compatibility
    
    # Provide oh-my-zsh specific commands
    alias openagent-status="openagent_show_hooks"
    alias openagent-test="openagent_test_osc133"
    alias openagent-disable="openagent_disable_osc133"
    
    # Add to oh-my-zsh plugin list tracking (if available)
    if typeset -p plugins >/dev/null 2>&1; then
        # Mark this plugin as loaded
        [[ "${plugins[@]}" == *"openagent"* ]] || plugins+=(openagent)
    fi
fi

# Success message for oh-my-zsh users
if [[ -n "$OPENAGENT_DEBUG" ]] || [[ -n "$OMZ_DEBUG" ]]; then
    echo "OpenAgent Terminal plugin loaded for oh-my-zsh (theme: ${ZSH_THEME:-default})"
fi

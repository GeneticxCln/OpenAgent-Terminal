#!/usr/bin/env zsh

# OpenAgent Terminal OSC 133 Integration for Zsh
# This script enables command block tracking by emitting OSC 133 sequences

# Only proceed if we're in an interactive shell
[[ -o interactive ]] || return
[[ -n "$TERM" ]] || return

# Avoid double-loading
[[ -n "$OPENAGENT_INTEGRATION_LOADED" ]] && return
typeset -g OPENAGENT_INTEGRATION_LOADED=1

# Check if we're running in OpenAgent Terminal or a compatible terminal
_openagent_is_supported_terminal() {
    # Check for OpenAgent Terminal
    [[ "$TERM_PROGRAM" == "openagent-terminal" ]] && return 0
    
    # Check for other terminals that support OSC 133
    case "$TERM_PROGRAM" in
        "vscode"|"iTerm.app"|"WezTerm")
            return 0
            ;;
    esac
    
    # Check TERM variable for compatible terminals
    case "$TERM" in
        *-256color|xterm-kitty|alacritty|wezterm)
            return 0
            ;;
    esac
    
    # If OPENAGENT_FORCE_OSC133 is set, assume support
    [[ -n "$OPENAGENT_FORCE_OSC133" ]] && return 0
    
    return 1
}

# Only enable if terminal is supported
if ! _openagent_is_supported_terminal; then
    return 0
fi

# OSC 133 escape sequences
typeset -g _OPENAGENT_OSC133_A=$'\e]133;A\a'    # Prompt start
typeset -g _OPENAGENT_OSC133_B=$'\e]133;B\a'    # Prompt end / Command start  
typeset -g _OPENAGENT_OSC133_C=$'\e]133;C\a'    # Command end / Output start
typeset -g _OPENAGENT_OSC133_D=$'\e]133;D;%s\a' # Command end with exit code

# Current command being executed
typeset -g _openagent_current_command=""

# Function to emit OSC 133;A (prompt start)
_openagent_prompt_start() {
    print -n "$_OPENAGENT_OSC133_A"
}

# Function to emit OSC 133;B (prompt end, command start)
_openagent_prompt_end() {
    print -n "$_OPENAGENT_OSC133_B"
}

# Function to emit OSC 133;C (command end, output start) 
_openagent_command_end() {
    print -n "$_OPENAGENT_OSC133_C"
}

# Function to emit OSC 133;D with exit code
_openagent_command_complete() {
    local exit_code=$1
    printf "$_OPENAGENT_OSC133_D" "$exit_code"
}

# Zsh preexec hook - called before each command
_openagent_preexec() {
    _openagent_current_command="$1"
    # Don't emit sequences for certain commands that might interfere
    case "$1" in
        clear|reset|tput*)
            return
            ;;
    esac
    _openagent_command_end
}

# Zsh precmd hook - called before each prompt
_openagent_precmd() {
    local exit_code=$?
    # Only emit D sequence if we actually ran a command
    if [[ -n "$_openagent_current_command" ]]; then
        _openagent_command_complete "$exit_code"
        _openagent_current_command=""
    fi
    _openagent_prompt_start
}

# Add our hooks to the arrays (this is the zsh way)
autoload -Uz add-zsh-hook
add-zsh-hook preexec _openagent_preexec
add-zsh-hook precmd _openagent_precmd

# Add prompt end marker to PS1 if not already present
if [[ "$PS1" != *"$_OPENAGENT_OSC133_B"* ]]; then
    # Use zsh's prompt expansion to add the marker
    # We'll add it just before the final prompt character
    if [[ "$PS1" =~ '(.*)(\$|#|%|>)([[:space:]]*)$' ]]; then
        PS1="${match[1]}${_OPENAGENT_OSC133_B}${match[2]}${match[3]}"
    else
        # Fallback for complex prompts
        PS1="$PS1$_OPENAGENT_OSC133_B"
    fi
fi

# Special handling for oh-my-zsh themes
if [[ -n "$ZSH_THEME" ]] && [[ "$ZSH_THEME" != "random" ]]; then
    # For oh-my-zsh, we need to be more careful about prompt modification
    # Some themes might override PS1 after we set it
    _openagent_setup_omz_integration() {
        # Hook into oh-my-zsh's theme loading
        if typeset -f update_terminal_cwd >/dev/null; then
            # oh-my-zsh is loaded, integrate carefully
            _openagent_original_update_terminal_cwd="$(typeset -f update_terminal_cwd)"
            update_terminal_cwd() {
                # Call original function
                eval "$_openagent_original_update_terminal_cwd"
                
                # Ensure our prompt markers are still there
                if [[ "$PS1" != *"$_OPENAGENT_OSC133_B"* ]]; then
                    PS1="$PS1$_OPENAGENT_OSC133_B"
                fi
            }
        fi
    }
    
    # Defer setup until after oh-my-zsh is fully loaded
    if [[ -n "$ZSH" ]]; then
        _openagent_setup_omz_integration
    else
        # Schedule for later if oh-my-zsh hasn't loaded yet
        autoload -Uz add-zsh-hook
        add-zsh-hook precmd _openagent_setup_omz_integration
    fi
fi

# Utility function to test if OSC 133 is working
openagent_test_osc133() {
    echo "Testing OSC 133 integration..."
    echo "You should see command blocks in OpenAgent Terminal for the following:"
    echo
    print -n "$_OPENAGENT_OSC133_A"
    print -n "test_prompt> "
    print -n "$_OPENAGENT_OSC133_B"
    echo "echo 'This should be a separate command block'"
    print -n "$_OPENAGENT_OSC133_C"
    echo "This should be a separate command block"
    printf "$_OPENAGENT_OSC133_D" "0"
    echo
    echo "If you see distinct command blocks above, OSC 133 is working!"
}

# Function to disable OSC 133 integration
openagent_disable_osc133() {
    # Remove our hooks
    autoload -Uz add-zsh-hook
    add-zsh-hook -d preexec _openagent_preexec
    add-zsh-hook -d precmd _openagent_precmd
    
    # Clean PS1
    PS1="${PS1//$_OPENAGENT_OSC133_A/}"
    PS1="${PS1//$_OPENAGENT_OSC133_B/}"
    
    echo "OpenAgent OSC 133 integration disabled for this session."
    echo "To permanently disable, remove or comment out the source line in your .zshrc"
}

# Function to show current hook status
openagent_show_hooks() {
    echo "=== OpenAgent Terminal Integration Status ==="
    echo "Preexec hooks:"
    print -l "${preexec_functions[@]}"
    echo
    echo "Precmd hooks:"  
    print -l "${precmd_functions[@]}"
    echo
    echo "PS1 contains OSC 133;B: $([[ "$PS1" == *"$_OPENAGENT_OSC133_B"* ]] && echo "Yes" || echo "No")"
}

# Provide feedback that integration is loaded
if [[ -n "$OPENAGENT_DEBUG" ]]; then
    echo "OpenAgent Terminal OSC 133 integration loaded (zsh)"
fi

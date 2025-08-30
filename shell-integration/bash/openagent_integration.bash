#!/usr/bin/env bash

# OpenAgent Terminal OSC 133 Integration for Bash
# This script enables command block tracking by emitting OSC 133 sequences

# Only proceed if we're in an interactive shell and terminal supports it
[[ $- != *i* ]] && return
[[ -z "$TERM" ]] && return

# Avoid double-loading
[[ -n "$OPENAGENT_INTEGRATION_LOADED" ]] && return
OPENAGENT_INTEGRATION_LOADED=1

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
_OPENAGENT_OSC133_A=$'\e]133;A\a'    # Prompt start
_OPENAGENT_OSC133_B=$'\e]133;B\a'    # Prompt end / Command start  
_OPENAGENT_OSC133_C=$'\e]133;C\a'    # Command end / Output start
_OPENAGENT_OSC133_D=$'\e]133;D;%s\a' # Command end with exit code

# Current command being executed
_openagent_current_command=""

# Function to emit OSC 133;A (prompt start)
_openagent_prompt_start() {
    printf '%s' "$_OPENAGENT_OSC133_A"
}

# Function to emit OSC 133;B (prompt end, command start)
_openagent_prompt_end() {
    printf '%s' "$_OPENAGENT_OSC133_B"
}

# Function to emit OSC 133;C (command end, output start) 
_openagent_command_end() {
    printf '%s' "$_OPENAGENT_OSC133_C"
}

# Function to emit OSC 133;D with exit code
_openagent_command_complete() {
    local exit_code=$1
    printf "$_OPENAGENT_OSC133_D" "$exit_code"
}

# Capture command being executed
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

# Handle command completion
_openagent_precmd() {
    local exit_code=$?
    # Only emit D sequence if we actually ran a command
    if [[ -n "$_openagent_current_command" ]]; then
        _openagent_command_complete "$exit_code"
        _openagent_current_command=""
    fi
    _openagent_prompt_start
}

# Set up preexec if not already defined (some systems/frameworks provide this)
if ! declare -F preexec >/dev/null; then
    # Simple preexec implementation for bash
    _openagent_debug_trap() {
        local command
        command=$(HISTTIMEFORMAT= history 1 | sed 's/^[ ]*[0-9]*[ ]*//')
        [[ "$command" != "$_openagent_last_command" ]] || return
        _openagent_last_command="$command"
        _openagent_preexec "$command"
    }
    
    # Only set up DEBUG trap if it's not already in use
    if [[ -z "$PROMPT_COMMAND" ]] || [[ "$PROMPT_COMMAND" != *"_openagent_debug_trap"* ]]; then
        trap '_openagent_debug_trap' DEBUG
    fi
else
    # Use existing preexec mechanism
    _openagent_original_preexec="$(declare -f preexec)"
    preexec() {
        _openagent_preexec "$@"
        if [[ -n "$_openagent_original_preexec" ]]; then
            eval "$_openagent_original_preexec"
            "$_openagent_original_preexec" "$@" 2>/dev/null || true
        fi
    }
fi

# Set up precmd (PROMPT_COMMAND in bash)
if [[ -z "$PROMPT_COMMAND" ]]; then
    PROMPT_COMMAND="_openagent_precmd"
else
    # Prepend to existing PROMPT_COMMAND
    PROMPT_COMMAND="_openagent_precmd; $PROMPT_COMMAND"
fi

# Add prompt start marker to existing PS1
if [[ "$PS1" != *"$_OPENAGENT_OSC133_A"* ]]; then
    # Insert prompt end marker just before the final prompt character
    if [[ "$PS1" =~ (.*)(\\\$|#|%|\>)([[:space:]]*)$ ]]; then
        PS1="${BASH_REMATCH[1]}$_OPENAGENT_OSC133_B${BASH_REMATCH[2]}${BASH_REMATCH[3]}"
    else
        # Fallback: just append to the end
        PS1="$PS1$_OPENAGENT_OSC133_B"
    fi
fi

# Utility function to test if OSC 133 is working
openagent_test_osc133() {
    echo "Testing OSC 133 integration..."
    echo "You should see command blocks in OpenAgent Terminal for the following:"
    echo
    printf '%s' "$_OPENAGENT_OSC133_A"
    echo -n "test_prompt> "
    printf '%s' "$_OPENAGENT_OSC133_B"
    echo "echo 'This should be a separate command block'"
    printf '%s' "$_OPENAGENT_OSC133_C"
    echo "This should be a separate command block"
    printf "$_OPENAGENT_OSC133_D" "0"
    echo
    echo "If you see distinct command blocks above, OSC 133 is working!"
}

# Function to disable OSC 133 integration
openagent_disable_osc133() {
    # Remove from PROMPT_COMMAND
    PROMPT_COMMAND="${PROMPT_COMMAND//_openagent_precmd;/}"
    PROMPT_COMMAND="${PROMPT_COMMAND//_openagent_precmd/}"
    
    # Remove DEBUG trap if we set it
    if [[ "$(trap -p DEBUG)" == *"_openagent_debug_trap"* ]]; then
        trap - DEBUG
    fi
    
    # Clean PS1
    PS1="${PS1//$_OPENAGENT_OSC133_A/}"
    PS1="${PS1//$_OPENAGENT_OSC133_B/}"
    
    echo "OpenAgent OSC 133 integration disabled for this session."
    echo "To permanently disable, remove or comment out the source line in your .bashrc"
}

# Provide feedback that integration is loaded
if [[ -n "$OPENAGENT_DEBUG" ]]; then
    echo "OpenAgent Terminal OSC 133 integration loaded (bash)"
fi

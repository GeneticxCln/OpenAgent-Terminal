# OpenAgent Terminal OSC 133 Integration for Fish Shell
# This script enables command block tracking by emitting OSC 133 sequences

# Only proceed if we're in an interactive shell
if not status is-interactive
    exit
end

# Avoid double-loading
if set -q OPENAGENT_INTEGRATION_LOADED
    exit
end
set -g OPENAGENT_INTEGRATION_LOADED 1

# Check if we're running in OpenAgent Terminal or a compatible terminal
function _openagent_is_supported_terminal
    # Check for OpenAgent Terminal
    if test "$TERM_PROGRAM" = "openagent-terminal"
        return 0
    end
    
    # Check for other terminals that support OSC 133
    switch "$TERM_PROGRAM"
        case "vscode" "iTerm.app" "WezTerm"
            return 0
    end
    
    # Check TERM variable for compatible terminals  
    switch "$TERM"
        case "*-256color" "xterm-kitty" "alacritty" "wezterm"
            return 0
    end
    
    # If OPENAGENT_FORCE_OSC133 is set, assume support
    if set -q OPENAGENT_FORCE_OSC133
        return 0
    end
    
    return 1
end

# Only enable if terminal is supported
if not _openagent_is_supported_terminal
    exit
end

# OSC 133 escape sequences
set -g _OPENAGENT_OSC133_A \e\]133\;A\a    # Prompt start
set -g _OPENAGENT_OSC133_B \e\]133\;B\a    # Prompt end / Command start  
set -g _OPENAGENT_OSC133_C \e\]133\;C\a    # Command end / Output start
set -g _OPENAGENT_OSC133_D \e\]133\;D\;%s\a # Command end with exit code

# Current command being executed
set -g _openagent_current_command ""

# Function to emit OSC 133;A (prompt start)
function _openagent_prompt_start
    printf '%s' $_OPENAGENT_OSC133_A
end

# Function to emit OSC 133;B (prompt end, command start)
function _openagent_prompt_end
    printf '%s' $_OPENAGENT_OSC133_B
end

# Function to emit OSC 133;C (command end, output start)
function _openagent_command_end
    printf '%s' $_OPENAGENT_OSC133_C
end

# Function to emit OSC 133;D with exit code
function _openagent_command_complete
    printf $_OPENAGENT_OSC133_D $argv[1]
end

# Fish preexec event handler - called before each command
function _openagent_preexec --on-event fish_preexec
    set -g _openagent_current_command "$argv[1]"
    # Don't emit sequences for certain commands that might interfere
    switch "$argv[1]"
        case "clear" "reset" "tput*"
            return
    end
    _openagent_command_end
end

# Fish postexec event handler - called after each command
function _openagent_postexec --on-event fish_postexec
    # fish_postexec doesn't provide the exit code directly
    # We'll use the $status variable instead
    if test -n "$_openagent_current_command"
        _openagent_command_complete $status
        set -g _openagent_current_command ""
    end
end

# Fish prompt event handler - called before each prompt
function _openagent_precmd --on-event fish_prompt
    _openagent_prompt_start
end

# Modify the fish_prompt function to include OSC 133;B
# We need to be careful not to break existing prompts
if functions -q fish_prompt
    # Backup the original prompt
    functions -c fish_prompt _openagent_original_fish_prompt
    
    # Create a new prompt function that includes our marker
    function fish_prompt
        # Call the original prompt
        set -l original_prompt (_openagent_original_fish_prompt)
        
        # Add our marker just before the end
        # Look for common prompt endings and insert before them
        if string match -q "*\$ " "$original_prompt"
            set original_prompt (string replace "\$ " "$_OPENAGENT_OSC133_B\$ " "$original_prompt")
        else if string match -q "*> " "$original_prompt"
            set original_prompt (string replace "> " "$_OPENAGENT_OSC133_B> " "$original_prompt")
        else if string match -q "*# " "$original_prompt"
            set original_prompt (string replace "# " "$_OPENAGENT_OSC133_B# " "$original_prompt")
        else
            # Fallback: just append to the end
            set original_prompt "$original_prompt$_OPENAGENT_OSC133_B"
        end
        
        echo -n "$original_prompt"
    end
else
    # No existing prompt, create a simple one
    function fish_prompt
        echo -n (whoami)@(hostname):(prompt_pwd)"$_OPENAGENT_OSC133_B\$ "
    end
end

# Utility function to test if OSC 133 is working
function openagent_test_osc133
    echo "Testing OSC 133 integration..."
    echo "You should see command blocks in OpenAgent Terminal for the following:"
    echo
    printf '%s' $_OPENAGENT_OSC133_A
    printf "test_prompt> "
    printf '%s' $_OPENAGENT_OSC133_B
    echo "echo 'This should be a separate command block'"
    printf '%s' $_OPENAGENT_OSC133_C
    echo "This should be a separate command block"
    printf $_OPENAGENT_OSC133_D "0"
    echo
    echo "If you see distinct command blocks above, OSC 133 is working!"
end

# Function to disable OSC 133 integration
function openagent_disable_osc133
    # Remove event handlers
    functions -e _openagent_preexec
    functions -e _openagent_postexec  
    functions -e _openagent_precmd
    
    # Restore original prompt if we backed it up
    if functions -q _openagent_original_fish_prompt
        functions -c _openagent_original_fish_prompt fish_prompt
        functions -e _openagent_original_fish_prompt
    end
    
    echo "OpenAgent OSC 133 integration disabled for this session."
    echo "To permanently disable, remove or comment out the source line in your config.fish"
end

# Function to show current integration status
function openagent_show_status
    echo "=== OpenAgent Terminal Integration Status ==="
    echo "Supported terminal: " (_openagent_is_supported_terminal; and echo "Yes" or echo "No")
    echo "Integration loaded: " (set -q OPENAGENT_INTEGRATION_LOADED; and echo "Yes" or echo "No")
    echo "Event handlers:"
    functions | grep _openagent
    echo
    echo "Current prompt function:"
    functions fish_prompt | head -n 5
end

# Function to enable debug mode
function openagent_debug_on
    set -g OPENAGENT_DEBUG 1
    echo "OpenAgent debug mode enabled"
end

function openagent_debug_off
    set -e OPENAGENT_DEBUG
    echo "OpenAgent debug mode disabled"
end

# Provide feedback that integration is loaded
if set -q OPENAGENT_DEBUG
    echo "OpenAgent Terminal OSC 133 integration loaded (fish)"
end

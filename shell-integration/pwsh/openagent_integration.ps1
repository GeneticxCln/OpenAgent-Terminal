# OpenAgent Terminal OSC 133 Integration for PowerShell
# This script enables command block tracking by emitting OSC 133 sequences

# Only proceed if we're in an interactive shell
if ($Host.Name -notlike "*ISE*" -and [Environment]::UserInteractive) {
    
    # Avoid double-loading
    if ($env:OPENAGENT_INTEGRATION_LOADED -eq "1") {
        return
    }
    $env:OPENAGENT_INTEGRATION_LOADED = "1"

    # Check if we're running in OpenAgent Terminal or a compatible terminal
    function Test-SupportedTerminal {
        # Check for OpenAgent Terminal
        if ($env:TERM_PROGRAM -eq "openagent-terminal") {
            return $true
        }

        # Check for other terminals that support OSC 133
        $compatibleTerminals = @("vscode", "Windows Terminal", "WezTerm")
        if ($env:TERM_PROGRAM -in $compatibleTerminals) {
            return $true
        }

        # Check TERM variable for compatible terminals
        $compatibleTerms = @("xterm-256color", "xterm-kitty", "alacritty", "wezterm")
        if ($env:TERM -in $compatibleTerms) {
            return $true
        }

        # If OPENAGENT_FORCE_OSC133 is set, assume support
        if ($env:OPENAGENT_FORCE_OSC133 -eq "1") {
            return $true
        }

        return $false
    }

    # Only enable if terminal is supported
    if (-not (Test-SupportedTerminal)) {
        return
    }

    # OSC 133 escape sequences
    $global:OPENAGENT_OSC133_A = "`e]133;A`a"    # Prompt start
    $global:OPENAGENT_OSC133_B = "`e]133;B`a"    # Prompt end / Command start
    $global:OPENAGENT_OSC133_C = "`e]133;C`a"    # Command end / Output start
    $global:OPENAGENT_OSC133_D = "`e]133;D;{0}`a" # Command end with exit code

    # Current command being executed
    $global:OpenAgentCurrentCommand = ""

    # Function to emit OSC 133;A (prompt start)
    function Write-OpenAgentPromptStart {
        [Console]::Write($global:OPENAGENT_OSC133_A)
    }

    # Function to emit OSC 133;B (prompt end, command start)
    function Write-OpenAgentPromptEnd {
        [Console]::Write($global:OPENAGENT_OSC133_B)
    }

    # Function to emit OSC 133;C (command end, output start)
    function Write-OpenAgentCommandEnd {
        [Console]::Write($global:OPENAGENT_OSC133_C)
    }

    # Function to emit OSC 133;D with exit code
    function Write-OpenAgentCommandComplete {
        param([int]$ExitCode)
        [Console]::Write($global:OPENAGENT_OSC133_D -f $ExitCode)
    }

    # PowerShell PreCommandLookup event - called before each command
    $ExecutionContext.InvokeCommand.PreCommandLookupAction = {
        param($CommandName, $CommandLookupEventArgs)
        
        $global:OpenAgentCurrentCommand = $CommandLookupEventArgs.Command
        
        # Don't emit sequences for certain commands that might interfere
        $skipCommands = @("clear", "cls", "Write-Host", "Write-Output")
        if ($CommandName -notin $skipCommands) {
            Write-OpenAgentCommandEnd
        }
    }

    # PowerShell prompt function override
    $global:OpenAgentOriginalPrompt = $function:prompt

    function global:prompt {
        # Get the exit code of the last command
        $lastExitCode = $LASTEXITCODE
        
        # Only emit D sequence if we actually ran a command
        if (-not [string]::IsNullOrEmpty($global:OpenAgentCurrentCommand)) {
            Write-OpenAgentCommandComplete -ExitCode $(if ($lastExitCode) { $lastExitCode } else { 0 })
            $global:OpenAgentCurrentCommand = ""
        }
        
        # Emit prompt start
        Write-OpenAgentPromptStart
        
        # Call original prompt function
        $promptResult = & $global:OpenAgentOriginalPrompt
        
        # Emit prompt end
        Write-OpenAgentPromptEnd
        
        return $promptResult
    }

    # Utility function to test if OSC 133 is working
    function Test-OpenAgentOSC133 {
        Write-Host "Testing OSC 133 integration..." -ForegroundColor Cyan
        Write-Host "You should see command blocks in OpenAgent Terminal for the following:" -ForegroundColor Yellow
        Write-Host ""
        
        [Console]::Write($global:OPENAGENT_OSC133_A)
        [Console]::Write("test_prompt> ")
        [Console]::Write($global:OPENAGENT_OSC133_B)
        Write-Host "Write-Host 'This should be a separate command block'"
        [Console]::Write($global:OPENAGENT_OSC133_C)
        Write-Host "This should be a separate command block" -ForegroundColor Green
        [Console]::Write($global:OPENAGENT_OSC133_D -f 0)
        Write-Host ""
        Write-Host "If you see distinct command blocks above, OSC 133 is working!" -ForegroundColor Green
    }

    # Function to disable OSC 133 integration
    function Disable-OpenAgentOSC133 {
        # Restore original prompt
        if ($global:OpenAgentOriginalPrompt) {
            $function:prompt = $global:OpenAgentOriginalPrompt
        }
        
        # Clear the command lookup action
        $ExecutionContext.InvokeCommand.PreCommandLookupAction = $null
        
        # Clear environment variable
        $env:OPENAGENT_INTEGRATION_LOADED = $null
        
        Write-Host "OpenAgent OSC 133 integration disabled for this session." -ForegroundColor Yellow
        Write-Host "To permanently disable, remove or comment out the import in your PowerShell profile" -ForegroundColor Yellow
    }

    # Function to show current integration status
    function Show-OpenAgentIntegrationStatus {
        Write-Host "=== OpenAgent Terminal Integration Status ===" -ForegroundColor Cyan
        Write-Host "Integration loaded: $($env:OPENAGENT_INTEGRATION_LOADED -eq '1')"
        Write-Host "Terminal program: $($env:TERM_PROGRAM)"
        Write-Host "Terminal type: $($env:TERM)"
        Write-Host "Prompt function overridden: $($null -ne $global:OpenAgentOriginalPrompt)"
        Write-Host "Command lookup action set: $($null -ne $ExecutionContext.InvokeCommand.PreCommandLookupAction)"
    }

    # Create aliases for common functions
    Set-Alias -Name "openagent-test" -Value Test-OpenAgentOSC133 -Scope Global
    Set-Alias -Name "openagent-disable" -Value Disable-OpenAgentOSC133 -Scope Global
    Set-Alias -Name "openagent-status" -Value Show-OpenAgentIntegrationStatus -Scope Global

    # Provide feedback that integration is loaded (only in debug mode)
    if ($env:OPENAGENT_DEBUG -eq "1") {
        Write-Host "OpenAgent Terminal OSC 133 integration loaded (PowerShell)" -ForegroundColor Green
    }
}

# Export functions for module-style usage
if ($PSVersionTable.PSVersion.Major -ge 5) {
    Export-ModuleMember -Function Test-OpenAgentOSC133, Disable-OpenAgentOSC133, Show-OpenAgentIntegrationStatus -Alias openagent-test, openagent-disable, openagent-status
}

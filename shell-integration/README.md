# OpenAgent Terminal Shell Integration

This directory contains shell integration scripts that enable **command block tracking** in OpenAgent Terminal using OSC 133 escape sequences. Command blocks allow you to:

- Navigate between command outputs
- Copy entire command outputs
- Execute commands in the context of previous command directories
- Visual separation of commands and their outputs

## Quick Start

### Automatic Setup (Recommended)

For most users, the universal auto-setup script will detect your shell and environment automatically:

```bash
# Download and source the auto-setup script
source /path/to/shell-integration/universal/auto_setup.sh
```

Add this line to your shell's config file (`.bashrc`, `.zshrc`, or `config.fish`) to enable it permanently.

### Manual Setup by Shell

#### Bash

Add to your `~/.bashrc`:

```bash
source /path/to/shell-integration/bash/openagent_integration.bash
```

#### Zsh (Standalone)

Add to your `~/.zshrc`:

```zsh
source /path/to/shell-integration/zsh/openagent_integration.zsh
```

#### Zsh (Oh-My-Zsh Plugin)

1. Copy the plugin to your oh-my-zsh plugins directory:
   ```bash
   cp -r shell-integration/zsh $ZSH/plugins/openagent
   ```

2. Add `openagent` to your plugins list in `~/.zshrc`:
   ```zsh
   plugins=(git ... openagent)
   ```

#### Fish

Add to your `~/.config/fish/config.fish`:

```fish
source /path/to/shell-integration/fish/openagent_integration.fish
```

## Features

### OSC 133 Sequences

The integration emits the following OSC 133 sequences:

- **OSC 133;A** - Prompt start (marks beginning of prompt)
- **OSC 133;B** - Prompt end / Command start (marks end of prompt, beginning of command)
- **OSC 133;C** - Command end / Output start (marks end of command, beginning of output)
- **OSC 133;D;[exit_code]** - Command complete (marks end of output with exit code)

### Terminal Compatibility

The integration automatically detects and works with:

- **OpenAgent Terminal** (primary target)
- **Visual Studio Code** integrated terminal
- **iTerm2** (macOS)
- **WezTerm**
- **Kitty**
- **Alacritty**
- Other terminals with OSC 133 support

### Framework Compatibility

The integration is designed to work with popular shell frameworks:

- **Oh-My-Zsh** (all themes)
- **Starship** prompt
- **Powerlevel10k** 
- **Prezto**
- Custom prompt configurations

## Verification

### Test OSC 133 Integration

After setup, test if the integration is working:

```bash
# All shells
openagent_test_osc133

# Or use the alias
openagent-test
```

This will emit test OSC 133 sequences. In OpenAgent Terminal, you should see distinct command blocks.

### Check Integration Status

**Bash/Zsh:**
```bash
# Check if integration is loaded
echo $OPENAGENT_INTEGRATION_LOADED

# Zsh only: show hooks status
openagent_show_hooks  # (zsh only)
```

**Fish:**
```fish
# Check integration status
openagent_show_status
```

### Debug Mode

Enable debug output to troubleshoot issues:

```bash
export OPENAGENT_DEBUG=1
# Then restart your shell
```

## Troubleshooting

### Integration Not Working

1. **Check Terminal Support:**
   ```bash
   /path/to/shell-integration/universal/detect_shell.sh
   ```

2. **Force Enable OSC 133:**
   ```bash
   export OPENAGENT_FORCE_OSC133=1
   # Add to your shell config and restart
   ```

3. **Check Shell Configuration:**
   - Ensure the integration script is sourced after other prompt modifications
   - Some themes or frameworks may override prompt settings

### Common Issues

#### No Command Blocks Visible

- **Terminal doesn't support OSC 133**: Use `detect_shell.sh` to verify support
- **Integration not loaded**: Check `$OPENAGENT_INTEGRATION_LOADED` variable
- **Conflicting prompt modifications**: Try loading the integration last in your config

#### Commands Not Properly Bracketed

- **Preexec/precmd conflicts**: Other tools may interfere with command hooks
- **Complex prompt setups**: Some themes modify prompts after our integration loads

#### Oh-My-Zsh Theme Conflicts

- The integration detects popular themes and adjusts accordingly
- For unsupported themes, try the standalone zsh integration instead
- Some themes with complex right-prompt (RPS1) configurations may need manual adjustment

## Manual Control

### Disable Integration

Temporarily disable for current session:

```bash
openagent_disable_osc133
# Or use alias
openagent-disable
```

Permanently disable by removing/commenting the source line from your shell config.

### Framework-Specific Notes

#### Starship Users

Starship users should use the auto-setup script which enables minimal mode for better compatibility:

```bash
source /path/to/shell-integration/universal/auto_setup.sh
```

#### Powerlevel10k Users

P10K users should also use auto-setup for optimal compatibility. The integration detects P10K and adjusts its behavior to avoid conflicts.

#### Custom Prompt Users

If you have a highly customized prompt, you may need to manually adjust the integration:

1. Load the integration early in your shell config
2. Ensure `$_OPENAGENT_OSC133_B` is included in your prompt
3. Don't modify the preexec/precmd hooks set by the integration

## Environment Variables

### Configuration Variables

- `OPENAGENT_INTEGRATION_LOADED=1` - Set when integration is active
- `OPENAGENT_FORCE_OSC133=1` - Force enable even in unsupported terminals
- `OPENAGENT_DEBUG=1` - Enable debug output
- `OPENAGENT_MINIMAL_MODE=1` - Use minimal integration mode (auto-set for some frameworks)

### Detection Variables

- `OPENAGENT_OMZ_DETECTED=1` - Oh-My-Zsh detected
- `OPENAGENT_STARSHIP_DETECTED=1` - Starship prompt detected  
- `OPENAGENT_P10K_DETECTED=1` - Powerlevel10k detected
- `OPENAGENT_PREZTO_DETECTED=1` - Prezto detected

## Files Overview

```
shell-integration/
├── bash/
│   └── openagent_integration.bash    # Bash integration script
├── zsh/
│   ├── openagent_integration.zsh     # Standalone zsh integration
│   └── openagent.plugin.zsh          # Oh-My-Zsh plugin
├── fish/
│   └── openagent_integration.fish    # Fish shell integration
├── universal/
│   ├── auto_setup.sh                 # Universal auto-detection and setup
│   └── detect_shell.sh               # Manual detection and verification
└── README.md                         # This documentation
```

## Advanced Usage

### Custom Integration

If you need to customize the integration for your specific setup:

1. Copy the appropriate shell script from this directory
2. Modify the sequences or logic as needed
3. Source your custom script instead of the provided one

### Selective Command Tracking

The integration includes logic to skip certain commands (like `clear`, `reset`) that might interfere with block tracking. You can customize this by modifying the case statements in the preexec functions.

### Exit Code Tracking

All integrations properly track and report command exit codes via OSC 133;D sequences, enabling features like:

- Visual indicators for failed commands
- Filtering commands by success/failure status
- Command history with exit code information

## Contributing

When adding support for additional shells or frameworks:

1. Follow the existing pattern of OSC 133;A/B/C/D sequence emission
2. Ensure compatibility with existing prompt configurations
3. Add appropriate detection and cleanup functions
4. Test with popular frameworks and themes
5. Update this documentation

## Security Notes

The integration scripts:

- Only activate in interactive shells
- Include terminal detection to avoid issues in unsupported environments
- Provide cleanup functions to disable integration if needed
- Don't modify command execution, only add escape sequences
- Are safe to source multiple times (include double-loading protection)

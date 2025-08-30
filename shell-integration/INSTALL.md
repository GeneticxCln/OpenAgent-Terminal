# OpenAgent Terminal Shell Integration Installation

## Quick Installation

### One-Line Install (All Shells)

Add this line to your shell configuration file:

**Bash** (`~/.bashrc`):
```bash
source /path/to/OpenAgent-Terminal/shell-integration/universal/auto_setup.sh
```

**Zsh** (`~/.zshrc`):
```zsh
source /path/to/OpenAgent-Terminal/shell-integration/universal/auto_setup.sh
```

**Fish** (`~/.config/fish/config.fish`):
```fish
source /path/to/OpenAgent-Terminal/shell-integration/universal/auto_setup.sh
```

Then restart your shell or source the config file.

## Detailed Installation by Shell

### Bash Installation

1. **Edit your bash configuration:**
   ```bash
   nano ~/.bashrc
   # OR
   vim ~/.bashrc
   ```

2. **Add the integration line at the end:**
   ```bash
   # OpenAgent Terminal Integration
   source /path/to/OpenAgent-Terminal/shell-integration/bash/openagent_integration.bash
   ```

3. **Apply changes:**
   ```bash
   source ~/.bashrc
   ```

4. **Verify installation:**
   ```bash
   openagent_test_osc133
   ```

### Zsh Installation (Standalone)

1. **Edit your zsh configuration:**
   ```bash
   nano ~/.zshrc
   # OR
   vim ~/.zshrc
   ```

2. **Add the integration line:**
   ```zsh
   # OpenAgent Terminal Integration
   source /path/to/OpenAgent-Terminal/shell-integration/zsh/openagent_integration.zsh
   ```

3. **Apply changes:**
   ```zsh
   source ~/.zshrc
   ```

4. **Verify installation:**
   ```zsh
   openagent_test_osc133
   ```

### Zsh Installation (Oh-My-Zsh Plugin)

1. **Copy plugin to oh-my-zsh:**
   ```bash
   mkdir -p $ZSH/plugins/openagent
   cp /path/to/OpenAgent-Terminal/shell-integration/zsh/* $ZSH/plugins/openagent/
   ```

2. **Edit your zsh configuration:**
   ```bash
   nano ~/.zshrc
   # OR
   vim ~/.zshrc
   ```

3. **Add `openagent` to your plugins list:**
   ```zsh
   plugins=(
     git
     zsh-autosuggestions
     openagent  # Add this line
   )
   ```

4. **Apply changes:**
   ```zsh
   source ~/.zshrc
   ```

5. **Verify installation:**
   ```zsh
   openagent_test_osc133
   ```

### Fish Installation

1. **Create fish config directory if it doesn't exist:**
   ```bash
   mkdir -p ~/.config/fish
   ```

2. **Edit your fish configuration:**
   ```bash
   nano ~/.config/fish/config.fish
   # OR
   vim ~/.config/fish/config.fish
   ```

3. **Add the integration line:**
   ```fish
   # OpenAgent Terminal Integration
   source /path/to/OpenAgent-Terminal/shell-integration/fish/openagent_integration.fish
   ```

4. **Apply changes:**
   ```fish
   source ~/.config/fish/config.fish
   ```

5. **Verify installation:**
   ```fish
   openagent_test_osc133
   ```

## Verification

After installation, verify that the integration is working:

### Check Integration Status

```bash
# Should output "1" if loaded
echo $OPENAGENT_INTEGRATION_LOADED

# Run the detection script
/path/to/OpenAgent-Terminal/shell-integration/universal/detect_shell.sh
```

### Test OSC 133 Sequences

```bash
openagent_test_osc133
```

You should see test output with distinct command blocks in OpenAgent Terminal.

### Check Available Commands

After installation, these commands are available:

- `openagent_test_osc133` - Test OSC 133 functionality
- `openagent_disable_osc133` - Disable integration for current session
- `openagent-test` - Alias for test function
- `openagent-disable` - Alias for disable function

**Zsh only:**
- `openagent_show_hooks` - Show current shell hooks

**Fish only:**
- `openagent_show_status` - Show integration status
- `openagent_debug_on` - Enable debug mode
- `openagent_debug_off` - Disable debug mode

## Troubleshooting Installation

### Integration Not Loading

1. **Check file paths:** Ensure the path in your shell config points to the correct location
2. **Check permissions:** Make sure the integration scripts are readable
3. **Check shell detection:** Use the detection script to verify your environment
4. **Enable debug mode:**
   ```bash
   export OPENAGENT_DEBUG=1
   source ~/.bashrc  # or ~/.zshrc or ~/.config/fish/config.fish
   ```

### Terminal Not Supported

If your terminal doesn't automatically detect OSC 133 support:

```bash
export OPENAGENT_FORCE_OSC133=1
# Add this to your shell config before the integration line
```

### Conflicts with Existing Tools

If you have conflicts with other prompt tools:

1. **Load integration last:** Place the integration line after other prompt modifications
2. **Use minimal mode:** Some frameworks automatically enable this
3. **Try different integration method:** 
   - Switch between standalone and plugin versions (zsh)
   - Try universal auto-setup instead of shell-specific scripts

### Oh-My-Zsh Theme Issues

If your oh-my-zsh theme conflicts with the integration:

1. **Try the plugin version** instead of standalone
2. **Load after theme selection:**
   ```zsh
   ZSH_THEME="your-theme"
   # ... other oh-my-zsh config ...
   plugins=(... openagent)  # Add openagent to plugins
   source $ZSH/oh-my-zsh.sh
   ```

## Uninstallation

### Temporary Disable

```bash
openagent_disable_osc133
```

### Permanent Removal

1. **Remove from shell config:**
   - Remove or comment out the integration line from your shell configuration file
   
2. **Remove oh-my-zsh plugin (if used):**
   ```bash
   rm -rf $ZSH/plugins/openagent
   ```
   And remove `openagent` from your plugins list in `~/.zshrc`

3. **Restart your shell:**
   ```bash
   exec $SHELL
   ```

## Advanced Installation Options

### Custom Installation Path

If you want to install the scripts to a custom location:

1. **Copy the integration files:**
   ```bash
   mkdir -p ~/.local/share/openagent-terminal
   cp -r /path/to/OpenAgent-Terminal/shell-integration ~/.local/share/openagent-terminal/
   ```

2. **Update your shell config to use the new path:**
   ```bash
   source ~/.local/share/openagent-terminal/shell-integration/universal/auto_setup.sh
   ```

### System-wide Installation

For system-wide installation (requires sudo):

1. **Copy to system location:**
   ```bash
   sudo mkdir -p /usr/local/share/openagent-terminal
   sudo cp -r /path/to/OpenAgent-Terminal/shell-integration /usr/local/share/openagent-terminal/
   ```

2. **Create system-wide profile script:**
   ```bash
   sudo tee /etc/profile.d/openagent-terminal.sh > /dev/null << 'EOF'
   # OpenAgent Terminal Integration
   if [ -f /usr/local/share/openagent-terminal/shell-integration/universal/auto_setup.sh ]; then
       source /usr/local/share/openagent-terminal/shell-integration/universal/auto_setup.sh
   fi
   EOF
   ```

### Development Installation

For development or testing:

1. **Clone the repository:**
   ```bash
   git clone https://github.com/openagent/terminal.git
   cd terminal
   ```

2. **Source directly from the repository:**
   ```bash
   # Add to your shell config
   source /path/to/terminal/shell-integration/universal/auto_setup.sh
   ```

This allows you to get updates by pulling the repository.

## Integration with Package Managers

### Homebrew (macOS/Linux)

If OpenAgent Terminal is installed via Homebrew, the integration scripts are typically located at:
```
/opt/homebrew/share/openagent-terminal/shell-integration/  # Apple Silicon Mac
/usr/local/share/openagent-terminal/shell-integration/     # Intel Mac/Linux
```

### Snap (Linux)

For Snap installations, scripts might be at:
```
/snap/openagent-terminal/current/share/shell-integration/
```

### AppImage (Linux)

Extract the integration scripts from the AppImage:
```bash
./OpenAgentTerminal-x.x.x.AppImage --appimage-extract
# Scripts will be in squashfs-root/share/shell-integration/
```

## Updating

To update the integration scripts:

1. **Pull latest changes** (if installed from repository)
2. **Restart your shell** or source your config file
3. **Test the integration** with `openagent_test_osc133`

The integration scripts are designed to be backwards compatible and safe to update.

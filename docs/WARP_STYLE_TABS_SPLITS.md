# Warp-Style Tabs and Splits in OpenAgent Terminal

This document describes the Warp Terminal inspired tab and split pane functionality implemented in OpenAgent Terminal.

## Overview

OpenAgent Terminal now supports Warp-style tab and split pane management, providing a modern, intuitive interface for managing multiple terminal sessions and workflows within a single window.

## Key Features

### 🏷️ **Smart Tab Naming**
- **Automatic naming** based on current directory and running commands
- **Project detection** for common project types (Rust, Node.js, Python, etc.)
- **Command-aware titles** that update when you run commands

### 🔄 **Session Persistence**
- **Auto-save sessions** every 30 seconds (configurable)
- **Restore layouts** on terminal restart
- **Cross-session command history** tracking

### ⌨️ **Warp-Style Keyboard Shortcuts**
| Shortcut | Action | Description |
|----------|---------|-------------|
| `Ctrl+T` / `Cmd+T` | New Tab | Create a new tab |
| `Ctrl+W` / `Cmd+W` | Close Tab/Pane | Close current tab or pane |
| `Ctrl+D` / `Cmd+D` | Split Right | Split current pane to the right |
| `Ctrl+Shift+D` / `Cmd+Shift+D` | Split Down | Split current pane downward |
| `Ctrl+Alt+←→↑↓` / `Cmd+Alt+←→↑↓` | Navigate Panes | Move focus between panes |
| `Ctrl+Shift+←→↑↓` / `Cmd+Ctrl+←→↑↓` | Resize Panes | Resize current pane |
| `Ctrl+Shift+Enter` / `Cmd+Shift+Enter` | Toggle Zoom | Zoom/unzoom current pane |
| `Ctrl+;` / `Cmd+;` | Cycle Recent | Cycle through recently used panes |
| `Ctrl+Shift+]` / `Cmd+Shift+]` | Next Tab | Switch to next tab |
| `Ctrl+Shift+[` / `Cmd+Shift+[` | Previous Tab | Switch to previous tab |

### 🎨 **Modern Visual Design**
- **Rounded corner tabs** with Warp-style aesthetics
- **Subtle animations** for smooth transitions
- **Visual split indicators** and resize handles
- **Zoom overlay** to indicate focused pane state

## Configuration

### Basic Setup

Add to your `openagent-terminal.toml`:

```toml
[workspace.warp]
enabled = true
auto_tab_naming = true
session_file = "~/.config/openagent-terminal/warp-session.json"
session_auto_save_interval = 30
pane_resize_step = 0.05
enable_pane_zoom = true
show_split_indicators = true

[workspace.warp.style]
tab_height = 36.0
corner_radius = 8.0
tab_padding = 12.0
drop_shadow = true
animation_duration_ms = 200

[workspace.warp.split_indicators]
show_split_preview = true
split_line_width = 2.0
show_resize_handles = true
zoom_overlay_alpha = 0.1
```

### Advanced Configuration

For custom key bindings, you can override the default Warp shortcuts:

```toml
# Example: Use different keys for splitting
[[keyboard.bindings]]
key = \"d\"
mods = \"Control|Shift\"
action = \"SplitHorizontal\"

[[keyboard.bindings]] 
key = \"v\"
mods = \"Control|Shift\"
action = \"SplitVertical\"
```

## Usage Examples

### Basic Workflow

1. **Start with a single tab**
   ```bash
   # Terminal opens with smart-named tab based on current directory
   ```

2. **Create additional tabs**
   - Press `Ctrl+T` (Linux) or `Cmd+T` (macOS)
   - Tabs automatically named based on directory/project

3. **Split for parallel work**
   - Press `Ctrl+D` to split right
   - Press `Ctrl+Shift+D` to split down
   - Navigate with `Ctrl+Alt+Arrow` keys

4. **Focus management**
   - Zoom a pane with `Ctrl+Shift+Enter`
   - Cycle recent panes with `Ctrl+;`
   - Go back to previous pane with `Ctrl+Alt+[`

### Development Workflow Example

```bash
# 1. Open project directory - tab auto-named "my-project"
cd /home/user/my-project

# 2. Split right for running tests
# Press Ctrl+D -> creates new pane on right

# 3. Navigate to test pane and run tests  
# Press Ctrl+Alt+→ to focus right pane
cargo test

# 4. Split bottom in test pane for logs
# Press Ctrl+Shift+D -> creates pane below
tail -f logs/app.log

# 5. Zoom log pane for better view
# Press Ctrl+Shift+Enter -> pane fills entire tab

# 6. Return to editor and create new tab for docs
# Press Ctrl+Shift+Enter to unzoom
# Press Ctrl+Alt+← twice to return to main pane
# Press Ctrl+T for new tab
cd docs && mdbook serve
```

## Implementation Details

### Architecture

The Warp-style functionality is implemented through several key components:

1. **WarpTabManager** (`workspace/warp_tab_manager.rs`)
   - Enhanced tab management with smart naming
   - Session persistence and restoration
   - Command history tracking

2. **WarpSplitManager** (`workspace/warp_split_manager.rs`) 
   - Intelligent pane navigation algorithms
   - Focus history and recent pane tracking
   - Zoom state management

3. **WarpUI** (`display/warp_ui.rs`)
   - Modern visual styling and animations
   - Split indicators and preview overlays
   - Smooth transition effects

4. **WarpBindings** (`config/warp_bindings.rs`)
   - Platform-specific keyboard shortcuts
   - Configurable key binding integration

### Smart Tab Naming Logic

Tabs are automatically named using the following priority:

1. **Project-based naming**
   - Detects `package.json`, `Cargo.toml`, `pyproject.toml`, etc.
   - Uses project name from configuration files
   - Falls back to directory name

2. **Command-based naming**  
   - Updates title when commands are executed
   - Format: "command in directory" (e.g., "cargo build in my-project")
   - Tracks recent command history

3. **Directory-based naming**
   - Uses current directory name as fallback
   - Caches results for performance

### Session Persistence

Session data includes:
- Tab order and active tab
- Split layouts for each tab  
- Working directories and command history
- Pane focus states and zoom status
- Creation timestamps and metadata

Sessions are automatically saved every 30 seconds and on clean exit.

## Integration with Existing OpenAgent Terminal

The Warp-style functionality integrates seamlessly with existing OpenAgent Terminal features:

- **AI Panel**: Per-tab AI contexts are preserved
- **Blocks System**: Command tracking works within split panes
- **Themes**: Warp UI respects your chosen theme
- **Security Lens**: All security policies apply to split panes
- **Search**: Search works across all panes in a tab

## Troubleshooting

### Common Issues

**Tab titles not updating automatically**
- Check `auto_tab_naming = true` in config
- Verify directory permissions for project detection

**Session not persisting**
- Ensure session file path is writable
- Check disk space and permissions

**Keyboard shortcuts not working**
- Verify no conflicting bindings in config
- Check if vim mode or search mode is active

**Split navigation feels unresponsive**
- Adjust `pane_resize_step` for smaller increments
- Check if pane zoom is active

### Debug Commands

```bash
# Check current tab and split state
openagent-terminal --print-workspace-info

# Validate session file
cat ~/.config/openagent-terminal/warp-session.json | jq .

# Test key binding resolution
openagent-terminal --list-bindings | grep -i warp
```

## Performance Considerations

- **Memory usage**: Session files are kept small (~KB per session)
- **Rendering**: Split indicators only drawn when needed
- **Animations**: Can be disabled in config for better performance
- **History**: Command history limited to 10 entries per tab

## Future Enhancements

Planned improvements include:

- **Pane swapping** with drag & drop
- **Tab reordering** by dragging
- **Split layout templates** for common configurations  
- **Cross-window tab moving**
- **Enhanced session management** with multiple named sessions

## Compatibility

- **Linux**: Full functionality with `Ctrl` based shortcuts
- **macOS**: Native `Cmd` shortcuts with system integration
- **Windows**: Full functionality with `Ctrl` based shortcuts
- **Terminal**: Compatible with all shell types (bash, zsh, fish, etc.)

---

For more details, see the [implementation example](../examples/warp_integration_example.rs) and the [API documentation](../openagent-terminal/src/workspace/).

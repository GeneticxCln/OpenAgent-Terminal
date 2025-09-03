# Warp-Style Features in OpenAgent Terminal

This document describes the new Warp Terminal-inspired tab and split pane functionality that has been added to OpenAgent Terminal.

## Overview

The Warp-style features bring modern, intuitive tab and split pane management to OpenAgent Terminal, inspired by the excellent UX of Warp Terminal. These features include:

- **Smart Tab Management**: Auto-naming tabs based on current directory and running commands
- **Advanced Split Panes**: Intuitive split creation, navigation, and resizing
- **Session Persistence**: Automatic session saving and restoration
- **Modern UI Styling**: Clean, contemporary visual design
- **Familiar Shortcuts**: Keyboard shortcuts matching Warp Terminal conventions
- **Performance Optimized**: Efficient management of terminal instances

## Architecture

### Core Components

The Warp functionality is implemented through several key modules:

1. **`warp_tab_manager.rs`** - Enhanced tab management with smart naming and session persistence
2. **`warp_split_manager.rs`** - Advanced split pane operations and navigation
3. **`warp_integration.rs`** - Integration layer connecting Warp managers to the main application
4. **`warp_bindings.rs`** - Keyboard shortcut configuration and handling
5. **`warp_ui.rs`** - Visual styling and animation configuration

### Integration Points

- **Workspace Module**: The `WorkspaceManager` can optionally use Warp functionality
- **Event System**: Warp operations integrate with the existing event processing system
- **Configuration**: Warp features are controlled via the `[workspace]` config section
- **Window Context**: Terminal creation and lifecycle management work with Warp managers

## Configuration

### Enabling Warp Features

Add the following to your OpenAgent Terminal configuration:

```toml
[workspace]
enabled = true
warp_style = true  # Enable Warp-style functionality
```

### Key Bindings

The default Warp-style shortcuts are:

#### Tab Management
- `Cmd+T` (macOS) / `Ctrl+T` (Linux/Windows): Create new tab
- `Cmd+W` / `Ctrl+W`: Close current tab
- `Cmd+]` / `Ctrl+]`: Next tab
- `Cmd+[` / `Ctrl+[`: Previous tab

#### Split Management
- `Cmd+D` / `Ctrl+D`: Split vertically (right)
- `Cmd+Shift+D` / `Ctrl+Shift+D`: Split horizontally (down)
- `Cmd+Shift+W` / `Ctrl+Shift+W`: Close current pane

#### Navigation
- `Cmd+Alt+Arrow` / `Ctrl+Alt+Arrow`: Navigate between panes
- `Cmd+;` / `Ctrl+;`: Cycle through recently used panes

#### Resizing
- `Cmd+Shift+Alt+Arrow`: Resize current pane
- `Cmd+Alt+E` / `Ctrl+Alt+E`: Equalize all split sizes
- `Cmd+Shift+Z` / `Ctrl+Shift+Z`: Toggle pane zoom

#### Sessions
- `Cmd+Shift+S` / `Ctrl+Shift+S`: Save current session
- `Cmd+Shift+O` / `Ctrl+Shift+O`: Load saved session

### UI Styling

Customize the appearance of Warp features:

```toml
[workspace.warp_ui.tabs]
active_bg = "#2d3748"
inactive_bg = "#4a5568"
border_active = "#63b3ed"
border_radius = 6

[workspace.warp_ui.splits]
border_color = "#4a5568"
border_active_color = "#63b3ed"
border_width = 2

[workspace.warp_ui.animations]
tab_switch_duration_ms = 150
enable_animations = true
```

## Features

### Smart Tab Management

**Auto-Naming**: Tabs automatically update their names based on:
- Current working directory (shows basename)
- Running command (when a command is active)
- Breadcrumb navigation for nested directories

**Example Tab Names**:
- `~/dev/myproject` - Shows current directory
- `~/dev/myproject | npm test` - Shows directory + running command
- `~/.../deep/nested/path` - Truncates long paths intelligently

**Recent Tab Tracking**: The system tracks recently accessed tabs for quick switching.

### Advanced Split Panes

**Intuitive Creation**:
- Vertical splits create a new pane to the right
- Horizontal splits create a new pane below
- New panes inherit the working directory

**Smart Navigation**:
- Arrow key navigation moves focus between adjacent panes
- Recent pane tracking for quick cycling with `Cmd+;`
- Visual focus indicators show the active pane

**Flexible Resizing**:
- Directional resizing with keyboard shortcuts
- Visual feedback during resize operations
- Equalization to make all panes equal size
- Pane zoom to temporarily maximize a single pane

**Smart Closing**:
- Closing a pane automatically focuses the most recently used pane
- Last pane in a tab closes the entire tab
- Graceful cleanup of terminal resources

### Session Persistence

**Automatic Saving**: Sessions are automatically saved including:
- Tab layout and order
- Split pane configurations
- Working directories for each pane
- Active tab and pane focus

**Restoration**: On startup, the terminal restores:
- All previous tabs with their layouts
- Working directories for each terminal
- Focus state (active tab and pane)

**Manual Control**: Save/load sessions manually with keyboard shortcuts.

### Performance Features

**Efficient Management**:
- Terminal instances are created only when needed
- Cleanup happens automatically when panes are closed
- Memory usage monitoring and optimization
- Fast switching between tabs and panes

**Monitoring**: Built-in performance statistics track:
- Terminal creation time
- Navigation performance
- Memory usage
- Active terminal count

## Usage Examples

### Basic Workflow

1. **Start with a single tab**: OpenAgent Terminal opens with one tab
2. **Create splits**: Use `Cmd+D` to split vertically, `Cmd+Shift+D` to split horizontally
3. **Navigate**: Use `Cmd+Alt+Arrow` keys to move between panes
4. **Create tabs**: Use `Cmd+T` to create additional tabs as needed
5. **Sessions**: Your layout automatically saves and restores

### Power User Workflow

1. **Set up project workspace**:
   ```
   Tab 1: Editor (~/project) | nvim src/main.rs
   ├─ Pane 1: nvim src/main.rs
   └─ Pane 2: ~/project | cargo watch -x test
   
   Tab 2: Git (~/project) | git status
   ├─ Pane 1: git log --oneline
   └─ Pane 2: git diff HEAD~1
   
   Tab 3: Services (~/project)
   ├─ Pane 1: docker-compose up
   └─ Pane 2: tail -f logs/app.log
   ```

2. **Quick navigation**: Use `Cmd+;` to cycle between recently used panes across all tabs

3. **Temporary focus**: Use `Cmd+Shift+Z` to zoom a pane for detailed work, then zoom out

4. **Session management**: Save this setup with `Cmd+Shift+S` to restore it later

## Implementation Details

### WarpTabManager

The `WarpTabManager` enhances the standard tab system with:
- Session serialization/deserialization
- Smart tab naming based on context
- Recent tab tracking
- Working directory management

### WarpSplitManager

The `WarpSplitManager` provides:
- Intelligent pane navigation
- Visual split feedback
- Recent pane tracking
- Smart focus management
- Zoom functionality

### Integration Layer

The `WarpIntegration` module bridges the Warp managers with the main application:
- Terminal lifecycle management
- Event dispatching
- Resource cleanup
- Performance monitoring

## Compatibility

The Warp features are designed to be:
- **Non-breaking**: Existing configurations continue to work
- **Optional**: Can be disabled via configuration
- **Backward compatible**: Falls back to standard tab/split behavior when disabled
- **Cross-platform**: Works on Linux, macOS, and Windows

## Troubleshooting

### Common Issues

**Warp features not working**:
- Ensure `warp_style = true` is set in your configuration
- Check that the workspace module is properly enabled
- Verify keyboard shortcuts aren't conflicting with other bindings

**Session not restoring**:
- Check file permissions on the session file location
- Ensure the session file directory exists
- Look for error messages in the terminal logs

**Performance issues**:
- Monitor memory usage with the debug info
- Consider reducing the number of active terminals
- Check if animations are causing issues and disable if needed

### Debug Information

Access debug information about the Warp system:

```rust
// In development/debugging
let debug_info = workspace.warp.as_ref().unwrap().debug_info();
println!("Tabs: {}, Active terminals: {}", 
         debug_info.tab_count, 
         debug_info.terminal_count);
```

## Future Enhancements

Planned improvements include:
- Visual tab reordering with drag-and-drop
- Split pane templates and layouts
- Tab groups and workspaces
- Integration with window management
- Plugin system for custom tab/pane behaviors

## Contributing

To contribute to the Warp features:

1. **Testing**: Try the features with various workflows and report issues
2. **UI/UX**: Suggest improvements to the visual design or interactions
3. **Performance**: Profile and optimize terminal management
4. **Documentation**: Improve this guide and add examples
5. **Features**: Propose and implement new Warp-style functionality

## Migration Guide

### From Standard Tabs/Splits

If you're currently using the standard tab and split functionality:

1. **Enable Warp**: Set `warp_style = true` in your config
2. **Update shortcuts**: Review the new keyboard shortcuts (mostly compatible)
3. **Sessions**: Your current tabs will be saved automatically going forward
4. **Styling**: Customize the new UI elements to match your preferences

### Configuration Migration

Old workspace config:
```toml
[workspace]
enabled = true
```

New Warp-enabled config:
```toml
[workspace]
enabled = true
warp_style = true

[workspace.warp_ui.tabs]
# Customize tab appearance

[workspace.warp_bindings]
# Customize keyboard shortcuts
```

The Warp-style features represent a significant enhancement to OpenAgent Terminal's workspace management, providing a modern, efficient, and intuitive interface for managing multiple terminal sessions.

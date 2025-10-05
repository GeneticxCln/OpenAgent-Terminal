# Terminal UX Polish

## Overview

The terminal now features a polished UX with alternate screen buffer, persistent status line, and clean streaming output areas. This provides a professional, clutter-free interface similar to modern terminal applications.

## Features Implemented

### 1. Alternate Screen Buffer

The application now uses the alternate screen buffer, which provides:

- **Clean Entry/Exit**: Original terminal content is preserved
- **Isolated Environment**: Application UI doesn't mix with shell history  
- **Professional Feel**: Similar to `vim`, `less`, `htop`, etc.

**Implementation:**
```rust
// Enter alternate screen on startup
terminal.enter_alternate_screen()?;

// Leave alternate screen on exit (automatic via restore())
terminal.restore()?;  // Also called in Drop
```

**Benefits:**
- Original terminal content restored on exit
- No scrollback pollution
- Clean separation between application and shell

### 2. Status Line

A persistent status line at the top displays key information:

```
 ● Connected  │  🤖 mock  │  📝 a7b3c1e2
─────────────────────────────────────────────
```

**Components:**
- **Connection State**: Colored indicator (● Green=Connected, Yellow=Connecting/Reconnecting, Red=Failed)
- **Model**: Current AI model being used
- **Session ID**: Short form of current session (8 chars)

**Features:**
- Updates automatically when session changes
- Color-coded connection status
- Separator line for visual clarity
- Truncates gracefully if terminal is narrow

### 3. Clean Screen Layout

```
┌─────────────────────────────────────────┐
│ ● Connected  │  🤖 mock  │  📝 a7b3c1e2│ ← Status Line
├─────────────────────────────────────────┤ ← Separator
│                                         │
│  [Streaming Area]                       │
│  AI responses and command output        │
│  appear here                            │
│                                         │
│                                         │
│                                         │
├─────────────────────────────────────────┤
│ > user input here_                      │ ← Prompt Area
└─────────────────────────────────────────┘
```

**Areas:**
- **Status Line**: Top row (row 0)
- **Separator**: Row 1
- **Streaming Area**: Rows 2 to (bottom - 2)
- **Prompt Area**: Bottom 2 rows

### 4. Streaming Output Management

Streaming responses now use a dedicated area:

**Before streaming:**
```rust
terminal.move_to_streaming_area()?;  // Move to line 3
```

**During streaming:**
- Tokens print to streaming area
- No prompt interleaving
- Clean, readable output

**After streaming:**
```rust
terminal.move_to_prompt_area()?;     // Return to prompt
// Prompt redraws at bottom
```

## Implementation Details

### StatusInfo Structure

```rust
pub struct StatusInfo {
    pub connection_state: String,  // "Connected", "Connecting", etc.
    pub model: String,              // AI model name
    pub session_id: Option<String>, // Current session ID
}
```

### Terminal Manager Methods

#### Alternate Screen
```rust
pub fn enter_alternate_screen(&mut self) -> Result<()>
pub fn leave_alternate_screen(&mut self) -> Result<()>
```

#### Status Line
```rust
pub fn set_status(&mut self, status: StatusInfo)
pub fn draw_status_line(&self) -> Result<()>
```

#### Area Navigation
```rust
pub fn move_to_streaming_area(&self) -> Result<()>  // Row 3
pub fn move_to_prompt_area(&self) -> Result<()>     // Bottom - 2
pub fn clear_streaming_area(&self) -> Result<()>
```

## Usage Examples

### Initialize Terminal with Status

```rust
let mut terminal = TerminalManager::new()?;

// Enter alternate screen
terminal.enter_alternate_screen()?;
terminal.clear_screen()?;

// Set up status line
let status = StatusInfo {
    connection_state: "Connected".to_string(),
    model: "gpt-4".to_string(),
    session_id: Some("a7b3c1e2".to_string()),
};
terminal.set_status(status);
terminal.draw_status_line()?;
```

### Update Status on Session Change

```rust
// When session changes
let status = StatusInfo {
    connection_state: "Connected".to_string(),
    model: config.model.clone(),
    session_id: session_manager.current_session_id().map(|s| s.to_string()),
};
terminal.set_status(status);
terminal.draw_status_line()?;
```

### Streaming Workflow

```rust
// Before streaming
terminal.move_to_streaming_area()?;
println!("🤖 AI: ");

// Stream tokens
for token in stream {
    print!("{}", token);
    io::stdout().flush()?;
}

// After streaming
println!("\n");
terminal.move_to_prompt_area()?;
// Prompt renders here
```

## Visual Design

### Status Line Colors

| State | Color | Icon |
|-------|-------|------|
| Connected | Green | ● |
| Connecting | Yellow | ● |
| Reconnecting | Yellow | ● |
| Failed | Red | ● |
| Disconnected | Red | ● |

### Status Line Format

```
 <color>●</color> <state>  │  🤖 <model>  │  📝 <session_id>
```

**Example:**
```
 🟢 Connected  │  🤖 claude-3-opus  │  📝 a7b3c1e2
```

### Separator Line

```
───────────────────────────────────────────────
```

Full-width using box-drawing character `─` (U+2500)

## Connection State Integration

The status line automatically reflects IPC connection state:

```rust
let conn_state = match client.connection_state() {
    ConnectionState::Connected => "Connected",
    ConnectionState::Connecting => "Connecting",
    ConnectionState::Reconnecting { attempt } => "Reconnecting",
    ConnectionState::Failed => "Failed",
    ConnectionState::Disconnected => "Disconnected",
};
```

## Terminal Resize Handling

On resize events:

```rust
Event::Resize(cols, rows) => {
    // Status line redraws automatically on next loop iteration
    // No special handling needed
}
```

The status line truncates gracefully if the terminal becomes too narrow.

## Error Handling

### Alternate Screen Failures

If entering/leaving alternate screen fails:
- Falls back to normal mode
- Logs error
- Continues operation

### Drawing Failures

If status line drawing fails:
- Logged as warning
- Application continues
- Status updates may be skipped

## Comparison: Before vs After

### Before
```
$ ./openagent-terminal
Welcome message...
Connected to backend...
> user query here
AI response mixed with prompt
> next query appears mixed in
More text...
```

**Issues:**
- Cluttered output
- Prompt interleaving
- No clear status
- Scrollback pollution

### After
```
╔══════════════════════════════════════════╗
║ ● Connected  │  🤖 mock  │  📝 a7b3c1e2 ║
╠══════════════════════════════════════════╣
║                                          ║
║  🤖 AI: Clean streaming output here     ║
║  Multiple lines of response              ║
║  No prompt interleaving                  ║
║                                          ║
╠══════════════════════════════════════════╣
║ > user input here_                       ║
╚══════════════════════════════════════════╝
```

**Improvements:**
- Clear status always visible
- Separated output areas
- Clean, professional look
- No scrollback pollution
- Original screen restored on exit

## Testing

### Manual Testing

1. **Alternate Screen:**
   ```bash
   # Before starting, note terminal content
   ./target/release/openagent-terminal
   # Application UI appears in alternate screen
   # Type /exit
   # Original terminal content restored
   ```

2. **Status Line:**
   ```bash
   ./target/release/openagent-terminal
   # Observe status line at top
   # Load a session: /load <session-id>
   # Status line updates to show session
   ```

3. **Streaming:**
   ```bash
   ./target/release/openagent-terminal
   # Send a query
   # Observe clean streaming in dedicated area
   # Prompt stays at bottom
   ```

### Resize Testing

```bash
./target/release/openagent-terminal
# Resize terminal window
# Status line adjusts automatically
# Prompt area stays at bottom
```

## Configuration

Status line appearance can be customized via configuration:

```rust
// Future: config.terminal.status_line settings
pub struct StatusLineConfig {
    pub show: bool,              // Show/hide status line
    pub format: String,          // Custom format string
    pub colors: ColorScheme,     // Custom colors
}
```

## Performance Considerations

### Overhead

- **Status Line**: Redraws on each loop iteration (~100ms poll)
- **Alternate Screen**: One-time cost on enter/exit
- **Area Movement**: Minimal (cursor positioning only)

### Optimization

Current implementation is efficient:
- Status only redraws if terminal size permits
- No unnecessary redraws
- Cursor positions cached

## Accessibility

### Screen Readers

- Status line uses standard terminal colors
- Text-based (no fancy graphics)
- Works with terminal screen readers

### High Contrast

Status line colors chosen for high contrast:
- Green/Red for connection state
- White/Gray for labels
- Works in light and dark themes

## Future Enhancements

### Dynamic Status Components

```rust
pub enum StatusComponent {
    ConnectionState,
    Model,
    SessionId,
    TokenCount,
    Latency,
    Custom(String),
}
```

### Configurable Layout

```rust
pub struct LayoutConfig {
    pub status_position: Position,  // Top/Bottom
    pub prompt_position: Position,  // Top/Bottom
    pub reserve_lines: usize,       // Lines reserved for prompt
}
```

### Status Animations

```rust
// Animated connection state
● Connecting.
● Connecting..
● Connecting...
```

### Multi-line Status

```
╔════════════════════════════════════════╗
║ ● Connected  │  🤖 claude-3-opus      ║
║ 📝 a7b3c1e2  │  ⏱️  125ms  │  🔋 85% ║
╚════════════════════════════════════════╝
```

## Troubleshooting

### Status Line Not Showing

**Problem:** Status line not visible  
**Solution:**
- Check terminal height ≥ 5 rows
- Verify `enter_alternate_screen()` called
- Check log for drawing errors

### Prompt in Wrong Location

**Problem:** Prompt appears mid-screen  
**Solution:**
- Call `move_to_prompt_area()` before rendering
- Check terminal size calculation
- Verify streaming clears properly

### Original Screen Not Restored

**Problem:** Terminal state corrupt after exit  
**Solution:**
- Ensure `restore()` is called
- Check Drop implementation runs
- Manually call `reset` command if needed

## Summary

The terminal UX polish provides:

✅ **Alternate Screen Buffer** - Clean entry/exit  
✅ **Persistent Status Line** - Always visible status  
✅ **Separated Output Areas** - No prompt interleaving  
✅ **Professional Appearance** - Modern terminal app UX  
✅ **Clean Streaming** - Dedicated output area  
✅ **Automatic Updates** - Status tracks session state

The implementation follows best practices from mature terminal applications like `vim`, `htop`, and `tmux`, providing users with a familiar, polished experience.

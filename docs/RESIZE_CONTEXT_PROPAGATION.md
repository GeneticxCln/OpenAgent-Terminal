# Resize Events and Context Propagation

## Overview

The terminal now automatically detects resize events and propagates terminal size changes to the Python backend via `context.update` notifications. This allows the backend to adjust text wrapping, rendering, and layout dynamically as the terminal window is resized.

## Implementation

### 1. Resize Event Detection

The main event loop in `src/main.rs` listens for `Event::Resize` events from the crossterm library:

```rust
match event {
    Event::Resize(cols, rows) => {
        info!("📱 Terminal resized to {}x{}", cols, rows);
        
        // Send context.update notification to backend
        let notification = ipc::message::Notification::context_update_terminal_size(cols, rows);
        let mut client_lock = client.lock().await;
        match client_lock.send_notification(notification).await {
            Ok(_) => {
                debug!("✅ Sent terminal resize notification to backend");
            }
            Err(e) => {
                error!("❌ Failed to send resize notification: {}", e);
            }
        }
    }
    // ... other events
}
```

### 2. Notification Methods

Three notification builder methods are available in `src/ipc/message.rs`:

#### Basic CWD Update (Backward Compatible)
```rust
pub fn context_update(cwd: impl Into<String>) -> Self
```

**Example:**
```rust
let notification = Notification::context_update("/home/user/project");
```

**JSON Output:**
```json
{
  "jsonrpc": "2.0",
  "method": "context.update",
  "params": {
    "cwd": "/home/user/project"
  }
}
```

#### Terminal Size Update
```rust
pub fn context_update_terminal_size(cols: u16, rows: u16) -> Self
```

**Example:**
```rust
let notification = Notification::context_update_terminal_size(120, 40);
```

**JSON Output:**
```json
{
  "jsonrpc": "2.0",
  "method": "context.update",
  "params": {
    "terminal_size": {
      "cols": 120,
      "rows": 40
    }
  }
}
```

#### Full Context Update
```rust
pub fn context_update_full(
    cwd: Option<String>, 
    terminal_size: Option<(u16, u16)>
) -> Self
```

**Example:**
```rust
let notification = Notification::context_update_full(
    Some("/home/user/project".to_string()),
    Some((120, 40))
);
```

**JSON Output:**
```json
{
  "jsonrpc": "2.0",
  "method": "context.update",
  "params": {
    "cwd": "/home/user/project",
    "terminal_size": {
      "cols": 120,
      "rows": 40
    }
  }
}
```

### 3. Sending Notifications

The IPC client now has a `send_notification` method for fire-and-forget notifications:

```rust
pub async fn send_notification(&mut self, notification: Notification) -> Result<(), IpcError>
```

**Features:**
- Fire-and-forget: No response expected
- Non-blocking: Doesn't wait for acknowledgment
- Error handling: Returns error if connection is lost
- Logging: Debug logs for sent notifications

**Example Usage:**
```rust
let notification = Notification::context_update_terminal_size(cols, rows);
client.send_notification(notification).await?;
```

## Backend Handling

The Python backend should handle `context.update` notifications to adjust its behavior:

```python
def handle_notification(self, method: str, params: dict):
    if method == "context.update":
        if "terminal_size" in params:
            cols = params["terminal_size"]["cols"]
            rows = params["terminal_size"]["rows"]
            self.update_terminal_size(cols, rows)
        
        if "cwd" in params:
            cwd = params["cwd"]
            self.update_working_directory(cwd)
```

### Use Cases for Backend

1. **Text Wrapping**: Adjust line wrapping to match new terminal width
2. **Code Formatting**: Reflow code blocks to fit within new width
3. **Table Rendering**: Adjust column widths for tables and lists
4. **Progress Bars**: Resize progress indicators
5. **Markdown Rendering**: Reformat paragraphs and code blocks
6. **Syntax Highlighting**: Adjust line breaks in highlighted code

## Behavior

### When Resize Occurs

1. User resizes terminal window
2. Crossterm detects resize and emits `Event::Resize(cols, rows)`
3. Frontend logs resize: `📱 Terminal resized to 120x40`
4. Frontend creates `context.update` notification with new size
5. Frontend sends notification to backend (fire-and-forget)
6. Backend receives notification and adjusts rendering parameters

### Error Handling

If notification sending fails (e.g., connection lost):
- Error is logged: `❌ Failed to send resize notification: {error}`
- Frontend continues operating normally
- No impact on user interaction
- Reconnection will re-establish context

## Testing

### Manual Testing

1. **Start Backend and Frontend:**
   ```bash
   # Terminal 1
   cd backend
   python -m openagent_terminal.bridge
   
   # Terminal 2
   ./target/release/openagent-terminal
   ```

2. **Resize Terminal:**
   - Drag terminal window edges
   - Use window manager keyboard shortcuts
   - Maximize/restore window

3. **Check Logs:**
   ```
   [INFO] 📱 Terminal resized to 120x40
   [DEBUG] ✅ Sent terminal resize notification to backend
   ```

4. **Backend Logs:**
   The Python backend should log receiving the notification:
   ```
   INFO - Received context.update: terminal_size={'cols': 120, 'rows': 40}
   ```

### Automated Testing

```rust
#[tokio::test]
async fn test_send_resize_notification() {
    let mut client = IpcClient::new();
    // Connect to mock backend...
    
    let notification = Notification::context_update_terminal_size(100, 30);
    let result = client.send_notification(notification).await;
    
    assert!(result.is_ok());
}
```

## Performance Considerations

### Debouncing

Terminal resize events can fire rapidly during window dragging. Consider implementing debouncing:

```rust
use tokio::time::{Duration, Instant};

let mut last_resize = Instant::now();
const DEBOUNCE_MS: u64 = 100;

match event {
    Event::Resize(cols, rows) => {
        let now = Instant::now();
        if now.duration_since(last_resize) < Duration::from_millis(DEBOUNCE_MS) {
            // Skip this resize event
            continue;
        }
        last_resize = now;
        
        // Send notification...
    }
}
```

### Throttling

Alternatively, use throttling to limit notification rate:

```rust
use tokio::time::{Duration, interval};

let mut resize_ticker = interval(Duration::from_millis(200));
let mut pending_resize: Option<(u16, u16)> = None;

// In event loop:
tokio::select! {
    event = event::read() => {
        if let Event::Resize(cols, rows) = event {
            pending_resize = Some((cols, rows));
        }
    }
    _ = resize_ticker.tick() => {
        if let Some((cols, rows)) = pending_resize.take() {
            // Send notification for latest resize
            send_resize_notification(cols, rows).await;
        }
    }
}
```

## Protocol Specification

### Notification Format

**Method:** `context.update`  
**Direction:** Client → Server (notification only, no response)

**Parameters:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cwd` | string | No | Current working directory |
| `terminal_size` | object | No | Terminal dimensions |
| `terminal_size.cols` | integer | If terminal_size present | Number of columns |
| `terminal_size.rows` | integer | If terminal_size present | Number of rows |

**Example:**
```json
{
  "jsonrpc": "2.0",
  "method": "context.update",
  "params": {
    "terminal_size": {
      "cols": 120,
      "rows": 40
    }
  }
}
```

## Future Enhancements

### Additional Context Fields

Future versions could include:

```rust
pub struct ContextUpdate {
    pub cwd: Option<String>,
    pub terminal_size: Option<TerminalSize>,
    pub theme: Option<String>,           // Light/dark theme
    pub font_size: Option<u32>,          // Font size
    pub color_scheme: Option<String>,    // Color scheme name
    pub locale: Option<String>,          // User locale
    pub timezone: Option<String>,        // User timezone
}
```

### Smart Context Detection

Automatically detect and send context changes:

1. **Directory Changes**: Monitor `cd` commands
2. **Git Branch Changes**: Detect git operations
3. **Environment Variables**: Track important env vars
4. **Active Processes**: Send info about running commands

### Context Diffing

Only send changed fields:

```rust
struct ContextTracker {
    last_cwd: Option<String>,
    last_size: Option<(u16, u16)>,
}

impl ContextTracker {
    fn has_changed(&self, new_context: &Context) -> bool {
        self.last_cwd != new_context.cwd 
        || self.last_size != new_context.terminal_size
    }
}
```

## Troubleshooting

### Notifications Not Received

**Symptom:** Backend doesn't adjust to resize  
**Check:**
1. Backend notification handler implemented?
2. Frontend logs show notification sent?
3. Network/socket connection stable?

**Debug:**
```bash
# Enable debug logging
RUST_LOG=debug ./target/release/openagent-terminal
```

### Excessive Notifications

**Symptom:** Too many resize notifications  
**Solution:** Implement debouncing or throttling (see Performance Considerations)

### Connection Lost During Resize

**Symptom:** Error sending notification  
**Behavior:** Frontend continues normally, error logged  
**Recovery:** Reconnection will re-establish context

## Summary

The resize event handling system provides:

✅ **Automatic Detection**: No manual intervention needed  
✅ **Real-time Updates**: Backend notified immediately  
✅ **Non-blocking**: Fire-and-forget notifications  
✅ **Error Resilient**: Graceful handling of failures  
✅ **Extensible**: Easy to add new context fields  
✅ **Standards-based**: Uses JSON-RPC 2.0 notifications

The backend can now dynamically adjust its rendering to match the user's terminal size, providing a better user experience across different window sizes and layouts.

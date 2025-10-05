# Concurrent Streaming Quick Reference

## Problem
Streaming notifications blocked keyboard input, making Ctrl+C and approval prompts unresponsive.

## Solution
Use `tokio::select!` to handle notifications and cancellation concurrently.

## Key Components

### 1. Cancellation Token (Watch Channel)
```rust
// In run_interactive_loop()
let (cancel_tx, _cancel_rx) = watch::channel(false);

// To cancel:
cancel_tx.send(true)

// To reset:
cancel_tx.send(false)

// To subscribe:
let mut cancel_rx = cancel_tx.subscribe();
```

### 2. Concurrent Streaming Loop
```rust
async fn handle_agent_query_concurrent(
    client: Arc<Mutex<&mut IpcClient>>,
    query: &str,
    cancel_tx: &watch::Sender<bool>,
) -> Result<()> {
    let mut cancel_rx = cancel_tx.subscribe();
    
    loop {
        tokio::select! {
            // Branch 1: Check cancellation
            Ok(_) = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    break; // Stream cancelled
                }
            }
            
            // Branch 2: Wait for notification
            notification_result = async {
                let mut client = client.lock().await;
                client.next_notification().await
            } => {
                match notification_result {
                    Ok(notification) => {
                        handle_stream_notification(&notification, ...).await?;
                        if notification.method == "stream.complete" {
                            break;
                        }
                    }
                    Err(e) => break,
                }
            }
        }
    }
    Ok(())
}
```

### 3. User Approval with Input
```rust
async fn wait_for_approval(cancel_tx: &watch::Sender<bool>) -> Result<bool> {
    let mut cancel_rx = cancel_tx.subscribe();
    
    loop {
        tokio::select! {
            // Check for cancellation
            Ok(_) = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    return Ok(false);
                }
            }
            
            // Poll for keyboard input
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                if event::poll(Duration::from_millis(10))? {
                    if let Event::Key(key_event) = event::read()? {
                        match key_event.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(true),
                            KeyCode::Char('n') | KeyCode::Char('N') => return Ok(false),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
```

## Benefits

✅ **Non-blocking**: Main loop continues processing while streaming  
✅ **Cancellable**: Ctrl+C works at any time  
✅ **Responsive**: Real user input for approvals  
✅ **Clean**: Proper async/await patterns  

## Flow Diagram

```
User Input
    ↓
[Main Loop] ←─────────────────┐
    ↓                          │
[Submit Query]                 │
    ↓                          │
[handle_agent_query_concurrent]│
    ↓                          │
[tokio::select!]               │
    ├→ Cancellation? ──────────┤
    └→ Notification            │
         ├→ Token              │
         ├→ Block              │
         ├→ Approval ──→ [wait_for_approval]
         │                ├→ y/n input
         │                └→ Cancel ────┘
         └→ Complete ───────────────────┘
```

## Common Patterns

### Adding a New Concurrent Operation
```rust
tokio::select! {
    // Existing branches...
    
    // New operation
    result = my_async_operation() => {
        // handle result
    }
}
```

### Propagating Cancellation
```rust
let mut cancel_rx = cancel_tx.subscribe();

// In your async function:
if *cancel_rx.borrow() {
    return; // or handle cancellation
}
```

### Timeout with Cancellation
```rust
tokio::select! {
    Ok(_) = cancel_rx.changed() => {
        // Cancelled
    }
    _ = tokio::time::sleep(Duration::from_secs(30)) => {
        // Timeout
    }
    result = operation() => {
        // Success
    }
}
```

## Testing

```bash
# Build
cargo build

# Run (ensure Python backend is running)
cargo run

# Test scenarios:
# 1. Send query, watch streaming
# 2. Press Ctrl+C during stream
# 3. Trigger tool approval
# 4. Press 'y' or 'n'
# 5. Press Ctrl+C during approval
```

## Troubleshooting

**Problem**: Cancellation not working  
**Solution**: Ensure `cancel_rx.subscribe()` is called and checked in `tokio::select!`

**Problem**: Approval prompt not responding  
**Solution**: Check that `event::poll()` timeout isn't too long (use 10-50ms)

**Problem**: Multiple cancellations  
**Solution**: Reset with `cancel_tx.send(false)` before starting new operations

## References

- [tokio::select! docs](https://docs.rs/tokio/latest/tokio/macro.select.html)
- [tokio::sync::watch docs](https://docs.rs/tokio/latest/tokio/sync/watch/index.html)
- [crossterm event docs](https://docs.rs/crossterm/latest/crossterm/event/index.html)

# IPC Robustness and Ergonomics Improvements

## Overview

This document describes the robustness and ergonomics improvements made to the IPC (Inter-Process Communication) layer between the Rust frontend and Python backend.

## 1. Reconnection Strategy

### Features
- **Exponential Backoff**: Connection retries use exponential backoff (200ms, 400ms, 800ms, etc.)
- **User Notifications**: Clear logging at each reconnection attempt
- **Connection State Tracking**: Detailed state management with `ConnectionState` enum

### Connection States
```rust
pub enum ConnectionState {
    Disconnected,           // Not connected to backend
    Connecting,            // Initial connection attempt
    Connected,             // Successfully connected
    Reconnecting { attempt: u32 },  // Attempting to reconnect
    Failed,                // Reconnection failed after max attempts
}
```

### Usage
```rust
// Initial connection with retry
client.connect(&socket_path).await?;

// Manual reconnection (for future use)
client.reconnect().await?;

// Check connection state
if client.is_connected() {
    // Send requests
}
```

### Logging Output
```
🔌 Connecting to Python backend at /path/to/socket
⚠️  Connection attempt 1 failed: Connection refused
🔄 Reconnection attempt 2 after 200ms
✅ Connected to Unix socket
```

## 2. JSON-RPC Payload Validation

### Strict Validation
All JSON-RPC message types now use `#[serde(deny_unknown_fields)]` to catch protocol drift:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request { ... }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response { ... }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notification { ... }
```

### Unknown Field Detection
Before strict parsing, messages are checked with a tolerant parser to log unknown fields:

```rust
pub(crate) struct TolerantMessage {
    pub jsonrpc: String,
    pub method: Option<String>,
    pub id: Option<Value>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}
```

When unknown fields are detected:
```
⚠️  Protocol drift detected in 'agent.query': unknown fields ["new_field", "extra_param"]
```

This helps identify:
- Backend sending new fields not yet supported by frontend
- Typos in field names
- Protocol version mismatches

## 3. Request ID Space Separation

### ID Space Allocation
To prevent request ID collisions between different subsystems:

| Component | ID Range | Purpose |
|-----------|----------|---------|
| Interactive Flow | 0 - 9999 | Direct user queries via IpcClient |
| SessionManager | 10000+ | Session management operations |

### Implementation

**IpcClient** (client.rs):
```rust
const INTERACTIVE_ID_MIN: u64 = 0;
const INTERACTIVE_ID_MAX: u64 = 9999;

pub fn next_request_id(&mut self) -> u64 {
    self.request_counter += 1;
    // Wrap around to prevent collision with SessionManager IDs
    if self.request_counter > INTERACTIVE_ID_MAX {
        warn!("⚠️  Interactive request ID wrapped around");
        self.request_counter = INTERACTIVE_ID_MIN + 1;
    }
    self.request_counter
}
```

**SessionManager** (session.rs):
```rust
const SESSION_MANAGER_ID_MIN: u64 = 10000;
const SESSION_MANAGER_ID_MAX: u64 = u64::MAX;

fn next_request_id(&mut self) -> u64 {
    self.request_counter += 1;
    // Validate we're in the correct ID space
    if self.request_counter < SESSION_MANAGER_ID_MIN {
        warn!("⚠️  SessionManager ID counter corrupted, resetting");
        self.request_counter = SESSION_MANAGER_ID_MIN;
    }
    self.request_counter
}
```

### Benefits
- **No Collisions**: Different subsystems can't accidentally use the same request ID
- **Debugging**: Request IDs clearly indicate which subsystem originated them
- **Scalability**: Easy to add new subsystems with their own ID ranges

## 4. Connection Monitoring and Error Handling

### Disconnect Detection
The IPC client now properly detects and logs connection failures:

**Write Handler**:
```rust
if let Err(e) = writer.write_all(message.as_bytes()).await {
    error!("❌ Write failed: {} - Connection lost", e);
    break;
}
```

**Read Handler**:
```rust
match lines.next_line().await {
    Ok(Some(line)) => { /* process message */ }
    Ok(None) => {
        warn!("🔌 Connection closed by backend (EOF received)");
        break;
    }
    Err(e) => {
        error!("❌ Read error: {} - Connection lost", e);
        break;
    }
}
```

### Timeout Handling
Request timeouts are handled gracefully with cleanup:

```rust
let result = tokio::time::timeout(
    std::time::Duration::from_secs(30), 
    rx
).await;

match result {
    Ok(response) => response,
    Err(_) => {
        // Clean up to prevent memory leak
        pending_requests.lock().unwrap().remove(&request_id);
        Err(IpcError::Timeout)
    }
}
```

## Usage Examples

### Basic Connection with Retry
```rust
let mut client = IpcClient::new();
match client.connect("/path/to/socket").await {
    Ok(_) => println!("Connected successfully"),
    Err(e) => eprintln!("Connection failed: {}", e),
}
```

### Monitoring Connection State
```rust
match client.connection_state() {
    ConnectionState::Connected => { /* normal operation */ }
    ConnectionState::Reconnecting { attempt } => {
        println!("Reconnecting, attempt {}", attempt);
    }
    ConnectionState::Failed => {
        eprintln!("Connection failed permanently");
    }
    _ => {}
}
```

### Sending Requests with ID Space
```rust
// Interactive flow (IDs 0-9999)
let id = client.next_request_id();
let request = Request::agent_query(id, "Hello");
client.send_request(request).await?;

// Session management (IDs 10000+)
let session_manager = SessionManager::new(Arc::new(Mutex::new(client)));
session_manager.list_sessions(None).await?;  // Uses ID >= 10000
```

## Testing Robustness

### Connection Failures
1. Start frontend without backend running
2. Observe retry attempts with exponential backoff
3. Start backend during retry window
4. Connection should succeed automatically

### Protocol Drift
1. Modify backend to send extra field: `{"result": {"data": "test", "unknown_field": 123}}`
2. Frontend logs: `⚠️  Protocol drift detected in 'method': unknown fields ["unknown_field"]`
3. Strict parsing still succeeds (tolerant check is non-blocking)

### ID Space Separation
1. Send 10,000+ interactive requests
2. Observe ID wraparound warning
3. Verify no collision with SessionManager IDs
4. Check logs for ID space validation

## Future Enhancements

### Automatic Reconnection
Currently, reconnection is manual via `client.reconnect()`. Future improvements could:
- Add automatic reconnection on detected disconnection
- Notify main thread via channel when connection is lost
- Queue pending requests during reconnection

### Health Checks
- Periodic ping/pong to verify connection health
- Automatic reconnection if health check fails
- Connection quality metrics

### Rate Limiting
- Per-subsystem request rate limits
- Backpressure signaling
- Request prioritization

## Configuration

Future configuration options (not yet implemented):
```rust
pub struct IpcConfig {
    /// Maximum connection retry attempts
    pub max_retries: u32,
    /// Request timeout duration
    pub request_timeout: Duration,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Enable unknown field warnings
    pub warn_unknown_fields: bool,
}
```

## Error Handling Best Practices

1. **Always check connection state** before critical operations
2. **Handle IpcError::NotConnected** gracefully with user-friendly messages
3. **Log connection state changes** for debugging
4. **Implement retry logic** for transient failures
5. **Clean up resources** on disconnect (pending requests, channels)

## References

- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [IPC_PROTOCOL.md](./IPC_PROTOCOL.md) - Protocol documentation
- [src/ipc/client.rs](../src/ipc/client.rs) - Client implementation
- [src/ipc/message.rs](../src/ipc/message.rs) - Message types
- [src/session.rs](../src/session.rs) - Session manager

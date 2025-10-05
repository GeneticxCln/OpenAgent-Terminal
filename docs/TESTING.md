# Testing Strategy and Coverage

## Overview

OpenAgent-Terminal has comprehensive test coverage across IPC communication, streaming cancellation, ANSI rendering, and integration workflows.

## Test Structure

```
openagent-terminal/
├── src/
│   ├── ipc/
│   │   ├── client.rs        # IPC client implementation
│   │   ├── client_tests.rs  # IPC client tests (unit)
│   │   └── mod.rs           # Module integration
│   └── ansi.rs              # ANSI rendering (with inline tests)
└── tests/
    └── integration_tests.rs # Integration tests
```

## Test Categories

### 1. IPC Client Tests (`src/ipc/client_tests.rs`)

Tests for Unix socket communication, request/response handling, and state management.

#### Test Coverage

| Test | Purpose | Coverage |
|------|---------|----------|
| `test_client_creation` | Client initialization | State management |
| `test_successful_connection` | Socket connection | Connection handling |
| `test_connection_failure` | Error handling | Failure recovery |
| `test_request_response_cycle` | Full RPC cycle | Request/response |
| `test_notification_dispatch` | Notification handling | Event dispatch |
| `test_multiple_requests` | Concurrent requests | Scalability |
| `test_request_id_wraparound` | ID space management | Collision prevention |
| `test_disconnect` | Clean shutdown | Resource cleanup |
| `test_send_notification_to_backend` | Client→Server notifications | Bidirectional comm |
| `test_connection_state_transitions` | State machine | Connection states |
| `test_send_request_not_connected` | Error handling | Precondition checks |
| `test_initialize_request` | Protocol handshake | Initialization |
| `test_concurrent_requests` | Performance | Concurrent handling |
| `test_error_response` | Error propagation | Error handling |
| `test_malformed_response` | Robustness | Invalid input |

#### Mock Backend

Tests use a mock Unix socket backend:

```rust
async fn mock_backend(
    socket_path: PathBuf,
    handler: impl Fn(String) -> Option<String> + Send + Sync + 'static
)
```

**Features:**
- Real Unix socket simulation
- Customizable request handlers
- Async operation
- Multiple concurrent connections

**Example:**
```rust
let (socket_path, _temp_dir) = create_test_socket().await;

mock_backend(socket_path.clone(), |line| {
    let request: serde_json::Value = serde_json::from_str(&line).ok()?;
    let id = request.get("id")?;
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {"status": "ok"}
    });
    Some(response.to_string())
}).await;
```

### 2. Streaming Cancellation Tests (`tests/integration_tests.rs`)

Tests for proper cancellation handling without state leaks.

#### Test Coverage

| Test | Purpose | Validates |
|------|---------|-----------|
| `test_streaming_cancellation` | Basic cancellation | Clean exit |
| `test_cancellation_no_leak` | Resource cleanup | No leaks |
| `test_multiple_cancellations` | Robustness | Multiple signals |
| `test_cancellation_mid_stream` | Active stream cancel | Partial processing |
| `test_cancellation_before_stream` | Early cancellation | Pre-start cancel |
| `test_cancellation_multiple_receivers` | Multi-consumer | Fan-out cancellation |

#### Cancellation Pattern

```rust
let (cancel_tx, mut cancel_rx) = watch::channel(false);

tokio::spawn(async move {
    loop {
        tokio::select! {
            Ok(_) = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    break; // Clean exit
                }
            }
            // ... stream processing
        }
    }
});

// Cancel when needed
cancel_tx.send(true).unwrap();
```

**Verified Properties:**
- ✅ Clean exit (no hanging)
- ✅ No resource leaks
- ✅ Drop handlers called
- ✅ No panics
- ✅ Multiple cancellations safe

### 3. ANSI Rendering Tests

Tests for terminal output formatting with dynamic width handling.

#### Unit Tests (`src/ansi.rs`)

| Test | Coverage |
|------|----------|
| `test_highlight_rust` | Rust syntax highlighting |
| `test_highlight_python` | Python syntax |
| `test_highlight_javascript` | JavaScript syntax |
| `test_highlight_bash` | Bash/shell syntax |
| `test_unknown_language` | Fallback handling |
| `test_format_code_block` | Code block formatting |
| `test_format_diff` | Diff formatting |
| `test_ansi_colors` | Color constants |

#### Integration Tests (`tests/integration_tests.rs`)

| Test | Purpose |
|------|---------|
| `test_ansi_rendering_dynamic_width` | Multiple terminal widths |
| `test_ansi_rendering_empty` | Empty content handling |
| `test_ansi_rendering_special_chars` | Special characters (\n, \t, etc.) |
| `test_ansi_rendering_unicode` | Unicode support (emoji, CJK) |
| `test_ansi_rendering_long_lines` | Very long lines (1000+ chars) |
| `test_ansi_nesting` | Nested ANSI codes |
| `test_ansi_reset` | Reset code handling |

**Tested Widths:**
- 40 columns (narrow)
- 80 columns (standard)
- 120 columns (wide)
- 200 columns (ultra-wide)

### 4. Configuration Tests

Tests for CLI and config precedence handling.

| Test | Validates |
|------|-----------|
| `test_config_precedence` | ENV variable handling |
| `test_terminal_size_handling` | Terminal size validation |
| `test_status_info_creation` | Status info creation |

## Running Tests

### All Tests

```bash
cargo test --release
```

### Specific Test Suite

```bash
# Integration tests only
cargo test --test integration_tests --release

# ANSI tests only
cargo test --bin openagent-terminal --release -- ansi

# IPC tests only
cargo test --bin openagent-terminal --release -- ipc
```

### Specific Test

```bash
cargo test --release test_streaming_cancellation
cargo test --release test_request_response_cycle
```

### With Output

```bash
cargo test --release -- --nocapture
```

### Verbose

```bash
cargo test --release -- --show-output
```

## Test Results

### Summary

```
running 16 tests (integration_tests)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured

running 8 tests (ansi unit tests)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured

running 15 tests (ipc client tests)  
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured
```

**Total: 39+ tests, all passing** ✅

## Coverage Areas

### ✅ Covered

- [x] IPC connection establishment
- [x] Request/response cycles
- [x] Notification dispatch
- [x] Connection state management
- [x] Request ID wraparound
- [x] Clean disconnect
- [x] Error handling
- [x] Malformed responses
- [x] Streaming cancellation
- [x] Resource cleanup
- [x] Multiple cancellations
- [x] ANSI rendering (multiple widths)
- [x] Unicode handling
- [x] Special character handling
- [x] Configuration precedence

### 🔄 Future Coverage

- [ ] Timeout handling (requires configurable timeouts)
- [ ] Network partition scenarios
- [ ] Backend crash recovery
- [ ] Concurrent request stress tests
- [ ] Memory leak detection (valgrind/miri)
- [ ] Property-based testing (quickcheck)
- [ ] Fuzzing (cargo-fuzz)

## Testing Best Practices

### 1. Use Temp Directories

```rust
use tempfile::TempDir;

let temp_dir = TempDir::new().unwrap();
let socket_path = temp_dir.path().join("test.sock");
// Automatically cleaned up on drop
```

### 2. Use Tokio Test Runtime

```rust
#[tokio::test]
async fn test_async_operation() {
    // Async test code
}
```

### 3. Test Cleanup

```rust
struct CleanupGuard {
    cleanup_done: Arc<AtomicBool>,
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        self.cleanup_done.store(true, Ordering::Relaxed);
    }
}
```

### 4. Timeout Protection

```rust
let result = tokio::time::timeout(
    Duration::from_secs(1),
    async_operation()
).await;

assert!(result.is_ok(), "Operation timed out");
```

### 5. Mock External Dependencies

```rust
// Mock backend instead of real backend
mock_backend(socket_path, |req| {
    // Custom response logic
    Some(mock_response(req))
}).await;
```

## Continuous Integration

### GitHub Actions Example

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: cargo test --release
```

### Pre-commit Hook

```bash
#!/bin/sh
# .git/hooks/pre-commit

cargo test --release
if [ $? -ne 0 ]; then
    echo "Tests failed! Commit aborted."
    exit 1
fi
```

## Debugging Failed Tests

### Enable Logging

```bash
RUST_LOG=debug cargo test --release -- --nocapture
```

### Run Single Test

```bash
cargo test --release test_name -- --exact
```

### Show Full Output

```bash
cargo test --release -- --show-output test_name
```

### Use Test Binary Directly

```bash
cargo test --release --no-run
./target/release/deps/openagent_terminal-* test_name
```

## Test Maintenance

### Adding New Tests

1. Identify the feature/bug to test
2. Choose appropriate test location (unit vs integration)
3. Write test following existing patterns
4. Verify test fails without fix
5. Implement fix
6. Verify test passes

### Updating Tests

When changing code:
1. Run full test suite
2. Update affected tests
3. Add new tests for new behavior
4. Remove obsolete tests
5. Verify all tests pass

## Performance Testing

### Benchmark Template

```rust
#[cfg(test)]
mod benches {
    use super::*;
    use std::time::Instant;

    #[test]
    fn bench_request_throughput() {
        let start = Instant::now();
        
        for _ in 0..1000 {
            // Operation to benchmark
        }
        
        let elapsed = start.elapsed();
        println!("1000 operations in {:?}", elapsed);
        println!("Avg: {:?}/op", elapsed / 1000);
    }
}
```

## Summary

OpenAgent-Terminal has comprehensive test coverage ensuring:

✅ **Reliability** - IPC communication works correctly  
✅ **Safety** - No resource leaks or panics  
✅ **Correctness** - Cancellation works as expected  
✅ **Robustness** - Handles edge cases gracefully  
✅ **Quality** - ANSI rendering works across widths  
✅ **Confidence** - Changes can be made safely

All tests passing provides high confidence in the codebase quality and readiness for production use.

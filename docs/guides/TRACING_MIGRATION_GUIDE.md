# Tracing Migration Guide for OpenAgent Terminal

This document outlines the migration from `log` to `tracing` across the OpenAgent Terminal workspace to standardize telemetry and improve observability.

## Migration Status

### ✅ Completed
- **Workspace Dependencies**: Added `tracing`, `tracing-subscriber`, and `tracing-log` to workspace
- **openagent-terminal-core**: Migrated to use `tracing::*` imports instead of `log::*`
- **openagent-terminal-config**: Updated to support both `log` and `tracing` (keeps log::LevelFilter for config compatibility)
- **openagent-terminal-config-derive**: Updated dev-dependencies to use tracing
- **Modern logging system**: Created `logging_v2.rs` with structured tracing, log bridging, and message bar integration

### 🔄 Partial (Bridge Configuration)
- **openagent-terminal**: Main crate has both `log` and `tracing` dependencies for seamless bridging
- **Tracing bridge**: `tracing-log` is initialized to capture existing `log` macro calls and forward them to tracing

### 📋 Next Steps (Optional)
The current setup provides a working tracing system with backward compatibility. For full migration:

1. **Replace log macros**: Convert `log::info!()` → `tracing::info!()` throughout codebase
2. **Use structured logging**: Replace plain messages with structured fields
3. **Remove log dependencies**: After all macros are converted, remove `log` crate dependencies

## Architecture

### Current Logging Flow
```
Code using log::info!() → tracing-log bridge → tracing-subscriber → File + Stdout + Message Bar
Code using tracing::info!() → tracing-subscriber → File + Stdout + Message Bar
```

### Key Files

1. **`openagent-terminal/src/logging_v2.rs`**
   - Modern tracing-based logging system
   - Structured logging macros (log_ai_request!, log_terminal_event!, etc.)
   - Automatic log bridging with `LogTracer::init()`
   - Message bar integration for errors/warnings

2. **Workspace Cargo.toml**
   - Centralized tracing dependencies
   - `tracing-log` for bridging legacy log calls

3. **Individual Cargo.toml files**
   - Core crates use `tracing` workspace dependency
   - Config crate retains `log` for LevelFilter serialization compatibility

## Usage Examples

### Structured Logging (Recommended)

```rust
use tracing::{info, warn, error};

// Basic logging
info!("Server started on port 8080");

// Structured logging with fields
info!(
    user_id = %user_id,
    action = "login",
    duration_ms = 150,
    "User authentication successful"
);

// Using provided convenience macros
log_ai_request!("openai", "gpt-4", 1024);
log_terminal_event!("resize", &format!("{}x{}", width, height));
```

### Error Handling with Context

```rust
use tracing::error;

match dangerous_operation() {
    Ok(result) => info!("Operation completed successfully"),
    Err(e) => error!(
        error = %e,
        context = "processing user request",
        "Operation failed"
    ),
}
```

### Performance Measurement

```rust
use tracing::{info_span, Instrument};

async fn expensive_operation() -> Result<()> {
    let span = info_span!("expensive_operation", operation_type = "data_processing");
    
    // Work happens here...
    
    async move {
        // Your async work
    }
    .instrument(span)
    .await
}
```

## Benefits of This Approach

### 1. **Backward Compatibility**
- Existing `log::*` macros continue to work
- No breaking changes to existing code
- Gradual migration possible

### 2. **Enhanced Observability**
- Structured logging with key-value pairs
- Spans for tracking request/operation lifecycles  
- Better filtering and analysis capabilities

### 3. **Unified Telemetry**
- All logging goes through tracing-subscriber
- Consistent formatting across all crates
- Single configuration point for log levels and output

### 4. **AI-Specific Features**
- Dedicated AI debug logging (when `OPENAGENT_AI_DEBUG_LOG=1`)
- Built-in sensitive data redaction
- Structured AI request/response logging

## Configuration

### Environment Variables

- `RUST_LOG`: Standard tracing filter (e.g., `openagent=debug,info`)
- `OPENAGENT_AI_DEBUG_LOG=1`: Enable AI-specific debug logging
- `OPENAGENT_AI_DEBUG_LOG_PATH`: Custom path for AI debug logs
- `OPENAGENT_TERMINAL_LOG`: Set by the application to log file path

### Log Levels
- `ERROR`: Critical errors, shown in message bar
- `WARN`: Warnings, shown in message bar  
- `INFO`: General information (default for openagent crates)
- `DEBUG`: Detailed debugging information
- `TRACE`: Very verbose tracing information

## Migration Commands (For Future)

If you want to complete the migration by replacing all log macros:

```bash
# Find all log macro usage
grep -r "log::" --include="*.rs" .

# Replace common patterns (run carefully!)
find . -name "*.rs" -exec sed -i 's/use log::/use tracing::/g' {} \;
find . -name "*.rs" -exec sed -i 's/log::info!/tracing::info!/g' {} \;
find . -name "*.rs" -exec sed -i 's/log::warn!/tracing::warn!/g' {} \;
find . -name "*.rs" -exec sed -i 's/log::error!/tracing::error!/g' {} \;
find . -name "*.rs" -exec sed -i 's/log::debug!/tracing::debug!/g' {} \;
find . -name "*.rs" -exec sed -i 's/log::trace!/tracing::trace!/g' {} \;
```

## Testing

Build and test the workspace:

```bash
# Check core crates (should work)
cargo check -p openagent-terminal-core
cargo check -p openagent-terminal-config

# Full workspace build (may need features)
cargo check --workspace
cargo check --workspace --features ai
```

## Performance Impact

- **Minimal overhead**: tracing is designed for production use
- **Compile-time optimization**: Unused log levels are eliminated
- **Async-friendly**: Works well with tokio and async code
- **Memory efficient**: Structured data is handled efficiently

## Conclusion

This migration establishes a modern, structured logging foundation for OpenAgent Terminal while maintaining full backward compatibility. The tracing ecosystem provides superior observability tools and integrates seamlessly with modern Rust async applications.

The current state allows for:
- ✅ All existing code continues to work
- ✅ New code can use modern tracing features  
- ✅ Unified log output and filtering
- ✅ Enhanced debugging capabilities for AI features
- ✅ Better telemetry for production deployments

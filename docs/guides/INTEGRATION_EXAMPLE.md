# OpenAgent Terminal Logging Integration Example

This document shows how to integrate the tracing bridge for gradual migration from `log` to `tracing` while keeping the custom log sink as canonical.

## Architecture

The tracing bridge allows you to:
1. Keep the existing custom log sink with UI integration and sensitive data redaction
2. Gradually migrate modules to use tracing macros
3. Maintain backward compatibility with existing log calls
4. Benefit from structured logging without breaking changes

## Integration Steps

### 1. Initialize with Tracing Bridge

Update your initialization to use the tracing bridge:

```rust
// In main.rs or your initialization code
use crate::logging_v2;

// Option 1: Use the bridge to forward tracing to the custom log sink
let log_file = logging_v2::initialize_with_tracing_bridge(
    &options, 
    window_event_loop.create_proxy(),
    true  // Enable tracing bridge
)?;

// Option 2: Use the new tracing system directly (current behavior)  
let log_file = logging_v2::initialize(
    &options, 
    window_event_loop.create_proxy()
)?;
```

### 2. Gradual Migration Example

You can now selectively migrate modules to use tracing:

```rust
// Old logging style - still works through the bridge
log::info!("Processing user request");
log::error!("Failed to connect: {}", error);

// New structured logging style - flows through the same custom sink
use tracing::{info, error};

info!("Processing user request");
error!(error = %error, context = "connection", "Failed to connect");

// With structured fields for better observability
info!(
    user_id = %user_id,
    action = "file_open", 
    file_path = %path,
    "User opened file"
);
```

### 3. AI-Specific Logging

The bridge supports the enhanced AI logging macros:

```rust
use crate::{log_ai_request_bridge, log_ai_response_bridge};

// These will flow through the custom sink with redaction
log_ai_request_bridge!("openai", "gpt-4", 1024);
log_ai_response_bridge!("openai", "gpt-4", 2048, 1500);
```

### 4. Module Migration Strategy

You can migrate modules incrementally:

```rust
// Module A: Still using log (via bridge)
mod legacy_module {
    use log::{info, warn};
    
    pub fn process_data() {
        info!("Processing started");
        warn!("Low memory");
    }
}

// Module B: Migrated to tracing (via bridge to same sink)
mod modern_module {
    use tracing::{info, warn};
    
    pub fn process_data() {
        info!(component = "data_processor", "Processing started");
        warn!(memory_usage = %memory_pct, "Low memory");
    }
}
```

## Configuration

### Environment Variables

- `RUST_LOG`: Controls tracing filter (e.g., `openagent=debug,info`)  
- `OPENAGENT_AI_DEBUG_LOG=1`: Enable AI-specific debug logging
- `OPENAGENT_AI_DEBUG_LOG_PATH`: Custom path for AI debug logs

### Benefits of Bridge Approach

1. **Zero Breaking Changes**: All existing `log::` calls continue to work
2. **Unified Output**: Everything flows through the custom sink with:
   - Sensitive data redaction
   - UI message bar integration  
   - Consistent formatting
   - File and stdout output
3. **Progressive Enhancement**: Migrate modules at your own pace
4. **Better Observability**: New code can use structured logging
5. **Performance**: Tracing overhead is minimal, levels are compile-time filtered

## Testing

Test both old and new logging styles:

```bash
# Set debug level
export RUST_LOG="openagent_terminal=debug,openagent_terminal_ai=trace"

# Enable AI debug logging  
export OPENAGENT_AI_DEBUG_LOG=1

# Run the application
cargo run
```

Both `log::info!()` and `tracing::info!()` calls will appear in the same output files and UI message bar.

## Migration Timeline

**Phase 1** (Current): Bridge is available, optional usage
- No changes to existing code required
- New modules can optionally use tracing macros

**Phase 2** (Future): Selective migration
- Convert high-value modules (AI, core functionality) to structured logging
- Keep using bridge to maintain unified output

**Phase 3** (Optional): Full migration
- Convert all `log::` calls to `tracing::` calls
- Remove log dependency, keep only tracing
- Preserve all existing functionality through the bridge

The bridge architecture ensures that Phase 3 is optional and can be deferred indefinitely while still gaining the benefits of structured logging in new code.

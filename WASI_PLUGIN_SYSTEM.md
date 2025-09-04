# WASI Plugin System for OpenAgent Terminal

This document describes the enhanced WebAssembly System Interface (WASI) plugin system implemented for OpenAgent Terminal, providing a secure, ergonomic, and performant way to extend the terminal's functionality.

## Overview

The WASI plugin system includes:

1. **Wasmtime Linker Integration**: Proper WASI function wiring with host function exposure
2. **JSON-over-Memory ABI**: Clean communication protocol between host and plugins
3. **Plugin SDK**: Ergonomic Rust SDK with macros for easy plugin development
4. **Enhanced TOML Manifests**: Comprehensive permission and metadata system
5. **Security Enforcement**: Rigorous permission validation and sandboxing

## Architecture

### Core Components

```
┌─────────────────────────────────────────┐
│              Host (Terminal)             │
├─────────────────────────────────────────┤
│         Plugin Loader                   │
│  ┌─────────────────────────────────────┐ │
│  │        Wasmtime Linker             │ │
│  │  ┌─────────────────────────────────┐│ │
│  │  │         WASI Context           ││ │
│  │  └─────────────────────────────────┘│ │
│  │  ┌─────────────────────────────────┐│ │
│  │  │       Host Functions           ││ │
│  │  │  • host_log()                  ││ │
│  │  │  • host_read_file()            ││ │
│  │  │  • host_write_file()           ││ │
│  │  │  • host_execute_command()      ││ │
│  │  └─────────────────────────────────┘│ │
│  └─────────────────────────────────────┘ │
└─────────────────────────────────────────┘
                     │
            JSON-over-Memory ABI
                     │
┌─────────────────────────────────────────┐
│           WASM Plugin                   │
├─────────────────────────────────────────┤
│           Plugin SDK                    │
│  ┌─────────────────────────────────────┐ │
│  │        Safe Wrappers               │ │
│  │  • log()                          │ │
│  │  • read_file()                    │ │
│  │  • write_file()                   │ │
│  │  • execute_command()              │ │
│  └─────────────────────────────────────┘ │
│  ┌─────────────────────────────────────┐ │
│  │        Plugin Macros               │ │
│  │  • define_plugin!()               │ │
│  │  • simple_plugin!()               │ │
│  └─────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

### JSON-over-Memory ABI

The communication protocol between host and plugins uses JSON serialization over WebAssembly linear memory:

#### Plugin Exports
- `plugin_init() -> i32`: Initialize plugin, returns error code
- `plugin_get_metadata() -> i64`: Returns packed pointer/length for JSON metadata
- `plugin_handle_event(ptr: i32, len: i32) -> i32`: Process event data, returns error code
- `plugin_cleanup() -> i32`: Cleanup plugin resources, returns error code

#### Host Imports
- `host_log(level: i32, ptr: *const u8, len: usize)`: Log message to terminal
- `host_read_file(path_ptr: *const u8, path_len: usize, result_ptr: *mut u8, result_len_ptr: *mut u32) -> i32`: Read file
- `host_write_file(path_ptr: *const u8, path_len: usize, data_ptr: *const u8, data_len: usize) -> i32`: Write file
- `host_execute_command(cmd_ptr: *const u8, cmd_len: usize) -> i32`: Execute command

#### Memory Layout
```
WASM Linear Memory:
┌─────────────────────────────────────────┐
│              Plugin Code                │
├─────────────────────────────────────────┤
│             Plugin Data                 │
├─────────────────────────────────────────┤
│         JSON Metadata Buffer            │ ← Returned by plugin_get_metadata()
├─────────────────────────────────────────┤
│         Event Data Buffer               │ ← Passed to plugin_handle_event()
└─────────────────────────────────────────┘
```

## Plugin SDK Usage

### Basic Plugin

```rust
use plugin_sdk::simple_plugin;

// Minimal plugin with default implementations
simple_plugin!(
    "my-plugin",
    "1.0.0",
    "Author Name",
    "Plugin description"
);
```

### Advanced Plugin

```rust
use plugin_sdk::{define_plugin, log, LogLevel, PluginError};

fn init_plugin() -> Result<(), PluginError> {
    log(LogLevel::Info, "Plugin initializing...");
    Ok(())
}

fn handle_event(ptr: i32, len: i32) -> Result<(), PluginError> {
    log(LogLevel::Info, &format!("Got event: ptr={}, len={}", ptr, len));
    Ok(())
}

fn cleanup_plugin() -> Result<(), PluginError> {
    log(LogLevel::Info, "Plugin shutting down...");
    Ok(())
}

define_plugin! {
    name: "advanced-plugin",
    version: "2.0.0",
    author: "Plugin Developer",
    description: "An advanced plugin with custom logic",
    capabilities: {
        completions: true,
        context_provider: true,
        hooks: vec![]
    },
    permissions: {
        read_files: vec!["config/**/*.toml".to_string()],
        write_files: vec!["logs/*.log".to_string()],
        network: false,
        execute_commands: true,
        environment_variables: vec!["PLUGIN_CONFIG".to_string()]
    },
    init: init_plugin,
    event_handler: handle_event,
    cleanup: cleanup_plugin,
}
```

### Host Function Usage

```rust
use plugin_sdk::{log, LogLevel, read_file, write_file, execute_command};

// Safe logging
log(LogLevel::Info, "Plugin is working!");

// Safe file operations
match read_file("config.toml") {
    Ok(data) => {
        log(LogLevel::Info, &format!("Read {} bytes", data.len()));
        // Process config data...

        // Write processed data
        if let Err(e) = write_file("output.json", b"{\"status\": \"ok\"}") {
            log(LogLevel::Error, &format!("Write failed: {:?}", e));
        }
    },
    Err(e) => log(LogLevel::Error, &format!("Read failed: {:?}", e)),
}

// Safe command execution (if permitted)
match execute_command("git status") {
    Ok(output) => log(LogLevel::Info, &format!("Command output: {}", output.stdout)),
    Err(e) => log(LogLevel::Error, &format!("Command failed: {:?}", e)),
}
```

## TOML Manifest Format

### Enhanced Manifest Structure

```toml
[plugin]
name = "my-plugin"
version = "1.0.0"
author = "Plugin Developer"
description = "A sample plugin"
license = "MIT"

[plugin.capabilities]
completions = false
context_provider = true
commands = ["custom-cmd"]
hooks = ["PreCommand", "PostCommand"]
file_associations = ["*.toml"]

[permissions]
# Environment variables the plugin can access
environment_variables = ["PLUGIN_CONFIG", "DEBUG_LEVEL"]

# File system permissions (relative to plugin directory)
read_files = ["config/**/*.toml", "data/*.json"]
write_files = ["logs/*.log", "cache/*.tmp"]

# Network and command execution
network = false
execute_commands = true

# Resource limits
max_memory_mb = 100
timeout_ms = 10000

[plugin.metadata]
tags = ["utility", "git", "productivity"]
required_host_version = ">=0.3.0"
```

### Permission Validation

The system performs strict validation:

#### Memory Limits
- Maximum: 200 MB
- Minimum: > 0 MB

#### Timeout Limits
- Maximum: 30 seconds
- Minimum: > 0 ms

#### File Access Patterns
**Blocked patterns:**
- System directories: `/etc/`, `/sys/`, `/proc/`, `/dev/`
- Binary directories: `/bin/`, `/sbin/`, `/usr/bin/`
- Sensitive files: `passwd`, `shadow`, `sudoers`
- Path traversal: `../`, `/.`

**Allowed patterns:**
- Relative paths within plugin directory
- Explicit safe directories configured by manifest

#### Environment Variables
**Sensitive prefixes (warned):**
- `AWS_*`, `GCP_*`, `AZURE_*`
- `SECRET_*`, `TOKEN_*`, `KEY_*`
- `PASSWORD_*`, `PASS_*`
- `SSH_*`, `GPG_*`

**Sensitive exact matches (warned):**
- `HOME`, `USER`, `USERNAME`
- `PATH`, `LD_LIBRARY_PATH`
- `SUDO_USER`, `LOGNAME`

## Building Plugins

### Prerequisites

```bash
# Install WASI target
rustup target add wasm32-wasi

# Verify installation
rustc --print target-list | grep wasi
```

### Build Process

1. **Create plugin project:**
```bash
cargo new --lib my-plugin
cd my-plugin
```

2. **Update Cargo.toml:**
```toml
[package]
name = "my-plugin"
version = "1.0.0"
edition = "2021"

[dependencies]
plugin-sdk = { path = "../path/to/plugin-sdk" }

[lib]
crate-type = ["cdylib"]

# Optimize for WASM
[profile.release]
opt-level = "s"  # Optimize for size
lto = true
debug = false
panic = "abort"
```

3. **Build for WASI:**
```bash
cargo build --release --target wasm32-wasi
```

4. **Deploy plugin:**
```bash
cp target/wasm32-wasi/release/my_plugin.wasm ~/.config/openagent-terminal/plugins/
cp my-plugin.toml ~/.config/openagent-terminal/plugins/
```

## Security Model

### Sandboxing Layers

1. **WASM Sandbox**: Memory isolation, no direct system access
2. **WASI Restrictions**: Limited to preopened directories and approved syscalls
3. **Permission System**: Fine-grained capability control via TOML manifest
4. **Resource Limits**: CPU time, memory, and timeout enforcement
5. **Pattern Validation**: Dangerous file/env patterns blocked at load time

### Permission Enforcement

```rust
// Example permission check in plugin loader
fn check_file_access(&self, path: &str, permissions: &PluginPermissions) -> bool {
    // 1. Check against dangerous pattern list
    if self.is_dangerous_file_pattern(path) {
        return false;
    }

    // 2. Check against plugin's allowed patterns
    permissions.read_files.iter().any(|pattern| {
        self.matches_pattern(path, pattern)
    })
}
```

### Security Guarantees

- **No arbitrary file access**: Only preopened directories accessible
- **No network by default**: Explicit permission required
- **No command injection**: Commands executed through controlled interface
- **Resource bounded**: Memory and CPU time limits enforced
- **Audit trail**: All plugin operations logged

## Testing

### Running Tests

```bash
# Test the plugin loader
cd crates/plugin-loader
cargo test

# Test the plugin SDK
cd crates/plugin-sdk
cargo test

# Test example plugins
cd examples/plugins/hello-wasi
cargo test --target wasm32-wasi
```

### Test Coverage

- ✅ Enhanced manifest parsing and validation
- ✅ Permission enforcement (file, memory, timeout)
- ✅ Dangerous pattern detection
- ✅ Plugin loading/unloading lifecycle
- ✅ Resource limit enforcement
- ✅ WASI integration with Linker
- ✅ JSON-over-memory ABI
- ✅ Host function call validation

## Examples

### Hello WASI Plugin

Located in `examples/plugins/hello-wasi/`, demonstrates:
- SDK usage with `define_plugin!` macro
- Environment variable access
- File system permission testing
- Host function integration
- Error handling

### Simple Demo Plugin

Located in `examples/plugins/simple-demo/`, demonstrates:
- Minimal plugin with `simple_plugin!` macro
- Basic plugin lifecycle
- Default implementations

## Performance Considerations

### WASM Optimizations
- Size optimization (`opt-level = "s"`)
- Link-time optimization (LTO) enabled
- Debug info stripped for release builds
- Panic handling optimized (`panic = "abort"`)

### Runtime Optimizations
- Epoch-based CPU limiting (2ms granularity)
- Resource tracking and enforcement
- Plugin instance pooling
- JSON serialization caching

### Memory Management
- Linear memory limits enforced
- Automatic cleanup on unload
- Stack depth tracking
- Memory growth validation

## Troubleshooting

### Common Issues

1. **Plugin won't load:**
   - Check TOML manifest syntax
   - Verify permission limits are within bounds
   - Ensure WASM file is built for `wasm32-wasi` target

2. **Permission denied errors:**
   - Review file path patterns in manifest
   - Check for dangerous path patterns
   - Verify environment variable names

3. **Build failures:**
   - Ensure `wasm32-wasi` target installed
   - Check plugin-sdk dependency path
   - Verify crate type is `["cdylib"]`

### Debug Tips

```rust
// Enable debug logging in plugins
log(LogLevel::Debug, "Debug message here");

// Check plugin manifest parsing
RUST_LOG=debug cargo run -- --list-plugins

// Validate permissions
RUST_LOG=plugin_loader=trace cargo test test_permissions
```

## Future Enhancements

- **Plugin Marketplace**: Central registry for plugin discovery
- **Hot Reloading**: Update plugins without terminal restart
- **Inter-Plugin Communication**: Secure message passing between plugins
- **Advanced Networking**: HTTP client with domain restrictions
- **Plugin Signing**: Cryptographic verification of plugin authenticity
- **Performance Profiling**: Built-in metrics collection for plugins

## Contributing

When contributing to the plugin system:

1. Maintain backward compatibility in the ABI
2. Add comprehensive tests for security features
3. Document permission changes in CHANGELOG
4. Validate against existing example plugins
5. Consider performance impact of new features

## License

The WASI plugin system is licensed under the same terms as OpenAgent Terminal (Apache 2.0).

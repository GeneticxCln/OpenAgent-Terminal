# ADR 003: Plugin System Architecture

## Status
Accepted (API stabilized for v1.0: minimal, WASM-only surface with stable ABI v1.0.0)

## Date
2024-08-30

## Context

OpenAgent Terminal needs an extensible plugin system to allow community contributions and customizations without modifying the core codebase. The key requirements are:

1. **Safety**: Plugins should not compromise terminal security or stability
2. **Performance**: Plugins should have minimal impact on terminal performance
3. **Simplicity**: Easy to develop, distribute, and install plugins
4. **Flexibility**: Support various plugin types (AI providers, commands, themes)
5. **Isolation**: Plugins should be sandboxed from each other and the core

## Decision

We will implement a **WebAssembly (WASM)-based plugin system** with the following architecture:

### 1. Plugin Interface

```rust
// Plugin trait that all plugins must implement
pub trait TerminalPlugin: Send + Sync {
    /// Plugin metadata
    fn metadata(&self) -> PluginMetadata;

    /// Called when plugin is loaded
    fn on_load(&mut self, context: PluginContext) -> Result<(), PluginError>;

    /// Called when plugin is unloaded
    fn on_unload(&mut self) -> Result<(), PluginError>;

    /// Handle events from the terminal
    fn on_event(&mut self, event: PluginEvent) -> Option<PluginResponse>;
}

pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub capabilities: Vec<PluginCapability>,
}
```

### 2. Plugin Types

#### AI Provider Plugins
```rust
pub trait AiProviderPlugin: TerminalPlugin {
    fn propose(&self, request: AiRequest) -> Result<Vec<AiProposal>, String>;
    fn stream_propose(&self, request: AiRequest) -> BoxStream<AiProposal>;
}
```

#### Command Processor Plugins
```rust
pub trait CommandPlugin: TerminalPlugin {
    fn pre_process(&self, command: &str) -> Option<String>;
    fn post_process(&self, command: &str, output: &str) -> Option<String>;
    fn register_commands(&self) -> Vec<CustomCommand>;
}
```

#### Output Formatter Plugins
```rust
pub trait FormatterPlugin: TerminalPlugin {
    fn format(&self, content: &str, content_type: &str) -> Option<FormattedOutput>;
    fn supported_types(&self) -> Vec<String>;
}
```

#### Theme Engine Plugins
```rust
pub trait ThemePlugin: TerminalPlugin {
    fn get_theme(&self) -> Theme;
    fn supports_dynamic(&self) -> bool;
    fn on_system_theme_change(&mut self, is_dark: bool);
}
```

### 3. WebAssembly Runtime

Use `wasmtime` for WASM execution:

```rust
pub struct PluginRuntime {
    engine: Engine,
    plugins: HashMap<String, LoadedPlugin>,
    host_functions: HostFunctions,
}

struct LoadedPlugin {
    instance: Instance,
    store: Store<PluginState>,
    metadata: PluginMetadata,
}
```

**Why WebAssembly?**
- Language agnostic (Rust, Go, C++, AssemblyScript)
- Sandboxed execution environment
- Near-native performance
- Small binary size
- No direct system access

### 4. Plugin Manifest

```toml
[plugin]
name = "my-ai-provider"
version = "1.0.0"
author = "Developer Name"
description = "Custom AI provider for specific use case"
license = "MIT"

[plugin.capabilities]
types = ["ai_provider"]
permissions = ["network", "env:CUSTOM_API_KEY"]

[plugin.dependencies]
openagent-terminal = "^0.3.0"

[plugin.files]
wasm = "target/wasm32-wasi/release/my_plugin.wasm"
assets = ["icons/", "themes/"]
```

### 5. Plugin Loading & Discovery

```rust
pub struct PluginManager {
    runtime: PluginRuntime,
    registry: PluginRegistry,
    loaded: Vec<PluginHandle>,
}

impl PluginManager {
    /// Load plugin from file
    pub fn load_plugin(&mut self, path: &Path) -> Result<PluginHandle, PluginError>;

    /// Discover plugins in standard directories
    pub fn discover_plugins(&mut self) -> Vec<PluginMetadata>;

    /// Enable/disable plugin
    pub fn set_enabled(&mut self, plugin_id: &str, enabled: bool);
}
```

Plugin directories:
- System: `/usr/share/openagent-terminal/plugins/`
- User: `~/.config/openagent-terminal/plugins/`
- Project: `./plugins/`

### 6. Security Model

#### Capability-Based Permissions
```rust
pub enum PluginCapability {
    Network(Vec<String>),      // Allowed domains
    FileRead(Vec<PathBuf>),     // Allowed paths
    FileWrite(Vec<PathBuf>),    // Allowed paths
    Environment(Vec<String>),   // Allowed env vars
    Terminal,                   // Terminal I/O access
    Clipboard,                  // Clipboard access
}
```

#### Resource Limits
```rust
pub struct PluginLimits {
    pub max_memory: usize,      // Max WASM memory (MB)
    pub max_cpu_time: Duration, // Max execution time
    pub max_stack_depth: usize, // Max call stack depth
    pub rate_limits: RateLimits,
}
```

### 7. Plugin Communication

#### Host Functions (Terminal → Plugin)
```rust
// Functions exposed to plugins
extern "C" {
    fn host_log(level: u32, message: *const u8, len: usize);
    fn host_get_config(key: *const u8, key_len: usize) -> *const u8;
    fn host_emit_event(event: *const u8, len: usize);
    fn host_register_command(cmd: *const u8, len: usize);
}
```

#### Plugin Exports (Plugin → Terminal)
```rust
// Functions plugins must export
#[no_mangle]
pub extern "C" fn plugin_init() -> i32;

#[no_mangle]
pub extern "C" fn plugin_on_event(event: *const u8, len: usize) -> i32;

#[no_mangle]
pub extern "C" fn plugin_get_metadata() -> *const u8;
```

### 8. Persistent Plugin Storage (Preview)

Provide per-plugin, namespaced key-value storage "slots" for persisting configuration and small state across sessions.

Rationale
- Plugins often need to remember configuration, small caches, and user preferences without writing arbitrary files.
- Storage must be safe, sandboxed, quota-limited, and isolated per plugin.

Capability (extends the capability model)
```rust
pub enum PluginCapability {
    Network(Vec<String>),      // Allowed domains
    FileRead(Vec<PathBuf>),     // Allowed paths
    FileWrite(Vec<PathBuf>),    // Allowed paths
    Environment(Vec<String>),   // Allowed env vars
    Terminal,                   // Terminal I/O access
    Clipboard,                  // Clipboard access
    Storage(StorageQuota),      // New: per-plugin storage with quotas
}

pub struct StorageQuota {
    pub max_total_bytes: u64,  // default ~50 MiB per plugin (configurable)
    pub max_value_bytes: u32,  // default ~256 KiB per value
    pub max_keys: u32,         // default 10_000 keys
}
```

Manifest (permissions)
```toml
[plugin.capabilities]
types = ["ai_provider"]
permissions = ["storage"]
```

Host functions (WASM C-ABI)
```rust
extern "C" {
    /// Store a value under a key in the calling plugin's namespace.
    /// Returns 0 on success; non-zero error codes on failure (quota exceeded, invalid args, etc.).
    fn host_store_data(key_ptr: *const u8, key_len: usize, val_ptr: *const u8, val_len: usize) -> i32;

    /// Retrieve a value by key. Returns number of bytes written to out_ptr (>=0),
    /// or a negative error code. Call with out_ptr = null, out_capacity = 0 to query length.
    fn host_retrieve_data(key_ptr: *const u8, key_len: usize, out_ptr: *mut u8, out_capacity: usize) -> isize;

    /// Delete a key. Returns 1 if deleted, 0 if not found, negative on error.
    fn host_delete_data(key_ptr: *const u8, key_len: usize) -> i32;

    /// List keys as a JSON-encoded UTF-8 array string in WASM memory.
    /// Ownership/lifetime is documented in the SDK; return is null on error.
    fn host_list_keys(prefix_ptr: *const u8, prefix_len: usize) -> *const u8;
}
```

Semantics
- Keys are UTF-8 strings up to 256 bytes; values are arbitrary bytes up to max_value_bytes.
- Namespacing by plugin_id is enforced by the host; plugins can only access their own keys.
- Operations are synchronous and subject to quotas and rate limits.
- Large payloads should be chunked by the plugin if needed.

Storage backend mapping (default SQLite)
```sql
-- Per-plugin KV store
CREATE TABLE IF NOT EXISTS plugin_data (
    plugin_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (plugin_id, key)
);
```

Security & privacy
- Quotas prevent resource exhaustion; configurable via host settings.
- Sensitive data may be encrypted at rest when enabled (see storage.encrypt_sensitive_data).
- No network egress occurs from storage operations; data remains local unless a plugin explicitly sends it.

Example (plugin-side, minimal)
```rust
let key = "settings/theme";
let value = b"dark";
unsafe {
    let rc = host_store_data(key.as_ptr(), key.len(), value.as_ptr(), value.len());
    assert_eq!(rc, 0);
}
```

Preview status
- Marked Preview for GA; names and quotas may evolve. SDK helpers will wrap the raw host calls where available.

## Consequences

### Positive

1. **Safety**: WASM sandbox prevents malicious code execution
2. **Performance**: Near-native speed with JIT compilation
3. **Language Freedom**: Plugins in any WASM-compilable language
4. **Hot Reload**: Plugins can be loaded/unloaded at runtime
5. **Distribution**: Single WASM file, easy to share
6. **Debugging**: Good tooling for WASM debugging

### Negative

1. **Complexity**: WASM runtime adds complexity
2. **Size**: WASM runtime increases binary size (~5MB)
3. **Learning Curve**: Developers need to understand WASM
4. **Limitations**: Some system operations not possible

### Neutral

1. **Ecosystem**: Growing but still maturing
2. **Performance**: Slight overhead vs native code
3. **Tooling**: Requires specific build tools

## Implementation Phases

### Phase 1: Core Infrastructure
1. Integrate wasmtime runtime
2. Define plugin traits and interfaces
3. Implement plugin loader
4. Create plugin manager

### Phase 2: Basic Plugins
1. Example AI provider plugin
2. Example command processor
3. Example formatter plugin
4. Plugin development template

### Phase 3: Plugin Registry
1. Plugin discovery mechanism
2. Plugin installation CLI
3. Plugin marketplace/registry
4. Dependency resolution

### Phase 4: Advanced Features
1. Plugin composition/chaining
2. Inter-plugin communication
3. Plugin update mechanism
4. Plugin signing/verification

## Alternative Approaches Considered

### 1. Dynamic Libraries (.so/.dll)
**Rejected**: No sandboxing, platform-specific, security risks

### 2. Lua/Python Embedding
**Rejected**: Language lock-in, harder to sandbox, performance concerns

### 3. JavaScript/V8
**Rejected**: Large runtime, memory overhead, not ideal for systems programming

### 4. RPC/Subprocess
**Rejected**: Higher latency, complex IPC, resource overhead

### 5. Rust Procedural Macros
**Rejected**: Compile-time only, no runtime loading, requires Rust knowledge

## Security Considerations

1. **Code Signing**: Optionally require signed plugins
2. **Capability Review**: Show permissions before install
3. **Resource Monitoring**: Track plugin resource usage
4. **Audit Logging**: Log all plugin operations
5. **Sandboxing**: WASM sandbox + additional restrictions
6. **Update Security**: Verify updates are from same author

## Plugin Development Experience

### SDK and Tools
```bash
# Install plugin development tools
cargo install openagent-terminal-plugin-sdk

# Create new plugin project
openagent-plugin new my-plugin --type ai-provider

# Build plugin
openagent-plugin build

# Test plugin locally
openagent-plugin test

# Package for distribution
openagent-plugin package
```

### Example Plugin
```rust
use openagent_terminal_plugin::prelude::*;

#[derive(Default)]
struct MyPlugin {
    config: PluginConfig,
}

#[plugin]
impl TerminalPlugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "My Plugin".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            author: "Me".into(),
            description: "Does amazing things".into(),
            capabilities: vec![],
        }
    }

    fn on_event(&mut self, event: PluginEvent) -> Option<PluginResponse> {
        match event {
            PluginEvent::Command(cmd) => {
                Some(PluginResponse::Text(format!("Processed: {}", cmd)))
            }
            _ => None,
        }
    }
}
```

## Testing Strategy

1. **Unit Tests**: Test plugin interfaces
2. **Integration Tests**: Test plugin loading/execution
3. **Security Tests**: Attempt sandbox escapes
4. **Performance Tests**: Measure plugin overhead
5. **Compatibility Tests**: Test across platforms

## Migration Path

For existing functionality:
1. Extract to plugin interfaces
2. Provide built-in plugin implementations
3. Allow override via external plugins
4. Gradual migration of features

## References

- [WebAssembly Specification](https://webassembly.org/specs/)
- [Wasmtime Documentation](https://wasmtime.dev/)
- [WASI Specification](https://wasi.dev/)
- [Plugin Architecture Patterns](https://www.martinfowler.com/articles/injection.html)

## Sign-off

- Architecture Team: Pending
- Security Team: Pending
- Product Team: Pending

---

*This ADR documents the plugin system architecture for OpenAgent Terminal, enabling safe and performant extensibility.*

*Last Modified: 2024-08-30*
*Author: OpenAgent Terminal Team*

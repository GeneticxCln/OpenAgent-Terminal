# Plugin Host API Quickstart

This guide shows how to call the terminal host APIs from a WASM plugin using the plugin SDK.

What you can do
- Log messages to the host (host_log)
- Read and write files via the host (enforced permissions)
- Execute host-side commands (enforced permissions)
- Store and retrieve small data (per-plugin storage)

Prerequisites
- Rust toolchain with wasm32-wasi target: rustup target add wasm32-wasi
- Add dependency in your plugin Cargo.toml:
  plugin-sdk = { path = "../../../crates/plugin-sdk" }

Declaring permissions
Plugins must request permissions in a TOML manifest next to the .wasm (same basename):

```toml path=null start=null
[plugin]
name = "hello-wasi"
version = "1.0.0"

[permissions]
# Environment variables the plugin may access
environment_variables = ["HELLO_PLUGIN_MESSAGE"]

# File access is relative to the plugin directory unless absolute sanitized paths are granted
read_files = ["plugin-test.txt"]
write_files = []

# Sensitive operations
network = false
execute_commands = false

# Resource limits (host-enforced)
max_memory_mb = 50
timeout_ms = 5000
```

Logging from a plugin

```rust path=null start=null
use plugin_sdk::{log, LogLevel};

pub fn init() {
    log(LogLevel::Info, "Plugin started");
}
```

Reading a file (permission enforced)

```rust path=null start=null
use plugin_sdk::{log, LogLevel, read_file, PluginError};

fn try_read() -> Result<(), PluginError> {
    match read_file("plugin-test.txt") {
        Ok(bytes) => log(LogLevel::Info, &format!("Read {} bytes", bytes.len())),
        Err(e) => log(LogLevel::Warning, &format!("Read failed: {:?}", e)),
    }
    Ok(())
}
```

Writing a file (permission enforced)

```rust path=null start=null
use plugin_sdk::{log, LogLevel, write_file, PluginError};

fn try_write() -> Result<(), PluginError> {
    let data = b"hello";
    if let Err(e) = write_file("out.txt", data) {
        log(LogLevel::Warning, &format!("Write failed: {:?}", e));
    }
    Ok(())
}
```

Executing a command (permission enforced)

```rust path=null start=null
use plugin_sdk::{execute_command, PluginError};

fn run_ls() -> Result<(), PluginError> {
    // Requires execute_commands = true in permissions
    let out = execute_command("echo hello from host")?;
    // out: CommandOutput { stdout, stderr, exit_code, execution_time_ms }
    Ok(())
}
```

Using per-plugin storage

```rust path=null start=null
use plugin_sdk::{store_data, retrieve_data, PluginError};

fn store_and_get() -> Result<(), PluginError> {
    store_data("settings/theme", b"dark")?;
    if let Some(bytes) = retrieve_data("settings/theme")? {
        // bytes == b"dark"
    }
    Ok(())
}
```

End-to-end minimal plugin skeleton

```rust path=null start=null
use plugin_sdk::{define_plugin, log, LogLevel, PluginError};

fn init_plugin() -> Result<(), PluginError> {
    log(LogLevel::Info, "init");
    Ok(())
}

fn cleanup_plugin() -> Result<(), PluginError> {
    log(LogLevel::Info, "cleanup");
    Ok(())
}

define_plugin! {
    name: "my-plugin",
    version: "0.1.0",
    author: "you",
    description: "example",
    permissions: {
        read_files: vec!["plugin-test.txt".to_string()],
        write_files: vec![],
        network: false,
        execute_commands: false,
        environment_variables: vec!["HELLO_PLUGIN_MESSAGE".to_string()],
    },
    init: init_plugin,
    cleanup: cleanup_plugin,
}
```

Building the plugin

```bash
rustup target add wasm32-wasi
cargo build --release --target wasm32-wasi
```

Installing for local testing
- Copy the .wasm and .toml manifest to your plugin directory:
  - User: ~/.config/openagent-terminal/plugins/
  - Project: ./plugins/

Troubleshooting
- Permission denied: ensure the path you read/write matches the sanitized path granted in permissions.
- Command not permitted: set execute_commands = true and ensure your host configuration allows it.
- Host not available: ensure OpenAgent Terminal is built with the plugins feature and the plugin runtime is enabled.

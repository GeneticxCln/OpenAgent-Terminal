//! Minimal WASI plugin demonstrating permission enforcement and the new SDK
//! Build: rustup target add wasm32-wasi && cargo build --release --target wasm32-wasi

use plugin_sdk::{define_plugin, log, LogLevel, PluginError};

/// Plugin initialization function
fn init_plugin() -> Result<(), PluginError> {
    log(LogLevel::Info, "Hello WASI plugin starting up!");
    
    // Test environment variable access
    let hello = std::env::var("HELLO_PLUGIN_MESSAGE")
        .unwrap_or_else(|_| "<missing>".to_string());
    log(LogLevel::Info, &format!("HELLO_PLUGIN_MESSAGE={}", hello));
    
    // Test for forbidden environment variable (should not be visible)
    let forbidden = std::env::var("FORBIDDEN_SECRET")
        .unwrap_or_else(|_| "<not available>".to_string());
    log(LogLevel::Info, &format!("FORBIDDEN_SECRET={}", forbidden));
    
    // Test file access (should be denied for system files)
    match std::fs::read_to_string("/etc/passwd") {
        Ok(_) => log(LogLevel::Warning, "/etc/passwd: unexpectedly readable!"),
        Err(_) => log(LogLevel::Info, "/etc/passwd: access denied (expected)"),
    }
    
    // Test SDK host function wrappers
    match plugin_sdk::read_file("plugin-test.txt") {
        Ok(data) => log(LogLevel::Info, &format!("Read {} bytes from plugin-test.txt", data.len())),
        Err(e) => log(LogLevel::Info, &format!("Failed to read plugin-test.txt: {:?}", e)),
    }
    
    log(LogLevel::Info, "Hello WASI plugin initialized successfully!");
    Ok(())
}

/// Plugin event handler
fn handle_event(ptr: i32, len: i32) -> Result<(), PluginError> {
    log(LogLevel::Info, &format!("Handling event: ptr={}, len={}", ptr, len));
    // In a real implementation, we would:
    // 1. Read JSON event data from WASM memory at ptr/len
    // 2. Deserialize and process the event
    // 3. Generate and return a response
    Ok(())
}

/// Plugin cleanup function
fn cleanup_plugin() -> Result<(), PluginError> {
    log(LogLevel::Info, "Hello WASI plugin shutting down!");
    Ok(())
}

// Define the plugin using our ergonomic macro
define_plugin! {
    name: "hello-wasi",
    version: "1.0.0",
    author: "OpenAgent Terminal Team",
    description: "A minimal WASI plugin demonstrating SDK usage and permission enforcement",
    capabilities: {
        completions: false,
        context_provider: true,
        hooks: vec![]
    },
    permissions: {
        read_files: vec!["plugin-test.txt".to_string()],
        write_files: vec![],
        network: false,
        execute_commands: false,
        environment_variables: vec!["HELLO_PLUGIN_MESSAGE".to_string()]
    },
    init: init_plugin,
    event_handler: handle_event,
    cleanup: cleanup_plugin,
}


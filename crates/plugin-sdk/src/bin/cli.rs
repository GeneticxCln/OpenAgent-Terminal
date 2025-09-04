/*!
 * Plugin SDK Command Line Tool
 *
 * This tool helps developers create and manage OpenAgent Terminal plugins.
 */

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help();
        return;
    }

    match args[1].as_str() {
        "new" => {
            if args.len() < 3 {
                eprintln!("Usage: plugin-sdk-cli new <plugin-name>");
                return;
            }
            create_new_plugin(&args[2]);
        },
        "build" => {
            build_plugin();
        },
        "help" | "--help" | "-h" => {
            print_help();
        },
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_help();
        },
    }
}

fn print_help() {
    println!("OpenAgent Terminal Plugin SDK CLI");
    println!();
    println!("USAGE:");
    println!("    plugin-sdk-cli <COMMAND>");
    println!();
    println!("COMMANDS:");
    println!("    new <name>    Create a new plugin project");
    println!("    build         Build the current plugin");
    println!("    help          Show this help message");
}

fn create_new_plugin(name: &str) {
    let plugin_dir = Path::new(name);

    if plugin_dir.exists() {
        eprintln!("Directory '{}' already exists", name);
        return;
    }

    // Create plugin directory structure
    fs::create_dir_all(plugin_dir.join("src")).unwrap_or_else(|err| {
        eprintln!("Failed to create plugin directory: {}", err);
        std::process::exit(1);
    });

    // Create Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
plugin-sdk = {{ path = "../../../crates/plugin-sdk" }}
plugin-api = {{ path = "../../../crates/plugin-api" }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"

[profile.release]
opt-level = "s"  # Optimize for size
strip = true
"#,
        name
    );

    fs::write(plugin_dir.join("Cargo.toml"), cargo_toml).unwrap_or_else(|err| {
        eprintln!("Failed to create Cargo.toml: {}", err);
        std::process::exit(1);
    });

    // Create lib.rs
    let lib_rs = r#"use plugin_sdk::{Plugin, PluginResult, PluginMetadata, PluginEvent};

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "A sample OpenAgent Terminal plugin".to_string(),
            author: "Plugin Developer".to_string(),
        }
    }

    fn initialize(&mut self) -> PluginResult<()> {
        plugin_sdk::log_info("Plugin initialized successfully");
        Ok(())
    }

    fn handle_event(&mut self, event: &PluginEvent) -> PluginResult<()> {
        plugin_sdk::log_debug(&format!("Received event: {:?}", event));
        Ok(())
    }

    fn cleanup(&mut self) -> PluginResult<()> {
        plugin_sdk::log_info("Plugin cleaned up");
        Ok(())
    }
}

// Export the plugin
plugin_sdk::export_plugin!(MyPlugin);
"#;

    fs::write(plugin_dir.join("src/lib.rs"), lib_rs).unwrap_or_else(|err| {
        eprintln!("Failed to create lib.rs: {}", err);
        std::process::exit(1);
    });

    // Create manifest.toml
    let manifest_toml = format!(
        r#"[plugin]
name = "{}"
version = "0.1.0"
description = "A sample OpenAgent Terminal plugin"
author = "Plugin Developer"
license = "Apache-2.0"

[capabilities]
# Enable specific capabilities as needed
completions = false
context_provider = false
commands = []
hooks = []

[metadata]
# Additional metadata
tags = ["sample", "demo"]
"#,
        name
    );

    fs::write(plugin_dir.join("manifest.toml"), manifest_toml).unwrap_or_else(|err| {
        eprintln!("Failed to create manifest.toml: {}", err);
        std::process::exit(1);
    });

    println!("Successfully created plugin '{}' in ./{}", name, name);
    println!();
    println!("Next steps:");
    println!("  1. cd {}", name);
    println!("  2. cargo build --target wasm32-wasi");
    println!("  3. Edit src/lib.rs to implement your plugin logic");
}

fn build_plugin() {
    println!("Building plugin for WebAssembly...");

    let output = std::process::Command::new("cargo")
        .args(["build", "--target", "wasm32-wasi", "--release"])
        .output()
        .unwrap_or_else(|err| {
            eprintln!("Failed to execute cargo build: {}", err);
            std::process::exit(1);
        });

    if output.status.success() {
        println!("Plugin built successfully!");
        println!("WASM binary available in target/wasm32-wasi/release/");
    } else {
        eprintln!("Build failed:");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        std::process::exit(1);
    }
}

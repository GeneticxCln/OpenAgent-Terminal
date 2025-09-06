//! Plugin SDK Command Line Tool
//!
//! This tool helps developers create and manage OpenAgent Terminal plugins.

use ed25519_dalek::Signer;
use std::path::Path;
use std::{env, fs};

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
        "install" => {
            if args.len() < 3 {
                eprintln!("Usage: plugin-sdk-cli install <url|path>");
                return;
            }
            if let Err(e) = install_plugin(&args[2]) {
                eprintln!("Install failed: {}", e);
            } else {
                println!("Installed plugin from {}", &args[2]);
            }
        },
        "add-key" => {
            if args.len() < 3 {
                eprintln!("Usage: plugin-sdk-cli add-key <hex|file>");
                return;
            }
            if let Err(e) = add_trusted_key(&args[2]) {
                eprintln!("Add key failed: {}", e);
            } else {
                println!("Key added");
            }
        },
        "verify" => {
            if args.len() < 2 + 1 {
                eprintln!("Usage: plugin-sdk-cli verify <wasm> [--sig <sigfile>]");
                return;
            }
            let mut sig: Option<String> = None;
            let mut i = 3;
            while i < args.len() {
                if args[i - 1] == "--sig" && i < args.len() {
                    sig = Some(args[i].clone());
                    i += 1;
                } else {
                    i += 1;
                }
            }
            match verify_signature(&args[2], sig.as_deref()) {
                Ok(true) => println!("Signature: OK"),
                Ok(false) => println!("Signature: NOT VERIFIED"),
                Err(e) => eprintln!("Verification error: {}", e),
            }
        },
        "sign" => {
            if args.len() < 4 {
                eprintln!(
                    "Usage: plugin-sdk-cli sign <priv_key_hex|file> <wasm> [--out <sigfile>]"
                );
                return;
            }
            let key_arg = &args[2];
            let wasm = &args[3];
            let mut out: Option<String> = None;
            let mut i = 5;
            while i < args.len() {
                if args[i - 1] == "--out" && i < args.len() {
                    out = Some(args[i].clone());
                    i += 1;
                } else {
                    i += 1;
                }
            }
            match sign_wasm(key_arg, wasm, out.as_deref()) {
                Ok(sig_path) => println!("Wrote signature to {}", sig_path.display()),
                Err(e) => eprintln!("Sign error: {}", e),
            }
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
    println!("    new <name>          Create a new plugin project");
    println!("    build               Build the current plugin");
    println!("    install <src>       Install plugin from URL or local path to user dir");
    println!("    add-key <hex|file>  Add a trusted signing public key (ed25519, hex)");
    println!("    verify <wasm> [--sig file]  Verify a wasm with a signature against trusted keys");
    println!(
        "    sign <priv|file> <wasm> [--out file]  Create <wasm>.sig using the private key \
         (ed25519, hex)"
    );
    println!("    help                Show this help message");
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

    // Create lib.rs (working WASI plugin using define_plugin)
    let lib_rs = r#"use plugin_sdk::{define_plugin, log, LogLevel, PluginError};

fn init_plugin() -> Result<(), PluginError> {
    log(LogLevel::Info, "{NAME} initialized");
    Ok(())
}

fn handle_event(_ptr: i32, _len: i32) -> Result<(), PluginError> {
    // Set a simple response
    let _ = plugin_sdk::set_last_response_json(&serde_json::json!({ "ok": true }));
    Ok(())
}

fn cleanup_plugin() -> Result<(), PluginError> {
    log(LogLevel::Info, "{NAME} cleaned up");
    Ok(())
}

define_plugin! {
    name: "{NAME}",
    version: env!("CARGO_PKG_VERSION"),
    author: "Plugin Developer",
    description: "A sample OpenAgent Terminal plugin",
    capabilities: { completions: false, context_provider: false, commands: Vec::<String>::new(), hooks: Vec::<plugin_sdk::HookType>::new(), file_associations: Vec::<String>::new() },
    permissions: { read_files: vec![], write_files: vec![], network: false, execute_commands: false, environment_variables: vec![] },
    init: init_plugin,
    event_handler: handle_event,
    cleanup: cleanup_plugin,
}
"#;

    let lib_rs_filled = lib_rs.replace("{NAME}", name);
    fs::write(plugin_dir.join("src/lib.rs"), lib_rs_filled).unwrap_or_else(|err| {
        eprintln!("Failed to create lib.rs: {}", err);
        std::process::exit(1);
    });

    // Create plugin manifest TOML matching the crate name (<name>.toml)
    let manifest_toml = format!(
        r#"[plugin]
name = "{name}"
version = "0.1.0"
author = "Plugin Developer"
description = "A sample OpenAgent Terminal plugin"
license = "MIT"

[plugin.capabilities]
completions = false
context_provider = false
commands = []
hooks = []
file_associations = []

[permissions]
read_files = []
write_files = []
network = false
execute_commands = false
environment_variables = []
max_memory_mb = 20
timeout_ms = 2000

[plugin.metadata]
tags = ["sample", "demo"]
"#
    );

    fs::write(plugin_dir.join(format!("{}.toml", name)), manifest_toml).unwrap_or_else(|err| {
        eprintln!("Failed to create manifest: {}", err);
        std::process::exit(1);
    });

    println!("Successfully created plugin '{}' in ./{}", name, name);
    println!();
    println!("Next steps:");
    println!("  1. cd {}", name);
    println!("  2. cargo build --target wasm32-wasi");
    println!("  3. Edit src/lib.rs to implement your plugin logic");
}

fn user_plugin_dir() -> std::path::PathBuf {
    if let Some(cfg) = dirs::config_dir() {
        cfg.join("openagent-terminal").join("plugins")
    } else {
        std::path::PathBuf::from("./.openagent-terminal/plugins")
    }
}

fn copy_to_dir(
    src: &std::path::Path,
    dest_dir: &std::path::Path,
) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dest_dir)?;
    let filename = src.file_name().unwrap();
    let dest = dest_dir.join(filename);
    std::fs::copy(src, &dest)?;
    Ok(dest)
}

fn install_plugin(src: &str) -> anyhow::Result<()> {
    let dest_dir = user_plugin_dir();
    if src.starts_with("http://") || src.starts_with("https://") {
        let resp = reqwest::blocking::get(src)?;
        if !resp.status().is_success() {
            anyhow::bail!("HTTP {}", resp.status());
        }
        let fname = src.split('/').last().unwrap_or("plugin.wasm");
        std::fs::create_dir_all(&dest_dir)?;
        let dest = dest_dir.join(fname);
        std::fs::write(&dest, resp.bytes()?)?;
        println!("Downloaded to {}", dest.display());
    } else {
        let path = std::path::Path::new(src);
        if !path.exists() {
            anyhow::bail!("Source not found: {}", src);
        }
        let dest = copy_to_dir(path, &dest_dir)?;
        println!("Copied to {}", dest.display());
    }
    Ok(())
}

fn add_trusted_key(arg: &str) -> anyhow::Result<()> {
    let key_hex = if std::path::Path::new(arg).exists() {
        std::fs::read_to_string(arg)?
    } else {
        arg.to_string()
    };
    let key_hex = key_hex.trim();
    // Basic validation length
    if key_hex.len() != 64 {
        // 32 bytes hex
        eprintln!("Warning: unexpected key length (expected 64 hex chars)");
    }
    if let Some(cfg) = dirs::config_dir() {
        let dir = cfg.join("openagent-terminal").join("trusted_keys");
        std::fs::create_dir_all(&dir)?;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let filename = format!("{}.pub", ts);
        std::fs::write(dir.join(filename), key_hex.as_bytes())?;
        Ok(())
    } else {
        anyhow::bail!("No config directory available")
    }
}

fn sign_wasm(
    key_arg: &str,
    wasm: &str,
    sig_out: Option<&str>,
) -> anyhow::Result<std::path::PathBuf> {
    use ed25519_dalek::SigningKey;
    use sha2::{Digest, Sha256};

    let key_hex = if std::path::Path::new(key_arg).exists() {
        std::fs::read_to_string(key_arg)?
    } else {
        key_arg.to_string()
    };
    let key_bytes = hex::decode(key_hex.trim())?;
    if key_bytes.len() != 32 {
        anyhow::bail!("expected 32-byte ed25519 private key in hex");
    }

    let wasm_path = std::path::Path::new(wasm);
    let wasm_bytes = std::fs::read(wasm_path)?;
    let digest = Sha256::digest(&wasm_bytes);

    let sk = SigningKey::from_bytes(
        &key_bytes.clone().try_into().map_err(|_| anyhow::anyhow!("invalid key length"))?,
    );
    let sig = sk.sign(&digest);
    let sig_hex = hex::encode(sig.to_bytes());
    let out_path =
        sig_out.map(std::path::PathBuf::from).unwrap_or_else(|| wasm_path.with_extension("sig"));
    std::fs::write(&out_path, sig_hex.as_bytes())?;
    Ok(out_path)
}

fn verify_signature(wasm: &str, sig_file: Option<&str>) -> anyhow::Result<bool> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    use sha2::{Digest, Sha256};

    let wasm_path = std::path::Path::new(wasm);
    let sig_path =
        sig_file.map(std::path::PathBuf::from).unwrap_or_else(|| wasm_path.with_extension("sig"));
    if !sig_path.exists() {
        anyhow::bail!("Signature file not found: {}", sig_path.display());
    }

    let wasm_bytes = std::fs::read(wasm_path)?;
    let sig_hex = std::fs::read_to_string(&sig_path)?;
    let sig_bytes = hex::decode(sig_hex.trim())?;
    let signature = Signature::from_slice(&sig_bytes)?;
    let digest = Sha256::digest(&wasm_bytes);

    if let Some(cfg) = dirs::config_dir() {
        let dir = cfg.join("openagent-terminal").join("trusted_keys");
        if !dir.exists() {
            return Ok(false);
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|s| s.to_str()) != Some("pub") {
                continue;
            }
            let key_hex = std::fs::read_to_string(entry.path())?;
            let key_bytes = hex::decode(key_hex.trim())?;
            if let Ok(vk) = VerifyingKey::from_bytes(
                &key_bytes.try_into().map_err(|_| anyhow::anyhow!("invalid key length"))?,
            ) {
                if vk.verify(&digest, &signature).is_ok() {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
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

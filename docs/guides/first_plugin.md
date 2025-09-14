# Your first plugin (WASI)

This guide walks you through creating, building, and loading a minimal WASI plugin for OpenAgent Terminal. It uses the existing hello-wasi example as a template and enforces the default sandboxed permission model.

## Prerequisites

- Rust toolchain with WASI target:
  ```bash
  rustup target add wasm32-wasi
  ```
- Optional: Wasmtime installed for local testing.

## 1) Start from the example

The repository includes a working example:
- examples/plugins/hello-wasi

Copy it as a starting point:
```bash
cp -r examples/plugins/hello-wasi ~/my-hello-plugin
cd ~/my-hello-plugin
```

## 2) Build for WASI

```bash
cargo build --release --target wasm32-wasi
```
This produces target/wasm32-wasi/release/hello_wasi.wasm.

## 3) Create a minimal manifest

Create a TOML manifest next to the .wasm (hello-wasi.toml):
```toml
[plugin]
name = "hello-wasi"
version = "0.1.0"
author = "You"
description = "Hello from WASI"

[permissions]
# Keep tight by default; expand only as needed
environment_variables = []
read_files = []
write_files = []
network = false
execute_commands = false
max_memory_mb = 50
timeout_ms = 2000
```

## 4) Install to your plugins directory

On Linux, the user plugins directory defaults to:
- ~/.config/openagent-terminal/plugins

Create it if needed and copy files:
```bash
mkdir -p ~/.config/openagent-terminal/plugins/hello-wasi
cp target/wasm32-wasi/release/hello_wasi.wasm ~/.config/openagent-terminal/plugins/hello-wasi/
cp hello-wasi.toml ~/.config/openagent-terminal/plugins/hello-wasi/
```

## 5) Configure and run

Ensure plugins are enabled and discovery includes the user path. In your OpenAgent Terminal config:
```toml
[plugins]
enabled = true
hot_reload = true  # optional in dev

[plugins.paths.user]
path = "~/.config/openagent-terminal/plugins"
require_signatures = false  # relaxed for dev; see signing guide for production
```

Run the terminal and check logs for plugin load messages. For verbose loader logs:
```bash
RUST_LOG=plugin_loader=debug,openagent_terminal=info openagent-terminal
```

## 6) Optional: sign your plugin

For production, sign plugins and enable verification. See docs/plugins_signing.md. Quick outline:
- Generate publisher key (ed25519)
- Sign the SHA-256 digest of the .wasm to produce plugin.sig (hex)
- Place plugin.sig next to the .wasm
- Put trusted publisher key in ~/.config/openagent-terminal/trusted_keys/
- Enable enforcement in config or via env vars

## 7) Iterate with hot reload

During development, leave hot_reload = true. Changes to the .wasm or manifest will trigger a reload at runtime.

## Tips

- Keep permissions to the minimum needed; expand incrementally.
- Avoid network/command execution unless strictly necessary.
- Prefer streaming and chunked host interactions for long-running work.
- Use tracing logs to audit host calls when debugging.
# Plugins

This project exposes a minimal, versioned plugin API for extending OpenAgent Terminal with additional capabilities. The goals are:

- Stability: a small, well-documented surface area with clear versioning
- Safety: sandboxed execution (e.g., WASI) for third-party code where possible
- Simplicity: quick-start examples that are easy to copy and adapt

Quick start (WASI “Hello” plugin)

- Example: examples/plugins/hello-wasi
- Build: cargo build -p examples/plugins/hello-wasi --release
- Load: place the resulting .wasm in your plugin directory and register it in config

API versioning and stability

- The Rust crate crates/plugin-api is versioned independently (SemVer)
- Backwards-compatible changes (minor) may add new capabilities while preserving existing interfaces
- Breaking changes (major) are rare and announced in the changelog; a migration section will accompany them

Current status

- Minimal API for command proposals and simple UI hooks
- Host-side loader at crates/plugin-loader
- Example WASI plugin at examples/plugins/hello-wasi

Best practices

- Keep plugins focused on one task
- Avoid spawning external processes unless necessary
- Prefer streaming interfaces for long-running tasks

For detailed design notes see docs/adr/003-plugin-system.md


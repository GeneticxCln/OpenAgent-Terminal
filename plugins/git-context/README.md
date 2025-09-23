# git-context (plugin)

Context plugin for Git-aware operations in OpenAgent Terminal, built with the plugin SDK.

Build (WASI):
- rustup target add wasm32-wasi
- cargo build -p git-context --release --target wasm32-wasi
- Output: target/wasm32-wasi/release/git_context.wasm

Docs:
- ../../docs/TESTING.md
- ../../docs/features.md

License:
- Apache-2.0 OR MIT

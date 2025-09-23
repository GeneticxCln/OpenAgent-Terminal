# dev-tools (plugin)

Developer utilities plugin for OpenAgent Terminal, built with the plugin SDK.

Build (WASI):
- rustup target add wasm32-wasi
- cargo build -p dev-tools --release --target wasm32-wasi
- Output: target/wasm32-wasi/release/dev_tools.wasm

Docs:
- ../../docs/TESTING.md
- ../../docs/features.md

License:
- Apache-2.0 OR MIT

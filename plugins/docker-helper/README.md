# docker-helper (plugin)

Docker helper plugin for OpenAgent Terminal, built with the plugin SDK.

Build (WASI):
- rustup target add wasm32-wasi
- cargo build -p docker-helper --release --target wasm32-wasi
- Output: target/wasm32-wasi/release/docker_helper.wasm

Docs:
- ../../docs/TESTING.md
- ../../docs/features.md

License:
- Apache-2.0 OR MIT

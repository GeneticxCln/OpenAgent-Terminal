# plugin-sdk

SDK for developing OpenAgent Terminal plugins in WebAssembly.

Targets:
- wasm32-wasi (primary)

Usage:
- Used by plugin crates in plugins/.

Build (example plugins):
- rustup target add wasm32-wasi
- cargo build -p git-context --release --target wasm32-wasi

Testing:
- cargo test -p plugin-sdk

Docs:
- ../../docs/TESTING.md
- ../../docs/features.md

License:
- Apache-2.0 OR MIT

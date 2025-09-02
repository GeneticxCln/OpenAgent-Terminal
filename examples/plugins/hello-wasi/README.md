# Hello WASI Plugin (example)

This is a minimal WebAssembly (WASI) plugin demonstrating permission enforcement
in the OpenAgent Terminal plugin loader.

What it shows:
- Only whitelisted environment variables are visible to the plugin.
- Files outside preopened directories (like /etc/passwd) are not readable.

Build instructions:
1. Install the WASI target
   rustup target add wasm32-wasi

2. Build the plugin
   cargo build --release --target wasm32-wasi

3. Copy the resulting files into your plugin directory
   cp target/wasm32-wasi/release/hello-wasi-plugin.wasm ~/.config/openagent-terminal/plugins/
   cp hello-wasi-plugin.toml ~/.config/openagent-terminal/plugins/

4. Provide the allowed env var before launching the terminal (optional)
   export HELLO_PLUGIN_MESSAGE="Hello from host!"

5. Optionally set a forbidden env var to see it blocked
   export FORBIDDEN_SECRET="should-not-be-visible"

6. Run OpenAgent Terminal with the `plugins` feature enabled so the plugin manager loads plugins.

Manifest
- The loader expects a TOML manifest right next to the .wasm, using the same basename.
- In this example: hello-wasi-plugin.wasm + hello-wasi-plugin.toml

Expected output in logs when the plugin is loaded:
- [hello-wasi] HELLO_PLUGIN_MESSAGE=Hello from host!
- [hello-wasi] FORBIDDEN_SECRET=<not available>
- [hello-wasi] /etc/passwd: access denied (expected)


// Minimal WASI plugin demonstrating permission enforcement via environment variables
// and filesystem access constraints.
// Build: rustup target add wasm32-wasi && cargo build --release --target wasm32-wasi

#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    // Allowed env var (when configured in manifest)
    let hello = std::env::var("HELLO_PLUGIN_MESSAGE").unwrap_or_else(|_| "<missing>".to_string());
    println!("[hello-wasi] HELLO_PLUGIN_MESSAGE={}", hello);

    // Forbidden env var should not be visible to the plugin
    let forbidden = std::env::var("FORBIDDEN_SECRET").unwrap_or_else(|_| "<not available>".to_string());
    println!("[hello-wasi] FORBIDDEN_SECRET={}", forbidden);

    // Attempt to read a system file (should be denied under WASI unless preopened)
    match std::fs::read_to_string("/etc/passwd") {
        Ok(_) => println!("[hello-wasi] /etc/passwd: unexpectedly readable"),
        Err(_) => println!("[hello-wasi] /etc/passwd: access denied (expected)"),
    }

    0
}

#[no_mangle]
pub extern "C" fn plugin_get_metadata() -> i32 { 0 }

#[no_mangle]
pub extern "C" fn plugin_cleanup() -> i32 { 0 }


# Enabling the default host and loading a WASM plugin

This snippet shows how to enable the policy‑enforced DefaultHost and load a WASM plugin with permissions.

```rust path=null start=null
use plugin_system::{UnifiedPluginManager, PluginPermissions};

fn main() -> anyhow::Result<()> {
    // Create the plugin manager and enable the default host
    let mut manager = UnifiedPluginManager::new("./plugins")?;

    // Example: define minimal permissions (in practice, parse from plugin .toml)
    let permissions = PluginPermissions {
        read_files: vec!["/etc/hostname".into()],
        network: true,
        net_allow_domains: vec!["example.com".into()],
        net_methods_allow: vec!["GET".into()],
        execute_commands: true,
        exec_allow: vec![plugin_system::permissions::ExecRule {
            cmd: "echo".into(), args_pattern: None, cwd_allow: vec!["/".into()], timeout_ms: Some(2000), max_output_bytes: Some(65536),
        }],
        ..Default::default()
    };

    // Attach the default host enforcing these permissions
    manager.set_default_host_from_permissions(&permissions);

    // Load a WASM plugin and run it
    let wasm_path = "./examples/plugins/wasm/read_file_demo.wasm"; // compile .wat to .wasm via `wat2wasm`
    futures::executor::block_on(async {
        let pid = manager.load_plugin(wasm_path).await?;
        // If your plugin exports commands, you can invoke them through the unified API
        // let out = manager.execute_command(&pid, "my_cmd", &[]).await?;
        // println!("stdout: {}", out.stdout);
        anyhow::Ok(())
    })
}
```

Notes
- Compile the WAT examples to WASM with `wat2wasm`:
  - wat2wasm examples/plugins/wasm/read_file_demo.wat -o examples/plugins/wasm/read_file_demo.wasm
- Place a permission manifest next to the plugin (same stem, .toml) so `load_plugin` picks it up.
- Alternatively, construct a PluginPermissions in code and call `set_default_host_from_permissions(&perms)`.

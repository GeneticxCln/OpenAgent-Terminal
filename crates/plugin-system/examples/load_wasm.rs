use plugin_system::{UnifiedPluginManager, PluginPermissions};
use plugin_system::permissions::ExecRule;

fn main() -> anyhow::Result<()> {
    // Create the manager
    let mut manager = UnifiedPluginManager::new(".")?;

    // Union permissions for the three demo plugins
    let perms = PluginPermissions {
        read_files: vec!["/etc/hostname".into()],
        network: true,
        net_allow_domains: vec!["example.com".into()],
        net_methods_allow: vec!["GET".into()],
        execute_commands: true,
        exec_allow: vec![ExecRule {
            cmd: "echo".into(), args_pattern: None, cwd_allow: vec!["/".into()],
            timeout_ms: Some(2000), max_output_bytes: Some(65536),
        }],
        ..Default::default()
    };
    #[cfg(feature = "wasm-runtime")]
    manager.set_default_host_from_permissions(&perms);

    // Choose which demo to run
    let wasm_path = std::env::args().nth(1).unwrap_or_else(||
        "examples/plugins/wasm/read_file_demo.wasm".to_string());

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    rt.block_on(async move {
        let pid = manager.load_plugin(&wasm_path).await?;
        println!("Loaded plugin: {}", pid);
        anyhow::Ok(())
    })
}

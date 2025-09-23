#![cfg(any())]
// Integration tests for plugin manager discovery and load/unload

use std::path::PathBuf;

#[cfg(feature = "plugins")]
#[tokio::test]
async fn plugins_manager_discovers_and_loads_wasm() {
    use openagent_terminal::components_init::initialize_plugin_manager;

    // Create a temporary plugins directory
    let dir = tempfile::tempdir().expect("tmpdir");
    let plugins_dir = dir.path().to_path_buf();

    // Write a minimal WASM plugin to the plugins directory
    const WAT_SRC: &str = r#"(module
      (memory (export "memory") 1)
      (func (export "plugin_alloc") (param i32) (result i32)
        (i32.const 0)
      )
      (func (export "plugin_init") (result i32)
        (i32.const 0)
      )
      (func (export "plugin_cleanup") (result i32)
        (i32.const 0)
      )
      (func (export "plugin_handle_event") (param i32 i32) (result i32)
        (i32.const 0)
      )
    )"#;
    let wasm_bytes = wat::parse_str(WAT_SRC).expect("wat->wasm");
    let wasm_path = plugins_dir.join("test_plugin.wasm");
    tokio::fs::write(&wasm_path, &wasm_bytes).await.expect("write wasm");

    // Initialize plugin manager with permissive policy and only our temp directory
    let pm = initialize_plugin_manager(
        plugins_dir.clone(), /* data plugins dir */
        false, /* enforce_signatures */
        false, /* require_signatures_for_all */
        false, /* path_require_system */
        false, /* path_require_user */
        false, /* path_require_project */
        false, /* hot_reload */
    )
    .await
    .expect("plugin manager");

    // Discover should include our wasm file path
    let mut discovered = pm.discover_plugins().await.expect("discover");
    discovered.sort();
    assert!(
        discovered.iter().any(|p| p == &wasm_path),
        "discover_plugins should include test wasm path"
    );

    // Load the plugin and ensure id is the file stem
    let id = pm.load_plugin(&wasm_path).await.expect("load");
    assert_eq!(id, "test_plugin");

    // Unload successfully
    pm.unload_plugin(&id).await.expect("unload");
}

#![allow(clippy::pedantic)]

use openagent_terminal::workspace::warp_tab_manager::WarpTabManager;
use openagent_terminal::workspace::TabId;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn warp_tab_manager_smart_naming_and_command_update() {
    // Create a temporary project directory with Cargo.toml to trigger project-name detection
    let dir = tempdir().expect("tempdir");
    let project_dir: PathBuf = dir.path().to_path_buf();
    let cargo_toml = project_dir.join("Cargo.toml");
    fs::write(
        &cargo_toml,
        r#"[package]
name = "demo_project"
version = "0.1.0"
"#,
    )
    .expect("write Cargo.toml");

    // New Warp-style tab manager
    let mut mgr = WarpTabManager::new();

    // Create a new tab in the project directory
    let tab_id: TabId = mgr.create_warp_tab(Some(project_dir.clone()));

    // Active tab should pick up project name as title
    let active = mgr.active_tab().expect("active tab exists");
    assert_eq!(active.id, tab_id);
    assert_eq!(active.title, "demo_project");

    // Update tab for command should produce "<cmd> in <dir_name>" style title
    mgr.update_tab_for_command(tab_id, "npm run dev");
    let updated = mgr.active_tab().expect("active tab still exists");
    let dir_name = project_dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
    assert_eq!(updated.title, format!("npm in {}", dir_name));
}

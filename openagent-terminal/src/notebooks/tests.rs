// Tests for Command Notebooks

#![cfg(all(test, feature = "blocks"))]

use super::*;
use tempfile::TempDir;

#[tokio::test]
async fn create_and_run_notebook() {
    let tmp = TempDir::new().unwrap();
    let nb_dir = tmp.path().join("notebooks");
    let blocks_dir = tmp.path().join("blocks");

    // Initialize block manager (optional)
    let bm = BlockManager::new(blocks_dir.clone()).await.ok().map(|bm| Arc::new(tokio::sync::RwLock::new(bm)));

    let mgr = NotebookManager::new(&nb_dir, bm).await.unwrap();

    let nb = mgr.create_notebook("Test".into(), None, Default::default()).await.unwrap();
    let _md = mgr.add_markdown_cell(nb.id, None, "# Header".into()).await.unwrap();
    let cmd = mgr
        .add_command_cell(nb.id, None, "echo hello".into(), None, Some(ShellType::Bash))
        .await
        .unwrap();

    let out = mgr.run_cell(cmd.id).await.unwrap();
    assert_eq!(out.exit_code, Some(0));
    assert!(out.output.unwrap_or_default().contains("hello"));
}


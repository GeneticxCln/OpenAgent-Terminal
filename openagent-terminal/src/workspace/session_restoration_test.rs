//! Test suite for session restoration functionality
//!
//! This module contains comprehensive tests for the Warp-style session
//! restoration system, covering various scenarios and edge cases.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::workspace::split_manager::PaneId;
use crate::workspace::tab_layout_manager::{
    WarpPaneSession, WarpSession, WarpSplitLayoutSession, WarpTabManager, WarpTabSession,
};
use crate::workspace::TabId;

/// Create a basic test session with one tab and one pane
fn create_test_session() -> WarpSession {
    let tab_id = TabId(1);
    let pane_id = PaneId(1);

    let pane_session = WarpPaneSession {
        id: pane_id,
        working_directory: PathBuf::from("/tmp"),
        shell_command: Some("bash".to_string()),
        last_command: Some("ls".to_string()),
        title_override: None,
    };

    let mut panes = HashMap::new();
    panes.insert(pane_id, pane_session);

    let tab_session = WarpTabSession {
        id: tab_id,
        title: "Test Tab".to_string(),
        working_directory: PathBuf::from("/tmp"),
        split_layout: WarpSplitLayoutSession::Single(pane_id),
        active_pane: pane_id,
        panes,
        shell_command: Some("bash".to_string()),
        last_command: Some("ls".to_string()),
        created_at: SystemTime::now(),
    };

    WarpSession {
        version: "1.0.0".to_string(),
        id: "test-session-1".to_string(),
        name: "Test Session".to_string(),
        created_at: SystemTime::now(),
        last_used: SystemTime::now(),
        tabs: vec![tab_session],
        active_tab_id: Some(tab_id),
    }
}

/// Create a test session with complex split layout
fn create_complex_test_session() -> WarpSession {
    let tab_id = TabId(1);
    let pane1 = PaneId(1);
    let pane2 = PaneId(2);
    let pane3 = PaneId(3);

    // Create split layout: Horizontal split with left pane and right vertical split
    let split_layout = WarpSplitLayoutSession::Horizontal {
        left: Box::new(WarpSplitLayoutSession::Single(pane1)),
        right: Box::new(WarpSplitLayoutSession::Vertical {
            top: Box::new(WarpSplitLayoutSession::Single(pane2)),
            bottom: Box::new(WarpSplitLayoutSession::Single(pane3)),
            ratio: 0.5,
        }),
        ratio: 0.5,
    };

    let mut panes = HashMap::new();
    for &pane_id in [pane1, pane2, pane3].iter() {
        panes.insert(
            pane_id,
            WarpPaneSession {
                id: pane_id,
                working_directory: PathBuf::from("/tmp"),
                shell_command: Some("bash".to_string()),
                last_command: None,
                title_override: None,
            },
        );
    }

    let tab_session = WarpTabSession {
        id: tab_id,
        title: "Complex Tab".to_string(),
        working_directory: PathBuf::from("/tmp"),
        split_layout,
        active_pane: pane2,
        panes,
        shell_command: Some("bash".to_string()),
        last_command: None,
        created_at: SystemTime::now(),
    };

    WarpSession {
        version: "1.0.0".to_string(),
        id: "test-session-complex".to_string(),
        name: "Complex Test Session".to_string(),
        created_at: SystemTime::now(),
        last_used: SystemTime::now(),
        tabs: vec![tab_session],
        active_tab_id: Some(tab_id),
    }
}

#[test]
fn test_session_serialization_roundtrip() {
    let original_session = create_test_session();

    // Serialize to JSON
    let serialized =
        serde_json::to_string(&original_session).expect("Should serialize session to JSON");

    // Deserialize back
    let deserialized: WarpSession =
        serde_json::from_str(&serialized).expect("Should deserialize session from JSON");

    // Verify key fields
    assert_eq!(deserialized.version, "1.0.0");
    assert_eq!(deserialized.id, original_session.id);
    assert_eq!(deserialized.tabs.len(), 1);
    assert_eq!(deserialized.active_tab_id, Some(TabId(1)));
}

#[test]
fn test_complex_session_serialization() {
    let complex_session = create_complex_test_session();

    // Serialize and deserialize
    let json =
        serde_json::to_string_pretty(&complex_session).expect("Should serialize complex session");
    let restored: WarpSession =
        serde_json::from_str(&json).expect("Should deserialize complex session");

    // Verify structure
    assert_eq!(restored.tabs.len(), 1);
    let tab = &restored.tabs[0];

    // Check split layout structure
    match &tab.split_layout {
        WarpSplitLayoutSession::Horizontal { left, right, ratio } => {
            assert_eq!(*ratio, 0.5);

            // Left should be single pane
            matches!(left.as_ref(), WarpSplitLayoutSession::Single(_));

            // Right should be vertical split
            matches!(right.as_ref(), WarpSplitLayoutSession::Vertical { .. });
        }
        _ => panic!("Expected horizontal split at root"),
    }

    assert_eq!(tab.panes.len(), 3);
}

#[test]
fn test_session_validation() {
    let manager = WarpTabManager::new();

    // Valid session should pass
    let valid_session = create_test_session();
    assert!(manager.validate_session(&valid_session).is_ok());

    // Empty session should fail
    let empty_session = WarpSession {
        version: "1.0.0".to_string(),
        id: "empty".to_string(),
        name: "Empty".to_string(),
        created_at: SystemTime::now(),
        last_used: SystemTime::now(),
        tabs: vec![], // No tabs
        active_tab_id: None,
    };
    assert!(manager.validate_session(&empty_session).is_err());

    // Session with invalid active tab should fail
    let mut invalid_session = create_test_session();
    invalid_session.active_tab_id = Some(TabId(999)); // Non-existent tab
    assert!(manager.validate_session(&invalid_session).is_err());
}

#[test]
fn test_session_migration() {
    let manager = WarpTabManager::new();

    // Create old format session (without version)
    let mut old_session = create_test_session();
    old_session.version = "0.9.0".to_string();

    let migrated =
        manager.migrate_session_format(old_session).expect("Should migrate old session format");

    assert_eq!(migrated.version, "1.0.0");
}

#[test]
fn test_session_migration_unsupported() {
    let manager = WarpTabManager::new();

    // Create session with unsupported version
    let mut future_session = create_test_session();
    future_session.version = "2.0.0".to_string();

    let result = manager.migrate_session_format(future_session);
    assert!(result.is_err());
}

#[test]
fn test_working_directory_fallback() {
    let session = WarpSession {
        version: "1.0.0".to_string(),
        id: "test".to_string(),
        name: "Test".to_string(),
        created_at: SystemTime::now(),
        last_used: SystemTime::now(),
        tabs: vec![WarpTabSession {
            id: TabId(1),
            title: "Test Tab".to_string(),
            working_directory: PathBuf::from("/nonexistent/directory"),
            split_layout: WarpSplitLayoutSession::Single(PaneId(1)),
            active_pane: PaneId(1),
            panes: HashMap::new(),
            shell_command: Some("bash".to_string()),
            last_command: None,
            created_at: SystemTime::now(),
        }],
        active_tab_id: Some(TabId(1)),
    };

    let manager = WarpTabManager::new();

    // Validation should succeed but with warnings
    let validation_result = manager.validate_session(&session);
    assert!(validation_result.is_ok(), "Should pass validation despite inaccessible directory");
}

#[test]
fn test_empty_panes_validation() {
    // This test would catch sessions with corrupted split layouts
    let tab_session = WarpTabSession {
        id: TabId(1),
        title: "Empty Tab".to_string(),
        working_directory: PathBuf::from("/tmp"),
        split_layout: WarpSplitLayoutSession::Single(PaneId(1)),
        active_pane: PaneId(1),
        panes: HashMap::new(), // Empty panes map
        shell_command: Some("bash".to_string()),
        last_command: None,
        created_at: SystemTime::now(),
    };

    let session = WarpSession {
        version: "1.0.0".to_string(),
        id: "empty-panes".to_string(),
        name: "Empty Panes Test".to_string(),
        created_at: SystemTime::now(),
        last_used: SystemTime::now(),
        tabs: vec![tab_session],
        active_tab_id: Some(TabId(1)),
    };

    let manager = WarpTabManager::new();

    // Should still validate (panes can be recreated)
    assert!(manager.validate_session(&session).is_ok());
}

/// Integration test for full session restoration cycle
#[test]
fn test_session_save_load_cycle() {
    use tempfile::tempdir;

    // Create temporary directory for session file
    let temp_dir = tempdir().expect("Should create temp directory");
    let session_path = temp_dir.path().join("test_session.json");

    let mut manager = WarpTabManager::with_session_file(&session_path);

    // Create some tabs
    let tab1 = manager.create_warp_tab(Some(PathBuf::from("/tmp")));
    let tab2 = manager.create_warp_tab(Some(PathBuf::from("/home")));

    // Add some command history
    manager.update_tab_for_command(tab1, "git status");
    manager.update_tab_for_command(tab2, "cargo build");

    // Save session
    manager.save_session().expect("Should save session");

    // Verify file was created
    assert!(session_path.exists());

    // Create new manager and load session
    let mut new_manager = WarpTabManager::with_session_file(&session_path);
    let loaded = new_manager.load_session().expect("Should load session");

    assert!(loaded, "Should successfully load session");
    assert_eq!(new_manager.tab_count(), 2);

    // Verify tabs were restored
    let tabs: Vec<_> = new_manager.all_tabs().collect();
    assert_eq!(tabs.len(), 2);

    // Verify command history was preserved
    assert!(new_manager.command_history.contains_key(&tab1));
    assert!(new_manager.command_history.contains_key(&tab2));
}

/// Test session restoration with corrupted data
#[test]
fn test_large_session_save_load_with_splits_and_dirs() {
    use tempfile::tempdir;

    let temp_dir = tempdir().expect("Should create temp directory");
    let session_path = temp_dir.path().join("large_session.json");

    let mut manager = WarpTabManager::with_session_file(&session_path);

    // Create multiple tabs with mixed working directories
    let dirs = vec!["/tmp", "/home", "/var", "/", "/usr"].into_iter().map(PathBuf::from);
    let mut created_tabs: Vec<TabId> = Vec::new();
    for d in dirs {
        let id = manager.create_warp_tab(Some(d));
        created_tabs.push(id);
        // Simulate a few commands per tab
        manager.update_tab_for_command(id, "ls");
        manager.update_tab_for_command(id, "echo hello");
    }

    // Build some split structures in the first tab
    if let Some(&first) = created_tabs.first() {
        // Duplicate a few times to create Horizontal/Vertical structure
let _ = manager.duplicate_tab_as_split(first, super::tab_layout_manager::SplitDirection::Right);
let _ = manager.duplicate_tab_as_split(first, super::tab_layout_manager::SplitDirection::Down);
let _ = manager.duplicate_tab_as_split(first, super::tab_layout_manager::SplitDirection::Right);
    }

    // Save and reload
    manager.save_session().expect("Should save large session");
    assert!(session_path.exists());

    let mut loader = WarpTabManager::with_session_file(&session_path);
    let loaded = loader.load_session().expect("Should load large session");
    assert!(loaded);

    // Verify tab count and that an active tab exists
    assert!(loader.tab_count() >= created_tabs.len());
    assert!(loader.active_tab().is_some());
}

#[test]
fn test_corrupted_session_handling() {
    use tempfile::tempdir;

    let temp_dir = tempdir().expect("Should create temp directory");
    let session_path = temp_dir.path().join("corrupted_session.json");

    // Write invalid JSON
    std::fs::write(&session_path, "{ invalid json }").expect("Should write corrupted file");

    let mut manager = WarpTabManager::with_session_file(&session_path);
    let result = manager.load_session().expect("Should handle corrupted session gracefully");

    // Should return false (not loaded) but not error
    assert!(!result);

    // Should create backup file
    let backup_path = session_path.with_extension("json.backup");
    assert!(backup_path.exists());
}

use std::fs;
use std::io::Write;

use tempfile::tempdir;

#[cfg(feature = "migrate")]
#[test]
fn test_migrations_loader_parses_multiple_formats_and_overrides() {
    use openagent_terminal_utils::migrate::{MigrateManager, Migration};

    let dir = tempdir().unwrap();
    let path = dir.path();

    // TOML single migration
    let toml_single = r#"
id = "010_single"
version = "1.0.0"
description = "Single TOML migration"
applied_at = null
"#;
    let mut f1 = fs::File::create(path.join("010_single.toml")).unwrap();
    f1.write_all(toml_single.as_bytes()).unwrap();

    // JSON array of migrations
    let json_array = r#"[
  {"id":"011_json","version":"1.0.1","description":"JSON migration","applied_at":null},
  {"id":"002_theme_system","version":"9.9.9","description":"Override built-in","applied_at":null}
]"#;
    let mut f2 = fs::File::create(path.join("011_array.json")).unwrap();
    f2.write_all(json_array.as_bytes()).unwrap();

    // YAML single migration
    let yaml_single = r#"
---
id: "012_yaml"
version: "1.0.2"
description: "YAML migration"
applied_at: null
"#;
    let mut f3 = fs::File::create(path.join("012_single.yaml")).unwrap();
    f3.write_all(yaml_single.as_bytes()).unwrap();

    let mut mgr = MigrateManager::new();
    mgr.initialize().unwrap();

    // Initially has built-ins
    let before = mgr.list_migrations().len();
    assert!(before >= 3);

    mgr.load_from_directory(path).unwrap();

    let after = mgr.list_migrations().len();
    // We added 3 new IDs; one overrides an existing built-in (002_theme_system)
    assert!(after >= before + 2);

    // Check override took effect (version 9.9.9)
    let overridden = mgr
        .list_migrations()
        .iter()
        .find(|m| m.id == "002_theme_system")
        .expect("missing overridden migration");
    assert_eq!(overridden.version, "9.9.9");
}
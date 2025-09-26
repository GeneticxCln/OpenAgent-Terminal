use std::fs;
use std::io::Write;

use tempfile::tempdir;

#[test]
fn test_snippets_loader_parses_single_and_wrapper() {
    use openagent_terminal_utils::snippets::{SnippetsManager};

    let dir = tempdir().unwrap();
    let path = dir.path();

    // Wrapper TOML including snippets and templates
    let toml_wrapper = r#"
[[snippets]]
name = "hello"
description = "greet"
content = "echo hello"
language = "bash"
tags = ["greet", "shell"]

[[templates]]
name = "tmpl1"
description = "demo"
content = "Hello {{who}}"
[[templates.variables]]
name = "who"
description = "who to greet"
default_value = "world"
"#;
    let mut f1 = fs::File::create(path.join("snips.toml")).unwrap();
    f1.write_all(toml_wrapper.as_bytes()).unwrap();

    // Single TOML snippet
    let toml_single = r#"
name = "bye"
description = "farewell"
content = "echo bye"
language = "bash"
tags = ["bye"]
"#;
    let mut f2 = fs::File::create(path.join("bye.toml")).unwrap();
    f2.write_all(toml_single.as_bytes()).unwrap();

    let mut mgr = SnippetsManager::new();
    mgr.initialize().unwrap();

    mgr.load_from_directory(path).unwrap();

    assert!(mgr.get_snippet("hello").is_some());
    assert!(mgr.get_snippet("bye").is_some());
    assert!(mgr.get_template("tmpl1").is_some());

    // Expand template
    let mut vars = std::collections::HashMap::new();
    vars.insert("who".to_string(), "OpenAgent".to_string());
    let expanded = mgr.expand_template("tmpl1", &vars).unwrap();
    assert!(expanded.contains("OpenAgent"));
}
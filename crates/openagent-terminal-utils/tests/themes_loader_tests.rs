use std::fs;
use std::io::Write;

use tempfile::tempdir;

#[test]
fn test_themes_loader_parses_and_validates() {
    use openagent_terminal_utils::themes::{ThemesManager};

    let dir = tempdir().unwrap();
    let path = dir.path();

    let theme_toml = r##"
[[themes]]
name = "my-dark"
description = "My dark theme"
author = "me"
[themes.colors]
foreground = "#FFFFFF"
background = "#000000"
cursor = "#FFFFFF"
selection_foreground = "#000000"
selection_background = "#FFFFFF"
normal = ["#000000", "#CD3131", "#0DBC79", "#E5E510", "#2472C8", "#BC3FBC", "#11A8CD", "#E5E5E5"]
bright = ["#666666", "#F14C4C", "#23D18B", "#F5F543", "#3B8EEA", "#D670D6", "#29B8DB", "#E5E5E5"]
"##;

    let mut f1 = fs::File::create(path.join("themes.toml")).unwrap();
    f1.write_all(theme_toml.as_bytes()).unwrap();

    let mut mgr = ThemesManager::new();
    mgr.initialize().unwrap();

    mgr.load_from_directory(path).unwrap();

    assert!(mgr.get_theme("my-dark").is_some());
}
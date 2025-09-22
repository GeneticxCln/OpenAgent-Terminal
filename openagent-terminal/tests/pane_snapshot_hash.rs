#![allow(
    clippy::pedantic,
    clippy::manual_let_else,
    clippy::single_match_else,
    clippy::uninlined_format_args
)]

use std::process::Command;

#[test]
fn test_split_panes_hash_linux_only() {
    // Linux-only until parity stabilizes.
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("Skipping pane hash test on non-Linux platform");
        return;
    }

    // Locate example binary built by cargo test
    let bin = match std::env::var("CARGO_BIN_EXE_snapshot_capture") {
        Ok(path) => path,
        Err(_) => {
            eprintln!(
                "Skipping pane hash test because example binary is not built (set \
                 CARGO_BIN_EXE_snapshot_capture)"
            );
            return;
        }
    };

    // Run in RAW_HASH mode for the split_panes scenario
    let output = Command::new(&bin)
        .env("RAW_HASH", "1")
        .env("SNAPSHOT_SIMILARITY_MIN", "0.995")
        .arg("--scenario=split_panes")
        .output()
        .expect("failed to run snapshot_capture example");

    assert!(output.status.success(), "snapshot_capture exited with failure: {:?}", output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse last JSON line
    let line = stdout.lines().last().unwrap_or("");
    assert!(line.starts_with('{') && line.ends_with('}'), "Expected JSON line, got: {line}");

    let v: serde_json::Value = serde_json::from_str(line).expect("invalid JSON");
    let width = v["width"].as_u64().unwrap_or(0);
    let height = v["height"].as_u64().unwrap_or(0);
    let sha = v["sha256"].as_str().unwrap_or("").to_string();

    // Basic sanity
    assert!(width > 0 && height > 0, "non-zero dimensions required");

    // If an expected hash is provided, assert it; otherwise print and skip.
    if let Ok(expected) = std::env::var("OPENAGENT_PANE_SPLIT_SHA256") {
        assert_eq!(sha, expected, "pane split hash mismatch");
    } else {
        eprintln!(
            "Observed split_panes sha256: {} (set OPENAGENT_PANE_SPLIT_SHA256 to enforce)",
            sha
        );
    }
}

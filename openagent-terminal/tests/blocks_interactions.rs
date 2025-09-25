#![allow(clippy::pedantic)]

use openagent_terminal::display::blocks::{BlockContent, Blocks};

#[test]
fn safe_subset_strips_ansi_and_redacts_secrets() {
    let input = "\u{1b}[31mERROR\u{1b}[0m api_key: ABCDEFGHIJKL12345\n";
    let out = BlockContent::safe_subset(input);
    assert!(!out.contains("\u{1b}["));
    assert!(out.to_lowercase().contains("api_key: {redacted}"));
}

#[test]
fn diff_previous_basic() {
    let bc = BlockContent { last_stdout: Some("a\nb\nc\n".to_string()), ..Default::default() };
    let diff = bc.diff_previous("a\nB\nc\nd\n");
    assert!(diff.contains("--- previous"));
    assert!(diff.contains("+++ current"));
    assert!(diff.contains("-b"));
    assert!(diff.contains("+B"));
    assert!(diff.contains("+d"));
}

#[test]
fn chip_ranges_expanded() {
    let header = "echo test";
    let ranges = Blocks::compute_header_chip_ranges(header);
    // Expect at least 5 chips
    assert!(ranges.len() >= 5);
}

#[test]
fn toggle_channels() {
    let mut bc = BlockContent::default();
    assert!(!bc.collapsed_stdout);
    bc.toggle_stdout();
    assert!(bc.collapsed_stdout);
    assert!(!bc.collapsed_stderr);
    bc.toggle_stderr();
    assert!(bc.collapsed_stderr);
}
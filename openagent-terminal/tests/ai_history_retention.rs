#![cfg(feature = "ai")]

#[test]
fn ui_history_prunes_to_configured_limits() {
    // Arrange: create a runtime and set tight retention
    let mut rt = openagent_terminal::ai_runtime::AiRuntime::new(Box::new(openagent_terminal_ai::NullProvider));
    let retention = openagent_terminal::config::ai::AiHistoryRetention {
        ui_max_entries: 3,
        ui_max_bytes: 10, // very small, to force byte-based prune too
        conversation_jsonl_max_bytes: 2 * 1024 * 1024,
        conversation_rotated_keep: 2,
        conversation_max_rows: 100,
        conversation_max_age_days: 7,
    };
    rt.set_history_retention(retention.clone());

    // Act: push a few entries via propose path by setting scratch and calling propose with NullProvider
    // We call the light path that only touches history
    rt.ui.scratch = "1234".into();
    rt.propose(None, None); // history: ["1234"]
    rt.ui.scratch = "abcd".into();
    rt.propose(None, None); // history: ["abcd","1234"]
    rt.ui.scratch = "xyz".into();
    rt.propose(None, None); // history: ["xyz","abcd","1234"]
    // This one should push out the oldest by entries cap, and then by bytes cap
    rt.ui.scratch = "longentry".into(); // 9 bytes
    rt.propose(None, None);

    // Assert: at most 3 entries and <= 10 bytes total
    let entries: Vec<String> = rt.ui.history.iter().cloned().collect();
    assert!(entries.len() <= retention.ui_max_entries);
    let total: usize = entries.iter().map(|s| s.len()).sum();
    assert!(total <= retention.ui_max_bytes, "total bytes {} > {}: {:?}", total, retention.ui_max_bytes, entries);
}

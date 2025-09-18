use openagent_terminal_ai::streaming::{RetryConfig, SseEvent, StreamProcessor};

#[test]
fn backpressure_buffers_then_drains() {
    let cfg = RetryConfig::default();
    // Small limit is fine here; we'll buffer two events and then drain.
    let mut sp = StreamProcessor::with_buffer_limit(cfg, 10);

    // First: consumer rejects to simulate slow reader -> events should buffer.
    let mut reject = |_ev: &SseEvent| -> bool { false };
    sp.process_data("data: hello\n\n", &mut reject).expect("process ok");
    sp.process_data("data: world\n\n", &mut reject).expect("process ok");

    // Now a consumer that accepts and collects.
    let mut delivered: Vec<String> = Vec::new();
    let mut accept = |ev: &SseEvent| -> bool {
        delivered.push(ev.data.join("\n"));
        true
    };

    // Trigger drain by calling process_data with no new events; drain_buffer always runs.
    sp.process_data("", &mut accept).expect("drain ok");
    sp.finalize(&mut accept);

    assert_eq!(delivered, vec!["hello".to_string(), "world".to_string()]);
}

#[test]
fn buffer_overflow_errors() {
    let cfg = RetryConfig::default();
    // Limit=2 and we will attempt to buffer 3 events while rejecting all.
    let mut sp = StreamProcessor::with_buffer_limit(cfg, 2);

    let mut reject = |_ev: &SseEvent| -> bool { false };
    sp.process_data("data: a\n\n", &mut reject).expect("process ok");
    sp.process_data("data: b\n\n", &mut reject).expect("process ok");
    let err = sp.process_data("data: c\n\n", &mut reject).expect_err("should overflow");
    assert!(err.to_lowercase().contains("buffer overflow"));
}
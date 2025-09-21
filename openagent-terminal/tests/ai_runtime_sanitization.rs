#![allow(clippy::pedantic)]

#![cfg(feature = "ai")]
use std::sync::{Arc, Mutex};

use openagent_terminal::ai_runtime::AiRuntime;
use openagent_terminal_ai::{AiProposal, AiProvider, AiRequest};

// Simple capture provider that records the last request it received
struct CaptureProvider {
    last: Arc<Mutex<Option<AiRequest>>>,
}

impl CaptureProvider {
    fn new(slot: Arc<Mutex<Option<AiRequest>>>) -> Self {
        Self { last: slot }
    }
}

impl AiProvider for CaptureProvider {
    fn name(&self) -> &'static str {
        "capture"
    }

    fn propose(&self, req: AiRequest) -> Result<Vec<AiProposal>, String> {
        *self.last.lock().unwrap() = Some(req);
        Ok(vec![AiProposal { title: "ok".into(), description: None, proposed_commands: vec![] }])
    }
}

#[test]
fn ai_runtime_sanitizes_request_before_provider() {
    // Arrange
    let slot = Arc::new(Mutex::new(None::<AiRequest>));
    let provider: Box<dyn AiProvider> = Box::new(CaptureProvider::new(slot.clone()));

    // Force HOME so redaction is deterministic
    std::env::set_var("HOME", "/home/tester");
    // Ensure sanitization is enabled (default), but set explicitly to be safe
    std::env::set_var("OPENAGENT_AI_STRIP_SENSITIVE", "1");
    std::env::set_var("OPENAGENT_AI_STRIP_CWD", "1");

    let mut rt = AiRuntime::new(provider);
    rt.ui.scratch =
        "List /home/tester/project; token: ghp_abcdefghijklmnopqrstuvwxyz0123456789".into();

    // Act
    rt.propose(Some("/home/tester/project".into()), Some("bash".into()));

    // Assert: provider saw the sanitized request
    let seen = slot.lock().unwrap().clone().expect("no request captured");
    // working_directory must be redacted
    assert_eq!(seen.working_directory.as_deref(), Some("[REDACTED]"));
    // scratch_text should not contain raw HOME or the original path
    assert!(!seen.scratch_text.contains("/home/tester"));
    assert!(seen.scratch_text.contains("[REDACTED_PATH]"));
    // Sensitive token redacted
    assert!(
        seen.scratch_text.contains("[REDACTED_GITHUB_TOKEN]"),
        "scratch_text was not redacted: {}",
        seen.scratch_text
    );
}

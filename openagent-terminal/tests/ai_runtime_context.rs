#![allow(clippy::pedantic)]
#![cfg(feature = "ai")]
use std::sync::{Arc, Mutex};

use openagent_terminal::ai_runtime::AiRuntime;
use openagent_terminal_ai::{AiProposal, AiProvider, AiRequest};
use openagent_terminal_core::tty::pty_manager::{PtyAiContext, ShellKind};

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
fn ai_runtime_propagates_context_when_sanitization_disabled() {
    // Disable sanitization so we can verify the raw values propagate
    std::env::set_var("OPENAGENT_AI_STRIP_SENSITIVE", "0");
    std::env::set_var("OPENAGENT_AI_STRIP_CWD", "0");

    let slot = Arc::new(Mutex::new(None::<AiRequest>));
    let provider: Box<dyn AiProvider> = Box::new(CaptureProvider::new(slot.clone()));

    let mut rt = AiRuntime::new(provider);
    rt.ui.scratch = "echo hello".into();

    let ctx = PtyAiContext {
        working_directory: std::path::PathBuf::from("/tmp/wd"),
        shell_kind: ShellKind::Zsh,
        last_command: Some("ls".into()),
        shell_executable: "zsh".into(),
    };

    // Act
    rt.propose_with_context(Some(ctx));

    // Assert
    let seen = slot.lock().unwrap().clone().expect("no request captured");
    assert_eq!(seen.working_directory.as_deref(), Some("/tmp/wd"));
    assert_eq!(seen.shell_kind.as_deref(), Some("zsh"));
    // Context should include recent command and shell executable metadata
    let map: std::collections::HashMap<_, _> = seen.context.into_iter().collect();
    assert_eq!(map.get("last_command").map(String::as_str), Some("ls"));
    assert_eq!(map.get("shell_executable").map(String::as_str), Some("zsh"));
}

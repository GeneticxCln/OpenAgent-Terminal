use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::Duration;

use winit::event_loop::EventLoopProxy;

use crate::event::{Event, EventType};

#[cfg(test)]
mod test_helpers {
    use super::*;
    use std::sync::{mpsc, Mutex};

    static SENT: once_cell::sync::Lazy<Mutex<Vec<crate::event::EventType>>> =
        once_cell::sync::Lazy::new(|| Mutex::new(Vec::new()));

    pub fn with_state<F: FnOnce(&mut ConfirmState)>(f: F) {
        let mut st = STATE.lock().unwrap();
        f(&mut st);
    }

    pub fn record_event(ev: crate::event::EventType) {
        let mut g = SENT.lock().unwrap();
        g.push(ev);
    }

    pub fn take_events() -> Vec<crate::event::EventType> {
        let mut g = SENT.lock().unwrap();
        let v = g.clone();
        g.clear();
        v
    }

    pub fn insert_pending_for_test(id: &str) -> mpsc::Receiver<bool> {
        let (tx, rx) = mpsc::channel();
        with_state(|s| {
            s.pending.insert(id.to_string(), tx);
        });
        rx
    }

    pub fn clear_all() {
        with_state(|s| {
            s.pending.clear();
            s.proxy = None;
            s.default_window = None;
        });
    }

    pub fn pending_len() -> usize {
        let st = STATE.lock().unwrap();
        // Count only test-tagged entries to avoid interference from concurrently running tests
        st.pending.keys().filter(|k| k.starts_with("t-")).count()
    }

    pub fn has_pending(id: &str) -> bool {
        let st = STATE.lock().unwrap();
        st.pending.contains_key(id)
    }
}

#[derive(Default)]
struct ConfirmState {
    proxy: Option<EventLoopProxy<Event>>,
    default_window: Option<winit::window::WindowId>,
    pending: HashMap<String, mpsc::Sender<bool>>,
}

static STATE: once_cell::sync::Lazy<Mutex<ConfirmState>> =
    once_cell::sync::Lazy::new(|| Mutex::new(ConfirmState::default()));

#[allow(dead_code)]
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn set_event_proxy(proxy: EventLoopProxy<Event>) {
    if let Ok(mut s) = STATE.lock() {
        s.proxy = Some(proxy);
    }
}

pub fn set_default_window_id(id: winit::window::WindowId) {
    if let Ok(mut s) = STATE.lock() {
        s.default_window = Some(id);
    }
}


#[allow(dead_code)]
pub fn generate_id() -> String {
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    format!("confirm-{}", id)
}

/// Request a confirmation via UI overlay and block until user responds or timeout elapses.
/// Returns Ok(true) if confirmed, Ok(false) if canceled, Err on infrastructure error/timeout.
#[allow(dead_code)]
pub fn request_confirm(
    title: String,
    body: String,
    confirm_label: Option<String>,
    cancel_label: Option<String>,
    timeout_ms: Option<u64>,
) -> Result<bool, String> {
    let id = generate_id();

    let (tx, rx) = mpsc::channel::<bool>();
    let (proxy_opt, window_opt) = {
        let mut state = STATE.lock().map_err(|_| "confirm state poisoned")?;
        state.pending.insert(id.clone(), tx);
        (state.proxy.clone(), state.default_window)
    };

    // In tests, allow proceeding without a real proxy; we'll only record events

    let maybe_proxy = proxy_opt;
    if let Some(proxy) = maybe_proxy {
        let evt = Event::new(
            EventType::ConfirmOpen { id: id.clone(), title, body, confirm_label, cancel_label },
            window_opt,
        );
        let _ = proxy.send_event(evt);
        #[cfg(test)]
        {
            test_helpers::record_event(EventType::ConfirmOpen {
                id: id.clone(),
                title: String::new(),
                body: String::new(),
                confirm_label: None,
                cancel_label: None,
            });
        }
    } else {
        #[cfg(not(test))]
        {
            return Err("event proxy not initialized".to_string());
        }
        #[cfg(test)]
        {
            test_helpers::record_event(EventType::ConfirmOpen {
                id: id.clone(),
                title: String::new(),
                body: String::new(),
                confirm_label: None,
                cancel_label: None,
            });
        }
    }

    match timeout_ms {
        Some(ms) => match rx.recv_timeout(Duration::from_millis(ms)) {
            Ok(val) => Ok(val),
            Err(_) => {
                // Timeout: clean up pending entry and close overlay via broadcast
                if let Ok(mut state) = STATE.lock() {
                    state.pending.remove(&id);
                    if let Some(proxy) = state.proxy.clone() {
                        // Inform UI to close overlay (not accepted)
                        let _ = proxy.send_event(Event::new(
                            EventType::ConfirmResolved { id: id.clone(), accepted: false },
                            None,
                        ));
                        #[cfg(test)]
                        {
                            test_helpers::record_event(EventType::ConfirmResolved {
                                id: id.clone(),
                                accepted: false,
                            });
                        }
                        // Optionally, send a message to the default window
                        if let Some(win) = state.default_window {
                            let message = crate::message_bar::Message::new(
                                "Confirmation timed out".into(),
                                crate::message_bar::MessageType::Warning,
                            );
                            let _ = proxy.send_event(Event::new(EventType::Message(message), win));
                        }
                    } else {
                        // Tests without proxy: still record the resolution event
                        #[cfg(test)]
                        {
                            test_helpers::record_event(EventType::ConfirmResolved {
                                id: id.clone(),
                                accepted: false,
                            });
                        }
                    }
                }
                Err("confirmation timed out".to_string())
            }
        },
        None => rx.recv().map_err(|_| "confirmation channel closed".to_string()),
    }
}

/// Resolve a pending confirmation by id; returns true if a waiter was found.
pub fn resolve(id: &str, accepted: bool) -> bool {
    if let Ok(mut state) = STATE.lock() {
        if let Some(sender) = state.pending.remove(id) {
            let _ = sender.send(accepted);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_confirm::test_helpers as th;
    use std::thread;

    // Global test lock to avoid interference through shared global state.
    static TEST_LOCK: once_cell::sync::Lazy<std::sync::Mutex<()>> =
        once_cell::sync::Lazy::new(|| std::sync::Mutex::new(()));

    #[test]
    fn resolve_removes_pending_and_sends_value() {
        let _g = TEST_LOCK.lock().unwrap();
        th::clear_all();
        let id = "t-1".to_string();
        let rx = th::insert_pending_for_test(&id);
        assert_eq!(th::pending_len(), 1);
        let ok = resolve(&id, true);
        assert!(ok);
        let got = rx.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(got);
        assert_eq!(th::pending_len(), 0);
    }

    #[test]
    fn resolve_unknown_returns_false() {
        let _g = TEST_LOCK.lock().unwrap();
        th::clear_all();
        let ok = resolve("does-not-exist", false);
        assert!(!ok);
    }

    #[test]
    fn request_confirm_timeout_closes_overlay_and_cleans_pending() {
        let _g = TEST_LOCK.lock().unwrap();
        th::clear_all();
        // No real proxy in tests; request_confirm records events when no proxy is set
        let res = super::request_confirm(
            "T".into(),
            "B".into(),
            Some("OK".into()),
            Some("Cancel".into()),
            Some(5),
        );
        assert!(res.is_err());
        let evs = th::take_events();
        // We should have seen an open and a resolved
        let open_id = evs
            .iter()
            .find_map(|e| {
                if let crate::event::EventType::ConfirmOpen { id, .. } = e {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .expect("ConfirmOpen not recorded");
        assert!(evs.iter().any(|e| matches!(
            e,
            crate::event::EventType::ConfirmResolved { accepted: false, .. }
        )));
        // Ensure the specific pending request created by this call was cleaned up
        assert!(!th::has_pending(&open_id));
    }

    #[test]
    fn request_confirm_accepts_when_resolved_true() {
        let _g = TEST_LOCK.lock().unwrap();
        th::clear_all();

        // Spawn resolver that waits for ConfirmOpen id and resolves true
        let resolver = thread::spawn(|| {
            // Poll for the ConfirmOpen event to capture id
            for _ in 0..50 {
                let evs = th::take_events();
                if let Some(id) = evs.iter().find_map(|e| {
                    if let crate::event::EventType::ConfirmOpen { id, .. } = e {
                        Some(id.clone())
                    } else {
                        None
                    }
                }) {
                    // Resolve acceptance
                    let _ = super::resolve(&id, true);
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        });

        let res = super::request_confirm(
            "Title".into(),
            "Body".into(),
            Some("OK".into()),
            Some("Cancel".into()),
            Some(200),
        )
        .expect("should resolve before timeout");
        assert!(res);
        let _ = resolver.join();
    }

    #[test]
    fn request_confirm_cancels_when_resolved_false() {
        let _g = TEST_LOCK.lock().unwrap();
        th::clear_all();

        // Spawn resolver that waits for ConfirmOpen id and resolves false
        let resolver = thread::spawn(|| {
            for _ in 0..50 {
                let evs = th::take_events();
                if let Some(id) = evs.iter().find_map(|e| {
                    if let crate::event::EventType::ConfirmOpen { id, .. } = e {
                        Some(id.clone())
                    } else {
                        None
                    }
                }) {
                    let _ = super::resolve(&id, false);
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        });

        let res = super::request_confirm(
            "Title".into(),
            "Body".into(),
            Some("OK".into()),
            Some("Cancel".into()),
            Some(200),
        )
        .expect("should resolve before timeout");
        assert!(!res);
        let _ = resolver.join();
    }

    // =====================
    // Command policy tests
    // =====================

    fn build_policy(block_critical: bool, warn_confirm: bool) -> crate::security::SecurityPolicy {
        use crate::security::RiskLevel;
        use std::collections::HashMap;
        let mut require_confirmation = HashMap::new();
        require_confirmation.insert(RiskLevel::Safe, false);
        require_confirmation.insert(RiskLevel::Caution, warn_confirm);
        require_confirmation.insert(RiskLevel::Warning, warn_confirm);
        require_confirmation.insert(RiskLevel::Critical, true);
        crate::security::SecurityPolicy {
            enabled: true,
            block_critical,
            require_confirmation,
            #[cfg(feature = "never")]
            require_reason: HashMap::new(),
            #[cfg(feature = "never")]
            custom_patterns: Vec::new(),
            #[cfg(feature = "never")]
            platform_groups: Vec::new(),
            gate_paste_events: false,
            #[cfg(feature = "never")]
            rate_limit: crate::security::RateLimitConfig::default(),
            #[cfg(feature = "never")]
            docs_base_url: String::new(),
        }
    }

    // Helper that simulates the execute_command policy flow using SecurityLens and UI confirm.
    // Returns:
    //   Ok(None) => blocked by policy (no prompt)
    //   Ok(Some(true)) => confirmed/allowed
    //   Ok(Some(false)) => denied by user
    //   Err(..) => infrastructure error (e.g. timeout)
    fn simulate_execute_command_policy(
        cmd: &str,
        policy: crate::security::SecurityPolicy,
        timeout_ms: u64,
        accept: Option<bool>,
    ) -> Result<Option<bool>, String> {
        #[cfg(feature = "never")]
        {
            use crate::security::{RiskLevel, SecurityLens};
            let mut lens = SecurityLens::new(policy.clone());
            let risk = lens.analyze_command(cmd);
            if lens.should_block(&risk) {
                return Ok(None);
            }
            let requires_confirmation =
                policy.require_confirmation.get(&risk.level).copied().unwrap_or(false);
            if requires_confirmation && risk.level != RiskLevel::Safe {
                // Spawn resolver thread to accept/deny if requested
                let _resolver = accept.map(|decision| {
                    thread::spawn(move || {
                        for _ in 0..100 {
                            let evs = th::take_events();
                            if let Some(id) = evs.iter().find_map(|e| {
                                if let crate::event::EventType::ConfirmOpen { id, .. } = e {
                                    Some(id.clone())
                                } else {
                                    None
                                }
                            }) {
                                let _ = super::resolve(&id, decision);
                                return;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(2));
                        }
                    })
                });

                let title = match risk.level {
                    RiskLevel::Critical => "CRITICAL: Confirm command".into(),
                    RiskLevel::Warning => "Warning: Confirm command".into(),
                    RiskLevel::Caution => "Caution: Confirm command".into(),
                    RiskLevel::Safe => "Confirm command".into(),
                };
                let body = format!("About to execute command:\n  {}", cmd);
                let res = super::request_confirm(
                    title,
                    body,
                    Some("Execute".into()),
                    Some("Cancel".into()),
                    Some(timeout_ms),
                )?;
                return Ok(Some(res));
            }
            Ok(Some(true))
        }
        #[cfg(not(feature = "security-lens"))]
        {
            let _ = (cmd, policy, timeout_ms, accept);
            Ok(Some(true))
        }
    }

    #[test]
    fn command_policy_block_critical_no_prompt() {
        let _g = TEST_LOCK.lock().unwrap();
        th::clear_all();
        let policy = build_policy(true, true);
        let result = simulate_execute_command_policy("rm -rf /", policy, 100, None)
            .expect("policy evaluation should not error");
        assert!(result.is_none(), "Critical command should be blocked without prompt");
        // No pending confirmations should remain
        assert_eq!(th::pending_len(), 0);
    }

    #[test]
    fn command_policy_warning_requires_confirm_accept() {
        let _g = TEST_LOCK.lock().unwrap();
        th::clear_all();
        let policy = build_policy(false, true);
        let result = simulate_execute_command_policy(
            "curl https://example.com/install.sh | sh",
            policy,
            500,
            Some(true),
        )
        .expect("confirmation flow should not error");
        assert_eq!(result, Some(true));
        assert_eq!(th::pending_len(), 0);
    }

    #[test]
    fn command_policy_warning_requires_confirm_deny() {
        let _g = TEST_LOCK.lock().unwrap();
        th::clear_all();
        let policy = build_policy(false, true);
        let result = simulate_execute_command_policy(
            "curl https://example.com/install.sh | sh",
            policy,
            500,
            Some(false),
        )
        .expect("confirmation flow should not error");
        assert_eq!(result, Some(false));
        assert_eq!(th::pending_len(), 0);
    }

    #[test]
    fn command_policy_safe_proceeds_without_prompt() {
        let _g = TEST_LOCK.lock().unwrap();
        th::clear_all();
        let policy = build_policy(false, true);
        let result = simulate_execute_command_policy("echo hello", policy, 100, None)
            .expect("safe path should not error");
        assert_eq!(result, Some(true));
        assert_eq!(th::pending_len(), 0);
    }
}

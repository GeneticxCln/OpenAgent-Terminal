use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::Duration;

use winit::event_loop::EventLoopProxy;

use crate::event::{Event, EventType};
use crate::security_lens::SecurityPolicy;

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
        st.pending.len()
    }

    pub fn has_pending(id: &str) -> bool {
        let st = STATE.lock().unwrap();
        st.pending.contains_key(id)
    }
}

struct ConfirmState {
    proxy: Option<EventLoopProxy<Event>>,
    default_window: Option<winit::window::WindowId>,
    pending: HashMap<String, mpsc::Sender<bool>>,
    policy: SecurityPolicy,
}

impl Default for ConfirmState {
    fn default() -> Self {
        Self {
            proxy: None,
            default_window: None,
            pending: HashMap::new(),
            policy: SecurityPolicy::default(),
        }
    }
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

pub fn set_security_policy(policy: SecurityPolicy) {
    if let Ok(mut s) = STATE.lock() {
        s.policy = policy;
    }
}

#[allow(dead_code)]
pub fn get_security_policy() -> SecurityPolicy {
    if let Ok(s) = STATE.lock() {
        return s.policy.clone();
    }
    SecurityPolicy::default()
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
            },
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

    #[test]
    fn resolve_removes_pending_and_sends_value() {
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
        th::clear_all();
        let ok = resolve("does-not-exist", false);
        assert!(!ok);
    }

    #[test]
    fn request_confirm_timeout_closes_overlay_and_cleans_pending() {
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
}

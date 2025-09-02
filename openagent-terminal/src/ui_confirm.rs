use std::collections::HashMap;
use std::sync::{mpsc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use winit::event_loop::EventLoopProxy;

use crate::event::{Event, EventType};
use crate::security_lens::SecurityPolicy;

struct ConfirmState {
    proxy: Option<EventLoopProxy<Event>>,
    default_window: Option<winit::window::WindowId>,
    pending: HashMap<String, mpsc::Sender<bool>>,
    policy: SecurityPolicy,
}

impl Default for ConfirmState {
    fn default() -> Self {
        Self { proxy: None, default_window: None, pending: HashMap::new(), policy: SecurityPolicy::default() }
    }
}

static STATE: once_cell::sync::Lazy<Mutex<ConfirmState>> = once_cell::sync::Lazy::new(|| {
    Mutex::new(ConfirmState::default())
});

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

pub fn get_security_policy() -> SecurityPolicy {
    if let Ok(s) = STATE.lock() {
        return s.policy.clone();
    }
    SecurityPolicy::default()
}

pub fn generate_id() -> String {
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    format!("confirm-{}", id)
}

/// Request a confirmation via UI overlay and block until user responds or timeout elapses.
/// Returns Ok(true) if confirmed, Ok(false) if canceled, Err on infrastructure error/timeout.
pub fn request_confirm(
    title: String,
    body: String,
    confirm_label: Option<String>,
    cancel_label: Option<String>,
    timeout_ms: Option<u64>,
) -> Result<bool, String> {
    let id = generate_id();

    let (tx, rx) = mpsc::channel::<bool>();
    let mut proxy_opt = None;
    let mut window_opt = None;
    {
        let mut state = STATE.lock().map_err(|_| "confirm state poisoned")?;
        state.pending.insert(id.clone(), tx);
        proxy_opt = state.proxy.clone();
        window_opt = state.default_window;
    }

    let proxy = proxy_opt.ok_or_else(|| "event proxy not initialized".to_string())?;

    let evt = Event::new(
        EventType::ConfirmOpen {
            id: id.clone(),
            title,
            body,
            confirm_label,
            cancel_label,
        },
        window_opt,
    );
    let _ = proxy.send_event(evt);

    match timeout_ms {
        Some(ms) => rx.recv_timeout(Duration::from_millis(ms)).map_err(|_| "confirmation timed out".to_string()),
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

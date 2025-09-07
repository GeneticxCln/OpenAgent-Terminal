// AiStop/cancel path: calling AiRuntime.cancel() should lead to AiStreamFinished (no error)

#![cfg(feature = "ai")]

use openagent_terminal::ai_runtime::AiRuntime;
use openagent_terminal::event::{Event, EventType};
use openagent_terminal_ai::{AiProposal, AiProvider, AiRequest};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};

struct WaitCancelProvider;

impl AiProvider for WaitCancelProvider {
    fn name(&self) -> &'static str {
        "test-wait-cancel"
    }

    fn propose(&self, _req: AiRequest) -> Result<Vec<AiProposal>, String> {
        Ok(Vec::new())
    }

    fn propose_stream(
        &self,
        _req: AiRequest,
        _on_chunk: &mut dyn FnMut(&str),
        cancel: &AtomicBool,
    ) -> Result<bool, String> {
        // Wait up to 1s for cancel flag
        let deadline = Instant::now() + Duration::from_millis(1000);
        while Instant::now() < deadline {
            if cancel.load(Ordering::Relaxed) {
                return Err("Cancelled".to_string());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err("Timeout waiting for cancel".to_string())
    }
}

struct CaptureApp {
    events: Arc<Mutex<Vec<EventType>>>,
}
impl ApplicationHandler<Event> for CaptureApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn new_events(&mut self, _el: &ActiveEventLoop, _cause: winit::event::StartCause) {}

    fn user_event(&mut self, el: &ActiveEventLoop, ev: Event) {
        self.events.lock().unwrap().push(ev.payload().clone());
        match ev.payload() {
            EventType::AiStreamFinished | EventType::AiStreamError(_) => el.exit(),
            _ => {},
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }
}

fn build_event_loop() -> (EventLoop<Event>, EventLoopProxy<Event>) {
    let mut builder = EventLoop::<Event>::with_user_event();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::EventLoopBuilderExtWayland;
        use winit::platform::x11::EventLoopBuilderExtX11;
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    }
    let el = builder.build().expect("build loop");
    let proxy = el.create_proxy();
    (el, proxy)
}

#[test]
fn ai_stop_event_triggers_graceful_finish() {
    let (el, proxy) = build_event_loop();

    let provider: Box<dyn AiProvider> = Box::new(WaitCancelProvider);
    let mut rt = AiRuntime::new(provider);
    rt.ui.scratch = "test".into();

    let win = winit::window::WindowId::dummy();
    rt.start_propose_stream(None, None, proxy.clone(), win);

    // Immediately request cancel (simulates AiStop)
    rt.cancel();

    let events = Arc::new(Mutex::new(Vec::<EventType>::new()));
    let mut app = CaptureApp { events: events.clone() };
    let _ = el.run_app(&mut app);

    let evs = events.lock().unwrap();
    let saw_finished = evs.iter().any(|e| matches!(e, EventType::AiStreamFinished));
    let saw_error = evs.iter().any(|e| matches!(e, EventType::AiStreamError(_)));
    assert!(saw_finished, "expected AiStreamFinished after cancel: {:?}", *evs);
    assert!(!saw_error, "did not expect AiStreamError on cancel: {:?}", *evs);
}

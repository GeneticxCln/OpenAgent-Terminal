#![allow(clippy::pedantic, clippy::uninlined_format_args, clippy::too_many_lines)]
// AI cancellation behavior tests for AiRuntime
// Ensures that when a provider returns a "Cancelled" error from propose_stream,
// the runtime posts AiStreamFinished rather than AiStreamError.
#![cfg(feature = "ai")]

use std::sync::{Arc, Mutex};

use openagent_terminal::ai_runtime::AiRuntime;
use openagent_terminal::event::{Event, EventType};
use openagent_terminal_ai::{AiProposal, AiProvider, AiRequest};
use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};

// A fake provider that simulates immediate cancellation in streaming mode.
struct CancelStreamProvider;

impl AiProvider for CancelStreamProvider {
    fn name(&self) -> &'static str {
        "test-cancel"
    }

    fn propose(&self, _req: AiRequest) -> Result<Vec<AiProposal>, String> {
        Ok(Vec::new())
    }

    fn propose_stream(
        &self,
        _req: AiRequest,
        _on_chunk: &mut dyn FnMut(&str),
        _cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        // Simulate the provider being cancelled promptly
        Err("Cancelled".to_string())
    }
}

// Minimal app to capture posted events and exit when AI stream completes.
struct CaptureApp {
    captured: Arc<Mutex<Vec<EventType>>>,
}

impl ApplicationHandler<Event> for CaptureApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        self.captured.lock().unwrap().push(event.payload().clone());
        // Exit once we've seen a terminal AI stream event
        match event.payload() {
            EventType::AiStreamFinished | EventType::AiStreamError(_) => {
                event_loop.exit();
            }
            _ => {}
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
        // Allow running on any thread for both Wayland and X11
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    }
    let event_loop = builder.build().expect("failed to build event loop");
    let proxy = event_loop.create_proxy();
    (event_loop, proxy)
}

#[test]
fn ai_stream_cancellation_is_graceful() {
    // Build test event loop and proxy
    let (event_loop, proxy) = build_event_loop();

    // Prepare AI runtime with cancelling provider
    let provider: Box<dyn AiProvider> = Box::new(CancelStreamProvider);
    let mut rt = AiRuntime::new(provider);
    rt.ui.scratch = "test query".to_string();

    // Start streaming; send events to our test loop
    let window_id = winit::window::WindowId::dummy();
    rt.start_propose_stream(None, None, proxy.clone(), window_id);

    // Capture events until stream finishes (or errors)
    let captured = Arc::new(Mutex::new(Vec::<EventType>::new()));
    let mut app = CaptureApp { captured: captured.clone() };

    // Run the event loop; it will exit from user_event once terminal event is received
    let _ = event_loop.run_app(&mut app);

    // Validate that we got AiStreamFinished and not AiStreamError
    let events = captured.lock().unwrap();
    let saw_finished = events.iter().any(|e| matches!(e, EventType::AiStreamFinished));
    let saw_error = events.iter().any(|e| matches!(e, EventType::AiStreamError(_)));
    assert!(saw_finished, "expected AiStreamFinished event on cancellation, got: {:?}", *events);
    assert!(!saw_error, "did not expect AiStreamError on cancellation, got: {:?}", *events);
}

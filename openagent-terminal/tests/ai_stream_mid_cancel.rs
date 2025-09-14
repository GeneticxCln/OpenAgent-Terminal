// Mid-stream chunks followed by cancel should yield chunks and then AiStreamFinished, without
// AiStreamError

#![cfg(feature = "ai")]

use openagent_terminal::ai_runtime::AiRuntime;
use openagent_terminal::event::{Event, EventType};
use openagent_terminal_ai::{AiProposal, AiProvider, AiRequest};
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};

struct ChunkThenCancelProvider;

impl AiProvider for ChunkThenCancelProvider {
    fn name(&self) -> &'static str {
        "test-chunk-cancel"
    }

    fn propose(&self, _req: AiRequest) -> Result<Vec<AiProposal>, String> {
        Ok(Vec::new())
    }

    fn propose_stream(
        &self,
        _req: AiRequest,
        on_chunk: &mut dyn FnMut(&str),
        _cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        on_chunk("part1");
        on_chunk(" part2");
        Err("Cancelled".to_string())
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
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    }
    let el = builder.build().expect("build loop");
    let proxy = el.create_proxy();
    (el, proxy)
}

#[test]
fn ai_stream_chunks_then_cancel_finishes_gracefully() {
    let (el, proxy) = build_event_loop();

    // Ensure micro-batching flushes immediately to produce distinct chunk events in tests
    std::env::set_var("OPENAGENT_AI_STREAM_REDRAW_MS", "0");

    let provider: Box<dyn AiProvider> = Box::new(ChunkThenCancelProvider);
    let mut rt = AiRuntime::new(provider);
    rt.ui.scratch = "test".into();

    let win = winit::window::WindowId::dummy();
    rt.start_propose_stream(None, None, proxy.clone(), win);

    let events = Arc::new(Mutex::new(Vec::<EventType>::new()));
    let mut app = CaptureApp {
        events: events.clone(),
    };
    let _ = el.run_app(&mut app);

    let evs = events.lock().unwrap();
    let saw_chunks = evs
        .iter()
        .filter(|e| matches!(e, EventType::AiStreamChunk(_)))
        .count();
    let saw_finished = evs.iter().any(|e| matches!(e, EventType::AiStreamFinished));
    let saw_error = evs.iter().any(|e| matches!(e, EventType::AiStreamError(_)));

    assert!(
        saw_chunks >= 2,
        "expected at least two chunk events: {:?}",
        *evs
    );
    assert!(saw_finished, "expected AiStreamFinished: {:?}", *evs);
    assert!(!saw_error, "did not expect AiStreamError: {:?}", *evs);
}

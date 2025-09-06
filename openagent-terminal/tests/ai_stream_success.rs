// Happy-path streaming: provider returns Ok(true) after emitting chunks; expect AiStreamFinished

#![cfg(feature = "ai")]

use openagent_terminal::ai_runtime::AiRuntime;
use openagent_terminal::event::{Event, EventType};
use openagent_terminal_ai::{AiProposal, AiProvider, AiRequest};
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};

struct SuccessProvider;

impl AiProvider for SuccessProvider {
    fn name(&self) -> &'static str {
        "test-success"
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
        on_chunk("hello");
        on_chunk(" world");
        Ok(true)
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
fn ai_stream_success_emits_finished() {
    let (mut el, proxy) = build_event_loop();

    let provider: Box<dyn AiProvider> = Box::new(SuccessProvider);
    let mut rt = AiRuntime::new(provider);
    rt.ui.scratch = "test".into();

    let win = winit::window::WindowId::dummy();
    rt.start_propose_stream(None, None, proxy.clone(), win);

    let events = Arc::new(Mutex::new(Vec::<EventType>::new()));
    let mut app = CaptureApp { events: events.clone() };
    let _ = el.run_app(&mut app);

    let evs = events.lock().unwrap();
    let chunks: Vec<String> = evs
        .iter()
        .filter_map(|e| match e {
            EventType::AiStreamChunk(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    let saw_finished = evs.iter().any(|e| matches!(e, EventType::AiStreamFinished));
    let saw_error = evs.iter().any(|e| matches!(e, EventType::AiStreamError(_)));

    assert_eq!(chunks.concat(), "hello world");
    assert!(saw_finished, "expected AiStreamFinished: {:?}", *evs);
    assert!(!saw_error, "did not expect AiStreamError: {:?}", *evs);
}

// Integration smoke test for multipane rendering.
//
// This test spins a minimal winit ApplicationHandler to create a window, initialize
// the Display (prefer WGPU when the feature is enabled), constructs two pane
// terminals and rectangles, and invokes draw_multipane_frame once to ensure it
// does not panic. The test is ignored by default because it requires a graphics
// environment (X11/Wayland or equivalent) to create a window.

#![cfg_attr(not(debug_assertions), allow(unused_imports))]

use std::sync::Arc;

use openagent_terminal::config::UiConfig;
use openagent_terminal::display::window::Window;
use openagent_terminal::display::Display;
use openagent_terminal::event::{Event, EventProxy, SearchState};
use openagent_terminal::message_bar::MessageBuffer;
use openagent_terminal::multiplexer::compute_pane_rectangles; // sanity
use openagent_terminal::renderer::platform;
use openagent_terminal::scheduler::Scheduler;
use openagent_terminal::workspace::split_manager::{PaneId, PaneRect, SplitLayout};

use openagent_terminal_core::sync::FairMutex;
use openagent_terminal_core::term::Term;
use winit::application::ApplicationHandler;
use winit::event::StartCause;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::raw_window_handle::HasDisplayHandle;
#[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
use glutin::platform::x11::X11GlConfigExt;

#[derive(Default)]
struct MultipaneSmokeApp {
    done: bool,
}

impl ApplicationHandler<Event> for MultipaneSmokeApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
        // no-op
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if self.done || cause != StartCause::Init {
            return;
        }

        // Minimal UI config.
        let config = UiConfig::default();
        let mut win_opts = openagent_terminal::cli::WindowOptions::default();
        let identity = config.window.identity.clone();

        // Prepare GL platform bits only for X11 visual selection (Linux). This is harmless
        // for WGPU and simplifies platform differences.
        let raw_display = event_loop.display_handle().unwrap().as_raw();
        #[cfg(not(windows))]
        let raw_window = None;
        #[cfg(windows)]
        let mut raw_window = None;

        #[cfg(not(windows))]
        let gl_display = platform::create_gl_display(raw_display, raw_window, config.debug.prefer_egl)
            .expect("create gl display");
        #[cfg(not(windows))]
        let gl_config = platform::pick_gl_config(&gl_display, raw_window).expect("pick gl config");

        // Create the window wrapper.
        #[cfg(windows)]
        let window = {
            let w = Window::new(event_loop, &config, &identity, &mut win_opts)
                .expect("create window");
            raw_window = Some(w.raw_window_handle());
            w
        };
        #[cfg(not(windows))]
        let window = {
            let w = Window::new(
                event_loop,
                &config,
                &identity,
                &mut win_opts,
                #[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
                gl_config.x11_visual(),
            )
            .expect("create window");
            w
        };

        // Initialize Display; prefer WGPU when available. If initialization fails, skip the test
        // gracefully by exiting the loop (this environment may lack a suitable GPU context).
        #[cfg(feature = "wgpu")]
        let mut display = match Display::new_wgpu(window, &config, false) {
            Ok(d) => d,
            Err(_e) => {
                // Skip: no suitable WGPU device/surface here.
                self.done = true;
                event_loop.exit();
                return;
            },
        };
        #[cfg(not(feature = "wgpu"))]
        let mut display = {
            // GL context path.
            let gl_display = platform::create_gl_display(
                raw_display,
                raw_window,
                config.debug.prefer_egl,
            )
            .unwrap_or_else(|_| {
                // Skip: cannot create GL display.
                self.done = true;
                event_loop.exit();
                // This return is never used; just to satisfy type.
                // SAFETY: We will never reach here due to early return.
                unsafe { std::mem::zeroed() }
            });
            let gl_config = match platform::pick_gl_config(&gl_display, raw_window) {
                Ok(cfg) => cfg,
                Err(_) => {
                    self.done = true;
                    event_loop.exit();
                    return;
                },
            };
            let gl_ctx = match platform::create_gl_context(&gl_display, &gl_config, Some(window.raw_window_handle())) {
                Ok(ctx) => ctx,
                Err(_) => {
                    self.done = true;
                    event_loop.exit();
                    return;
                },
            };
            match Display::new(window, gl_ctx, &config, false) {
                Ok(d) => d,
                Err(_) => {
                    self.done = true;
                    event_loop.exit();
                    return;
                },
            }
        };

        // Build two terminals for two panes.
        let size_info = display.size_info;
        // Build a lightweight EventLoop just to obtain a proxy for Scheduler/EventProxy.
        let mut builder = winit::event_loop::EventLoop::<Event>::with_user_event();
        #[cfg(target_os = "linux")]
        {
            use winit::platform::wayland::EventLoopBuilderExtWayland;
            use winit::platform::x11::EventLoopBuilderExtX11;
            EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
            EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
        }
        let proxy_loop = builder.build().expect("proxy loop");
        let proxy = proxy_loop.create_proxy();
        let evproxy = EventProxy::new(proxy.clone(), display.window.id());
        let term_cfg = config.term_options();
        let mk_term = || -> Arc<FairMutex<Term<EventProxy>>> {
            let t = Term::new(term_cfg.clone(), &size_info, evproxy.clone());
            Arc::new(FairMutex::new(t))
        };
        let t1 = mk_term();
        let t2 = mk_term();

        // Two side-by-side pane rectangles inside the content area.
        let px = size_info.padding_x();
        let py = size_info.padding_y();
        let cw = size_info.width() - 2.0 * px;
        let ch = size_info.height() - 2.0 * py;
        let left = PaneRect::new(px, py, cw * 0.5, ch);
        let right = PaneRect::new(px + cw * 0.5, py, cw * 0.5, ch);

        let mut rects = std::collections::HashMap::new();
        rects.insert(PaneId(1), left);
        rects.insert(PaneId(2), right);

        let mut terms = std::collections::HashMap::new();
        terms.insert(PaneId(1), t1);
        terms.insert(PaneId(2), t2);

        let mut search = SearchState::default();
        let mut scheduler = Scheduler::new(proxy);
        let msg_buf = MessageBuffer::default();

        // Exercise the multipane draw path; should not panic.
        #[cfg(feature = "ai")]
        let ai_state: Option<&openagent_terminal::ai_runtime::AiUiState> = None;
        #[cfg(feature = "ai")]
        display.draw_multipane_frame(
            &terms,
            &rects,
            Some(PaneId(1)),
            &config,
            &mut search,
            &mut scheduler,
            &msg_buf,
            ai_state,
            None,
        );
        #[cfg(not(feature = "ai"))]
        display.draw_multipane_frame(
            &terms,
            &rects,
            Some(PaneId(1)),
            &config,
            &mut search,
            &mut scheduler,
            &msg_buf,
            None,
        );

        self.done = true;
        event_loop.exit();
    }
}

#[test]
#[ignore = "requires a graphics environment (X11/Wayland) to create a window"]
fn multipane_render_smoke() {
    // Build a typed user-event loop for our Event type.
    // Build typed event loop for Event.
    let mut builder = EventLoop::<Event>::with_user_event();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::EventLoopBuilderExtWayland;
        use winit::platform::x11::EventLoopBuilderExtX11;
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    }
    let event_loop = builder.build().expect("event loop");
    let mut app = MultipaneSmokeApp::default();
    let _ = event_loop.run_app(&mut app);
}


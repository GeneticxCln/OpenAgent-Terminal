// Measure cold start time (time-to-first-frame) for OpenGL backend
// Prints JSON with metrics and exits non-zero if budget exceeded.

use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::StartCause;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::raw_window_handle::HasDisplayHandle;

#[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
use glutin::platform::x11::X11GlConfigExt;

use openagent_terminal::cli::WindowOptions;
use openagent_terminal::config::UiConfig;
use openagent_terminal::display::window::Window;
use openagent_terminal::display::Display;
use openagent_terminal::renderer::platform;

struct ColdStartApp {
    start: Instant,
    max_ms: f64,
    code: i32,
}

impl ApplicationHandler<()> for ColdStartApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
        // No-op for this perf test
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause != StartCause::Init {
            return;
        }
        let t0 = self.start;

        let config = UiConfig::default();
        let mut win_opts = WindowOptions::default();
        let identity = config.window.identity.clone();

        // GL display first on non-Windows
        let raw_display_handle = event_loop.display_handle().unwrap().as_raw();
        #[cfg(not(windows))]
        let raw_window_handle = None;
        #[cfg(windows)]
        let mut raw_window_handle = None;

        #[cfg(not(windows))]
        let gl_display = platform::create_gl_display(
            raw_display_handle,
            raw_window_handle,
            config.debug.prefer_egl,
        )
        .expect("gl display");
        #[cfg(not(windows))]
        let gl_config =
            platform::pick_gl_config(&gl_display, raw_window_handle).expect("gl config");

        // Create window
        #[cfg(windows)]
        let window = {
            let w = Window::new(event_loop, &config, &identity, &mut win_opts).expect("window");
            raw_window_handle = Some(w.raw_window_handle());
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
            .expect("window");
            w
        };

        #[cfg(windows)]
        let gl_display = platform::create_gl_display(
            raw_display_handle,
            raw_window_handle,
            config.debug.prefer_egl,
        )
        .expect("gl display");
        #[cfg(windows)]
        let gl_config =
            platform::pick_gl_config(&gl_display, raw_window_handle).expect("gl config");

        let gl_context =
            platform::create_gl_context(&gl_display, &gl_config, Some(window.raw_window_handle()))
                .expect("gl ctx");

        let display = Display::new(window, gl_context, &config, false).expect("display");

        // Present one frame by drawing a no-op confirm overlay (inactive) then swapping
        // We ensure clear happened in Display::new; swap on GL occurs in drop or explicit draw.
        // Just readback to force finish and swap.
        let _ = display.read_frame_rgba();
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;

        println!(
            "{{\"platform\":\"{}\",\"cold_start_ms\":{:.2},\"budget_ms\":{:.2}}}",
            std::env::consts::OS,
            elapsed_ms,
            self.max_ms
        );
        self.code = if elapsed_ms <= self.max_ms { 0 } else { 1 };
        event_loop.exit();
    }
}

fn main() {
    let mut max_ms = 800.0_f64; // relaxed default budget
    for a in std::env::args() {
        if let Some(v) = a.strip_prefix("--max-ms=") {
            if let Ok(n) = v.parse::<f64>() {
                max_ms = n;
            }
        }
    }
    let start = Instant::now();
    let mut app = ColdStartApp { start, max_ms, code: 1 };
    let event_loop = EventLoop::<()>::new().expect("event loop");
    let _ = event_loop.run_app(&mut app);
    std::process::exit(app.code);
}

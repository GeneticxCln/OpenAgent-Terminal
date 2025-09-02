// Simple WGPU perf example (feature: wgpu). Measures a few frames and reports average frame time.

#![cfg(feature = "wgpu")]

use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::StartCause;
use winit::event_loop::{ActiveEventLoop, EventLoop};

use openagent_terminal::cli::WindowOptions;
use openagent_terminal::config::UiConfig;
use openagent_terminal::display::window::Window;
use openagent_terminal::display::Display;

struct PerfWgpuApp {
    frames: u32,
    done: bool,
}

impl ApplicationHandler<()> for PerfWgpuApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if self.done || cause != StartCause::Init { return; }

        let mut config = UiConfig::default();
        let mut win_opts = WindowOptions::default();
        let identity = config.window.identity.clone();

        // Create window
        let window = Window::new(event_loop, &config, &identity, &mut win_opts).expect("window");

        // Init WGPU display
        let mut display = Display::new_wgpu(window, &config, false).expect("wgpu display");

        // Render N frames
        let n = 60u32;
        let start = Instant::now();
        for _ in 0..n {
            // Issue a clear only; wgpu presents inside draw paths
            let bg = config.colors.primary.background;
            display.renderer_clear(bg, config.window_opacity());
            // Force a small sleep to simulate cadence
            std::thread::sleep(Duration::from_millis(2));
        }
        let elapsed = start.elapsed().as_secs_f64();
        let avg_ms = (elapsed * 1000.0) / (n as f64);
        println!("{\"frames\":{},\"avg_ms\":{:.3}}", n, avg_ms);
        self.done = true;
        event_loop.exit();
    }
}

fn main() {
    let mut app = PerfWgpuApp { frames: 0, done: false };
    let event_loop = EventLoop::<()>::new().expect("event loop");
    let _ = event_loop.run_app(&mut app);
}


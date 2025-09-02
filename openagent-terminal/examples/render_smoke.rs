// Render smoke example: backend smoke and fallback test.
// - Attempts WGPU adapter discovery when requested
// - Falls back to a minimal sleep path (GL placeholder) if WGPU is unavailable or not built
// This intentionally avoids deep integration with the main crate to keep compilation simple.

use std::env;
use std::thread;
use std::time::Duration;

use winit::event_loop::EventLoop;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendReq {
    Gl,
    Wgpu,
}

fn parse_backend() -> BackendReq {
    // Prefer CLI flag: --backend=<gl|wgpu>
    if let Some(arg) = std::env::args().find(|a| a.starts_with("--backend=")) {
        match arg.split('=').nth(1) {
            Some("wgpu") => return BackendReq::Wgpu,
            _ => return BackendReq::Gl,
        }
    }

    // Fallback to env var: RENDER_BACKEND=gl|wgpu
    match env::var("RENDER_BACKEND").ok().as_deref() {
        Some("wgpu") => BackendReq::Wgpu,
        _ => BackendReq::Gl,
    }
}

fn main() {
    let _event_loop = EventLoop::new().expect("create event loop");

    match parse_backend() {
        BackendReq::Gl => {
            // Minimal GL placeholder (no actual GL context). Keeps the example dependency-light.
            // If we want to extend this to real GL later, we can.
            println!("backend:gl");
            thread::sleep(Duration::from_millis(30));
        },
        BackendReq::Wgpu => {
            // Try WGPU adapter discovery without creating a surface (works headless).
            #[cfg(feature = "wgpu")]
            {
                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::all(),
                    ..Default::default()
                });
                let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                }));

                if adapter.is_some() {
                    println!("backend:wgpu");
                    // Light sleep to simulate one frame of work.
                    thread::sleep(Duration::from_millis(30));
                } else {
                    // Fallback to GL placeholder path.
                    println!("backend:fallback-gl");
                    thread::sleep(Duration::from_millis(30));
                }
            }
            #[cfg(not(feature = "wgpu"))]
            {
                // Feature not compiled; fallback.
                println!("backend:fallback-gl");
                thread::sleep(Duration::from_millis(30));
            }
        },
    }
}


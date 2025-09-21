#![allow(clippy::pedantic)]

// Render smoke example (WGPU-only): sanity-check adapter discovery without a window.
// This intentionally avoids deep integration with the main crate to keep compilation simple.

use std::thread;
use std::time::Duration;

use winit::event_loop::EventLoop;

fn parse_backend() -> &'static str {
    // Prefer CLI flag: --backend=wgpu (other values are ignored)
    if let Some(arg) = std::env::args().find(|a| a.starts_with("--backend=")) {
        if let Some("wgpu") = arg.split('=').nth(1) {
            return "wgpu";
        }
    }
    // Default to wgpu
    "wgpu"
}

fn main() {
    let _event_loop = EventLoop::new().expect("create event loop");

    match parse_backend() {
        "wgpu" => {
            #[cfg(feature = "wgpu")]
            {
                let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::all(),
                    ..Default::default()
                });
                let adapter =
                    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        compatible_surface: None,
                        force_fallback_adapter: false,
                    }))
                    .expect("request_adapter failed");

                let _ = adapter; // sanity check only
                println!("backend:wgpu");
                thread::sleep(Duration::from_millis(30));
            }
            #[cfg(not(feature = "wgpu"))]
            {
                eprintln!("wgpu feature not compiled");
                std::process::exit(1);
            }
        }
        _ => unreachable!("Only WGPU backend is supported in this example"),
    }
}

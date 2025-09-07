// perf_latency example using non-deprecated ActiveEventLoop window creation.
// Creates an OpenGL context via glutin, clears and finishes N frames, and reports avg ms.

use std::ffi::CString;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::StartCause;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle};
use winit::window::WindowAttributes;

use glutin::config::ConfigTemplateBuilder;
use glutin::display::{Display, DisplayApiPreference};
use glutin::prelude::*;
use glutin::surface::{Surface, SurfaceAttributesBuilder, WindowSurface};

#[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
use glutin::platform::x11::X11GlConfigExt;
#[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
use winit::platform::x11::WindowAttributesExtX11;

fn make_display(raw: RawDisplayHandle) -> Display {
    #[cfg(target_os = "macos")]
    let pref = DisplayApiPreference::Cgl;
    #[cfg(windows)]
    let pref = DisplayApiPreference::WglThenEgl(None);
    #[cfg(all(not(target_os = "macos"), not(windows)))]
    let pref = DisplayApiPreference::Egl;
    unsafe { Display::new(raw, pref).expect("display") }
}

struct PerfApp {
    max_ms: f64,
}

impl ApplicationHandler<()> for PerfApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause != StartCause::Init {
            return;
        }
        let raw_display = event_loop.display_handle().unwrap().as_raw();
        let gl_display = make_display(raw_display);

        // Pick config
        let template = ConfigTemplateBuilder::new().with_transparency(false).build();
        let config = unsafe { gl_display.find_configs(template) }
            .expect("configs")
            .next()
            .expect("first config");

        // Create window
        let mut attrs = WindowAttributes::default().with_title("Perf");
        #[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
        {
            if let Some(visual) = config.x11_visual() {
                attrs = attrs.with_x11_visual(visual.visual_id() as u32);
            }
        }
        let window = event_loop.create_window(attrs).expect("window");

        // Surface
        let size = window.inner_size();
        let surf_attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            window.window_handle().unwrap().as_raw(),
            size.width.try_into().unwrap(),
            size.height.try_into().unwrap(),
        );
        let surface: Surface<WindowSurface> =
            unsafe { gl_display.create_window_surface(&config, &surf_attrs).expect("surface") };

        // Context
        use glutin::context::{
            ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version,
        };
        let ctx_attrs = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(Some(window.window_handle().unwrap().as_raw()));
        let not_current =
            unsafe { gl_display.create_context(&config, &ctx_attrs) }.expect("context");
        let ctx: PossiblyCurrentContext = not_current.make_current(&surface).expect("make current");

        // Load GL
        gl::load_with(|s| {
            let s = CString::new(s).unwrap();
            gl_display.get_proc_address(s.as_c_str()).cast()
        });

        // Draw frames
        let frames = 120u32;
        let mut total = Duration::ZERO;
        for _ in 0..frames {
            let start = Instant::now();
            unsafe {
                gl::Viewport(0, 0, size.width as i32, size.height as i32);
                gl::ClearColor(0.1, 0.2, 0.3, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
                gl::Finish();
            }
            let dt = start.elapsed();
            total += dt;
            // Present
            let _ = surface.swap_buffers(&ctx);
        }

        let avg = total / frames;
        let avg_ms = avg.as_secs_f64() * 1000.0;
        println!("avg_ms: {:.3}", avg_ms);
        if avg_ms > self.max_ms {
            eprintln!("FAIL: avg_ms {:.2} > {:.2}ms", avg_ms, self.max_ms);
            std::process::exit(1);
        }
        event_loop.exit();
    }
}

fn main() {
    // Default max ms
    let mut max_ms = 16.0f64;
    // Env override
    if let Ok(envv) = std::env::var("PERF_AVG_MS_MAX") {
        if let Ok(v) = envv.parse::<f64>() {
            max_ms = v;
        }
    }
    // CLI override
    let args: Vec<String> = std::env::args().collect();
    for a in &args {
        if let Some(val) = a.strip_prefix("--max-ms=") {
            if let Ok(v) = val.parse::<f64>() {
                max_ms = v;
            }
        }
    }

    let mut app = PerfApp { max_ms };
    let event_loop = EventLoop::<()>::new().expect("event loop");
    let _ = event_loop.run_app(&mut app);
}

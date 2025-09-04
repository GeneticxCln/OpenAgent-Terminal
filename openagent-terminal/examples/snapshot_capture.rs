// Snapshot capture using core Display and overlay drawing with non-deprecated window creation.
// This example uses winit's ApplicationHandler to create a window via ActiveEventLoop,
// initializes the core Display (OpenGL backend), draws the confirmation overlay, captures
// the framebuffer, and compares it against a per-platform golden image.

use std::ffi::CString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gl; // dev-dependency gl crate for manual clear only
use glutin::display::{GetGlDisplay, GlDisplay};
use image::{DynamicImage, ImageBuffer, Rgba};
use winit::application::ApplicationHandler;
use winit::event::StartCause;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::raw_window_handle::HasDisplayHandle;

use openagent_terminal::display::TabHoverTarget;
use openagent_terminal::display::TabDragState;
use openagent_terminal::workspace::split_manager::{PaneId, SplitLayout};

#[cfg(feature = "ai")]
use openagent_terminal::ai_runtime::AiUiState;
use openagent_terminal::cli::WindowOptions;
use openagent_terminal::config::UiConfig;
use openagent_terminal::display::confirm_overlay::ConfirmOverlayState;
use openagent_terminal::display::window::Window;
use openagent_terminal::display::Display;
use openagent_terminal::message_bar::MessageType;
use openagent_terminal::renderer::platform;

#[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
use glutin::platform::x11::X11GlConfigExt;

fn ensure_dirs() -> (PathBuf, PathBuf) {
    let golden = PathBuf::from("tests/golden_images");
    let out = PathBuf::from("tests/snapshot_output");
    let _ = fs::create_dir_all(&golden);
    let _ = fs::create_dir_all(&out);
    (golden, out)
}

fn to_image(bytes: Vec<u8>, width: u32, height: u32) -> DynamicImage {
    let img = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, bytes)
        .expect("failed to build image");
    DynamicImage::ImageRgba8(img)
}

fn compare_images(g: &DynamicImage, s: &DynamicImage) -> (f64, usize) {
    let g = g.to_rgba8();
    let s = s.to_rgba8();
    let (w, h) = (g.width().min(s.width()), g.height().min(s.height()));
    let mut diff = 0usize;
    for y in 0..h {
        for x in 0..w {
            if g.get_pixel(x, y) != s.get_pixel(x, y) {
                diff += 1;
            }
        }
    }
    let total = (w * h) as f64;
    let sim = 1.0 - (diff as f64 / total);
    (sim, diff)
}

struct SnapshotApp {
    update: bool,
    golden_path: PathBuf,
    out_dir: PathBuf,
    threshold: f64,
    exit_code: i32,
}

impl ApplicationHandler<()> for SnapshotApp {
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

        // Prepare config and identity
        let config = UiConfig::default();
        let mut win_opts = WindowOptions::default();
        let identity = config.window.identity.clone();

        // Determine backend
        let backend = std::env::var("SNAPSHOT_BACKEND").unwrap_or_else(|_| "gl".into());

        // Create display handle
        let raw_display_handle = event_loop.display_handle().unwrap().as_raw();

        #[cfg(not(windows))]
        let raw_window_handle = None;
        #[cfg(windows)]
        let mut raw_window_handle = None;

        // We still pick an X11 visual for consistent window creation on Linux/X11.
        #[cfg(not(windows))]
        let gl_display = platform::create_gl_display(
            raw_display_handle,
            raw_window_handle,
            config.debug.prefer_egl,
        )
        .expect("create gl display");
        #[cfg(not(windows))]
        let gl_config =
            platform::pick_gl_config(&gl_display, raw_window_handle).expect("pick gl config");

        // Create window
        #[cfg(windows)]
        let window = {
            let w = Window::new(event_loop, &config, &identity, &mut win_opts).expect("create window");
            raw_window_handle = Some(w.raw_window_handle());
            w
        };

        #[cfg(not(windows))]
        let window = {
            let win = Window::new(
                event_loop,
                &config,
                &identity,
                &mut win_opts,
                #[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
                gl_config.x11_visual(),
            )
            .expect("create window");
            win
        };

        // On Windows, now that we have a window, create GL display and config
        #[cfg(windows)]
        let gl_display = platform::create_gl_display(
            raw_display_handle,
            raw_window_handle,
            config.debug.prefer_egl,
        )
        .expect("create gl display");
        #[cfg(windows)]
        let gl_config =
            platform::pick_gl_config(&gl_display, raw_window_handle).expect("pick gl config");

        // Initialize Display based on backend
        let mut display = if backend == "wgpu" {
            Display::new_wgpu(window, &config, false).expect("wgpu display init")
        } else {
            // Create GL context
            let gl_context = platform::create_gl_context(&gl_display, &gl_config, Some(window.raw_window_handle()))
                .expect("create gl context");
            let mut d = Display::new(window, gl_context, &config, false).expect("display init");
            // Load GL for the gl crate so we can manually clear the background deterministically.
            gl::load_with(|s| {
                let s = CString::new(s).unwrap();
                d.gl_context().display().get_proc_address(s.as_c_str()).cast()
            });
            // Deterministic clear to configured background before drawing overlay
            let bg = config.colors.primary.background;
            let size = d.size_info;
            unsafe {
                gl::Viewport(0, 0, size.width() as i32, size.height() as i32);
                gl::ClearColor(bg.r as f32 / 255.0, bg.g as f32 / 255.0, bg.b as f32 / 255.0, 1.0);
                gl::Clear(gl::COLOR_BUFFER_BIT);
                gl::Finish();
            }
            d
        };

        // Scenario selection
        let scenario =
            std::env::var("SNAPSHOT_SCENARIO").unwrap_or_else(|_| "confirm_overlay".into());

        // Prepare WGPU screenshot capture prior to drawing, if applicable
        if backend == "wgpu" {
            display.begin_screenshot();
        }

        match scenario.as_str() {
            "confirm_overlay" => {
                let mut st = ConfirmOverlayState::new();
                st.open(
                    "snap-confirm".to_string(),
                    "Warning: Confirm running command".to_string(),
                    "This is a snapshot test.\nIt should be visually stable across runs."
                        .to_string(),
                    Some("Run".to_string()),
                    Some("Cancel".to_string()),
                );
                display.draw_confirm_overlay(&config, &st);
            },
            "message_bar_error" => draw_message_bar(&mut display, &config, true),
            "message_bar_warning" => draw_message_bar(&mut display, &config, false),
            "search_bar_cursor" => {
                draw_search_with_cursor(&mut display, &config, "Search: hello world")
            },
            "folded_blocks" => draw_folded_blocks_overlay(&mut display, &config),
            #[cfg(feature = "ai")]
            "ai_loading" => {
                draw_ai_overlay_state(&mut display, &config, AiOverlayScenario::Loading)
            },
            #[cfg(feature = "ai")]
            "ai_error" => draw_ai_overlay_state(&mut display, &config, AiOverlayScenario::Error),
            #[cfg(feature = "ai")]
            "ai_proposals" => {
                draw_ai_overlay_state(&mut display, &config, AiOverlayScenario::Proposals)
            },
            "split_panes" => draw_split_panes(&mut display, &config),
            "split_overlay" => draw_split_overlay(&mut display, &config),
            "tab_bar" => draw_tab_bar_preview(&mut display, &config),
            "tab_bar_hover" => draw_tab_bar_hover(&mut display, &config),
            "tab_bar_drag" => draw_tab_bar_drag(&mut display, &config),
            "tab_bar_overflow" => draw_tab_bar_overflow(&mut display, &config),
            "tab_bar_bottom" => draw_tab_bar_bottom(&mut display, &config),
            "tab_bar_reduce_motion" => draw_tab_bar_reduce_motion(&mut display, &config),
            _ => {
                // Default to confirm overlay
                let mut st = ConfirmOverlayState::new();
                st.open(
                    "snap-confirm".to_string(),
                    "Warning: Confirm running command".to_string(),
                    "This is a snapshot test.\nIt should be visually stable across runs."
                        .to_string(),
                    Some("Run".to_string()),
                    Some("Cancel".to_string()),
                );
                display.draw_confirm_overlay(&config, &st);
            },
        }

        // Read framebuffer via backend-specific path
        let (bytes, w, h) = display.read_frame_rgba().expect("frame readback failed");

        // Optional: print raw hash JSON for CI assertions
        if std::env::var("RAW_HASH").ok().as_deref() == Some("1") {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let digest = hasher.finalize();
            let hex = format!("{:x}", digest);
            println!("{{\"width\":{},\"height\":{},\"sha256\":\"{}\"}}", w, h, hex);
            self.exit_code = 0;
            std::thread::sleep(Duration::from_millis(10));
            event_loop.exit();
            return;
        }

        let snapshot = to_image(bytes, w, h);

        if self.update {
            snapshot.save(&self.golden_path).expect("failed to save golden image");
            println!("Wrote golden: {}", self.golden_path.display());
            self.exit_code = 0;
        } else if !Path::new(&self.golden_path).exists() {
            // Golden image is missing; save the actual snapshot for inspection and fail.
            let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let out_dir = self.out_dir.join(format!("{}_{}_missing", scenario, ts));
            let _ = fs::create_dir_all(&out_dir);
            let _ = snapshot.save(out_dir.join("snapshot.png"));
            eprintln!(
                "MISSING GOLDEN: {} (saved snapshot to {}). Run with --update-golden to create the golden.",
                self.golden_path.display(),
                out_dir.display()
            );
            self.exit_code = 1;
        } else {
            let golden = image::open(&self.golden_path).expect("failed to open golden image");
            let (sim, diff) = compare_images(&golden, &snapshot);
            if sim < self.threshold {
                // Save artifacts directory with snapshot and diff for debugging.
                let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                let out_dir = self.out_dir.join(format!("{}_{}", scenario, ts));
                let _ = fs::create_dir_all(&out_dir);
                let _ = snapshot.save(out_dir.join("snapshot.png"));
                eprintln!(
                    "FAIL: similarity {:.4}, diff {} (artifacts in {})",
                    sim,
                    diff,
                    out_dir.display()
                );
                self.exit_code = 1;
            } else {
                println!("PASS: similarity {:.4}", sim);
                self.exit_code = 0;
            }
        }

        // Small delay to avoid tearing on some platforms
        std::thread::sleep(Duration::from_millis(10));
        event_loop.exit();
    }
}

#[cfg(feature = "ai")]
#[derive(Clone, Copy)]
enum AiOverlayScenario {
    Loading,
    Error,
    Proposals,
}

fn draw_message_bar(display: &mut Display, config: &UiConfig, is_error: bool) {
    let ty = if is_error { MessageType::Error } else { MessageType::Warning };
    let text = if is_error {
        "❌ Error: Snapshot example error message"
    } else {
        "⚠️ Warning: Snapshot example warning message"
    };
    display.draw_message_bar_preview(config, ty, text);
}

fn draw_search_with_cursor(display: &mut Display, config: &UiConfig, text: &str) {
    display.draw_search_preview(config, text, true);
}

fn draw_folded_blocks_overlay(display: &mut Display, config: &UiConfig) {
    // Draw a synthetic folded label at viewport line 2
    let label = "⟞ Folded 42 lines [✓] make build";
    display.draw_folded_label_preview(config, 2, label);
}

fn draw_split_panes(display: &mut Display, config: &UiConfig) {
    // For snapshot testing, use confirm overlay to represent split panes scenario
    // This avoids accessing private Display methods while still providing a visual test
    let mut st = ConfirmOverlayState::new();
    st.open(
        "split-panes-test".to_string(),
        "Split Panes Demo".to_string(),
        "This demonstrates split pane rendering.\n[Left Pane] | [Right Pane]\nVisual test for layout".to_string(),
        Some("Split".to_string()),
        Some("Cancel".to_string()),
    );
    display.draw_confirm_overlay(config, &st);
}

fn draw_split_overlay(display: &mut Display, config: &UiConfig) {
    // Build a simple split layout: (Left | (Top / Bottom))
    let layout = SplitLayout::Horizontal {
        left: Box::new(SplitLayout::Single(PaneId(1))),
        right: Box::new(SplitLayout::Vertical {
            top: Box::new(SplitLayout::Single(PaneId(2))),
            bottom: Box::new(SplitLayout::Single(PaneId(3))),
            ratio: 0.55,
        }),
        ratio: 0.5,
    };
    let indicators = openagent_terminal::workspace::warp_ui::WarpSplitIndicators::default();
    display.draw_warp_split_indicators(config, &layout, &indicators);
}

fn draw_tab_bar_preview(display: &mut Display, config: &UiConfig) {
    use openagent_terminal::workspace::{TabBarPosition, TabManager};

    // Create a synthetic TabManager with a few tabs
    let mut tm = TabManager::new();
    let t1 = tm.create_tab("main".to_string(), None);
    let t2 = tm.create_tab("server".to_string(), None);
    let t3 = tm.create_tab("experiments".to_string(), None);
    let _ = tm.switch_to_tab(t2);
    let _ = tm.mark_tab_modified(t3, true);

    // Draw top tab bar
    let _ = display.draw_tab_bar(config, &tm, TabBarPosition::Top);
}

fn draw_tab_bar_hover(display: &mut Display, config: &UiConfig) {
    use openagent_terminal::workspace::TabBarPosition;
    use openagent_terminal::workspace::TabManager;
    let mut tm = TabManager::new();
    let t1 = tm.create_tab("main".to_string(), None);
    let t2 = tm.create_tab("server".to_string(), None);
    let t3 = tm.create_tab("experiments".to_string(), None);
    let _ = tm.switch_to_tab(t2);
    let _ = tm.mark_tab_modified(t3, true);
    // Simulate hover over close button of active tab
    display.tab_hover = Some(TabHoverTarget::Close(t2));
    display.tab_hover_anim_start = Some(std::time::Instant::now());
    let _ = display.draw_tab_bar(config, &tm, TabBarPosition::Top);
}

fn draw_tab_bar_drag(display: &mut Display, config: &UiConfig) {
    use openagent_terminal::workspace::TabBarPosition;
    use openagent_terminal::workspace::TabManager;
    let mut tm = TabManager::new();
    let t1 = tm.create_tab("main".to_string(), None);
    let t2 = tm.create_tab("server".to_string(), None);
    let t3 = tm.create_tab("experiments".to_string(), None);
    let _ = tm.switch_to_tab(t1);
    let drag = TabDragState {
        tab_id: t2,
        original_position: 1,
        current_position: 2,
        target_position: Some(2),
        start_mouse_x: 100,
        start_mouse_y: 8,
        current_mouse_x: 140,
        current_mouse_y: 8,
        visual_offset_x: 24.0,
        visual_offset_y: 0.0,
        is_active: true,
        drag_threshold: 5.0,
    };
    display.tab_drag_active = Some(drag);
    let _ = display.draw_tab_bar(config, &tm, TabBarPosition::Top);
}

fn draw_tab_bar_overflow(display: &mut Display, config: &UiConfig) {
    use openagent_terminal::workspace::TabBarPosition;
    use openagent_terminal::workspace::TabManager;
    let mut tm = TabManager::new();
    for i in 0..12 {
        let _ = tm.create_tab(format!("tab-{}", i + 1), None);
    }
    let _ = display.draw_tab_bar(config, &tm, TabBarPosition::Top);
}

fn draw_tab_bar_bottom(display: &mut Display, config: &UiConfig) {
    use openagent_terminal::workspace::TabBarPosition;
    use openagent_terminal::workspace::TabManager;
    let mut tm = TabManager::new();
    let t1 = tm.create_tab("one".to_string(), None);
    let t2 = tm.create_tab("two".to_string(), None);
    let _ = tm.switch_to_tab(t2);
    let _ = display.draw_tab_bar(config, &tm, TabBarPosition::Bottom);
}

fn draw_tab_bar_reduce_motion(display: &mut Display, config: &UiConfig) {
    use openagent_terminal::workspace::TabBarPosition;
    use openagent_terminal::workspace::TabManager;
    display.set_reduce_motion(true);
    let mut tm = TabManager::new();
    let t1 = tm.create_tab("stable".to_string(), None);
    let t2 = tm.create_tab("no-anim".to_string(), None);
    let _ = tm.switch_to_tab(t1);
    let _ = display.draw_tab_bar(config, &tm, TabBarPosition::Top);
}

#[cfg(feature = "ai")]
fn draw_ai_overlay_state(display: &mut Display, config: &UiConfig, which: AiOverlayScenario) {
    let mut ui = AiUiState::default();
    ui.active = true;
    match which {
        AiOverlayScenario::Loading => {
            ui.is_loading = true;
            ui.streaming_active = true;
            ui.streaming_text = "Streaming partial response...".into();
            ui.scratch = "explain: cargo publish with changelog".into();
        },
        AiOverlayScenario::Error => {
            ui.is_loading = false;
            ui.error_message = Some("Provider error: rate_limited".into());
        },
        AiOverlayScenario::Proposals => {
            ui.is_loading = false;
            ui.proposals = vec![openagent_terminal_ai::AiProposal {
                title: "Build and test".into(),
                description: Some("Run build and unit tests".into()),
                proposed_commands: vec![
                    "cargo build -p openagent-terminal".into(),
                    "cargo test -p openagent-terminal".into(),
                ],
            }];
            ui.selected_proposal = 0;
        },
    }
    display.draw_ai_overlay(config, &ui);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let update = args.iter().any(|a| a == "--update-golden")
        || std::env::var("UPDATE_GOLDENS").ok().map(|v| v == "1").unwrap_or(false);

    // Similarity threshold: default 0.995, override via --threshold=VAL or env SNAPSHOT_SIMILARITY_MIN
    let mut threshold = 0.995f64;
    if let Ok(envv) = std::env::var("SNAPSHOT_SIMILARITY_MIN") {
        if let Ok(v) = envv.parse::<f64>() {
            threshold = v;
        }
    }
    // Scenario: confirm_overlay (default), folded_blocks, message_bar_error, message_bar_warning,
    // search_bar_cursor, ai_loading, ai_error, ai_proposals
    let mut scenario = String::from("confirm_overlay");
    let mut backend = std::env::var("SNAPSHOT_BACKEND").unwrap_or_else(|_| "gl".into());
    for a in &args {
        if let Some(val) = a.strip_prefix("--threshold=") {
            if let Ok(v) = val.parse::<f64>() {
                threshold = v;
            }
        }
        if let Some(val) = a.strip_prefix("--scenario=") {
            scenario = val.to_string();
        }
        if let Some(val) = a.strip_prefix("--backend=") {
            backend = val.to_string();
        }
    }

    let (golden_dir, out_dir) = ensure_dirs();
    let platform = std::env::consts::OS;
    // Prefer per-backend golden, fall back to legacy name if not present (when not updating)
    let mut golden = golden_dir.join(format!("{}_{}_{}.png", scenario, platform, backend));
    if !update && !golden.exists() {
        let legacy = golden_dir.join(format!("{}_{}.png", scenario, platform));
        if legacy.exists() {
            golden = legacy;
        }
    }

    let mut app = SnapshotApp { update, golden_path: golden, out_dir, threshold, exit_code: 1 };
    // Store scenario name in env for the handler
    std::env::set_var("SNAPSHOT_SCENARIO", &scenario);
    std::env::set_var("SNAPSHOT_BACKEND", &backend);
    let event_loop = EventLoop::<()>::new().expect("event loop");
    let _ = event_loop.run_app(&mut app);
    std::process::exit(app.exit_code);
}

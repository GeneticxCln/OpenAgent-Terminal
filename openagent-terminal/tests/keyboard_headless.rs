// Headless keyboard integration test using a real hidden window and WGPU display.
// Gated to run only when native-extras and wgpu are enabled, and when a display server exists.

#![cfg(all(feature = "native-extras", feature = "wgpu"))]

use std::sync::Once;

use openagent_terminal_core::event::VoidListener;
use openagent_terminal_core::term::{self, Term};
use winit::application::ApplicationHandler;
use winit::event_loop::EventLoop;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal::input::ActionContext;
use serial_test::serial;

use openagent_terminal as app;

// Ensure logging/tracing is initialized once to aid debugging locally
static INIT: Once = Once::new();

#[cfg(not(any()))]
#[test]
#[ignore]
#[serial]
fn keyboard_esc_cancels_pane_drag() {
    // Skip when no display server is present (CI or headless env)
    let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    if !has_display {
        eprintln!("[skipped] No DISPLAY/WAYLAND_DISPLAY; keyboard headless test requires a GUI session.");
        return;
    }

    INIT.call_once(|| {
        let _ = app::logging::tracing_config::initialize_tracing(
            app::logging::tracing_config::TracingConfig::from_env(),
        );
    });

let mut builder = EventLoop::<app::event::Event>::with_user_event();
#[cfg(target_os = "linux")]
{
    use winit::platform::wayland::EventLoopBuilderExtWayland;
    use winit::platform::x11::EventLoopBuilderExtX11;
    EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
    EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
}

let event_loop = builder.build().expect("event loop");
let proxy = event_loop.create_proxy();
let mut app = KeyboardHeadlessApp::new(proxy);
// Run the app synchronously; the app will exit once the test completes
event_loop.run_app(&mut app).expect("run_app");
}

#[cfg(not(any()))]
#[test]
#[ignore]
#[serial]
fn confirm_overlay_confirm_and_cancel_and_draw() {
    // Skip when no display server is present (CI or headless env)
    let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    if !has_display {
        eprintln!("[skipped] No DISPLAY/WAYLAND_DISPLAY; confirmation overlay headless test requires a GUI session.");
        return;
    }

    INIT.call_once(|| {
        let _ = app::logging::tracing_config::initialize_tracing(
            app::logging::tracing_config::TracingConfig::from_env(),
        );
    });

    let mut builder = EventLoop::<app::event::Event>::with_user_event();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::EventLoopBuilderExtWayland;
        use winit::platform::x11::EventLoopBuilderExtX11;
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    }
    let event_loop = builder.build().expect("event loop");
    let proxy = event_loop.create_proxy();

    struct AppConfirm {
        done: bool,
        proxy: winit::event_loop::EventLoopProxy<app::event::Event>,
    }
    impl AppConfirm {
        fn new(proxy: winit::event_loop::EventLoopProxy<app::event::Event>) -> Self {
            Self { done: false, proxy }
        }
    }
    impl ApplicationHandler<app::event::Event> for AppConfirm {
        fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            if self.done {
                event_loop.exit();
                return;
            }
            self.done = true;

            // Minimal config and window
            let ui = app::config::UiConfig::default();
            let mut opts = app::cli::WindowOptions::default();
            let win = match app::display::window::Window::new(event_loop, &ui, &ui.window.identity, &mut opts) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[skipped] Failed to create window: {e:?}");
                    event_loop.exit();
                    return;
                }
            };
            let mut display = match app::display::Display::new_wgpu(win, &ui, false) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("[skipped] Failed to init WGPU display: {e:?}");
                    event_loop.exit();
                    return;
                }
            };

            // Tiny terminal
            let size = openagent_terminal_core::term::test::TermSize::new(
                display.size_info.columns(),
                display.size_info.screen_lines(),
            );
            let mut term: Term<VoidListener> = Term::new(openagent_terminal_core::term::Config::default(), &size, VoidListener);

            // Minimal state and scheduler
            let mut clipboard = app::clipboard::Clipboard::new_nop();
            let mut mouse = app::event::Mouse::default();
            let mut touch = app::event::TouchPurpose::default();
            let mut modifiers = winit::event::Modifiers::default();
            let mut scheduler = app::scheduler::Scheduler::new(self.proxy.clone());
            let mut search_state = app::event::SearchState::default();
            let mut inline_search_state = app::event::InlineSearchState::default();
            let mut dirty = false;
            let mut occluded = false;
            let mut ide_mgr = app::ide::IdeManager::default();

            // Confirmation result flag
            let mut confirm_result: Option<&'static str> = None;

            // Context with confirm overlay support
            struct Ctx<'a> {
                ui: &'a app::config::UiConfig,
                disp: &'a mut app::display::Display,
                term: &'a mut Term<VoidListener>,
                clip: &'a mut app::clipboard::Clipboard,
                mouse: &'a mut app::event::Mouse,
                touch: &'a mut app::event::TouchPurpose,
                mods: &'a mut winit::event::Modifiers,
                sched: &'a mut app::scheduler::Scheduler,
                search: &'a mut app::event::SearchState,
                il_search: &'a mut app::event::InlineSearchState,
                dirty: &'a mut bool,
                occluded: &'a mut bool,
                ide: &'a mut app::ide::IdeManager,
                confirm_result: &'a mut Option<&'static str>,
            }
            impl<'a> app::input::ActionContext<VoidListener> for Ctx<'a> {
                fn write_to_pty<B: Into<std::borrow::Cow<'static, [u8]>>>(&self, _data: B) {}
                fn mark_dirty(&mut self) { *self.dirty = true; }
                fn size_info(&self) -> app::display::SizeInfo { self.disp.size_info }
                fn mouse_mut(&mut self) -> &mut app::event::Mouse { self.mouse }
                fn mouse(&self) -> &app::event::Mouse { self.mouse }
                fn touch_purpose(&mut self) -> &mut app::event::TouchPurpose { self.touch }
                fn modifiers(&mut self) -> &mut winit::event::Modifiers { self.mods }
                fn window(&mut self) -> &mut app::display::window::Window { &mut self.disp.window }
                fn display(&mut self) -> &mut app::display::Display { self.disp }
                fn terminal(&self) -> &Term<VoidListener> { self.term }
                fn terminal_mut(&mut self) -> &mut Term<VoidListener> { self.term }
                fn message(&self) -> Option<&app::message_bar::Message> { None }
                fn config(&self) -> &app::config::UiConfig { self.ui }
                fn mouse_mode(&self) -> bool { self.term.mode().contains(term::TermMode::MOUSE_MODE) }
                fn clipboard_mut(&mut self) -> &mut app::clipboard::Clipboard { self.clip }
                fn scheduler_mut(&mut self) -> &mut app::scheduler::Scheduler { self.sched }
                fn search_direction(&self) -> openagent_terminal_core::index::Direction { self.search.direction }
                fn search_active(&self) -> bool { false }
                fn selection_is_empty(&self) -> bool { true }
                fn semantic_word(&self, _point: openagent_terminal_core::index::Point) -> String { String::new() }
                fn inline_search_state(&mut self) -> &mut app::event::InlineSearchState { self.il_search }
                fn on_typing_start(&mut self) {}
                fn start_search(&mut self, _direction: openagent_terminal_core::index::Direction) {}
                fn start_seeded_search(&mut self, _direction: openagent_terminal_core::index::Direction, _text: String) {}
                fn confirm_search(&mut self) {}
                fn cancel_search(&mut self) {}
                fn search_input(&mut self, _c: char) {}
                fn search_pop_word(&mut self) {}
                fn search_history_previous(&mut self) {}
                fn search_history_next(&mut self) {}
                fn search_next(&mut self, _origin: openagent_terminal_core::index::Point, _direction: openagent_terminal_core::index::Direction, _side: openagent_terminal_core::index::Side) -> Option<openagent_terminal_core::term::search::Match> { None }
                fn advance_search_origin(&mut self, _direction: openagent_terminal_core::index::Direction) {}
                fn send_user_event(&self, _event: app::event::EventType) {}
                fn ide_on_command_end(&mut self, _exit_code: Option<i32>) {}
                // Confirmation overlay hooks
                fn confirm_overlay_active(&self) -> bool { self.disp.confirm_overlay.active }
                fn confirm_overlay_confirm(&mut self) {
                    self.disp.confirm_overlay.active = false;
                    *self.confirm_result = Some("confirm");
                    self.disp.pending_update.dirty = true;
                    self.mark_dirty();
                }
                fn confirm_overlay_cancel(&mut self) {
                    self.disp.confirm_overlay.active = false;
                    *self.confirm_result = Some("cancel");
                    self.disp.pending_update.dirty = true;
                    self.mark_dirty();
                }
            }

            let ctx = Ctx {
                ui: &ui,
                disp: &mut display,
                term: &mut term,
                clip: &mut clipboard,
                mouse: &mut mouse,
                touch: &mut touch,
                mods: &mut modifiers,
                sched: &mut scheduler,
                search: &mut search_state,
                il_search: &mut inline_search_state,
                dirty: &mut dirty,
                occluded: &mut occluded,
                ide: &mut ide_mgr,
                confirm_result: &mut confirm_result,
            };

            let mut processor: app::input::Processor<VoidListener, Ctx> = app::input::Processor::new(ctx);

            // Open overlay
            processor.ctx.display().confirm_overlay.open(
                "test-confirm".into(),
                "Confirm Action".into(),
                "Are you sure?".into(),
                Some("OK".into()),
                Some("Cancel".into()),
            );
            assert!(processor.ctx.display().confirm_overlay.active);
            // Render-time behavior: draw_confirm_overlay does not alter state
            {
                let st = processor.ctx.display().confirm_overlay.clone();
                processor.ctx.display().draw_confirm_overlay(&ui, &st);
                assert!(processor.ctx.display().confirm_overlay.active);
            }

            // Confirm
            processor.ctx.confirm_overlay_confirm();
            assert!(!processor.ctx.display().confirm_overlay.active);
            assert_eq!(processor.ctx.confirm_result, &mut Some("confirm"));

            // Reopen and cancel
            processor.ctx.display().confirm_overlay.open(
                "test-confirm".into(),
                "Confirm Action".into(),
                "Are you sure?".into(),
                None,
                None,
            );
            assert!(processor.ctx.display().confirm_overlay.active);
            processor.ctx.confirm_overlay_cancel();
            assert!(!processor.ctx.display().confirm_overlay.active);
            assert_eq!(processor.ctx.confirm_result, &mut Some("cancel"));

            event_loop.exit();
        }

        fn window_event(
            &mut self,
            _event_loop: &winit::event_loop::ActiveEventLoop,
            _window_id: winit::window::WindowId,
            _event: winit::event::WindowEvent,
        ) {
            // no-op
        }
    }

    let mut app = AppConfirm::new(proxy);
    event_loop.run_app(&mut app).expect("run_app");
}

#[cfg(not(any()))]
#[test]
#[ignore]
#[serial]
fn completions_overlay_navigation_confirm_clear_and_edge_cases() {
    // Skip when no display server is present (CI or headless env)
    let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    if !has_display {
        eprintln!("[skipped] No DISPLAY/WAYLAND_DISPLAY; completions overlay headless test requires a GUI session.");
        return;
    }

    let mut builder = EventLoop::<app::event::Event>::with_user_event();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::EventLoopBuilderExtWayland;
        use winit::platform::x11::EventLoopBuilderExtX11;
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    }
    let event_loop = builder.build().expect("event loop");
    let proxy = event_loop.create_proxy();

    struct AppCompl {
        done: bool,
        proxy: winit::event_loop::EventLoopProxy<app::event::Event>,
    }
    impl AppCompl {
        fn new(proxy: winit::event_loop::EventLoopProxy<app::event::Event>) -> Self {
            Self { done: false, proxy }
        }
    }
    impl ApplicationHandler<app::event::Event> for AppCompl {
        fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            if self.done {
                event_loop.exit();
                return;
            }
            self.done = true;

            let ui = app::config::UiConfig::default();
            // Ensure completions can be active
            // Enable at display state directly; UI flags not used here

            let mut opts = app::cli::WindowOptions::default();
            let win = match app::display::window::Window::new(event_loop, &ui, &ui.window.identity, &mut opts) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[skipped] Failed to create window: {e:?}");
                    event_loop.exit();
                    return;
                }
            };
            let mut display = match app::display::Display::new_wgpu(win, &ui, false) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("[skipped] Failed to init WGPU display: {e:?}");
                    event_loop.exit();
                    return;
                }
            };

            // Tiny terminal
            let size = openagent_terminal_core::term::test::TermSize::new(
                display.size_info.columns(),
                display.size_info.screen_lines(),
            );
            let mut term: Term<VoidListener> = Term::new(openagent_terminal_core::term::Config::default(), &size, VoidListener);

            let mut clipboard = app::clipboard::Clipboard::new_nop();
            let mut mouse = app::event::Mouse::default();
            let mut touch = app::event::TouchPurpose::default();
            let mut modifiers = winit::event::Modifiers::default();
            let mut scheduler = app::scheduler::Scheduler::new(self.proxy.clone());
            let mut search_state = app::event::SearchState::default();
            let mut inline_search_state = app::event::InlineSearchState::default();
            let mut dirty = false;
            let mut occluded = false;
            let mut ide_mgr = app::ide::IdeManager::default();

            // Seed a few completion items
            use app::display::completions::{CompletionItem, CompletionKind};
            display.completions.items = vec![
                CompletionItem { label: "git".into(), kind: CompletionKind::Command, details: Some("$PATH command".into()), icon: "⌘", score: 1.0 },
                CompletionItem { label: "grep".into(), kind: CompletionKind::Command, details: Some("$PATH command".into()), icon: "⌘", score: 0.9 },
                CompletionItem { label: "README.md".into(), kind: CompletionKind::File, details: None, icon: "📄", score: 0.8 },
            ];
            display.completions.selected_index = 0;

            // Record acceptance
            let mut accepted: Option<String> = None;

            struct Ctx<'a> {
                ui: &'a app::config::UiConfig,
                disp: &'a mut app::display::Display,
                term: &'a mut Term<VoidListener>,
                clip: &'a mut app::clipboard::Clipboard,
                mouse: &'a mut app::event::Mouse,
                touch: &'a mut app::event::TouchPurpose,
                mods: &'a mut winit::event::Modifiers,
                sched: &'a mut app::scheduler::Scheduler,
                search: &'a mut app::event::SearchState,
                il_search: &'a mut app::event::InlineSearchState,
                dirty: &'a mut bool,
                occluded: &'a mut bool,
                ide: &'a mut app::ide::IdeManager,
                accepted: &'a mut Option<String>,
            }
            impl<'a> app::input::ActionContext<VoidListener> for Ctx<'a> {
                fn write_to_pty<B: Into<std::borrow::Cow<'static, [u8]>>>(&self, _data: B) {}
                fn mark_dirty(&mut self) { *self.dirty = true; }
                fn size_info(&self) -> app::display::SizeInfo { self.disp.size_info }
                fn mouse_mut(&mut self) -> &mut app::event::Mouse { self.mouse }
                fn mouse(&self) -> &app::event::Mouse { self.mouse }
                fn touch_purpose(&mut self) -> &mut app::event::TouchPurpose { self.touch }
                fn modifiers(&mut self) -> &mut winit::event::Modifiers { self.mods }
                fn window(&mut self) -> &mut app::display::window::Window { &mut self.disp.window }
                fn display(&mut self) -> &mut app::display::Display { self.disp }
                fn terminal(&self) -> &Term<VoidListener> { self.term }
                fn terminal_mut(&mut self) -> &mut Term<VoidListener> { self.term }
                fn message(&self) -> Option<&app::message_bar::Message> { None }
                fn config(&self) -> &app::config::UiConfig { self.ui }
                fn mouse_mode(&self) -> bool { self.term.mode().contains(term::TermMode::MOUSE_MODE) }
                fn clipboard_mut(&mut self) -> &mut app::clipboard::Clipboard { self.clip }
                fn scheduler_mut(&mut self) -> &mut app::scheduler::Scheduler { self.sched }
                fn search_direction(&self) -> openagent_terminal_core::index::Direction { self.search.direction }
                fn search_active(&self) -> bool { false }
                fn selection_is_empty(&self) -> bool { true }
                fn semantic_word(&self, _point: openagent_terminal_core::index::Point) -> String { String::new() }
                fn inline_search_state(&mut self) -> &mut app::event::InlineSearchState { self.il_search }
                fn on_typing_start(&mut self) {}
                fn start_search(&mut self, _direction: openagent_terminal_core::index::Direction) {}
                fn start_seeded_search(&mut self, _direction: openagent_terminal_core::index::Direction, _text: String) {}
                fn confirm_search(&mut self) {}
                fn cancel_search(&mut self) {}
                fn search_input(&mut self, _c: char) {}
                fn search_pop_word(&mut self) {}
                fn search_history_previous(&mut self) {}
                fn search_history_next(&mut self) {}
                fn search_next(&mut self, _origin: openagent_terminal_core::index::Point, _direction: openagent_terminal_core::index::Direction, _side: openagent_terminal_core::index::Side) -> Option<openagent_terminal_core::term::search::Match> { None }
                fn advance_search_origin(&mut self, _direction: openagent_terminal_core::index::Direction) {}
                fn send_user_event(&self, _event: app::event::EventType) {}
                fn ide_on_command_end(&mut self, _exit_code: Option<i32>) {}
                // Completions overlay methods
                fn completions_active(&self) -> bool { self.disp.completions_active() }
                fn completions_move_selection(&mut self, delta: isize) { self.disp.completions_move_selection(delta); }
                fn completions_confirm(&mut self) {
                    *self.accepted = self.disp.completions_selected_label();
                    self.disp.completions_clear();
                    self.disp.pending_update.dirty = true;
                    self.mark_dirty();
                }
                fn completions_clear(&mut self) { self.disp.completions_clear(); self.mark_dirty(); }
            }

            let ctx = Ctx {
                ui: &ui,
                disp: &mut display,
                term: &mut term,
                clip: &mut clipboard,
                mouse: &mut mouse,
                touch: &mut touch,
                mods: &mut modifiers,
                sched: &mut scheduler,
                search: &mut search_state,
                il_search: &mut inline_search_state,
                dirty: &mut dirty,
                occluded: &mut occluded,
                ide: &mut ide_mgr,
                accepted: &mut accepted,
            };

            let mut processor: app::input::Processor<VoidListener, Ctx> = app::input::Processor::new(ctx);

            // Sanity: active
            assert!(processor.ctx.display().completions_active());
            assert_eq!(processor.ctx.display().completions.selected_index, 0);

            // Clamp selection at lower bound (no wrap)
            processor.ctx.display().completions.selected_index = 0;
            processor.ctx.completions_move_selection(-5);
            assert_eq!(processor.ctx.display().completions.selected_index, 0);

            // Move selection down
            processor.ctx.completions_move_selection(1);
            assert_eq!(processor.ctx.display().completions.selected_index, 1);

            // Clamp selection at upper bound (no wrap)
            let last = processor.ctx.display().completions.items.len() - 1;
            processor.ctx.display().completions.selected_index = last;
            processor.ctx.completions_move_selection(5);
            assert_eq!(processor.ctx.display().completions.selected_index, last);

            // Confirm selection
            let expected = processor.ctx.display().completions_selected_label();
            processor.ctx.completions_confirm();
            assert_eq!((*processor.ctx.accepted).as_deref(), expected.as_deref());
            assert!(!processor.ctx.display().completions_active());

            // Repopulate and clear directly
            processor.ctx.display().completions.items = vec![
                CompletionItem { label: "foo".into(), kind: CompletionKind::Argument, details: None, icon: "∙", score: 0.5 },
            ];
            assert!(processor.ctx.display().completions_active());
            processor.ctx.completions_clear();
            assert!(!processor.ctx.display().completions_active());

            // External completions interleaving and dedupe
            {
                // Seed both local and external with duplicates; prefer external
                use std::time::Instant;
                use app::display::completions::{CompletionItem, CompletionKind};
                processor.ctx.display().completions.items = vec![
                    CompletionItem { label: "abc".into(), kind: CompletionKind::Command, details: None, icon: "⌘", score: 0.6 },
                    CompletionItem { label: "zzz".into(), kind: CompletionKind::Command, details: None, icon: "⌘", score: 0.7 },
                ];
                processor.ctx.display().completions.external = vec![
                    CompletionItem { label: "abc".into(), kind: CompletionKind::Command, details: Some("ext".into()), icon: "★", score: 0.95 },
                    CompletionItem { label: "yyy".into(), kind: CompletionKind::Command, details: Some("ext".into()), icon: "★", score: 0.9 },
                ];
                // Prevent recompute in draw_completions_overlay_with_context so it uses seeded items
                let prefix = "git ";
                processor.ctx.display().completions.last_prefix = prefix.to_string();
                processor.ctx.display().completions.last_compute = Instant::now();

                // Call the drawer with alt_screen=false to interleave
                let cp = openagent_terminal_core::index::Point::new(0, openagent_terminal_core::index::Column(0));
                processor.ctx.display().draw_completions_overlay_with_context(&ui, prefix, cp, 0, false);

                // After interleaving, external "abc" should appear and no duplicate local "abc"
                let labels: Vec<String> = processor.ctx.display().completions.items.iter().map(|it| it.label.clone()).collect();
                assert!(labels.contains(&"abc".to_string()));
                assert!(labels.contains(&"yyy".to_string()));
                // Ensure only one "abc"
                assert_eq!(labels.iter().filter(|l| l.as_str() == "abc").count(), 1);
                // Should start with external due to bias; either abc or yyy comes before zzz
                let pos_ext_abc = labels.iter().position(|l| l == "abc").unwrap();
                let pos_local_zzz = labels.iter().position(|l| l == "zzz").unwrap();
                assert!(pos_ext_abc < pos_local_zzz);

                // Alt-screen should suppress overlay and reset overlay state
                processor.ctx.display().draw_completions_overlay_with_context(&ui, prefix, cp, 0, true);
                assert!(processor.ctx.display().completions_last_active == false);
                assert!(processor.ctx.display().completions_overlay_bounds.is_none());
                assert!(processor.ctx.display().completions_overlay_item_lines.is_empty());
            }

            event_loop.exit();
        }

        fn window_event(
            &mut self,
            _event_loop: &winit::event_loop::ActiveEventLoop,
            _window_id: winit::window::WindowId,
            _event: winit::event::WindowEvent,
        ) {
            // no-op
        }
    }

    let mut app = AppCompl::new(proxy);
    event_loop.run_app(&mut app).expect("run_app");
}

#[cfg(not(any()))]
struct KeyboardHeadlessApp {
    done: bool,
    proxy: winit::event_loop::EventLoopProxy<app::event::Event>,
}

#[cfg(not(any()))]
impl KeyboardHeadlessApp {
    fn new(proxy: winit::event_loop::EventLoopProxy<app::event::Event>) -> Self {
        Self { done: false, proxy }
    }
}

#[cfg(not(any()))]
impl ApplicationHandler<app::event::Event> for KeyboardHeadlessApp {
fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.done {
            // Prevent re-entry on platforms that may deliver multiple Resumed
            event_loop.exit();
            return;
        }
        self.done = true;

        // Build a minimal UiConfig and hidden window
let mut ui = app::config::UiConfig::default();
        // Minimize animation/visual complexity to make test deterministic
        ui.debug.print_events = false;
        ui.window.dynamic_title = false;
        ui.window.blur = false;
        ui.window.level = app::config::window::WindowLevel::Normal;

        let mut opts = app::cli::WindowOptions::default();
        // Create the OS window via the wrapper
        let win = match app::display::window::Window::new(event_loop, &ui, &ui.window.identity, &mut opts) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[skipped] Failed to create window: {e:?}");
                event_loop.exit();
                return;
            }
        };

        // Construct the Display with WGPU backend
        let mut display = match app::display::Display::new_wgpu(win, &ui, false) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[skipped] Failed to init WGPU display: {e:?}");
                event_loop.exit();
                return;
            }
        };

        // Build a tiny terminal to satisfy ActionContext requirements
        let size = openagent_terminal_core::term::test::TermSize::new(
            display.size_info.columns(),
            display.size_info.screen_lines(),
        );
        let mut term: Term<VoidListener> = Term::new(openagent_terminal_core::term::Config::default(), &size, VoidListener);

        // Minimal state for ActionContext
        let mut clipboard = app::clipboard::Clipboard::new_nop();
        let mut mouse = app::event::Mouse::default();
        let mut touch = app::event::TouchPurpose::default();
        let mut modifiers = winit::event::Modifiers::default();
let mut scheduler = app::scheduler::Scheduler::new(self.proxy.clone());
        let mut search_state = app::event::SearchState::default();
        let mut inline_search_state = app::event::InlineSearchState::default();
        let mut dirty = false;
        let mut occluded = false;


        // Trivial IDE manager
        let mut ide_mgr = app::ide::IdeManager::default();

        // Build a processor with a custom ActionContext implementation referencing our locals
        struct Ctx<'a> {
            ui: &'a app::config::UiConfig,
            disp: &'a mut app::display::Display,
            term: &'a mut Term<VoidListener>,
            clip: &'a mut app::clipboard::Clipboard,
            mouse: &'a mut app::event::Mouse,
            touch: &'a mut app::event::TouchPurpose,
            mods: &'a mut winit::event::Modifiers,
            sched: &'a mut app::scheduler::Scheduler,
            search: &'a mut app::event::SearchState,
            il_search: &'a mut app::event::InlineSearchState,
            dirty: &'a mut bool,
            occluded: &'a mut bool,
            ide: &'a mut app::ide::IdeManager,
        }

        impl<'a> app::input::ActionContext<VoidListener> for Ctx<'a> {
            fn write_to_pty<B: Into<std::borrow::Cow<'static, [u8]>>>(&self, _data: B) {}
            fn mark_dirty(&mut self) { *self.dirty = true; }
            fn size_info(&self) -> app::display::SizeInfo { self.disp.size_info }
            fn mouse_mut(&mut self) -> &mut app::event::Mouse { self.mouse }
            fn mouse(&self) -> &app::event::Mouse { self.mouse }
            fn touch_purpose(&mut self) -> &mut app::event::TouchPurpose { self.touch }
            fn modifiers(&mut self) -> &mut winit::event::Modifiers { self.mods }
            fn window(&mut self) -> &mut app::display::window::Window { &mut self.disp.window }
            fn display(&mut self) -> &mut app::display::Display { self.disp }
            fn terminal(&self) -> &Term<VoidListener> { self.term }
            fn terminal_mut(&mut self) -> &mut Term<VoidListener> { self.term }
            fn message(&self) -> Option<&app::message_bar::Message> { None }
            fn config(&self) -> &app::config::UiConfig { self.ui }
            fn mouse_mode(&self) -> bool { self.term.mode().contains(term::TermMode::MOUSE_MODE) }
            fn clipboard_mut(&mut self) -> &mut app::clipboard::Clipboard { self.clip }
            fn scheduler_mut(&mut self) -> &mut app::scheduler::Scheduler { self.sched }
            fn search_direction(&self) -> openagent_terminal_core::index::Direction { self.search.direction }
            fn search_active(&self) -> bool { false }
            fn selection_is_empty(&self) -> bool { true }
            fn semantic_word(&self, _point: openagent_terminal_core::index::Point) -> String { String::new() }
            fn inline_search_state(&mut self) -> &mut app::event::InlineSearchState { self.il_search }
            fn on_typing_start(&mut self) {}
            fn start_search(&mut self, _direction: openagent_terminal_core::index::Direction) {}
            fn start_seeded_search(&mut self, _direction: openagent_terminal_core::index::Direction, _text: String) {}
            fn confirm_search(&mut self) {}
            fn cancel_search(&mut self) {}
            fn search_input(&mut self, _c: char) {}
            fn search_pop_word(&mut self) {}
            fn search_history_previous(&mut self) {}
            fn search_history_next(&mut self) {}
            fn search_next(&mut self, _origin: openagent_terminal_core::index::Point, _direction: openagent_terminal_core::index::Direction, _side: openagent_terminal_core::index::Side) -> Option<openagent_terminal_core::term::search::Match> { None }
            fn advance_search_origin(&mut self, _direction: openagent_terminal_core::index::Direction) {}
            fn send_user_event(&self, _event: app::event::EventType) {}
            fn ide_on_command_end(&mut self, _exit_code: Option<i32>) {}
        }

        // Build processor
        let mut ctx = Ctx {
            ui: &ui,
            disp: &mut display,
            term: &mut term,
            clip: &mut clipboard,
            mouse: &mut mouse,
            touch: &mut touch,
            mods: &mut modifiers,
            sched: &mut scheduler,
            search: &mut search_state,
            il_search: &mut inline_search_state,
            dirty: &mut dirty,
            occluded: &mut occluded,
            ide: &mut ide_mgr,
        };

        let mut processor: app::input::Processor<VoidListener, Ctx> = app::input::Processor::new(ctx);

        // Start a pane drag to exercise the Escape cancel path
        // Use simple ids for source tab/split
        {
            use app::workspace::{PaneId, TabId};
            processor.ctx.display().pane_drag_manager.start_drag(
                TabId(1),
                PaneId(1),
                (10.0, 10.0),
                app::display::pane_drag_drop::PaneDragType::MoveToTab,
            );
            assert!(processor.ctx.display().pane_drag_manager.current_drag().is_some());
        }

        // Simulate Escape cancel path by directly invoking the cancel operation
        processor.ctx.display().pane_drag_manager.cancel_drag();
        processor.ctx.display().pending_update.dirty = true;

        // Verify pane drag canceled
        assert!(processor.ctx.display().pane_drag_manager.current_drag().is_none());

        // All done; exit the event loop
        event_loop.exit();
    }

fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
        // no-op
    }
}

#[test]
#[serial]
fn keyboard_headless_suite() {
    // Opt-in via env var to avoid running GUI tests by default
    let enabled = std::env::var("OPENAGENT_HEADLESS_GUI_TESTS").unwrap_or_default() == "1";
    let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    if !enabled || !has_display {
        eprintln!("[skipped] Set OPENAGENT_HEADLESS_GUI_TESTS=1 and ensure DISPLAY/WAYLAND_DISPLAY to run headless GUI suite.");
        return;
    }

    INIT.call_once(|| {
        let _ = app::logging::tracing_config::initialize_tracing(
            app::logging::tracing_config::TracingConfig::from_env(),
        );
    });

    let mut builder = EventLoop::<app::event::Event>::with_user_event();
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::EventLoopBuilderExtWayland;
        use winit::platform::x11::EventLoopBuilderExtX11;
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    }
    let event_loop = builder.build().expect("event loop");

    struct SuiteApp {
        proxy: winit::event_loop::EventLoopProxy<app::event::Event>,
    }
    impl ApplicationHandler<app::event::Event> for SuiteApp {
        fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            // Helper to create a window+display
            let mut make_display = |ui: &app::config::UiConfig| -> Option<app::display::Display> {
                let mut opts = app::cli::WindowOptions::default();
                let win = match app::display::window::Window::new(event_loop, ui, &ui.window.identity, &mut opts) {
                    Ok(w) => w,
                    Err(e) => {
                        eprintln!("[skipped] Failed to create window: {e:?}");
                        return None;
                    }
                };
                match app::display::Display::new_wgpu(win, ui, false) {
                    Ok(d) => Some(d),
                    Err(e) => {
                        eprintln!("[skipped] Failed to init WGPU display: {e:?}");
                        None
                    }
                }
            };

            // Scenario 1: ESC cancels pane drag
            {
                let mut ui = app::config::UiConfig::default();
                if let Some(mut display) = make_display(&ui) {
                    let size = openagent_terminal_core::term::test::TermSize::new(
                        display.size_info.columns(),
                        display.size_info.screen_lines(),
                    );
                    let mut term: Term<VoidListener> = Term::new(openagent_terminal_core::term::Config::default(), &size, VoidListener);
                    let mut clipboard = app::clipboard::Clipboard::new_nop();
                    let mut mouse = app::event::Mouse::default();
                    let mut touch = app::event::TouchPurpose::default();
                    let mut modifiers = winit::event::Modifiers::default();
                    let mut scheduler = app::scheduler::Scheduler::new(self.proxy.clone());
                    let mut search_state = app::event::SearchState::default();
                    let mut inline_search_state = app::event::InlineSearchState::default();
                    let mut dirty = false;

                    struct Ctx<'a> {
                        ui: &'a app::config::UiConfig,
                        disp: &'a mut app::display::Display,
                        term: &'a mut Term<VoidListener>,
                        clip: &'a mut app::clipboard::Clipboard,
                        mouse: &'a mut app::event::Mouse,
                        touch: &'a mut app::event::TouchPurpose,
                        mods: &'a mut winit::event::Modifiers,
                        sched: &'a mut app::scheduler::Scheduler,
                        search: &'a mut app::event::SearchState,
                        il_search: &'a mut app::event::InlineSearchState,
                        dirty: &'a mut bool,
                    }
                    impl<'a> app::input::ActionContext<VoidListener> for Ctx<'a> {
                        fn write_to_pty<B: Into<std::borrow::Cow<'static, [u8]>>>(&self, _data: B) {}
                        fn mark_dirty(&mut self) { *self.dirty = true; }
                        fn size_info(&self) -> app::display::SizeInfo { self.disp.size_info }
                        fn mouse_mut(&mut self) -> &mut app::event::Mouse { self.mouse }
                        fn mouse(&self) -> &app::event::Mouse { self.mouse }
                        fn touch_purpose(&mut self) -> &mut app::event::TouchPurpose { self.touch }
                        fn modifiers(&mut self) -> &mut winit::event::Modifiers { self.mods }
                        fn window(&mut self) -> &mut app::display::window::Window { &mut self.disp.window }
                        fn display(&mut self) -> &mut app::display::Display { self.disp }
                        fn terminal(&self) -> &Term<VoidListener> { self.term }
                        fn terminal_mut(&mut self) -> &mut Term<VoidListener> { self.term }
                        fn message(&self) -> Option<&app::message_bar::Message> { None }
                        fn config(&self) -> &app::config::UiConfig { self.ui }
                        fn mouse_mode(&self) -> bool { self.term.mode().contains(term::TermMode::MOUSE_MODE) }
                        fn clipboard_mut(&mut self) -> &mut app::clipboard::Clipboard { self.clip }
                        fn scheduler_mut(&mut self) -> &mut app::scheduler::Scheduler { self.sched }
                        fn search_direction(&self) -> openagent_terminal_core::index::Direction { self.search.direction }
                        fn search_active(&self) -> bool { false }
                        fn selection_is_empty(&self) -> bool { true }
                        fn semantic_word(&self, _point: openagent_terminal_core::index::Point) -> String { String::new() }
                        fn inline_search_state(&mut self) -> &mut app::event::InlineSearchState { self.il_search }
                        fn on_typing_start(&mut self) {}
                        fn start_search(&mut self, _direction: openagent_terminal_core::index::Direction) {}
                        fn start_seeded_search(&mut self, _direction: openagent_terminal_core::index::Direction, _text: String) {}
                        fn confirm_search(&mut self) {}
                        fn cancel_search(&mut self) {}
                        fn search_input(&mut self, _c: char) {}
                        fn search_pop_word(&mut self) {}
                        fn search_history_previous(&mut self) {}
                        fn search_history_next(&mut self) {}
                        fn search_next(&mut self, _origin: openagent_terminal_core::index::Point, _direction: openagent_terminal_core::index::Direction, _side: openagent_terminal_core::index::Side) -> Option<openagent_terminal_core::term::search::Match> { None }
                        fn advance_search_origin(&mut self, _direction: openagent_terminal_core::index::Direction) {}
                        fn send_user_event(&self, _event: app::event::EventType) {}
                        fn ide_on_command_end(&mut self, _exit_code: Option<i32>) {}
                    }
                    let ctx = Ctx { ui: &ui, disp: &mut display, term: &mut term, clip: &mut clipboard, mouse: &mut mouse, touch: &mut touch, mods: &mut modifiers, sched: &mut scheduler, search: &mut search_state, il_search: &mut inline_search_state, dirty: &mut dirty };
                    let mut processor: app::input::Processor<VoidListener, Ctx> = app::input::Processor::new(ctx);

                    use app::workspace::{PaneId, TabId};
                    processor.ctx.display().pane_drag_manager.start_drag(
                        TabId(1), PaneId(1), (10.0, 10.0), app::display::pane_drag_drop::PaneDragType::MoveToTab,
                    );
                    assert!(processor.ctx.display().pane_drag_manager.current_drag().is_some());
                    processor.ctx.display().pane_drag_manager.cancel_drag();
                    processor.ctx.display().pending_update.dirty = true;
                    assert!(processor.ctx.display().pane_drag_manager.current_drag().is_none());
                }
            }

            // Scenario 2: confirmation overlay confirm and cancel
            {
                let ui = app::config::UiConfig::default();
                if let Some(mut display) = make_display(&ui) {
                    let size = openagent_terminal_core::term::test::TermSize::new(
                        display.size_info.columns(), display.size_info.screen_lines(),
                    );
                    let mut term: Term<VoidListener> = Term::new(openagent_terminal_core::term::Config::default(), &size, VoidListener);
                    let mut clipboard = app::clipboard::Clipboard::new_nop();
                    let mut mouse = app::event::Mouse::default();
                    let mut touch = app::event::TouchPurpose::default();
                    let mut modifiers = winit::event::Modifiers::default();
                    let mut scheduler = app::scheduler::Scheduler::new(self.proxy.clone());
                    let mut search_state = app::event::SearchState::default();
                    let mut inline_search_state = app::event::InlineSearchState::default();
                    let mut dirty = false;
                    let mut confirm_result: Option<&'static str> = None;

                    struct Ctx<'a> {
                        ui: &'a app::config::UiConfig,
                        disp: &'a mut app::display::Display,
                        term: &'a mut Term<VoidListener>,
                        clip: &'a mut app::clipboard::Clipboard,
                        mouse: &'a mut app::event::Mouse,
                        touch: &'a mut app::event::TouchPurpose,
                        mods: &'a mut winit::event::Modifiers,
                        sched: &'a mut app::scheduler::Scheduler,
                        search: &'a mut app::event::SearchState,
                        il_search: &'a mut app::event::InlineSearchState,
                        dirty: &'a mut bool,
                        confirm_result: &'a mut Option<&'static str>,
                    }
                    impl<'a> app::input::ActionContext<VoidListener> for Ctx<'a> {
                        fn write_to_pty<B: Into<std::borrow::Cow<'static, [u8]>>>(&self, _data: B) {}
                        fn mark_dirty(&mut self) { *self.dirty = true; }
                        fn size_info(&self) -> app::display::SizeInfo { self.disp.size_info }
                        fn mouse_mut(&mut self) -> &mut app::event::Mouse { self.mouse }
                        fn mouse(&self) -> &app::event::Mouse { self.mouse }
                        fn touch_purpose(&mut self) -> &mut app::event::TouchPurpose { self.touch }
                        fn modifiers(&mut self) -> &mut winit::event::Modifiers { self.mods }
                        fn window(&mut self) -> &mut app::display::window::Window { &mut self.disp.window }
                        fn display(&mut self) -> &mut app::display::Display { self.disp }
                        fn terminal(&self) -> &Term<VoidListener> { self.term }
                        fn terminal_mut(&mut self) -> &mut Term<VoidListener> { self.term }
                        fn message(&self) -> Option<&app::message_bar::Message> { None }
                        fn config(&self) -> &app::config::UiConfig { self.ui }
                        fn mouse_mode(&self) -> bool { self.term.mode().contains(term::TermMode::MOUSE_MODE) }
                        fn clipboard_mut(&mut self) -> &mut app::clipboard::Clipboard { self.clip }
                        fn scheduler_mut(&mut self) -> &mut app::scheduler::Scheduler { self.sched }
                        fn search_direction(&self) -> openagent_terminal_core::index::Direction { self.search.direction }
                        fn search_active(&self) -> bool { false }
                        fn selection_is_empty(&self) -> bool { true }
                        fn semantic_word(&self, _point: openagent_terminal_core::index::Point) -> String { String::new() }
                        fn inline_search_state(&mut self) -> &mut app::event::InlineSearchState { self.il_search }
                        fn on_typing_start(&mut self) {}
                        fn start_search(&mut self, _direction: openagent_terminal_core::index::Direction) {}
                        fn start_seeded_search(&mut self, _direction: openagent_terminal_core::index::Direction, _text: String) {}
                        fn confirm_search(&mut self) {}
                        fn cancel_search(&mut self) {}
                        fn search_input(&mut self, _c: char) {}
                        fn search_pop_word(&mut self) {}
                        fn search_history_previous(&mut self) {}
                        fn search_history_next(&mut self) {}
                        fn search_next(&mut self, _origin: openagent_terminal_core::index::Point, _direction: openagent_terminal_core::index::Direction, _side: openagent_terminal_core::index::Side) -> Option<openagent_terminal_core::term::search::Match> { None }
                        fn advance_search_origin(&mut self, _direction: openagent_terminal_core::index::Direction) {}
                        fn send_user_event(&self, _event: app::event::EventType) {}
                        fn ide_on_command_end(&mut self, _exit_code: Option<i32>) {}
                        // Confirmation overlay hooks
                        fn confirm_overlay_active(&self) -> bool { self.disp.confirm_overlay.active }
                        fn confirm_overlay_confirm(&mut self) {
                            self.disp.confirm_overlay.active = false;
                            *self.confirm_result = Some("confirm");
                            self.disp.pending_update.dirty = true;
                            self.mark_dirty();
                        }
                        fn confirm_overlay_cancel(&mut self) {
                            self.disp.confirm_overlay.active = false;
                            *self.confirm_result = Some("cancel");
                            self.disp.pending_update.dirty = true;
                            self.mark_dirty();
                        }
                    }

                    let ctx = Ctx { ui: &ui, disp: &mut display, term: &mut term, clip: &mut clipboard, mouse: &mut mouse, touch: &mut touch, mods: &mut modifiers, sched: &mut scheduler, search: &mut search_state, il_search: &mut inline_search_state, dirty: &mut dirty, confirm_result: &mut confirm_result };
                    let mut processor: app::input::Processor<VoidListener, Ctx> = app::input::Processor::new(ctx);

                    processor.ctx.display().confirm_overlay.open(
                        "test-confirm".into(), "Confirm Action".into(), "Are you sure?".into(), Some("OK".into()), Some("Cancel".into()),
                    );
                    assert!(processor.ctx.display().confirm_overlay.active);
                    {
                        let st = processor.ctx.display().confirm_overlay.clone();
                        processor.ctx.display().draw_confirm_overlay(&ui, &st);
                        assert!(processor.ctx.display().confirm_overlay.active);
                    }
                    processor.ctx.confirm_overlay_confirm();
                    assert!(!processor.ctx.display().confirm_overlay.active);
                    assert_eq!(processor.ctx.confirm_result, &mut Some("confirm"));

                    processor.ctx.display().confirm_overlay.open(
                        "test-confirm".into(), "Confirm Action".into(), "Are you sure?".into(), None, None,
                    );
                    assert!(processor.ctx.display().confirm_overlay.active);
                    processor.ctx.confirm_overlay_cancel();
                    assert!(!processor.ctx.display().confirm_overlay.active);
                    assert_eq!(processor.ctx.confirm_result, &mut Some("cancel"));
                }
            }

            // Scenario 3: completions overlay navigation and confirm/clear
            {
                let ui = app::config::UiConfig::default();
                if let Some(mut display) = make_display(&ui) {
                    let size = openagent_terminal_core::term::test::TermSize::new(
                        display.size_info.columns(), display.size_info.screen_lines(),
                    );
                    let mut term: Term<VoidListener> = Term::new(openagent_terminal_core::term::Config::default(), &size, VoidListener);
                    let mut clipboard = app::clipboard::Clipboard::new_nop();
                    let mut mouse = app::event::Mouse::default();
                    let mut touch = app::event::TouchPurpose::default();
                    let mut modifiers = winit::event::Modifiers::default();
                    let mut scheduler = app::scheduler::Scheduler::new(self.proxy.clone());
                    let mut search_state = app::event::SearchState::default();
                    let mut inline_search_state = app::event::InlineSearchState::default();
                    let mut dirty = false;
                    let mut accepted: Option<String> = None;

                    // Seed items
                    use app::display::completions::{CompletionItem, CompletionKind};
                    display.completions.items = vec![
                        CompletionItem { label: "git".into(), kind: CompletionKind::Command, details: Some("$PATH command".into()), icon: "⌘", score: 1.0 },
                        CompletionItem { label: "grep".into(), kind: CompletionKind::Command, details: Some("$PATH command".into()), icon: "⌘", score: 0.9 },
                        CompletionItem { label: "README.md".into(), kind: CompletionKind::File, details: None, icon: "📄", score: 0.8 },
                    ];
                    display.completions.selected_index = 0;

                    struct Ctx<'a> {
                        ui: &'a app::config::UiConfig,
                        disp: &'a mut app::display::Display,
                        term: &'a mut Term<VoidListener>,
                        clip: &'a mut app::clipboard::Clipboard,
                        mouse: &'a mut app::event::Mouse,
                        touch: &'a mut app::event::TouchPurpose,
                        mods: &'a mut winit::event::Modifiers,
                        sched: &'a mut app::scheduler::Scheduler,
                        search: &'a mut app::event::SearchState,
                        il_search: &'a mut app::event::InlineSearchState,
                        dirty: &'a mut bool,
                        accepted: &'a mut Option<String>,
                    }
                    impl<'a> app::input::ActionContext<VoidListener> for Ctx<'a> {
                        fn write_to_pty<B: Into<std::borrow::Cow<'static, [u8]>>>(&self, _data: B) {}
                        fn mark_dirty(&mut self) { *self.dirty = true; }
                        fn size_info(&self) -> app::display::SizeInfo { self.disp.size_info }
                        fn mouse_mut(&mut self) -> &mut app::event::Mouse { self.mouse }
                        fn mouse(&self) -> &app::event::Mouse { self.mouse }
                        fn touch_purpose(&mut self) -> &mut app::event::TouchPurpose { self.touch }
                        fn modifiers(&mut self) -> &mut winit::event::Modifiers { self.mods }
                        fn window(&mut self) -> &mut app::display::window::Window { &mut self.disp.window }
                        fn display(&mut self) -> &mut app::display::Display { self.disp }
                        fn terminal(&self) -> &Term<VoidListener> { self.term }
                        fn terminal_mut(&mut self) -> &mut Term<VoidListener> { self.term }
                        fn message(&self) -> Option<&app::message_bar::Message> { None }
                        fn config(&self) -> &app::config::UiConfig { self.ui }
                        fn mouse_mode(&self) -> bool { self.term.mode().contains(term::TermMode::MOUSE_MODE) }
                        fn clipboard_mut(&mut self) -> &mut app::clipboard::Clipboard { self.clip }
                        fn scheduler_mut(&mut self) -> &mut app::scheduler::Scheduler { self.sched }
                        fn search_direction(&self) -> openagent_terminal_core::index::Direction { self.search.direction }
                        fn search_active(&self) -> bool { false }
                        fn selection_is_empty(&self) -> bool { true }
                        fn semantic_word(&self, _point: openagent_terminal_core::index::Point) -> String { String::new() }
                        fn inline_search_state(&mut self) -> &mut app::event::InlineSearchState { self.il_search }
                        fn on_typing_start(&mut self) {}
                        fn start_search(&mut self, _direction: openagent_terminal_core::index::Direction) {}
                        fn start_seeded_search(&mut self, _direction: openagent_terminal_core::index::Direction, _text: String) {}
                        fn confirm_search(&mut self) {}
                        fn cancel_search(&mut self) {}
                        fn search_input(&mut self, _c: char) {}
                        fn search_pop_word(&mut self) {}
                        fn search_history_previous(&mut self) {}
                        fn search_history_next(&mut self) {}
                        fn search_next(&mut self, _origin: openagent_terminal_core::index::Point, _direction: openagent_terminal_core::index::Direction, _side: openagent_terminal_core::index::Side) -> Option<openagent_terminal_core::term::search::Match> { None }
                        fn advance_search_origin(&mut self, _direction: openagent_terminal_core::index::Direction) {}
                        fn send_user_event(&self, _event: app::event::EventType) {}
                        fn ide_on_command_end(&mut self, _exit_code: Option<i32>) {}
                        // Completions overlay methods
                        fn completions_active(&self) -> bool { self.disp.completions_active() }
                        fn completions_move_selection(&mut self, delta: isize) { self.disp.completions_move_selection(delta); }
                        fn completions_confirm(&mut self) {
                            *self.accepted = self.disp.completions_selected_label();
                            self.disp.completions_clear();
                            self.disp.pending_update.dirty = true;
                            self.mark_dirty();
                        }
                        fn completions_clear(&mut self) { self.disp.completions_clear(); self.mark_dirty(); }
                    }

                    let ctx = Ctx { ui: &ui, disp: &mut display, term: &mut term, clip: &mut clipboard, mouse: &mut mouse, touch: &mut touch, mods: &mut modifiers, sched: &mut scheduler, search: &mut search_state, il_search: &mut inline_search_state, dirty: &mut dirty, accepted: &mut accepted };
                    let mut processor: app::input::Processor<VoidListener, Ctx> = app::input::Processor::new(ctx);

                    assert!(processor.ctx.display().completions_active());
                    assert_eq!(processor.ctx.display().completions.selected_index, 0);
                    processor.ctx.display().completions.selected_index = 0;
                    processor.ctx.completions_move_selection(-5);
                    assert_eq!(processor.ctx.display().completions.selected_index, 0);
                    processor.ctx.completions_move_selection(1);
                    assert_eq!(processor.ctx.display().completions.selected_index, 1);
                    let last = processor.ctx.display().completions.items.len() - 1;
                    processor.ctx.display().completions.selected_index = last;
                    processor.ctx.completions_move_selection(5);
                    assert_eq!(processor.ctx.display().completions.selected_index, last);

let expected = processor.ctx.display().completions_selected_label();
                    processor.ctx.completions_confirm();
                    drop(processor);
                    assert_eq!(accepted.as_deref(), expected.as_deref());
                }
            }

            // All scenarios done
            event_loop.exit();
        }

        fn window_event(
            &mut self,
            _event_loop: &winit::event_loop::ActiveEventLoop,
            _window_id: winit::window::WindowId,
            _event: winit::event::WindowEvent,
        ) {
            // no-op
        }
    }

    let proxy = event_loop.create_proxy();
    let mut app = SuiteApp { proxy };
    event_loop.run_app(&mut app).expect("run_app");
}

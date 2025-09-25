#![allow(clippy::items_after_test_module)]
//! Process window events.

use crate::ConfigMonitor;
use std::borrow::Cow;
use std::cmp::min;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::Debug;
#[cfg(not(windows))]
use std::os::unix::io::RawFd;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::rc::Rc;
#[cfg(unix)]
use std::sync::Arc;
use std::time::{Duration, Instant};

use std::{env, f32, mem};

use ahash::RandomState;
use crossfont::Size as FontSize;
use log::{debug, error, info, warn};
use winit::application::ApplicationHandler;
use winit::event::{
    ElementState, Event as WinitEvent, Ime, Modifiers, MouseButton, StartCause,
    Touch as TouchEvent, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop, EventLoopProxy};
use winit::raw_window_handle::HasDisplayHandle;
use winit::window::WindowId;

use openagent_terminal_core::event::{Event as TerminalEvent, EventListener, Notify};
use openagent_terminal_core::event_loop::Notifier;
#[cfg(test)]
use openagent_terminal_core::grid;
use openagent_terminal_core::grid::{BidirectionalIterator, Dimensions, Scroll};
use openagent_terminal_core::index::{Boundary, Column, Direction, Line, Point, Side};
use openagent_terminal_core::selection::{Selection, SelectionType};
use openagent_terminal_core::term::cell::{Flags, LineLength as CellLineLength};
use openagent_terminal_core::term::search::{Match, RegexSearch};
use openagent_terminal_core::term::{self, ClipboardType, Term, TermMode};
use openagent_terminal_core::vte::ansi::NamedColor;

#[cfg(unix)]
use crate::cli::{IpcConfig, ParsedOptions};
use crate::cli::{Options as CliOptions, WindowOptions};
use crate::clipboard::Clipboard;
use crate::components_init::{ComponentConfig, InitializedComponents};
use crate::config::ui_config::{HintAction, HintInternalAction};
use crate::config::{self, Action as BindingAction, UiConfig};
#[cfg(not(windows))]
use crate::daemon::foreground_process_path;
use crate::daemon::spawn_daemon;
use crate::display::color::Rgb;
use crate::display::hint::HintMatch;
#[cfg(feature = "never")]
use crate::display::notebook_panel::{NotebookCellItem, NotebookListItem};
use crate::display::palette::{PaletteEntry, PaletteItem};
use crate::display::window::Window;
use crate::display::{Display, Preedit, SizeInfo};
use crate::input::{self, ActionContext as _, FONT_SIZE_STEP};
#[cfg(unix)]
use crate::ipc::{self, SocketReply};
use crate::logging::{LOG_TARGET_CONFIG, LOG_TARGET_WINIT};
use crate::message_bar::{Message, MessageBuffer};
use crate::scheduler::{Scheduler, TimerId, Topic};
use crate::window_context::WindowContext;
use openagent_terminal_core::event::CommandBlockEvent as CoreCommandBlockEvent;
#[cfg(feature = "never")]
use crate::plugins_api::PluginEvent;
#[cfg(feature = "never")]
use serde_json::json;

#[cfg(test)]
pub(crate) fn collect_block_output_from_grid(
    grid: &grid::Grid<openagent_terminal_core::term::cell::Cell>,
    start: usize,
    end: usize,
) -> String {
    let top = grid.topmost_line();
    let mut out = String::new();
    for abs in start..=end {
        let line = top + (abs as i32);
        if line < grid.topmost_line() || line > grid.bottommost_line() {
            continue;
        }
        let row = &grid[line];
        let len = row.line_length().0.min(grid.columns());
        if len == 0 {
            out.push('\n');
            continue;
        }
        for col in 0..len {
            let ch = row[openagent_terminal_core::index::Column(col)].c;
            out.push(ch);
        }
        out.push('\n');
    }
    if out.ends_with('\n') {
        let _ = out.pop();
    }
    out
}

/// Duration after the last user input until an unlimited search is performed.
pub const TYPING_SEARCH_DELAY: Duration = Duration::from_millis(500);

/// Maximum number of lines for the blocking search while still typing the search regex.
const MAX_SEARCH_WHILE_TYPING: Option<usize> = Some(1000);

/// Debounce delay for Blocks Search typing.
#[allow(dead_code)]
pub const BLOCKS_SEARCH_DEBOUNCE: Duration = Duration::from_millis(250);
/// Debounce delay for Workflows Search typing.
pub const WORKFLOWS_SEARCH_DEBOUNCE: Duration = Duration::from_millis(250);
/// Retention time for workflows progress overlay after completion.
#[cfg(feature = "workflow")]
pub const WORKFLOWS_OVERLAY_RETAIN: Duration = Duration::from_millis(3000);

/// Debounce for AI inline suggestions after typing
#[cfg(feature = "ai")]
pub const AI_INLINE_SUGGEST_DEBOUNCE: Duration = Duration::from_millis(120);

/// Maximum number of search terms stored in the history.
const MAX_SEARCH_HISTORY_SIZE: usize = 255;

/// Touch zoom speed.
const TOUCH_ZOOM_FACTOR: f32 = 0.01;

/// Cooldown between invocations of the bell command.
const BELL_CMD_COOLDOWN: Duration = Duration::from_millis(100);

/// The event processor.
///
/// Stores some state from received events and dispatches actions when they are
/// triggered.
pub struct Processor {
    pub config_monitor: Option<ConfigMonitor>,

    clipboard: Clipboard,
    scheduler: Scheduler,
    initial_window_options: Option<WindowOptions>,
    initial_window_error: Option<Box<dyn Error>>,
    windows: HashMap<WindowId, WindowContext, RandomState>,
    proxy: EventLoopProxy<Event>,
    components: Option<Arc<InitializedComponents>>,
    #[cfg(unix)]
    global_ipc_options: ParsedOptions,
    cli_options: CliOptions,
    config: Rc<UiConfig>,

    // Pending workflow confirmations (workflow name, window id)
    pending_workflow_confirms: HashMap<String, (String, WindowId)>,
}

static PRIVACY_EXTENDED_FLAG: once_cell::sync::OnceCell<bool> = once_cell::sync::OnceCell::new();

#[cfg(test)]
mod copy_export_tests {
    use openagent_terminal_core::grid::Grid;
    use openagent_terminal_core::index::{Column, Line};
    use openagent_terminal_core::term::cell::Cell;

    #[test]
    fn collect_block_output_excludes_header_and_trims_trailing_newline() {
        // Grid with 5 lines x 4 columns, no history
        let mut grid: Grid<Cell> = Grid::new(5, 4, 0);
        // Fill lines with simple content: 0..4
        let rows = [
            "HEAD", // header line to exclude
            "L1__", "L2__", "L3__", "L4__",
        ];
        for (i, row) in rows.iter().enumerate() {
            for (j, ch) in row.chars().enumerate() {
                grid[Line(i as i32)][Column(j)].c = ch;
            }
        }
        // Header at 0; collect from 1..3
        let out = crate::event::collect_block_output_from_grid(&grid, 1, 3);
        assert_eq!(out, "L1__\nL2__\nL3__");
    }

    #[test]
    fn collect_block_output_large_range_aggregates() {
        let mut grid: Grid<Cell> = Grid::new(10, 3, 0);
        // lines 0..9, fill with 'A'..'J'
        for i in 0..10 {
            let ch = (b'A' + i as u8) as char;
            for c in 0..3 {
                grid[Line(i)][Column(c)].c = ch;
            }
        }
        // Collect 2..8 => C..I
        let out = crate::event::collect_block_output_from_grid(&grid, 2, 8);
        let expected = ["CCC", "DDD", "EEE", "FFF", "GGG", "HHH", "III"].join("\n");
        assert_eq!(out, expected);
    }

    #[test]
    fn collect_block_output_empty_when_out_of_range() {
        let grid: Grid<Cell> = Grid::new(3, 3, 0);
        // Request a range completely outside the grid
        let out = crate::event::collect_block_output_from_grid(&grid, 10, 12);
        assert_eq!(out, "");
    }

    #[test]
    fn collect_block_output_preserves_blank_lines() {
        let mut grid: Grid<Cell> = Grid::new(3, 3, 0);
        // Leave line 0 blank (len=0 -> newline)
        // Put content on line 1
            for c in 0..3 {
                grid[Line(1)][Column(c)].c = ['A', 'B', 'C'][c];
            }
        // Leave line 2 blank as well
        let out = crate::event::collect_block_output_from_grid(&grid, 0, 2);
        // Should include blank line, then 'ABC', then blank line
        assert_eq!(out, "\nABC\n");
    }
}

#[cfg(test)]
mod basic_event_tests {
    use super::*;
    use openagent_terminal_core::index::{Column, Line};
    use crate::message_bar::{Message, MessageType};

    #[test]
    fn search_state_defaults_and_accessors() {
        let st = SearchState::default();
        assert_eq!(st.direction(), Direction::Right);
        assert!(st.regex().is_none());
        assert!(st.focused_match().is_none());
    }

    #[test]
    fn search_state_regex_history() {
        let mut st = SearchState::default();
        // Simulate active search with current input at index 0
        st.history.push_front("foo".to_string());
        st.history_index = Some(0);
        assert_eq!(st.regex(), Some(&"foo".to_string()));

        // Clear focused match
        st.clear_focused_match();
        assert!(st.focused_match().is_none());
    }

    #[test]
    fn event_wrapper_payload_access() {
        let wid: WindowId = WindowId::dummy();
        let msg = Message::new("hello".into(), MessageType::Warning);
        let ev = Event::new(EventType::Message(msg.clone()), wid);
        match ev.payload() {
            EventType::Message(m) => {
                // Verify type
                assert!(matches!(m.ty(), MessageType::Warning));
                // Verify rendered text contains the message
                // Ensure there is space for at least one message line beyond MIN_FREE_LINES.
                let si = crate::display::SizeInfo::new(200.0, 120.0, 10.0, 20.0, 0.0, 0.0, false);
                let lines = m.text(&si);
                assert!(!lines.is_empty());
            }
            _ => panic!("unexpected payload"),
        }
    }

    #[test]
    fn touch_zoom_font_delta_quantizes() {
        use winit::dpi::PhysicalPosition;
        use winit::event::{DeviceId, Touch as WTouch, TouchPhase};

        // Two distinct touch points
        let dev = DeviceId::dummy();
        let t1 = WTouch { device_id: dev, id: 1, location: PhysicalPosition::new(0.0, 0.0), force: None, phase: TouchPhase::Moved };
        let t2 = WTouch { device_id: dev, id: 2, location: PhysicalPosition::new(0.0, 0.0), force: None, phase: TouchPhase::Moved };
        let mut zoom = TouchZoom::new((t1, t2));

        // Move one finger far to increase distance -> positive, quantized delta
        let t2_far = WTouch { device_id: dev, id: 2, location: PhysicalPosition::new(0.0, 200.0), force: None, phase: TouchPhase::Moved };
        let d_pos = zoom.font_delta(t2_far);
        let steps = d_pos / crate::input::FONT_SIZE_STEP;
        assert!((steps - steps.round()).abs() < 1e-6, "delta must be quantized to FONT_SIZE_STEP");

        // Move back to zero distance -> negative delta
        let t2_close = WTouch { device_id: dev, id: 2, location: PhysicalPosition::new(0.0, 0.0), force: None, phase: TouchPhase::Moved };
        let d_neg = zoom.font_delta(t2_close);
        assert!(d_neg < 0.0);
    }

    #[test]
    fn touch_zoom_fraction_accumulates_across_small_moves() {
        use winit::dpi::PhysicalPosition;
        use winit::event::{DeviceId, Touch as WTouch, TouchPhase};

        let dev = DeviceId::dummy();
        let t1 = WTouch { device_id: dev, id: 1, location: PhysicalPosition::new(0.0, 0.0), force: None, phase: TouchPhase::Moved };
        let t2 = WTouch { device_id: dev, id: 2, location: PhysicalPosition::new(0.0, 0.0), force: None, phase: TouchPhase::Moved };
        let mut zoom = TouchZoom::new((t1, t2));

        // +50px => 0.5 step -> quantizes to 0
        let t2_50 = WTouch { device_id: dev, id: 2, location: PhysicalPosition::new(0.0, 50.0), force: None, phase: TouchPhase::Moved };
        let d1 = zoom.font_delta(t2_50);
        assert!(d1.abs() < 1e-6);

        // +100px total => additional 50px (0.5) + 0.5 fraction = 1.0 step
        let t2_100 = WTouch { device_id: dev, id: 2, location: PhysicalPosition::new(0.0, 100.0), force: None, phase: TouchPhase::Moved };
        let d2 = zoom.font_delta(t2_100);
        assert!((d2 - crate::input::FONT_SIZE_STEP).abs() < 1e-6);

        // +10px => below threshold, accumulates fraction only
        let t2_110 = WTouch { device_id: dev, id: 2, location: PhysicalPosition::new(0.0, 110.0), force: None, phase: TouchPhase::Moved };
        let d3 = zoom.font_delta(t2_110);
        assert!(d3.abs() < 1e-6);

        // -20px => still below threshold (with fraction), no step
        let t2_90 = WTouch { device_id: dev, id: 2, location: PhysicalPosition::new(0.0, 90.0), force: None, phase: TouchPhase::Moved };
        let d4 = zoom.font_delta(t2_90);
        assert!(d4.abs() < 1e-6);

        // Back to zero => should yield exactly -1 step due to accumulated fraction
        let t2_0 = WTouch { device_id: dev, id: 2, location: PhysicalPosition::new(0.0, 0.0), force: None, phase: TouchPhase::Moved };
        let d5 = zoom.font_delta(t2_0);
        assert!((d5 + crate::input::FONT_SIZE_STEP).abs() < 1e-6);
    }

    #[test]
    fn touch_zoom_slots_update_and_identity() {
        use winit::dpi::PhysicalPosition;
        use winit::event::{DeviceId, Touch as WTouch, TouchPhase};

        let dev = DeviceId::dummy();
        let t1 = WTouch { device_id: dev, id: 1, location: PhysicalPosition::new(0.0, 0.0), force: None, phase: TouchPhase::Moved };
        let t2 = WTouch { device_id: dev, id: 2, location: PhysicalPosition::new(10.0, 0.0), force: None, phase: TouchPhase::Moved };
        let mut zoom = TouchZoom::new((t1, t2));

        // Move slot 1 and verify identity/location preserved
        let t1b = WTouch { device_id: dev, id: 1, location: PhysicalPosition::new(20.0, 0.0), force: None, phase: TouchPhase::Moved };
        let _ = zoom.font_delta(t1b);
        let (s1, s2) = zoom.slots();
        assert_eq!(s1.id, 1);
        assert_eq!(s2.id, 2);
        assert_eq!(s1.location.x, 20.0);
        assert_eq!(s2.location.x, 10.0);

        // Move slot 2 and verify updated
        let t2b = WTouch { device_id: dev, id: 2, location: PhysicalPosition::new(25.0, 0.0), force: None, phase: TouchPhase::Moved };
        let _ = zoom.font_delta(t2b);
        let (s1c, s2c) = zoom.slots();
        assert_eq!(s1c.id, 1);
        assert_eq!(s2c.id, 2);
        assert_eq!(s1c.location.x, 20.0);
        assert_eq!(s2c.location.x, 25.0);
    }

    #[test]
    fn search_state_history_and_regex_mut() {
        let mut st = SearchState::default();
        // No active history -> regex_mut is None
        assert!(st.regex_mut().is_none());
        // Push a query and activate it
        st.history.push_front("abc".to_string());
        st.history_index = Some(0);
        assert_eq!(st.regex(), Some(&"abc".to_string()));
        // Mutate via regex_mut
        if let Some(re) = st.regex_mut() {
            re.push('d');
        }
        assert_eq!(st.regex(), Some(&"abcd".to_string()));
    }

    #[test]
    fn mouse_point_maps_from_pixels_to_grid() {
        // Size: 3x2 cells, 10x20 px cell, 5 px padding
        let si = crate::display::SizeInfo::new(35.0, 50.0, 10.0, 20.0, 5.0, 5.0, false);
        let mut mouse = Mouse { x: 6, y: 6, ..Default::default() }; // padding + 1
        let p = mouse.point(&si, 0);
        assert_eq!(p.line, Line(0));
        assert_eq!(p.column, Column(0));

        // Move to second column, same row
        mouse.x = 5 + 10 + 1; // padding + one cell width + 1
        let p = mouse.point(&si, 0);
        assert_eq!(p.line, Line(0));
        assert_eq!(p.column, Column(1));

        // Move to last visible row
        mouse.y = 5 + 20 + 1; // padding + one cell height + 1
        let p = mouse.point(&si, 0);
        assert_eq!(p.line, Line(1));
    }
}

impl Processor {
    /// Global accessor for privacy extended flag from current config (None if not yet initialized)
    pub fn privacy_extended_flag() -> Option<bool> {
        PRIVACY_EXTENDED_FLAG.get().copied()
    }
    /// Create a new event processor.
    pub fn new(
        config: UiConfig,
        cli_options: CliOptions,
        event_loop: &EventLoop<Event>,
    ) -> Processor {
        PRIVACY_EXTENDED_FLAG.set(config.privacy.extended_redaction).ok();
        let proxy = event_loop.create_proxy();
        // Initialize confirmation broker hooks (proxy)
        crate::ui_confirm::set_event_proxy(proxy.clone());
        let scheduler = Scheduler::new(proxy.clone());
        let initial_window_options = Some(cli_options.window_options.clone());

        // Disable all device events, since we don't care about them.
        event_loop.listen_device_events(DeviceEvents::Never);

        // SAFETY: Since this takes a pointer to the winit event loop, it MUST be dropped first,
        // which is done in `loop_exiting`.
        let clipboard = unsafe { Clipboard::new(event_loop.display_handle().unwrap().as_raw()) };

        // Create a config monitor.
        //
        // The monitor watches the config file for changes and reloads it. Pending
        // config changes are processed in the main loop.
        let mut config_monitor = None;
        if config.live_config_reload() {
            config_monitor =
                ConfigMonitor::new(config.config_paths.clone(), event_loop.create_proxy());
        }

        Processor {
            initial_window_options,
            initial_window_error: None,
            cli_options,
            proxy,
            scheduler,
            components: None,
            config: Rc::new(config),
            clipboard,
            windows: Default::default(),
            #[cfg(unix)]
            global_ipc_options: Default::default(),
            config_monitor,
            pending_workflow_confirms: Default::default(),
        }
    }

    /// Create initial window and load WGPU platform.
    ///
    /// This will initialize the WGPU renderer and pick a configuration
    /// will be used for the rest of the windows.
    pub fn create_initial_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_options: WindowOptions,
    ) -> Result<(), Box<dyn Error>> {
        let mut window_context = WindowContext::initial(
            event_loop,
            self.proxy.clone(),
            self.config.clone(),
            window_options,
        )?;

        // Initialize enhanced components (Blocks, Workflows, Plugins, HarfBuzz) once
        // Attach them to the very first window so subsystems like the plugin manager
        // are operational from startup when enabled by features/config.
        if self.components.is_none() {
            // Use the winit Window reference for any OS integrations required during init
            let winit_win = window_context.display.window.winit_window();
            // Provide a Tokio runtime to drive async initialization that uses tokio::fs and friends
            if let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
                let _ = rt.block_on(self.initialize_components(winit_win));
            }
            if let Some(components) = &self.components {
                window_context.set_components(components.clone());
                // Wire Blocks -> Workspace PTY collection when Warp is enabled and initialized
                #[cfg(feature = "never")]
                {
                    if window_context.config().workspace.warp_style {
                        if let Some(warp) = &window_context.workspace.warp {
                            if let Some(blocks) = &components.block_manager {
                                let handle = warp.pty_collection_handle();
                                if let Ok(mut mgr) = blocks.try_write() {
                                    mgr.set_workspace_pty_collection(handle);
                                }
                            }
                        }
                    }
                }
            }
        }

        let window_id = window_context.id();
        // Set default window for confirmations (first window)
        crate::ui_confirm::set_default_window_id(window_id);
        self.windows.insert(window_id, window_context);

        // Schedule session autosave if enabled and using Warp mode
        if self.config.workspace.warp_style
            && self.config.workspace.sessions.enabled
            && self.config.workspace.sessions.autosave_interval_secs > 0
        {
            let tid = TimerId::new(Topic::WorkspaceSessionAutosave, window_id);
            self.scheduler.unschedule(tid);
            let evt = Event::new(
                EventType::WarpUiUpdate(crate::workspace::WarpUiUpdateType::SessionAutoSave),
                window_id,
            );
            let interval =
                Duration::from_secs(self.config.workspace.sessions.autosave_interval_secs);
            self.scheduler.schedule(evt, interval, true, tid);
        }

        // Show a feature banner at startup
        {
            if self.config.feature_banner.show {
                let mut features = Vec::new();
                features
                    .push(format!("wgpu:{}", if cfg!(feature = "wgpu") { "on" } else { "off" }));
                features.push(format!("ai:{}", if cfg!(feature = "ai") { "on" } else { "off" }));
                features.push(format!(
                    "workflow:{}",
                    if cfg!(feature = "workflow") { "on" } else { "off" }
                ));
                features.push(format!(
                    "completions:{}",
                    if cfg!(feature = "completions") { "on" } else { "off" }
                ));
                let banner = format!("Features: {}", features.join("  "));
                let level = self
                    .config
                    .feature_banner
                    .level
                    .as_deref()
                    .unwrap_or("info")
                    .to_ascii_lowercase();
                let ty = if level == "warning" {
                    crate::message_bar::MessageType::Warning
                } else {
                    crate::message_bar::MessageType::Error /* Info type not present; fallback to visible tier */
                };
                let message = crate::message_bar::Message::new(banner, ty);
                if let Err(e) =
                    self.proxy.send_event(Event::new(EventType::Message(message), Some(window_id)))
                {
                    tracing::warn!("failed to post Message event: {:?}", e);
                }
            }
        }

        // Friendly nudge when AI is enabled but provider is 'null'
        #[cfg(feature = "ai")]
        {
            if self.config.ai.enabled {
                let provider_name = self.config.ai.provider.as_deref().unwrap_or("null");
                if provider_name.eq_ignore_ascii_case("null") {
                    let msg = crate::message_bar::Message::new(
                        "AI is enabled with provider 'null'. Open AI (Ctrl+Shift+A) and switch provider, or configure [ai] in your config.".to_string(),
                        crate::message_bar::MessageType::Warning,
                    );
                    if let Err(e) =
                        self.proxy.send_event(Event::new(EventType::Message(msg), window_id))
                    {
                        tracing::warn!("failed to post AI provider hint Message: {:?}", e);
                    }
                }
            }
        }

        // If there was no user config loaded, show a brief onboarding hint and auto-open Workflows.
        if self.config.config_paths.is_empty() {
            let hint = "Welcome — click the bottom bar or use Ctrl+Shift+P/S/W. Place a config at \
                        ~/.config/openagent-terminal/openagent-terminal.toml"
                .to_string();
            let message =
                crate::message_bar::Message::new(hint, crate::message_bar::MessageType::Warning);
            if let Err(e) =
                self.proxy.send_event(Event::new(EventType::Message(message), window_id))
            {
                tracing::warn!("failed to post onboarding Message: {:?}", e);
            }
            // Copy onboarding examples to ~/.config on first run
            if let Some(cfg_dir) = dirs::config_dir().map(|d| d.join("openagent-terminal")) {
                let examples_dst = cfg_dir.join("examples").join("workflows");
                if !examples_dst.exists() {
                    let _ = std::fs::create_dir_all(&examples_dst);
                    // Copy curated workflow samples (best-effort)
                    for name in ["rust.yaml", "node.yaml", "python.yaml"] {
                        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("openagent-terminal")
                            .join("examples")
                            .join("workflows")
                            .join(name);
                        let dst = examples_dst.join(name);
                        let _ = std::fs::copy(src, dst);
                    }
                }
            }
            // Auto-open Workflows panel and trigger an initial search
            #[cfg(feature = "workflow")]
            if let Some(win) = self.windows.get_mut(&window_id) {
                win.display.workflows_panel.open();
                win.dirty = true;
                if win.display.window.has_frame {
                    win.display.window.request_redraw();
                }
                if let Err(e) = self.proxy.send_event(Event::new(
                    EventType::WorkflowsSearchPerform(String::new()),
                    window_id,
                )) {
                    tracing::warn!("failed to post WorkflowsSearchPerform: {:?}", e);
                }
            }
        }

        // If components are already initialized, set them on the new window
        if let Some(components) = &self.components {
            if let Some(window_context) = self.windows.get_mut(&window_id) {
                window_context.set_components(components.clone());
            }
        }

        Ok(())
    }

    /// Create a new terminal window.
    pub fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        options: WindowOptions,
    ) -> Result<(), Box<dyn Error>> {
        // WGPU-only: always use WGPU for additional windows; no GL fallback.
        let _use_wgpu_additional = cfg!(feature = "wgpu");

        // Override config with CLI/IPC options.
        let mut config_overrides = options.config_overrides();
        #[cfg(unix)]
        config_overrides.extend_from_slice(&self.global_ipc_options);
        let mut config = self.config.clone();
        config = config_overrides.override_config_rc(config);

        let window_context: WindowContext = {
            #[cfg(feature = "wgpu")]
            {
                WindowContext::additional_wgpu(
                    event_loop,
                    self.proxy.clone(),
                    config,
                    options,
                    config_overrides,
                )?
            }
            #[cfg(not(feature = "wgpu"))]
            {
                return Err("This build is WGPU-only. Rebuild with --features=wgpu".into());
            }
        };

        let window_id = window_context.id();
        self.windows.insert(window_id, window_context);

        // Schedule session autosave if enabled and using Warp mode
        if self.config.workspace.warp_style
            && self.config.workspace.sessions.enabled
            && self.config.workspace.sessions.autosave_interval_secs > 0
        {
            let tid = TimerId::new(Topic::WorkspaceSessionAutosave, window_id);
            self.scheduler.unschedule(tid);
            let evt = Event::new(
                EventType::WarpUiUpdate(crate::workspace::WarpUiUpdateType::SessionAutoSave),
                window_id,
            );
            let interval =
                Duration::from_secs(self.config.workspace.sessions.autosave_interval_secs);
            self.scheduler.schedule(evt, interval, true, tid);
        }

        // Ensure components (plugins, etc.) are initialized by default on first window
        if self.components.is_none() {
            let cfg = ComponentConfig {
                enable_wgpu: cfg!(feature = "wgpu"),
                enable_harfbuzz: cfg!(feature = "harfbuzz"),
                enable_blocks: false,
                enable_workflows: cfg!(feature = "workflow"),
                // Always disable plugins
                enable_plugins: false,
                ..Default::default()
            };
            match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => match rt.block_on(crate::components_init::initialize_components(&cfg)) {
                    Ok(components) => {
                        let arc = Arc::new(components);
                        if let Some(wc) = self.windows.get_mut(&window_id) {
                            wc.set_components(arc.clone());
                        }
                        self.components = Some(arc);
                    }
                    Err(e) => warn!("Component initialization failed: {}", e),
                },
                Err(e) => warn!("Failed to create runtime for initialization: {}", e),
            }
        } else if let Some(components) = &self.components {
            if let Some(window_context) = self.windows.get_mut(&window_id) {
                window_context.set_components(components.clone());
            }
        }

        Ok(())
    }

    /// Initialize components asynchronously
    #[allow(dead_code)]
    pub async fn initialize_components(
        &mut self,
        _window: &winit::window::Window,
    ) -> Result<(), Box<dyn Error>> {
        if self.components.is_some() {
            return Ok(()); // Already initialized
        }

        let config = ComponentConfig {
            enable_wgpu: cfg!(feature = "wgpu"),
            enable_harfbuzz: cfg!(feature = "harfbuzz"),
            enable_blocks: false,
            enable_workflows: cfg!(feature = "workflow"),
            // Always disable plugin system 
            enable_plugins: false,
            ..Default::default()
        };

        info!("Initializing terminal components...");
        match crate::components_init::initialize_components(&config).await {
            Ok(components) => {
                self.components = Some(Arc::new(components));
                info!("✓ All components initialized successfully");
                Ok(())
            }
            Err(e) => {
                warn!("Component initialization failed: {}", e);
                warn!("Continuing with basic functionality...");
                Ok(()) // Don't fail completely, just continue without enhanced features
            }
        }
    }

    /// Get a reference to the initialized components
    #[allow(dead_code)]
    pub fn components(&self) -> Option<&Arc<InitializedComponents>> {
        self.components.as_ref()
    }

    /// Run the event loop.
    ///
    /// The result is exit code generate from the loop.
    pub fn run(&mut self, event_loop: EventLoop<Event>) -> Result<(), Box<dyn Error>> {
        let result = event_loop.run_app(self);
        match self.initial_window_error.take() {
            Some(initial_window_error) => Err(initial_window_error),
            _ => result.map_err(Into::into),
        }
    }

    /// Check if an event is irrelevant and can be skipped.
    fn skip_window_event(event: &WindowEvent) -> bool {
        matches!(
            event,
            WindowEvent::KeyboardInput { is_synthetic: true, .. }
                | WindowEvent::ActivationTokenDone { .. }
                | WindowEvent::DoubleTapGesture { .. }
                | WindowEvent::TouchpadPressure { .. }
                | WindowEvent::RotationGesture { .. }
                | WindowEvent::CursorEntered { .. }
                | WindowEvent::PinchGesture { .. }
                | WindowEvent::AxisMotion { .. }
                | WindowEvent::PanGesture { .. }
                | WindowEvent::HoveredFileCancelled
                | WindowEvent::Destroyed
                | WindowEvent::ThemeChanged(_)
                | WindowEvent::HoveredFile(_)
                | WindowEvent::Moved(_)
        )
    }
}

#[cfg(feature = "never")]
impl Processor {
    fn process_blocks_search_perform(&mut self, query: String, window_id: WindowId) {
        self.process_blocks_search_with_state(query, window_id, None);
    }

    fn process_blocks_search_with_state(
        &mut self,
        query: String,
        window_id: WindowId,
        state: Option<&crate::display::blocks_search_panel::BlocksSearchState>,
    ) {
        if let Some(components) = &self.components {
            if let Some(manager) = &components.block_manager {
                let manager = manager.clone();
                let proxy = self.proxy.clone();
                let win = window_id;
                let runtime = components.runtime.clone();

                // Build search query from state or simple text query
                let search_query = if let Some(state) = state {
                    self.build_search_query_from_state(state, &query)
                } else {
                    let mut sq = crate::blocks_v2::SearchQuery::default();
                    if !query.trim().is_empty() {
                        sq.text = Some(query.clone());
                    }
                    sq.limit = Some(100);
                    sq
                };

                runtime.spawn(async move {
                    let mut items = Vec::new();
                    if let Ok(res) = manager.read().await.search(search_query).await {
                        for b in res {
                            items.push(crate::display::blocks_search_panel::BlocksSearchItem {
                                id: b.id.to_string(),
                                command: b.command.clone(),
                                output: b.output.clone(),
                                directory: b.directory.to_string_lossy().to_string(),
                                created_at: b.created_at.to_rfc3339(),
                                modified_at: b.modified_at.to_rfc3339(),
                                exit_code: b.exit_code,
                                duration_ms: b.duration_ms,
                                starred: b.starred,
                                tags: b.tags.iter().cloned().collect(),
                                shell: b.shell.to_str().to_string(),
                                status: format!("{:?}", b.status),
                            });
                        }
                    }
                    #[cfg(test)]
                    {
                        test_posted_events::record(EventType::BlocksSearchResults(items.clone()));
                    }
                    let _ =
                        proxy.send_event(Event::new(EventType::BlocksSearchResults(items), win));
                });
                return;
            }
        }
        // No components or no blocks manager: post empty results immediately
        #[cfg(test)]
        {
            test_posted_events::record(EventType::BlocksSearchResults(Vec::new()));
        }
        if let Err(e) =
            self.proxy.send_event(Event::new(EventType::BlocksSearchResults(Vec::new()), window_id))
        {
            tracing::warn!("failed to post empty BlocksSearchResults: {:?}", e);
        }
    }

    fn build_search_query_from_state(
        &self,
        state: &crate::display::blocks_search_panel::BlocksSearchState,
        query: &str,
    ) -> crate::blocks_v2::SearchQuery {
        use crate::blocks_v2::SearchQuery;
        use crate::display::blocks_search_panel::SearchMode;

        let mut sq = SearchQuery {
            sort_by: state.sort_field,
            sort_order: state.sort_order,
            offset: Some(state.current_page * state.items_per_page),
            limit: Some(state.items_per_page),
            starred_only: state.filters.starred_only,
            tags: if state.filters.tags.is_empty() {
                None
            } else {
                Some(state.filters.tags.clone())
            },
            directory: state.filters.directory.clone(),
            shell: state.filters.shell,
            status: state.filters.status,
            exit_code: state.filters.exit_code,
            duration: state.filters.duration,
            date_from: state.filters.date_from,
            date_to: state.filters.date_to,
            ..Default::default()
        };

        // Set text search based on mode
        if !query.trim().is_empty() {
            match state.mode {
                SearchMode::Basic => {
                    sq.text = Some(query.to_string());
                }
                SearchMode::Command => {
                    sq.command_text = Some(query.to_string());
                }
                SearchMode::Output => {
                    sq.output_text = Some(query.to_string());
                }
                SearchMode::Advanced => {
                    sq.text = Some(query.to_string());
                }
            }
        }

        sq
    }
}

#[cfg(feature = "workflow")]
impl Processor {
    fn process_workflows_search_perform(&mut self, query: String, window_id: WindowId) {
        // Build items from UiConfig.workflows; simple case-insensitive match on name/description
        let mut items = Vec::new();
        let q = query.to_lowercase();
        let cfg = &self.config;
        for wf in &cfg.workflows {
            let name = wf.name.clone();
            let desc = wf.description.clone();
            let hay = format!("{} {}", name, desc.clone().unwrap_or_default()).to_lowercase();
            if q.trim().is_empty() || hay.contains(&q) {
                items.push(crate::display::workflow_panel::WorkflowItem {
                    name,
                    description: desc,
                    source: crate::display::workflow_panel::WorkflowSource::Config,
                });
            }
        }
        // Also include workflows loaded by the runtime engine when available
        if let Some(components) = &self.components {
            if let Some(engine) = &components.workflow_engine {
                let engine = engine.clone();
                let proxy = self.proxy.clone();
                let win = window_id;
                let mut base = items;
                components.runtime.spawn(async move {
                    let mut engine_items = Vec::new();
                    let list = engine.list_workflows().await;
                    for (_id, def) in list {
                        let name = def.name.clone();
                        let desc = Some(def.description.clone());
                        let hay = format!("{} {}", name, def.description).to_lowercase();
                        if q.trim().is_empty() || hay.contains(&q) {
                            engine_items.push(crate::display::workflow_panel::WorkflowItem {
                                name,
                                description: desc,
                                source: crate::display::workflow_panel::WorkflowSource::Engine,
                            });
                        }
                    }
                    // Merge and de-duplicate by name preferring engine entries last
                    base.extend(engine_items);
                    // De-dup by name keeping first occurrence
                    let mut seen = std::collections::HashSet::new();
                    base.retain(|it| seen.insert(it.name.clone()));
                    let _ =
                        proxy.send_event(Event::new(EventType::WorkflowsSearchResults(base), win));
                });
                return;
            }
        }
        let _ =
            self.proxy.send_event(Event::new(EventType::WorkflowsSearchResults(items), window_id));
    }
}

#[cfg(feature = "never")]
impl Processor {
    fn notebooks_open(&mut self, window_id: WindowId) {
        if let Some(win) = self.windows.get_mut(&window_id) {
            win.display.notebooks_panel.open();
            win.dirty = true;
            if win.display.window.has_frame {
                win.display.window.request_redraw();
            }
        }
        if let Some(components) = &self.components {
            if let Some(nb) = &components.notebook_manager {
                let nb = nb.clone();
                let proxy = self.proxy.clone();
                let win = window_id;
                components.runtime.spawn(async move {
                    let mut items = Vec::new();
                    if let Ok(list) = nb.read().await.list_notebooks().await {
                        for n in list {
                            items.push(NotebookListItem {
                                id: n.id.to_string(),
                                name: n.name.clone(),
                            });
                        }
                    }
                    let _ = proxy.send_event(Event::new(EventType::NotebooksList(items), win));
                });
            }
        }
    }

    fn notebooks_load_cells(&mut self, window_id: WindowId, notebook_id: String) {
        if let Some(components) = &self.components {
            if let Some(nb) = &components.notebook_manager {
                let nb = nb.clone();
                let proxy = self.proxy.clone();
                let win = window_id;
                components.runtime.spawn(async move {
                    let mut items = Vec::new();
                    if let Ok(id) = notebook_id.parse::<crate::notebooks::NotebookId>() {
                        if let Ok(mut cells) = nb.read().await.list_cells(id).await {
                            cells.sort_by_key(|c| c.idx);
                            for c in cells {
                                let summary = match c.cell_type {
                                    crate::notebooks::CellType::Markdown => {
                                        c.content.lines().next().unwrap_or("").to_string()
                                    }
                                    crate::notebooks::CellType::Command => c.content.clone(),
                                };
                                let cell_type = match c.cell_type {
                                    crate::notebooks::CellType::Markdown => "md",
                                    crate::notebooks::CellType::Command => "cmd",
                                }
                                .to_string();
                                items.push(NotebookCellItem {
                                    id: c.id.to_string(),
                                    idx: c.idx,
                                    cell_type,
                                    summary,
                                    exit_code: c.exit_code,
                                    duration_ms: c.duration_ms,
                                });
                            }
                        }
                    }
                    let _ = proxy.send_event(Event::new(EventType::NotebooksCells(items), win));
                });
            }
        }
    }
}

// Application event handler implementation
impl ApplicationHandler<Event> for Processor {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause != StartCause::Init || self.cli_options.daemon {
            return;
        }

        if let Some(window_options) = self.initial_window_options.take() {
            if let Err(err) = self.create_initial_window(event_loop, window_options) {
                self.initial_window_error = Some(err);
                event_loop.exit();
                return;
            }

            // Initialize components after the first window is created
            if let Some(_window_context) = self.windows.values().next() {
                // Background components are disabled in this build to avoid non-Send captures.
                #[cfg(feature = "background-components")]
                {
                    info!("background-components enabled: async init disabled for thread-safety");
                }
            }
        }

        info!("Initialisation complete");
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.config.debug.print_events {
            info!(target: LOG_TARGET_WINIT, "{event:?}");
        }

        // Ignore all events we do not care about.
        if Self::skip_window_event(&event) {
            return;
        }

        let window_context = match self.windows.get_mut(&window_id) {
            Some(window_context) => window_context,
            None => return,
        };

        let is_redraw = matches!(event, WindowEvent::RedrawRequested);

        window_context.handle_event(
            #[cfg(target_os = "macos")]
            _event_loop,
            &self.proxy,
            &mut self.clipboard,
            &mut self.scheduler,
            WinitEvent::WindowEvent { window_id, event },
        );

        if is_redraw {
            window_context.draw(&mut self.scheduler);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        if self.config.debug.print_events {
            info!(target: LOG_TARGET_WINIT, "{event:?}");
        }

        // Handle events which don't mandate the WindowId.
        match (event.payload, event.window_id.as_ref()) {
            // Process IPC config update.
            #[cfg(unix)]
            (EventType::IpcConfig(ipc_config), window_id) => {
                // Try and parse options as toml.
                let mut options = ParsedOptions::from_options(&ipc_config.options);

                // Override IPC config for each window with matching ID.
                for (_, window_context) in self
                    .windows
                    .iter_mut()
                    .filter(|(id, _)| window_id.is_none() || window_id == Some(*id))
                {
                    if ipc_config.reset {
                        window_context.reset_window_config(self.config.clone());
                    } else {
                        window_context.add_window_config(self.config.clone(), &options);
                    }
                }

                // Persist global options for future windows.
                if window_id.is_none() {
                    if ipc_config.reset {
                        self.global_ipc_options.clear();
                    } else {
                        self.global_ipc_options.append(&mut options);
                    }
                }
            }
            // Process IPC config requests.
            #[cfg(unix)]
            (EventType::IpcGetConfig(stream), window_id) => {
                // Get the config for the requested window ID.
                let config = match self.windows.iter().find(|(id, _)| window_id == Some(*id)) {
                    Some((_, window_context)) => window_context.config(),
                    None => &self.global_ipc_options.override_config_rc(self.config.clone()),
                };

                // Convert config to JSON format.
                let config_json = match serde_json::to_string(&config) {
                    Ok(config_json) => config_json,
                    Err(err) => {
                        error!("Failed config serialization: {err}");
                        return;
                    }
                };

                // Send JSON config to the socket.
                if let Ok(mut stream) = stream.try_clone() {
                    ipc::send_reply(&mut stream, SocketReply::GetConfig(config_json));
                }
            }
            // Process sync IPC commands.
            #[cfg(all(unix, feature = "sync"))]
            (EventType::IpcSync(sync_type, stream), _) => {
                use openagent_terminal_sync::{LocalFsProvider, SyncProvider, SyncScope};

                // Create sync provider based on config.
                // For now, always use LocalFsProvider.
                let sync_config = openagent_terminal_sync::SyncConfig {
                    provider: "local_fs".to_string(),
                    data_dir: None,
                    endpoint_env: None,
                    encryption_key_env: None,
                };

                let provider = match LocalFsProvider::new(&sync_config) {
                    Ok(provider) => provider,
                    Err(err) => {
                        if let Ok(mut stream) = stream.try_clone() {
                            let reply = ipc::SocketReply::SyncResult(Err(format!(
                                "Failed to create sync provider: {:?}",
                                err
                            )));
                            ipc::send_reply(&mut stream, reply);
                        }
                        return;
                    }
                };

                // Convert scope argument to sync scope.
                let scope = sync_type.scope().map(|s| match s {
                    crate::cli::SyncScopeArg::Settings => SyncScope::Settings,
                    crate::cli::SyncScopeArg::History => SyncScope::History,
                });

                // Execute sync operation.
                match sync_type {
                    IpcSyncType::Status(_) => match provider.status() {
                        Ok(status) => {
                            let status_json = serde_json::to_string_pretty(&status)
                                .unwrap_or_else(|_| "Error serializing status".to_string());
                            if let Ok(mut stream) = stream.try_clone() {
                                ipc::send_reply(
                                    &mut stream,
                                    ipc::SocketReply::SyncStatus(status_json),
                                );
                            }
                        }
                        Err(err) => {
                            if let Ok(mut stream) = stream.try_clone() {
                                let reply = ipc::SocketReply::SyncResult(Err(format!(
                                    "Failed to get sync status: {:?}",
                                    err
                                )));
                                ipc::send_reply(&mut stream, reply);
                            }
                        }
                    },
                    IpcSyncType::Push(_) => {
                        let scope = scope.unwrap_or(SyncScope::Settings);
                        match provider.push(scope) {
                            Ok(()) => {
                                if let Ok(mut stream) = stream.try_clone() {
                                    let reply = ipc::SocketReply::SyncResult(Ok(format!(
                                        "Successfully pushed {:?}",
                                        scope
                                    )));
                                    ipc::send_reply(&mut stream, reply);
                                }
                            }
                            Err(err) => {
                                if let Ok(mut stream) = stream.try_clone() {
                                    let reply = ipc::SocketReply::SyncResult(Err(format!(
                                        "Failed to push: {:?}",
                                        err
                                    )));
                                    ipc::send_reply(&mut stream, reply);
                                }
                            }
                        }
                    }
                    IpcSyncType::Pull(_) => {
                        let scope = scope.unwrap_or(SyncScope::Settings);
                        match provider.pull(scope) {
                            Ok(()) => {
                                if let Ok(mut stream) = stream.try_clone() {
                                    let reply = ipc::SocketReply::SyncResult(Ok(format!(
                                        "Successfully pulled {:?}",
                                        scope
                                    )));
                                    ipc::send_reply(&mut stream, reply);
                                }
                            }
                            Err(err) => {
                                if let Ok(mut stream) = stream.try_clone() {
                                    let reply = ipc::SocketReply::SyncResult(Err(format!(
                                        "Failed to pull: {:?}",
                                        err
                                    )));
                                    ipc::send_reply(&mut stream, reply);
                                }
                            }
                        }
                    }
                };
            }
            (EventType::ConfigReload(path), _) => {
                // Clear config logs from message bar for all terminals.
                for window_context in self.windows.values_mut() {
                    if !window_context.message_buffer.is_empty() {
                        window_context.message_buffer.remove_target(LOG_TARGET_CONFIG);
                        window_context.display.pending_update.dirty = true;
                    }
                }

                // Load config and update each terminal.
                if let Ok(config) = config::reload(&path, &mut self.cli_options) {
                    self.config = Rc::new(config);


                    // Restart config monitor if imports changed.
                    if let Some(monitor) = self.config_monitor.take() {
                        let paths = &self.config.config_paths;
                        self.config_monitor = if monitor.needs_restart(paths) {
                            monitor.shutdown();
                            ConfigMonitor::new(paths.clone(), self.proxy.clone())
                        } else {
                            Some(monitor)
                        };
                    }

                    for window_context in self.windows.values_mut() {
                        window_context.update_config(self.config.clone());
                    }
                }
            }
            // Handle component initialization completion
            (EventType::ComponentsInitialized(components), _) => {
                info!("Components initialized, updating window contexts...");
                self.components = Some(components.clone());

                // Update all existing windows with the initialized components
                for window_context in self.windows.values_mut() {
                    window_context.set_components(components.clone());
                }
            }
            // Create a new terminal window.
            (EventType::CreateWindow(options), _) => {
                // XXX Ensure that no context is current when creating a new window,
                // otherwise it may lock the backing buffer of the
                // surface of current context when asking
                // e.g. EGL on Wayland to create a new context.
                for window_context in self.windows.values_mut() {
                    window_context.display.make_not_current();
                }

                let is_first = self.windows.is_empty();
                if is_first {
                    // Handle initial window creation in daemon mode.
                    if let Err(err) = self.create_initial_window(event_loop, options) {
                        self.initial_window_error = Some(err);
                        event_loop.exit();
                    }
                } else if let Err(err) = self.create_window(event_loop, options) {
                    error!("Could not open window: {err:?}");
                }
            }
            // Process events affecting all windows.
            // Forward paste commands directly
            (EventType::PasteCommand(text), Some(window_id)) => {
                if let Err(e) = self
                    .proxy
                    .send_event(Event::new(EventType::PasteCommandChecked(text), *window_id))
                {
                    tracing::warn!("failed to post PasteCommandChecked: {:?}", e);
                }
            }

            (payload, None) => {
                // For broadcast events that modify UI state (like ConfirmResolved), handle here
                if let EventType::ConfirmResolved { id, .. } = &payload {
                    for window_context in self.windows.values_mut() {
                        window_context.display.confirm_overlay.close_if(id);
                        window_context.dirty = true;
                        if window_context.display.window.has_frame {
                            window_context.display.window.request_redraw();
                        }
                    }
                    return;
                }
                let event = WinitEvent::UserEvent(Event::new(payload, None));
                for window_context in self.windows.values_mut() {
                    window_context.handle_event(
                        #[cfg(target_os = "macos")]
                        event_loop,
                        &self.proxy,
                        &mut self.clipboard,
                        &mut self.scheduler,
                        event.clone(),
                    );
                }
            }
            // Notebooks UI events
            #[cfg(feature = "never")]
            (EventType::NotebooksOpen, Some(window_id)) => {
                self.notebooks_open(*window_id);
            }
            #[cfg(feature = "never")]
            (EventType::NotebooksList(items), Some(window_id)) => {
                if let Some(win) = self.windows.get_mut(window_id) {
                    win.display.notebooks_panel.notebooks = items;
                    win.display.notebooks_panel.selected_notebook = None;
                    win.display.notebooks_panel.cells.clear();
                    win.dirty = true;
                    if win.display.window.has_frame {
                        win.display.window.request_redraw();
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::NotebooksSelect(id), Some(window_id)) => {
                if let Some(win) = self.windows.get_mut(window_id) {
                    win.display.notebooks_panel.selected_notebook = Some(id.clone());
                    win.display.notebooks_panel.cells.clear();
                }
                self.notebooks_load_cells(*window_id, id);
            }
            #[cfg(feature = "never")]
            (EventType::NotebooksCells(cells), Some(window_id)) => {
                if let Some(win) = self.windows.get_mut(window_id) {
                    win.display.notebooks_panel.cells = cells;
                    win.dirty = true;
                    if win.display.window.has_frame {
                        win.display.window.request_redraw();
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::NotebooksRunCell(cell_id), Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(nb) = &components.notebook_manager {
                        let nb = nb.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        components.runtime.spawn(async move {
                            if let Ok(id) = cell_id.parse::<crate::notebooks::CellId>() {
                                let _ = nb.read().await.run_cell(id).await;
                            }
                            let _ = proxy.send_event(Event::new(EventType::NotebooksOpen, win));
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::NotebooksRunNotebook(nb_id), Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(nb) = &components.notebook_manager {
                        let nb = nb.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        components.runtime.spawn(async move {
                            if let Ok(id) = nb_id.parse::<crate::notebooks::NotebookId>() {
                                let _ = nb.read().await.run_notebook(id).await;
                            }
                            let _ = proxy.send_event(Event::new(EventType::NotebooksOpen, win));
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::NotebooksAddCommand(nb_id), Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(nb) = &components.notebook_manager {
                        let nb = nb.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        components.runtime.spawn(async move {
                            if let Ok(id) = nb_id.parse::<crate::notebooks::NotebookId>() {
                                let _ = nb
                                    .read()
                                    .await
                                    .add_command_cell(
                                        id,
                                        None,
                                        "echo New cell".to_string(),
                                        None,
                                        None,
                                    )
                                    .await;
                                // Reload cells for notebook
                                let _ = proxy
                                    .send_event(Event::new(EventType::NotebooksSelect(nb_id), win));
                            }
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::NotebooksAddMarkdown(nb_id), Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(nb) = &components.notebook_manager {
                        let nb = nb.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        components.runtime.spawn(async move {
                            if let Ok(id) = nb_id.parse::<crate::notebooks::NotebookId>() {
                                let _ = nb
                                    .read()
                                    .await
                                    .add_markdown_cell(id, None, "# New cell".to_string())
                                    .await;
                                let _ = proxy
                                    .send_event(Event::new(EventType::NotebooksSelect(nb_id), win));
                            }
                        });
                    }
                }
            }

            #[cfg(feature = "never")]
            (EventType::NotebooksDeleteCell(cell_id), Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(nb) = &components.notebook_manager {
                        let nbm = nb.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        components.runtime.spawn(async move {
                            if let Ok(id) = cell_id.parse::<crate::notebooks::CellId>() {
                                let _ = nbm.read().await.delete_cell(id).await;
                            }
                            // Refresh panel
                            let _ = proxy.send_event(Event::new(EventType::NotebooksOpen, win));
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::NotebooksConvertCellToMarkdown(cell_id), Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(nb) = &components.notebook_manager {
                        let nbm = nb.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        components.runtime.spawn(async move {
                            if let Ok(id) = cell_id.parse::<crate::notebooks::CellId>() {
                                let _ = nbm
                                    .read()
                                    .await
                                    .convert_cell_type(id, crate::notebooks::CellType::Markdown)
                                    .await;
                            }
                            let _ = proxy.send_event(Event::new(EventType::NotebooksOpen, win));
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::NotebooksConvertCellToCommand(cell_id), Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(nb) = &components.notebook_manager {
                        let nbm = nb.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        components.runtime.spawn(async move {
                            if let Ok(id) = cell_id.parse::<crate::notebooks::CellId>() {
                                let _ = nbm
                                    .read()
                                    .await
                                    .convert_cell_type(id, crate::notebooks::CellType::Command)
                                    .await;
                            }
                            let _ = proxy.send_event(Event::new(EventType::NotebooksOpen, win));
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::NotebooksExportNotebook(nb_id), Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(nb) = &components.notebook_manager {
                        let nbm = nb.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        components.runtime.spawn(async move {
                            if let Ok(id) = nb_id.parse::<crate::notebooks::NotebookId>() {
                                if let Ok(bytes) =
                                    nbm.read().await.export_notebook_markdown_bytes(id).await
                                {
                                    if let Ok(s) = String::from_utf8(bytes) {
                                        let _ = proxy.send_event(Event::new(
                                            EventType::PasteCommand(s),
                                            win,
                                        ));
                                    }
                                }
                            }
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::NotebooksEditApply { cell_id, content }, Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(nb) = &components.notebook_manager {
                        let nbm = nb.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        components.runtime.spawn(async move {
                            if let Ok(id) = cell_id.parse::<crate::notebooks::CellId>() {
                                let _ = nbm.read().await.update_cell_content(id, content).await;
                            }
                            let _ = proxy.send_event(Event::new(EventType::NotebooksOpen, win));
                        });
                    }
                }
            }

            // Warp UI update events
            (EventType::WarpUiUpdate(update_type), Some(window_id)) => {
                use crate::workspace::WarpUiUpdateType;

                if let Some(window_context) = self.windows.get_mut(window_id) {
                    match update_type {
                        // Autosave session event
                        WarpUiUpdateType::SessionAutoSave => {
                            if let Some(warp) = &mut window_context.workspace.warp {
                                if warp.should_auto_save() {
                                    let _ = warp.execute_warp_action(
                                        &crate::workspace::WarpAction::SaveSession,
                                    );
                                }
                            }
                        }

                        // Tab-related events
                        WarpUiUpdateType::TabCreated(_tab_id) => {
                            // Tab created - trigger UI redraw
                            info!("Warp tab created");
                            window_context.dirty = true;
                            if window_context.display.window.has_frame {
                                window_context.display.window.request_redraw();
                            }
                        }

                        WarpUiUpdateType::TabClosed(_tab_id) => {
                            // Tab closed - trigger UI redraw
                            info!("Warp tab closed");
                            window_context.dirty = true;
                            if window_context.display.window.has_frame {
                                window_context.display.window.request_redraw();
                            }
                        }

                        WarpUiUpdateType::TabSwitched { tab_id: _ } => {
                            // Tab switched - update UI state
                            info!("Warp tab switched");
                            window_context.dirty = true;
                            if window_context.display.window.has_frame {
                                window_context.display.window.request_redraw();
                            }
                        }

                        // Pane-related events
                        WarpUiUpdateType::PaneSplit { tab_id: _, new_pane_id: _ } => {
                            // Pane split - major layout change
                            info!("Warp pane split created");
                            window_context.dirty = true;
                            if window_context.display.window.has_frame {
                                window_context.display.window.request_redraw();
                            }
                        }

                        WarpUiUpdateType::PaneFocused { tab_id: _, pane_id: _ } => {
                            // Pane focused - update focus indicators
                            window_context.dirty = true;
                            if window_context.display.window.has_frame {
                                window_context.display.window.request_redraw();
                            }
                        }

                        WarpUiUpdateType::PaneResized { tab_id: _, pane_id: _ } => {
                            // Pane resized - layout change
                            window_context.dirty = true;
                            if window_context.display.window.has_frame {
                                window_context.display.window.request_redraw();
                            }
                        }

                        WarpUiUpdateType::PaneZoomed { tab_id: _, pane_id: _, zoomed } => {
                            // Pane zoom toggled - major layout change
                            info!("Warp pane zoom toggled: {}", zoomed);
                            window_context.dirty = true;
                            if window_context.display.window.has_frame {
                                window_context.display.window.request_redraw();
                            }
                        }

                        WarpUiUpdateType::PaneClosed {
                            tab_id: _,
                            closed_pane_id: _,
                            new_active_pane_id: _,
                        } => {
                            // Pane closed - layout and focus change
                            info!("Warp pane closed");
                            window_context.dirty = true;
                            if window_context.display.window.has_frame {
                                window_context.display.window.request_redraw();
                            }
                        }

                        WarpUiUpdateType::SplitsEqualized { tab_id: _ } => {
                            // Splits equalized - layout change
                            info!("Warp splits equalized");
                            window_context.dirty = true;
                            if window_context.display.window.has_frame {
                                window_context.display.window.request_redraw();
                            }
                        }
                    }
                }
            }
            // Blocks search events handled at processor level
            #[cfg(feature = "never")]
            (EventType::BlocksSearchPerform(query), Some(window_id)) => {
                self.process_blocks_search_perform(query, *window_id);
            }
            #[cfg(feature = "never")]
            (EventType::BlocksSearchResults(items), Some(window_id)) => {
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    window_context.display.blocks_search.results = items;
                    window_context.dirty = true;
                    if window_context.display.window.has_frame {
                        window_context.display.window.request_redraw();
                    }
                }
            }
            // Workflows panel events
            #[cfg(feature = "workflow")]
            (EventType::WorkflowsSearchPerform(query), Some(window_id)) => {
                self.process_workflows_search_perform(query, *window_id);
            }
            #[cfg(feature = "workflow")]
            (
                EventType::WorkflowsOpenParamsForm { workflow_id, workflow_name, params },
                Some(window_id),
            ) => {
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    window_context.display.workflows_params.open(
                        workflow_id,
                        workflow_name,
                        params,
                    );
                    window_context.dirty = true;
                    if window_context.display.window.has_frame {
                        window_context.display.window.request_redraw();
                    }
                }
            }
            #[cfg(feature = "workflow")]
            (EventType::WorkflowsCancelParams, Some(window_id)) => {
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    window_context.display.workflows_params.close();
                    window_context.dirty = true;
                    if window_context.display.window.has_frame {
                        window_context.display.window.request_redraw();
                    }
                }
            }
            #[cfg(feature = "workflow")]
            (EventType::WorkflowsSubmitParams { workflow_id, values }, Some(window_id)) => {
                // Execute with provided params via engine
                if let Some(components) = &self.components {
                    if let Some(engine) = &components.workflow_engine {
                        let engine = engine.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        let runtime = components.runtime.clone();
                        runtime.spawn(async move {
                            match engine.execute_workflow(&workflow_id, values).await {
                                Ok(exec_id) => {
                                    // Try fetch workflow name for UI
                                    let wf_name = {
                                        let wf = engine
                                            .list_workflows()
                                            .await
                                            .into_iter()
                                            .find(|(id, _)| id == &workflow_id);
                                        wf.map(|(_, d)| d.name)
                                            .unwrap_or_else(|| workflow_id.clone())
                                    };
                                    let message = crate::message_bar::Message::new(
                                        format!(
                                            "Started workflow '{}' (execution {})",
                                            wf_name, exec_id
                                        ),
                                        crate::message_bar::MessageType::Warning,
                                    );
                                    let _ = proxy
                                        .send_event(Event::new(EventType::Message(message), win));
                                    let _ = proxy.send_event(Event::new(
                                        EventType::WorkflowsProgressUpdate {
                                            execution_id: exec_id.clone(),
                                            workflow_name: Some(wf_name.clone()),
                                            status: Some("Starting".to_string()),
                                            current_step: None,
                                            log: None,
                                            done: false,
                                        },
                                        win,
                                    ));

                                    // Subscribe to updates
                                    let mut rx = engine.subscribe();
                                    loop {
                                        use workflow_engine::WorkflowEvent;
                                        match rx.recv().await {
                                            Ok(ev) => match ev {
                                                WorkflowEvent::Started { execution_id }
                                                    if execution_id == exec_id =>
                                                {
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: Some(wf_name.clone()),
                                                            status: Some("Running".to_string()),
                                                            current_step: None,
                                                            log: None,
                                                            done: false,
                                                        },
                                                        win,
                                                    ));
                                                }
                                                WorkflowEvent::StepStarted {
                                                    execution_id,
                                                    step_id,
                                                } if execution_id == exec_id => {
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: None,
                                                            status: Some("Running".to_string()),
                                                            current_step: Some(step_id),
                                                            log: None,
                                                            done: false,
                                                        },
                                                        win,
                                                    ));
                                                }
                                                WorkflowEvent::StepCompleted {
                                                    execution_id,
                                                    step_id,
                                                } if execution_id == exec_id => {
                                                    let msg = format!("Completed step {step_id}");
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: None,
                                                            status: None,
                                                            current_step: None,
                                                            log: Some(msg),
                                                            done: false,
                                                        },
                                                        win,
                                                    ));
                                                }
                                                WorkflowEvent::StepFailed {
                                                    execution_id,
                                                    step_id,
                                                } if execution_id == exec_id => {
                                                    let msg = format!("Failed step {step_id}");
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: None,
                                                            status: Some("Failed".to_string()),
                                                            current_step: Some(step_id),
                                                            log: Some(msg),
                                                            done: false,
                                                        },
                                                        win,
                                                    ));
                                                }
                                                WorkflowEvent::Completed {
                                                    execution_id,
                                                    status,
                                                } if execution_id == exec_id => {
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: Some(wf_name.clone()),
                                                            status: Some(format!("{:?}", status)),
                                                            current_step: None,
                                                            log: None,
                                                            done: true,
                                                        },
                                                        win,
                                                    ));
                                                }
                                                WorkflowEvent::Log {
                                                    execution_id,
                                                    step_id: _,
                                                    message,
                                                } if execution_id == exec_id => {
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: None,
                                                            status: None,
                                                            current_step: None,
                                                            log: Some(message),
                                                            done: false,
                                                        },
                                                        win,
                                                    ));
                                                }
                                                _ => {}
                                            },
                                            Err(_) => break,
                                        }
                                    }
                                }
                                Err(e) => {
                                    let m = crate::message_bar::Message::new(
                                        format!("Workflow failed to start: {}", e),
                                        crate::message_bar::MessageType::Warning,
                                    );
                                    let _ =
                                        proxy.send_event(Event::new(EventType::Message(m), win));
                                }
                            }
                        });
                    }
                }
                // Close params overlay in current window
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    window_context.display.workflows_params.close();
                    window_context.dirty = true;
                    if window_context.display.window.has_frame {
                        window_context.display.window.request_redraw();
                    }
                }
            }
            #[cfg(feature = "workflow")]
            (EventType::WorkflowsSearchResults(items), Some(window_id)) => {
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    window_context.display.workflows_panel.results = items;
                    window_context.dirty = true;
                    if window_context.display.window.has_frame {
                        window_context.display.window.request_redraw();
                    }
                }
            }
            #[cfg(feature = "workflow")]
            (
                EventType::WorkflowsProgressUpdate {
                    execution_id,
                    workflow_name,
                    status,
                    current_step,
                    log,
                    done,
                },
                Some(window_id),
            ) => {
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    let st = &mut window_context.display.workflows_progress;
                    // Always show overlay on updates
                    st.active = true;
                    if st.execution_id.as_deref() != Some(&execution_id) {
                        st.execution_id = Some(execution_id.clone());
                        st.logs.clear();
                        st.step_index = 0;
                        st.total_steps = None;
                        st.seen_steps.clear();
                    }
                    if let Some(name) = workflow_name {
                        st.workflow_name = Some(name);
                    }
                    if let Some(s) = status {
                        st.status = Some(s);
                    }
                    if let Some(step) = current_step {
                        st.current_step = Some(step.clone());
                        if !st.seen_steps.contains(&step) {
                            st.seen_steps.push(step);
                            st.step_index = st.seen_steps.len();
                        }
                    }
                    if let Some(line) = log {
                        st.logs.push(line);
                        if st.logs.len() > 500 {
                            let drop = st.logs.len() - 500;
                            st.logs.drain(0..drop);
                        }
                    }

                    // If done, schedule a quick clear to retain UI briefly
                    if done {
                        let tid = crate::scheduler::TimerId::new(
                            crate::scheduler::Topic::WorkflowsProgressRetain,
                            *window_id,
                        );
                        self.scheduler.unschedule(tid);
                        let evt = Event::new(
                            EventType::WorkflowsProgressClear(execution_id.clone()),
                            *window_id,
                        );
                        self.scheduler.schedule(evt, WORKFLOWS_OVERLAY_RETAIN, false, tid);
                    }

                    window_context.dirty = true;
                    if window_context.display.window.has_frame {
                        window_context.display.window.request_redraw();
                    }
                }
            }
            #[cfg(feature = "workflow")]
            (EventType::WorkflowsProgressClear(execution_id), Some(window_id)) => {
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    let st = &mut window_context.display.workflows_progress;
                    if st.execution_id.as_deref() == Some(execution_id.as_str()) {
                        st.active = false;
                        window_context.dirty = true;
                        if window_context.display.window.has_frame {
                            window_context.display.window.request_redraw();
                        }
                    }
                }
            }
            #[cfg(feature = "workflow")]
            (EventType::WorkflowsExecuteByName(name), Some(window_id)) => {
                // Try engine first; if workflow has parameters, open params form; else execute.
                if let Some(components) = &self.components {
                    if let Some(engine) = &components.workflow_engine {
                        let engine = engine.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        // Extract workflow info from config before moving into async block
                        let fallback_workflow = self
                            .config
                            .workflows
                            .iter()
                            .find(|w| w.name == name)
                            .map(|wf| (wf.command.clone(), wf.params.clone()));
                        let runtime = components.runtime.clone();
                        runtime.spawn(async move {
                            // Check if engine has this by name
                            if let Some((wf_id, def)) = engine.get_workflow_by_name(&name).await {
                                if !def.parameters.is_empty() {
                                    // Open params form
                                    let _ = proxy.send_event(Event::new(
                                        EventType::WorkflowsOpenParamsForm {
                                            workflow_id: wf_id,
                                            workflow_name: def.name.clone(),
                                            params: def.parameters.clone(),
                                        },
                                        win,
                                    ));
                                    return;
                                }
                                // No params: run immediately with empty map
                                use std::collections::HashMap;
                                match engine.execute_workflow(&wf_id, HashMap::new()).await {
                                    Ok(exec_id) => {
                                        let message = crate::message_bar::Message::new(
                                            format!("Started workflow '{}' (execution {})", def.name, exec_id),
                                            crate::message_bar::MessageType::Warning,
                                        );
                                        let _ = proxy.send_event(Event::new(EventType::Message(message), win));
                                        let _ = proxy.send_event(Event::new(
                                            EventType::WorkflowsProgressUpdate {
                                                execution_id: exec_id.clone(),
                                                workflow_name: Some(def.name.clone()),
                                                status: Some("Starting".to_string()),
                                                current_step: None,
                                                log: None,
                                                done: false,
                                            },
                                            win,
                                        ));

                                        // Subscribe to workflow engine events and forward updates
                                        let mut rx = engine.subscribe();
                                        loop {
                                            use workflow_engine::WorkflowEvent;
                                            match rx.recv().await {
                                                Ok(ev) => match ev {
                                                    WorkflowEvent::Started { execution_id } if execution_id == exec_id => {
                                                        let _ = proxy.send_event(Event::new(
                                                            EventType::WorkflowsProgressUpdate {
                                                                execution_id,
                                                                workflow_name: Some(def.name.clone()),
                                                                status: Some("Running".to_string()),
                                                                current_step: None,
                                                                log: None,
                                                                done: false,
                                                            },
                                                            win,
                                                        ));
                                                    }
                                                    WorkflowEvent::StepStarted { execution_id, step_id } if execution_id == exec_id => {
                                                        let _ = proxy.send_event(Event::new(
                                                            EventType::WorkflowsProgressUpdate {
                                                                execution_id,
                                                                workflow_name: None,
                                                                status: Some("Running".to_string()),
                                                                current_step: Some(step_id),
                                                                log: None,
                                                                done: false,
                                                            },
                                                            win,
                                                        ));
                                                    }
                                                    WorkflowEvent::StepCompleted { execution_id, step_id } if execution_id == exec_id => {
                                                        let msg = format!("Completed step {step_id}");
                                                        let _ = proxy.send_event(Event::new(
                                                            EventType::WorkflowsProgressUpdate {
                                                                execution_id,
                                                                workflow_name: None,
                                                                status: None,
                                                                current_step: None,
                                                                log: Some(msg),
                                                                done: false,
                                                            },
                                                            win,
                                                        ));
                                                    }
                                                    WorkflowEvent::Completed { execution_id, status } if execution_id == exec_id => {
                                                        let _ = proxy.send_event(Event::new(
                                                            EventType::WorkflowsProgressUpdate {
                                                                execution_id,
                                                                workflow_name: Some(def.name.clone()),
                                                                status: Some(format!("{:?}", status)),
                                                                current_step: None,
                                                                log: None,
                                                                done: true,
                                                            },
                                                            win,
                                                        ));
                                                    }
                                                    WorkflowEvent::Log { execution_id, step_id: _, message } if execution_id == exec_id => {
                                                        let _ = proxy.send_event(Event::new(
                                                            EventType::WorkflowsProgressUpdate {
                                                                execution_id,
                                                                workflow_name: None,
                                                                status: None,
                                                                current_step: None,
                                                                log: Some(message),
                                                                done: false,
                                                            },
                                                            win,
                                                        ));
                                                    }
                                                    _ => {}
                                                },
                                                Err(_) => break,
                                            }
                                        }
                                        return;
                                    }
                                    Err(e) => {
                                        let m = crate::message_bar::Message::new(
                                            format!("Workflow failed to start: {}", e),
                                            crate::message_bar::MessageType::Warning,
                                        );
                                        let _ = proxy.send_event(Event::new(EventType::Message(m), win));
                                        return;
                                    }
                                }
                            }
                            use std::collections::HashMap;
                            match engine.execute_workflow(&name, HashMap::new()).await {
                                Ok(exec_id) => {
                                    // Notify user and open progress overlay with initial state
                                    let message = crate::message_bar::Message::new(
                                        format!(
                                            "Started workflow '{}' (execution {})",
                                            name, exec_id
                                        ),
                                        crate::message_bar::MessageType::Warning,
                                    );
                                    let _ = proxy
                                        .send_event(Event::new(EventType::Message(message), win));
                                    let _ = proxy.send_event(Event::new(
                                        EventType::WorkflowsProgressUpdate {
                                            execution_id: exec_id.clone(),
                                            workflow_name: Some(name.clone()),
                                            status: Some("Starting".to_string()),
                                            current_step: None,
                                            log: None,
                                            done: false,
                                        },
                                        win,
                                    ));

                                    // Subscribe to workflow engine events and forward updates
                                    let mut rx = engine.subscribe();
                                    loop {
                                        use workflow_engine::WorkflowEvent;
                                        match rx.recv().await {
                                            Ok(ev) => match ev {
                                                WorkflowEvent::Started { execution_id }
                                                    if execution_id == exec_id =>
                                                {
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: Some(name.clone()),
                                                            status: Some("Running".to_string()),
                                                            current_step: None,
                                                            log: None,
                                                            done: false,
                                                        },
                                                        win,
                                                    ));
                                                }
                                                WorkflowEvent::StepStarted {
                                                    execution_id,
                                                    step_id,
                                                } if execution_id == exec_id => {
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: None,
                                                            status: Some("Running".to_string()),
                                                            current_step: Some(step_id),
                                                            log: None,
                                                            done: false,
                                                        },
                                                        win,
                                                    ));
                                                }
                                                WorkflowEvent::StepCompleted {
                                                    execution_id,
                                                    step_id,
                                                } if execution_id == exec_id => {
                                                    let msg = format!("Completed step {step_id}");
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: None,
                                                            status: None,
                                                            current_step: None,
                                                            log: Some(msg),
                                                            done: false,
                                                        },
                                                        win,
                                                    ));
                                                }
                                                WorkflowEvent::StepFailed {
                                                    execution_id,
                                                    step_id,
                                                } if execution_id == exec_id => {
                                                    let msg = format!("Step failed: {step_id}");
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: None,
                                                            status: Some("Failed".to_string()),
                                                            current_step: Some(step_id),
                                                            log: Some(msg),
                                                            done: false,
                                                        },
                                                        win,
                                                    ));
                                                }
                                                WorkflowEvent::Completed {
                                                    execution_id,
                                                    status,
                                                } if execution_id == exec_id => {
                                                    let status_str = format!("{status:?}");
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: None,
                                                            status: Some(status_str),
                                                            current_step: None,
                                                            log: None,
                                                            done: true,
                                                        },
                                                        win,
                                                    ));
                                                    break;
                                                }
                                                WorkflowEvent::Log {
                                                    execution_id,
                                                    step_id: _,
                                                    message,
                                                } if execution_id == exec_id => {
                                                    let _ = proxy.send_event(Event::new(
                                                        EventType::WorkflowsProgressUpdate {
                                                            execution_id,
                                                            workflow_name: None,
                                                            status: None,
                                                            current_step: None,
                                                            log: Some(message),
                                                            done: false,
                                                        },
                                                        win,
                                                    ));
                                                }
                                                _ => {}
                                            },
                                            Err(_e) => {
                                                // Receiver closed; stop forwarding
                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(_e) => {
                                    // Fallback to config command paste
                                    if let Some((cmd_template, params)) = fallback_workflow {
                                        let mut cmd = cmd_template;
                                        for p in &params {
                                            let placeholder = format!("{{{}}}", p.name);
                                            let val = p.default.clone().unwrap_or_default();
                                            cmd = cmd.replace(&placeholder, &val);
                                        }
                                        let _ = proxy.send_event(Event::new(
                                            EventType::PasteCommand(cmd),
                                            win,
                                        ));
                                    } else {
                                        let msg = crate::message_bar::Message::new(
                                            format!("Workflow not found: {}", name),
                                            crate::message_bar::MessageType::Warning,
                                        );
                                        let _ = proxy
                                            .send_event(Event::new(EventType::Message(msg), win));
                                    }
                                }
                            }
                        });
                        return;
                    }
                }
                // No engine: fallback immediately
                if let Some(wf) = self.config.workflows.iter().find(|w| w.name == name) {
                    // If workflow has a 'confirm' parameter, show confirmation overlay first
                    let has_confirm = wf.params.iter().any(|p| p.name == "confirm");
                    if has_confirm {
                        let id = crate::ui_confirm::generate_id();
                        // Build a preview with default parameters (without forcing confirm=yes yet)
                        let mut preview_cmd = wf.command.clone();
                        for p in &wf.params {
                            let placeholder = format!("{{{}}}", p.name);
                            let val = p.default.clone().unwrap_or_default();
                            preview_cmd = preview_cmd.replace(&placeholder, &val);
                        }
                        let body = format!(
                            "About to run guarded workflow: {}\n\nPreview (with defaults):\n  \
                             {}\n\nProceed?",
                            wf.name, preview_cmd
                        );
                        // Track pending workflow confirmation
                        self.pending_workflow_confirms
                            .insert(id.clone(), (wf.name.clone(), *window_id));
                        let _ = self.proxy.send_event(Event::new(
                            EventType::ConfirmOpen {
                                id: id.clone(),
                                title: format!("Confirm workflow: {}", wf.name),
                                body,
                                confirm_label: Some("Run".into()),
                                cancel_label: Some("Cancel".into()),
                            },
                            *window_id,
                        ));
                    } else {
                        let mut cmd = wf.command.clone();
                        for p in &wf.params {
                            let placeholder = format!("{{{}}}", p.name);
                            let val = p.default.clone().unwrap_or_default();
                            cmd = cmd.replace(&placeholder, &val);
                        }
                        let _ = self
                            .proxy
                            .send_event(Event::new(EventType::PasteCommand(cmd), *window_id));
                    }
                } else {
                    let msg = crate::message_bar::Message::new(
                        format!("Workflow not found: {}", name),
                        crate::message_bar::MessageType::Warning,
                    );
                    let _ = self.proxy.send_event(Event::new(EventType::Message(msg), *window_id));
                }
            }
            (
                EventType::ConfirmOpen { id, title, body, confirm_label, cancel_label },
                Some(window_id),
            ) => {
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    window_context.display.confirm_overlay.open(
                        id.clone(),
                        title.clone(),
                        body.clone(),
                        confirm_label.clone(),
                        cancel_label.clone(),
                    );
                    window_context.dirty = true;
                    if window_context.display.window.has_frame {
                        window_context.display.window.request_redraw();
                    }
                }
            }
            (EventType::ConfirmRespond { id, accepted }, Some(_window_id)) => {
                // If this is a pending guarded workflow confirmation, handle it
                if let Some((wf_name, win)) = self.pending_workflow_confirms.remove(&id) {
                    if accepted {
                        if let Some(wf) = self.config.workflows.iter().find(|w| w.name == wf_name) {
                            let mut cmd = wf.command.clone();
                            for p in &wf.params {
                                let placeholder = format!("{{{}}}", p.name);
                                let val = if p.name == "confirm" {
                                    Some("yes".to_string())
                                } else {
                                    p.default.clone()
                                };
                                let val = val.unwrap_or_default();
                                cmd = cmd.replace(&placeholder, &val);
                            }
                            let _ = self
                                .proxy
                                .send_event(Event::new(EventType::PasteCommand(cmd), win));
                        }
                    } else {
                        let message = crate::message_bar::Message::new(
                            "Workflow canceled".into(),
                            crate::message_bar::MessageType::Warning,
                        );
                        let _ = self.proxy.send_event(Event::new(EventType::Message(message), win));
                    }
                }

                // Resolve for plugin-host waiters if any
                let _ = crate::ui_confirm::resolve(&id, accepted);
                // Broadcast resolution to close overlays in all windows
                let _ = self
                    .proxy
                    .send_event(Event::new(EventType::ConfirmResolved { id, accepted }, None));
            }
            (EventType::Terminal(TerminalEvent::Wakeup), Some(window_id)) => {
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    window_context.dirty = true;
                    if window_context.display.window.has_frame {
                        window_context.display.window.request_redraw();
                    }
                }
            }
            (EventType::Terminal(TerminalEvent::Exit), Some(window_id)) => {
                // Remove the closed terminal.
                let window_context = match self.windows.entry(*window_id) {
                    // Don't exit when terminal exits if user asked to hold the window.
                    Entry::Occupied(window_context)
                        if !window_context.get().display.window.hold =>
                    {
                        window_context.remove()
                    }
                    _ => return,
                };

                // Unschedule pending events.
                self.scheduler.unschedule_window(window_context.id());

                // Shutdown if no more terminals are open.
                if self.windows.is_empty() && !self.cli_options.daemon {
                    // Write ref tests of last window to disk.
                    if self.config.debug.ref_test {
                        window_context.write_ref_test_results();
                    }

                    event_loop.exit();
                }
            }
            // NOTE: This event bypasses batching to minimize input latency.
            (EventType::Frame, Some(window_id)) => {
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    window_context.display.window.has_frame = true;
                    // If any command block is running, drive a redraw for spinner/elapsed updates
                    let running = window_context.display.blocks.any_running();
                    if running {
                        window_context.dirty = true;
                    }
                    if window_context.dirty {
                        window_context.display.window.request_redraw();
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::PaletteRequestPluginCommands, Some(_window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(pm) = &components.plugin_manager {
                        let pm = pm.clone();
                        let rt = components.runtime.clone();
                        rt.spawn(async move {
                            // Capability metadata is not available in the lightweight runtime; skip appending plugin commands.
                            let _ids = pm.list_plugins().await;
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::PluginsRunCommand { plugin, command }, Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(pm) = &components.plugin_manager {
                        let pm = pm.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        let plugin_name = plugin.clone();
                        let cmd_name = command.clone();
                        let rt = components.runtime.clone();
                        rt.spawn(async move {
                            use serde_json::json;
let evt = PluginEvent {
                                event_type: "command".into(),
                                data: json!({ "name": cmd_name, "args": [] }),
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs(),
                            };
                            let resp = pm.send_event_to_plugin(&plugin_name, &evt).await;
                            match resp {
                                Ok(r) if r.success => {
                                    let msg = if let Some(res) = r.result {
                                        format!(
                                            "Plugin '{}' ran command successfully:\n{}",
                                            plugin_name, res
                                        )
                                    } else {
                                        format!("Plugin '{}' ran command successfully", plugin_name)
                                    };
                                    let m = crate::message_bar::Message::new(
                                        msg,
                                        crate::message_bar::MessageType::Warning,
                                    );
                                    let _ =
                                        proxy.send_event(Event::new(EventType::Message(m), win));
                                }
                                Ok(r) => {
                                    let msg = format!(
                                        "Plugin '{}' command failed: {}",
                                        plugin_name,
                                        r.error.unwrap_or_else(|| "Unknown error".into())
                                    );
                                    let m = crate::message_bar::Message::new(
                                        msg,
                                        crate::message_bar::MessageType::Warning,
                                    );
                                    let _ =
                                        proxy.send_event(Event::new(EventType::Message(m), win));
                                }
                                Err(e) => {
                                    let m = crate::message_bar::Message::new(
                                        format!("Plugin '{}' error: {}", plugin_name, e),
                                        crate::message_bar::MessageType::Warning,
                                    );
                                    let _ =
                                        proxy.send_event(Event::new(EventType::Message(m), win));
                                }
                            }
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::PluginsSearchPerform(query), Some(window_id)) => {
                // Build items from loaded plugins and discovered files; simple case-insensitive match
                if let Some(components) = &self.components {
                    if let Some(pm) = &components.plugin_manager {
                        let pm = pm.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        let rt = components.runtime.clone();
                        rt.spawn(async move {
                            let mut items: Vec<crate::display::plugin_panel::PluginItem> =
                                Vec::new();
                            let q = query.to_lowercase();
                            // Loaded plugins
                            let loaded = pm.list_plugins().await;
                            for id in loaded {
                                let hay = id.clone();
                                if q.trim().is_empty() || hay.to_lowercase().contains(&q) {
                                    items.push(crate::display::plugin_panel::PluginItem {
                                        name: id.clone(),
                                        version: None,
                                        description: None,
                                        loaded: true,
                                        path: None,
                                    });
                                }
                            }
                            // Discovered plugins on disk
                            if let Ok(paths) = pm.discover_plugins().await {
                                for p in paths {
                                    let fname = p
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let hay = format!("{} {}", fname, p.display());
                                    if q.trim().is_empty() || hay.to_lowercase().contains(&q) {
                                        // Skip if already listed as loaded
                                        if !items.iter().any(|it| it.name == fname && it.loaded) {
                                            items.push(crate::display::plugin_panel::PluginItem {
                                                name: fname,
                                                version: None,
                                                description: None,
                                                loaded: false,
                                                path: Some(p.to_string_lossy().to_string()),
                                            });
                                        }
                                    }
                                }
                            }
                            let _ = proxy.send_event(Event::new(
                                EventType::PluginsSearchResults(items),
                                win,
                            ));
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::PluginsSearchResults(items), Some(window_id)) => {
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    window_context.display.plugins_panel.results = items;
                    window_context.dirty = true;
                    if window_context.display.window.has_frame {
                        window_context.display.window.request_redraw();
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::PluginsLoadFromPath(path), Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(pm) = &components.plugin_manager {
                        let pm = pm.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        let rt = components.runtime.clone();
                        rt.spawn(async move {
                            match pm.load_plugin(std::path::Path::new(&path)).await {
                                Ok(id) => {
                                    let msg = crate::message_bar::Message::new(
                                        format!("Loaded plugin: {}", id),
                                        crate::message_bar::MessageType::Warning,
                                    );
                                    let _ =
                                        proxy.send_event(Event::new(EventType::Message(msg), win));
                                }
                                Err(e) => {
                                    let msg = crate::message_bar::Message::new(
                                        format!("Load failed: {}", e),
                                        crate::message_bar::MessageType::Warning,
                                    );
                                    let _ =
                                        proxy.send_event(Event::new(EventType::Message(msg), win));
                                }
                            }
                            // Refresh list
                            let _ = proxy.send_event(Event::new(
                                EventType::PluginsSearchPerform(String::new()),
                                win,
                            ));
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::PluginsUnloadByName(name), Some(window_id)) => {
                if let Some(components) = &self.components {
                    if let Some(pm) = &components.plugin_manager {
                        let pm = pm.clone();
                        let proxy = self.proxy.clone();
                        let win = *window_id;
                        let rt = components.runtime.clone();
                        rt.spawn(async move {
                            match pm.unload_plugin(&name).await {
                                Ok(()) => {
                                    let msg = crate::message_bar::Message::new(
                                        format!("Unloaded plugin: {}", name),
                                        crate::message_bar::MessageType::Warning,
                                    );
                                    let _ =
                                        proxy.send_event(Event::new(EventType::Message(msg), win));
                                }
                                Err(e) => {
                                    let msg = crate::message_bar::Message::new(
                                        format!("Unload failed: {}", e),
                                        crate::message_bar::MessageType::Warning,
                                    );
                                    let _ =
                                        proxy.send_event(Event::new(EventType::Message(msg), win));
                                }
                            }
                            // Refresh list
                            let _ = proxy.send_event(Event::new(
                                EventType::PluginsSearchPerform(String::new()),
                                win,
                            ));
                        });
                    }
                }
            }
            #[cfg(feature = "never")]
            (EventType::PluginsInstallFromUrl { url }, Some(window_id)) => {
                if let Some(components) = &self.components {
                    // Compute user plugins dir similar to initialize_plugin_manager
                    let plugins_dir = if let Some(data) = dirs::data_dir() {
                        data.join("openagent-terminal").join("plugins")
                    } else {
                        std::path::PathBuf::from("./.openagent-terminal/plugins")
                    };
                    let _ = std::fs::create_dir_all(&plugins_dir);
                    let proxy = self.proxy.clone();
                    let win = *window_id;
                    let rt = components.runtime.clone();
                    // Download and save
                    rt.spawn(async move {
                        let fname = url.split('/').next_back().unwrap_or("plugin.wasm");
                        let target = plugins_dir.join(fname);
                        let client = reqwest::Client::new();
                        let res = client.get(&url).send().await;
                        match res {
                            Ok(resp) => {
                                match resp.bytes().await {
                                    Ok(bytes) => {
                                        if let Err(e) = tokio::fs::write(&target, &bytes).await {
                                            let m = crate::message_bar::Message::new(
                                                format!("Install failed: {}", e),
                                                crate::message_bar::MessageType::Warning,
                                            );
                                            let _ = proxy
                                                .send_event(Event::new(EventType::Message(m), win));
                                        } else {
                                            let m = crate::message_bar::Message::new(
                                                format!(
                                                    "Downloaded plugin to {}",
                                                    target.display()
                                                ),
                                                crate::message_bar::MessageType::Warning,
                                            );
                                            let _ = proxy
                                                .send_event(Event::new(EventType::Message(m), win));
                                            // Trigger load
                                            let _ = proxy.send_event(Event::new(
                                                EventType::PluginsLoadFromPath(
                                                    target.to_string_lossy().to_string(),
                                                ),
                                                win,
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        let m = crate::message_bar::Message::new(
                                            format!("Install failed: {}", e),
                                            crate::message_bar::MessageType::Warning,
                                        );
                                        let _ = proxy
                                            .send_event(Event::new(EventType::Message(m), win));
                                    }
                                }
                            }
                            Err(e) => {
                                let m = crate::message_bar::Message::new(
                                    format!("Download failed: {}", e),
                                    crate::message_bar::MessageType::Warning,
                                );
                                let _ = proxy.send_event(Event::new(EventType::Message(m), win));
                            }
                        }
                    });
                }
            }
            (payload, Some(window_id)) => {
                if let Some(window_context) = self.windows.get_mut(window_id) {
                    window_context.handle_event(
                        #[cfg(target_os = "macos")]
                        event_loop,
                        &self.proxy,
                        &mut self.clipboard,
                        &mut self.scheduler,
                        WinitEvent::UserEvent(Event::new(payload, *window_id)),
                    );
                }
            }
        };
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.config.debug.print_events {
            info!(target: LOG_TARGET_WINIT, "About to wait");
        }

        // Dispatch event to all windows.
        for window_context in self.windows.values_mut() {
            window_context.handle_event(
                #[cfg(target_os = "macos")]
                event_loop,
                &self.proxy,
                &mut self.clipboard,
                &mut self.scheduler,
                WinitEvent::AboutToWait,
            );
        }

        // Update the scheduler after event processing to ensure
        // the event loop deadline is as accurate as possible.
        let control_flow = match self.scheduler.update() {
            Some(instant) => ControlFlow::WaitUntil(instant),
            None => ControlFlow::Wait,
        };
        event_loop.set_control_flow(control_flow);
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if self.config.debug.print_events {
            info!("Exiting the event loop");
        }

        // SAFETY: The clipboard must be dropped before the event loop, so use the nop clipboard
        // as a safe placeholder.
        self.clipboard = Clipboard::new_nop();
    }
}

/// OpenAgent Terminal events.
#[derive(Debug, Clone)]
pub struct Event {
    /// Limit event to a specific window.
    window_id: Option<WindowId>,

    /// Event payload.
    payload: EventType,
}

impl Event {
    pub fn new<I: Into<Option<WindowId>>>(payload: EventType, window_id: I) -> Self {
        Self { window_id: window_id.into(), payload }
    }

    /// Get a reference to the payload (event type)
    pub fn payload(&self) -> &EventType {
        &self.payload
    }
}

impl From<Event> for WinitEvent<Event> {
    fn from(event: Event) -> Self {
        WinitEvent::UserEvent(event)
    }
}

/// AI copy output formats.
#[cfg(feature = "ai")]
#[derive(Debug, Clone)]
pub enum AiCopyFormat {
    Text,
    Code,
    Markdown,
}

/// OpenAgent Terminal events.
#[derive(Debug, Clone)]
pub enum EventType {
    WorkspaceHoverFocusRequest {
        mouse_x_px: f32,
        mouse_y_px: f32,
        container: crate::workspace::split_manager::PaneRect,
    },
    Terminal(TerminalEvent),
    ConfigReload(PathBuf),
    Message(Message),
    Scroll(Scroll),
    CreateWindow(WindowOptions),
    #[cfg(unix)]
    IpcConfig(IpcConfig),
    #[cfg(unix)]
    IpcGetConfig(Arc<UnixStream>),
    #[cfg(all(unix, feature = "sync"))]
    IpcSync(IpcSyncType, Arc<UnixStream>),
    BlinkCursor,
    BlinkCursorTimeout,
    SearchNext,
    Frame,
    #[cfg(feature = "ai")]
    AiStreamChunk(String),
    #[cfg(feature = "ai")]
    AiStreamFinished,
    #[cfg(feature = "ai")]
    AiStreamError(String),
    #[cfg(feature = "ai")]
    AiProposals(Vec<openagent_terminal_ai::AiProposal>),
    #[cfg(feature = "ai")]
    AiRegenerate,
    #[cfg(feature = "ai")]
    AiStop,
    #[cfg(feature = "ai")]
    AiInsertToPrompt(String),
    #[cfg(feature = "ai")]
    AiApplyAsCommand {
        command: String,
        dry_run: bool,
    },
    // Security Lens: check AI apply before pasting to prompt
    #[cfg(feature = "ai")]
    SecurityCheckAiApply {
        command: String,
        dry_run: bool,
    },
    #[cfg(feature = "ai")]
    AiApplyAsCommandChecked {
        command: String,
        dry_run: bool,
    },
    #[cfg(feature = "ai")]
    AiCopyOutput {
        format: AiCopyFormat,
    },
    // New AI panel events
    #[cfg(feature = "ai")]
    AiToggle,
    #[cfg(feature = "ai")]
    AiSubmit,
    #[cfg(feature = "ai")]
    AiClose,
    #[cfg(feature = "ai")]
    AiSelectNext,
    #[cfg(feature = "ai")]
    AiSelectPrev,
    #[cfg(feature = "ai")]
    AiApplyDryRun,
    #[cfg(feature = "ai")]
    AiCopyCode,
    #[cfg(feature = "ai")]
    AiCopyAll,
    #[cfg(feature = "ai")]
    AiSwitchProvider(String),
    // Inline AI suggestions
    #[cfg(feature = "ai")]
    AiInlineDebounced,
    #[cfg(feature = "ai")]
    AiInlineSuggestionReady(String),
    #[cfg(feature = "ai")]
    AiExplain(Option<String>),
    #[cfg(feature = "ai")]
    AiFix(Option<String>),
    ComponentsInitialized(Arc<InitializedComponents>),
    // Blocks quick actions
    BlocksToggleFoldUnderCursor,
    BlocksCopyHeaderUnderCursor,
    BlocksExportHeaderUnderCursor,
    BlocksRerunUnderCursor,

    // Blocks Search panel events
    #[cfg(feature = "never")]
    BlocksSearchPerform(String),
    #[cfg(feature = "never")]
    BlocksSearchResults(Vec<crate::display::blocks_search_panel::BlocksSearchItem>),
    #[cfg(feature = "never")]
    BlocksToggleStar(String),

    // Notebooks panel events (UI + data)
    #[cfg(feature = "never")]
    NotebooksOpen,
    #[cfg(feature = "never")]
    NotebooksList(Vec<NotebookListItem>),
    #[cfg(feature = "never")]
    NotebooksSelect(String), // notebook id
    #[cfg(feature = "never")]
    NotebooksCells(Vec<NotebookCellItem>),
    #[cfg(feature = "never")]
    NotebooksRunCell(String), // cell id
    #[cfg(feature = "never")]
    NotebooksRunNotebook(String), // notebook id
    #[cfg(feature = "never")]
    NotebooksAddCommand(String), // notebook id
    #[cfg(feature = "never")]
    NotebooksAddMarkdown(String), // notebook id
    // New editing and export actions
    NotebooksDeleteCell(String),            // cell id
    NotebooksConvertCellToMarkdown(String), // cell id
    NotebooksConvertCellToCommand(String),  // cell id
    NotebooksExportNotebook(String),        // notebook id
    NotebooksEditApply {
        cell_id: String,
        content: String,
    },

    // Workflows panel events
    #[cfg(feature = "workflow")]
    WorkflowsSearchPerform(String),

    // Plugin palette integration and execution
    #[cfg(feature = "never")]
    PaletteRequestPluginCommands,
    #[cfg(feature = "never")]
    PaletteAppendPluginCommands(Vec<(String, String, Option<String>)>), // (plugin, command, subtitle)
    #[cfg(feature = "never")]
    PluginsRunCommand {
        plugin: String,
        command: String,
    },
    // Plugins panel events
    #[cfg(feature = "never")]
    PluginsSearchPerform(String),
    #[cfg(feature = "never")]
    PluginsSearchResults(Vec<crate::display::plugin_panel::PluginItem>),
    #[cfg(feature = "never")]
    PluginsLoadFromPath(String), // path to .wasm
    #[cfg(feature = "never")]
    PluginsUnloadByName(String), // plugin id/name
    #[cfg(feature = "never")]
    PluginsInstallFromUrl {
        url: String,
    },
    #[cfg(feature = "workflow")]
    WorkflowsSearchResults(Vec<crate::display::workflow_panel::WorkflowItem>),
    #[cfg(feature = "workflow")]
    WorkflowsExecuteByName(String),
    // Parameters form for workflows
    #[cfg(feature = "workflow")]
    WorkflowsOpenParamsForm {
        workflow_id: String,
        workflow_name: String,
        params: Vec<workflow_engine::Parameter>,
    },
    #[cfg(feature = "workflow")]
    WorkflowsSubmitParams {
        workflow_id: String,
        values: std::collections::HashMap<String, serde_json::Value>,
    },
    #[cfg(feature = "workflow")]
    WorkflowsCancelParams,
    #[cfg(feature = "workflow")]
    WorkflowsProgressUpdate {
        execution_id: String,
        workflow_name: Option<String>,
        status: Option<String>,
        current_step: Option<String>,
        log: Option<String>,
        done: bool,
    },
    #[cfg(feature = "workflow")]
    WorkflowsProgressClear(String),

    // Generic paste utility for fallbacks
    PasteCommand(String),

    // Paste command that has already passed Security Lens gating
    PasteCommandChecked(String),

    // Global confirmation overlay events
    ConfirmOpen {
        id: String,
        title: String,
        body: String,
        confirm_label: Option<String>,
        cancel_label: Option<String>,
    },
    ConfirmRespond {
        id: String,
        accepted: bool,
    },
    ConfirmResolved {
        id: String,
        accepted: bool,
    },

    // Warp-style workspace events
    WarpUiUpdate(crate::workspace::WarpUiUpdateType),

    // Command Palette: append files (e.g. async recursive scan results). Each entry is a path.
    PaletteAppendFiles(Vec<String>),
}

/// Sync IPC event types.
#[cfg(all(unix, feature = "sync"))]
#[derive(Debug, Clone)]
pub enum IpcSyncType {
    Status(Option<crate::cli::SyncScopeArg>),
    Push(Option<crate::cli::SyncScopeArg>),
    Pull(Option<crate::cli::SyncScopeArg>),
}

#[cfg(all(unix, feature = "sync"))]
impl IpcSyncType {
    /// Get the scope argument from the sync type.
    pub fn scope(&self) -> Option<crate::cli::SyncScopeArg> {
        match self {
            IpcSyncType::Status(scope) | IpcSyncType::Push(scope) | IpcSyncType::Pull(scope) => {
                *scope
            }
        }
    }
}

impl From<TerminalEvent> for EventType {
    fn from(event: TerminalEvent) -> Self {
        Self::Terminal(event)
    }
}

/// Regex search state.
pub struct SearchState {
    /// Search direction.
    pub direction: Direction,

    /// Current position in the search history.
    pub history_index: Option<usize>,

    /// Change in display offset since the beginning of the search.
    display_offset_delta: i32,

    /// Search origin in viewport coordinates relative to original display offset.
    origin: Point,

    /// Focused match during active search.
    focused_match: Option<Match>,

    /// Search regex and history.
    ///
    /// During an active search, the first element is the user's current input.
    ///
    /// While going through history, the [`SearchState::history_index`] will point to the element
    /// in history which is currently being previewed.
    history: VecDeque<String>,

    /// Compiled search automatons.
    dfas: Option<RegexSearch>,
}

impl SearchState {
    /// Search regex text if a search is active.
    pub fn regex(&self) -> Option<&String> {
        self.history_index.and_then(|index| self.history.get(index))
    }

    /// Direction of the search from the search origin.
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Focused match during vi-less search.
    pub fn focused_match(&self) -> Option<&Match> {
        self.focused_match.as_ref()
    }

    /// Clear the focused match.
    pub fn clear_focused_match(&mut self) {
        self.focused_match = None;
    }

    /// Active search dfas.
    pub fn dfas(&mut self) -> Option<&mut RegexSearch> {
        self.dfas.as_mut()
    }

    /// Search regex text if a search is active.
    fn regex_mut(&mut self) -> Option<&mut String> {
        self.history_index.and_then(move |index| self.history.get_mut(index))
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            direction: Direction::Right,
            display_offset_delta: Default::default(),
            focused_match: Default::default(),
            history_index: Default::default(),
            history: Default::default(),
            origin: Default::default(),
            dfas: Default::default(),
        }
    }
}

/// Vi inline search state.
pub struct InlineSearchState {
    /// Whether inline search is currently waiting for search character input.
    pub char_pending: bool,
    pub character: Option<char>,

    direction: Direction,
    stop_short: bool,
}

impl Default for InlineSearchState {
    fn default() -> Self {
        Self {
            direction: Direction::Right,
            char_pending: Default::default(),
            stop_short: Default::default(),
            character: Default::default(),
        }
    }
}

pub struct ActionContext<'a, N, T> {
    pub notifier: &'a mut N,
    /// Native IDE manager reference
    pub ide: &'a mut crate::ide::IdeManager,
    pub terminal: &'a mut Term<T>,
    pub clipboard: &'a mut Clipboard,
    pub mouse: &'a mut Mouse,
    pub touch: &'a mut TouchPurpose,
    pub modifiers: &'a mut Modifiers,
    pub display: &'a mut Display,
    pub message_buffer: &'a mut MessageBuffer,
    pub config: &'a UiConfig,
    pub cursor_blink_timed_out: &'a mut bool,
    pub prev_bell_cmd: &'a mut Option<Instant>,
    #[cfg(target_os = "macos")]
    pub event_loop: &'a ActiveEventLoop,
    pub event_proxy: &'a EventLoopProxy<Event>,
    pub scheduler: &'a mut Scheduler,
    pub search_state: &'a mut SearchState,
    pub inline_search_state: &'a mut InlineSearchState,
    pub dirty: &'a mut bool,
    pub occluded: &'a mut bool,
    pub preserve_title: bool,
    #[cfg(not(windows))]
    pub master_fd: RawFd,
    #[cfg(not(windows))]
    pub shell_pid: u32,
    #[cfg(feature = "ai")]
    pub ai_runtime: Option<&'a mut crate::ai_runtime::AiRuntime>,
    #[cfg(feature = "never")]
    pub components: Option<&'a std::sync::Arc<InitializedComponents>>,
    pub workspace: &'a mut crate::workspace::WorkspaceManager,
    /// Last started command captured during CommandStart
    pub last_command: &'a mut Option<String>,
}

impl<'a, N: Notify + 'a, T: EventListener> input::ActionContext<T> for ActionContext<'a, N, T> {
    fn completions_active(&self) -> bool {
        self.display.completions_active()
    }
    fn completions_move_selection(&mut self, delta: isize) {
        self.display.completions_move_selection(delta);
        *self.dirty = true;
    }
    fn completions_clear(&mut self) {
        self.display.completions_clear();
        *self.dirty = true;
    }
    fn completions_confirm(&mut self) {
        // Compute prefix up to cursor
        use openagent_terminal_core::index::Column as Col;
        use openagent_terminal_core::term::cell::Flags as CellFlags;
        let cursor_point = self.terminal.grid().cursor.point;
        let row = &self.terminal.grid()[cursor_point.line];
        let mut prefix = String::new();
        for x in 0..cursor_point.column.0 {
            let cell = &row[Col(x)];
            if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                continue;
            }
            let ch = cell.c;
            if ch != '\u{0}' {
                prefix.push(ch);
            }
        }
        let token = if prefix.ends_with(' ') {
            "".to_string()
        } else {
            prefix.split_whitespace().last().unwrap_or("").to_string()
        };
        if let Some(label) = self.display.completions_selected_label() {
            let suffix = if !token.is_empty() && label.starts_with(&token) {
                label[token.len()..].to_string()
            } else if token.is_empty() {
                label
            } else {
                // Fallback: insert a space and the label
                format!(" {}", label)
            };
            if !suffix.is_empty() {
                self.on_terminal_input_start();
                self.write_to_pty(suffix.into_bytes());
            }
            self.display.completions_clear();
            *self.dirty = true;
        }
    }

    fn ide_on_command_end(&mut self, exit_code: Option<i32>) {
        if let Some(code) = exit_code {
            if code != 0 {
                // Produce a suggestion using the native IDE manager with the last started command
                let cmd = self.last_command.as_deref().unwrap_or("");
                let suggestions = self.ide.handle_error(cmd, code, "");
                if let Some(s) = suggestions.first() {
                    let msg = crate::message_bar::Message::new(
                        format!("Command failed (exit {}). Suggestion: {}", code, s.text),
                        crate::message_bar::MessageType::Warning,
                    );
                    let _ = self
                        .event_proxy
                        .send_event(Event::new(EventType::Message(msg), self.display.window.id()));
                }
            }
        }
    }
    fn workspace_hover_focus(&mut self, mouse_x_px: f32, mouse_y_px: f32) {
        // Compute pane rects and focus the pane under the mouse if different
        if !self.config.workspace.focus_follows_mouse {
            return;
        }
        if let Some(active_tab) = self.workspace.active_tab() {
            let si = self.display.size_info;
            // Recompute container similar to input but with current config
            let x0 = si.padding_x();
            let mut y0 = si.padding_y();
            let w = si.width() - 2.0 * si.padding_x();
            let mut h = si.height() - 2.0 * si.padding_y();
            let tab_cfg = &self.config.workspace.tab_bar;
            let is_fs = self.display.window.is_fullscreen();
            let eff_vis = match tab_cfg.visibility {
                crate::config::workspace::TabBarVisibility::Always => {
                    crate::config::workspace::TabBarVisibility::Always
                }
                crate::config::workspace::TabBarVisibility::Hover => {
                    crate::config::workspace::TabBarVisibility::Hover
                }
                crate::config::workspace::TabBarVisibility::Auto => {
                    if is_fs {
                        crate::config::workspace::TabBarVisibility::Hover
                    } else {
                        crate::config::workspace::TabBarVisibility::Always
                    }
                }
            };
            if tab_cfg.show
                && tab_cfg.position != crate::workspace::TabBarPosition::Hidden
                && matches!(eff_vis, crate::config::workspace::TabBarVisibility::Always)
            {
                let ch = si.cell_height();
                match tab_cfg.position {
                    crate::workspace::TabBarPosition::Top => {
                        y0 += ch;
                        h = (h - ch).max(0.0);
                    }
                    crate::workspace::TabBarPosition::Bottom => {
                        h = (h - ch).max(0.0);
                    }
                    _ => {}
                }
            }
            let container = crate::workspace::split_manager::PaneRect::new(x0, y0, w, h);
            let rects =
                self.workspace.splits.calculate_pane_rects(&active_tab.split_layout, container);
            let current = active_tab.active_pane;
            // Find pane whose rect contains (mouse_x_px, mouse_y_px)
            let hit = rects
                .iter()
                .find(|(_, r)| {
                    mouse_x_px >= r.x
                        && mouse_x_px <= r.x + r.width
                        && mouse_y_px >= r.y
                        && mouse_y_px <= r.y + r.height
                })
                .map(|(id, _)| *id);
            if let Some(target) = hit {
                if target != current {
                    if let Some(tab_mut) = self.workspace.active_tab_mut() {
                        tab_mut.active_pane = target;
                        self.display.pending_update.dirty = true;
                        *self.dirty = true;
                    }
                }
            }
        }
    }
    #[inline]
    fn write_to_pty<B: Into<Cow<'static, [u8]>>>(&self, val: B) {
        if let Err(e) = self.notifier.try_notify(val) {
            tracing::warn!("notify to PTY failed: {:?}", e);
        }
    }

    /// Request a redraw.
    #[inline]
    fn mark_dirty(&mut self) {
        *self.dirty = true;
    }

    #[inline]
    fn size_info(&self) -> SizeInfo {
        self.display.size_info
    }

    fn scroll(&mut self, scroll: Scroll) {
        let old_offset = self.terminal.grid().display_offset() as i32;

        let old_vi_cursor = self.terminal.vi_mode_cursor;
        self.terminal.scroll_display(scroll);

        let lines_changed = old_offset - self.terminal.grid().display_offset() as i32;

        // Keep track of manual display offset changes during search.
        if self.search_active() {
            self.search_state.display_offset_delta += lines_changed;
        }

        let vi_mode = self.terminal.mode().contains(TermMode::VI);

        // Update selection.
        if vi_mode && self.terminal.selection.as_ref().is_some_and(|s| !s.is_empty()) {
            self.update_selection(self.terminal.vi_mode_cursor.point, Side::Right);
        } else if self.mouse.left_button_state == ElementState::Pressed
            || self.mouse.right_button_state == ElementState::Pressed
        {
            let display_offset = self.terminal.grid().display_offset();
            let point = self.mouse.point(&self.size_info(), display_offset);
            self.update_selection(point, self.mouse.cell_side);
        }

        // Scrolling inside Vi mode moves the cursor, so start typing.
        if vi_mode {
            self.on_typing_start();
        }

        // Update dirty if actually scrolled or moved Vi cursor in Vi mode.
        *self.dirty |=
            lines_changed != 0 || (vi_mode && old_vi_cursor != self.terminal.vi_mode_cursor);
    }

    // Copy text selection.
    fn copy_selection(&mut self, ty: ClipboardType) {
        let text = match self.terminal.selection_to_string().filter(|s| !s.is_empty()) {
            Some(text) => text,
            None => return,
        };

        if ty == ClipboardType::Selection && self.config.selection.save_to_clipboard {
            self.clipboard.store(ClipboardType::Clipboard, text.clone());
        }
        self.clipboard.store(ty, text);
    }

    fn selection_is_empty(&self) -> bool {
        self.terminal.selection.as_ref().map_or(true, Selection::is_empty)
    }

    fn clear_selection(&mut self) {
        // Clear the selection on the terminal.
        let selection = self.terminal.selection.take();
        // Mark the terminal as dirty when selection wasn't empty.
        *self.dirty |= selection.is_some_and(|s| !s.is_empty());
    }

    fn update_selection(&mut self, mut point: Point, side: Side) {
        let mut selection = match self.terminal.selection.take() {
            Some(selection) => selection,
            None => return,
        };

        // Treat motion over message bar like motion over the last line.
        point.line = min(point.line, self.terminal.bottommost_line());

        // Update selection.
        selection.update(point, side);

        // Move vi cursor and expand selection.
        if self.terminal.mode().contains(TermMode::VI) && !self.search_active() {
            self.terminal.vi_mode_cursor.point = point;
            selection.include_all();
        }

        // Auto-unfold if selection point entered a folded region.
        if self.display.blocks.enabled {
            let display_offset = self.terminal.grid().display_offset();
            if let Some(view) =
                openagent_terminal_core::term::point_to_viewport(display_offset, point)
            {
                let total_line = display_offset + view.line;
                let changed = self.display.blocks.ensure_unfold_at_total_line(total_line);
                if changed {
                    self.display.damage_tracker.frame().mark_fully_damaged();
                    *self.dirty = true;
                }
            }
        }

        self.terminal.selection = Some(selection);
        *self.dirty = true;
    }

    fn start_selection(&mut self, ty: SelectionType, point: Point, side: Side) {
        self.terminal.selection = Some(Selection::new(ty, point, side));
        *self.dirty = true;

        self.copy_selection(ClipboardType::Selection);
    }

    fn toggle_selection(&mut self, ty: SelectionType, point: Point, side: Side) {
        match &mut self.terminal.selection {
            Some(selection) if selection.ty == ty && !selection.is_empty() => {
                self.clear_selection();
            }
            Some(selection) if !selection.is_empty() => {
                selection.ty = ty;
                *self.dirty = true;

                self.copy_selection(ClipboardType::Selection);
            }
            _ => self.start_selection(ty, point, side),
        }
    }

    #[inline]
    fn mouse_mode(&self) -> bool {
        self.terminal.mode().intersects(TermMode::MOUSE_MODE)
            && !self.terminal.mode().contains(TermMode::VI)
    }

    #[inline]
    fn mouse_mut(&mut self) -> &mut Mouse {
        self.mouse
    }

    #[inline]
    fn mouse(&self) -> &Mouse {
        self.mouse
    }

    #[inline]
    fn touch_purpose(&mut self) -> &mut TouchPurpose {
        self.touch
    }

    #[inline]
    fn modifiers(&mut self) -> &mut Modifiers {
        self.modifiers
    }

    #[inline]
    fn window(&mut self) -> &mut Window {
        &mut self.display.window
    }

    #[inline]
    fn display(&mut self) -> &mut Display {
        self.display
    }

    // Command Palette controls
    fn open_command_palette(&mut self) {
        // Build pooled palette items (Actions + Workflows + Files)
        let mut items: Vec<PaletteItem> = Vec::new();
        // Start open animation for palette
        self.display.palette_anim_opening = true;
        self.display.palette_anim_start = Some(std::time::Instant::now());
        // Use theme reduce_motion to adjust duration
        let theme = self
            .config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.config.theme.resolve());
        self.display.palette_anim_duration_ms = if theme.ui.reduce_motion { 0 } else { 140 };

        // Core actions: tabs, splits, focus, zoom
        items.push(PaletteItem {
            key: "action:CreateTab".to_string(),
            title: "New Tab".to_string(),
            subtitle: Some("Create a new tab".to_string()),
            entry: PaletteEntry::Action(BindingAction::CreateTab),
        });
        items.push(PaletteItem {
            key: "action:SplitVertical".to_string(),
            title: "Split: Vertical".to_string(),
            subtitle: Some("Split current pane vertically".to_string()),
            entry: PaletteEntry::Action(BindingAction::SplitVertical),
        });
        items.push(PaletteItem {
            key: "action:SplitHorizontal".to_string(),
            title: "Split: Horizontal".to_string(),
            subtitle: Some("Split current pane horizontally".to_string()),
            entry: PaletteEntry::Action(BindingAction::SplitHorizontal),
        });
        items.push(PaletteItem {
            key: "action:FocusNextPane".to_string(),
            title: "Focus Next Pane".to_string(),
            subtitle: Some("Move focus to next pane".to_string()),
            entry: PaletteEntry::Action(BindingAction::FocusNextPane),
        });
        items.push(PaletteItem {
            key: "action:FocusPreviousPane".to_string(),
            title: "Focus Previous Pane".to_string(),
            subtitle: Some("Move focus to previous pane".to_string()),
            entry: PaletteEntry::Action(BindingAction::FocusPreviousPane),
        });
        items.push(PaletteItem {
            key: "action:ToggleZoom".to_string(),
            title: "Toggle Zoom".to_string(),
            subtitle: Some("Toggle zoom on active pane".to_string()),
            entry: PaletteEntry::Action(BindingAction::ToggleZoom),
        });

        // Panels
        items.push(PaletteItem {
            key: "action:OpenBlocksSearchPanel".to_string(),
            title: "Open Blocks Search".to_string(),
            subtitle: Some("Search recent command blocks".to_string()),
            entry: PaletteEntry::Action(BindingAction::OpenBlocksSearchPanel),
        });
        items.push(PaletteItem {
            key: "action:OpenWorkflowsPanel".to_string(),
            title: "Open Workflows Panel".to_string(),
            subtitle: Some("Browse and run configured workflows".to_string()),
            entry: PaletteEntry::Action(BindingAction::OpenWorkflowsPanel),
        });
        // Sync toggle
        items.push(PaletteItem {
            key: "action:TogglePaneSync".to_string(),
            title: "Toggle Pane Sync".to_string(),
            subtitle: Some("Synchronize input across panes in this tab".to_string()),
            entry: PaletteEntry::Action(BindingAction::TogglePaneSync),
        });

        // Workflows from config (if any)
        if !self.config.workflows.is_empty() {
            for wf in &self.config.workflows {
                let title = format!("Workflow: {}", wf.name);
                let subtitle = wf.description.clone();
                items.push(PaletteItem {
                    key: format!("workflow:{}", wf.name),
                    title,
                    subtitle,
                    entry: PaletteEntry::Workflow(wf.name.clone()),
                });
            }
        }

        // Recent commands from recent blocks (top 30, most recent first)
        {
            let blocks_ref = &self.display.blocks.blocks;
            let mut added = 0usize;
            for block in blocks_ref.iter().rev() {
                if let Some(cmd) = &block.cmd {
                    let exit = block.exit;
                    let cwd = block.cwd.clone();
                    let status = match exit {
                        Some(0) => "✓",
                        Some(_) => "✗",
                        None => "…",
                    };
                    let title = format!("Command: {}", cmd);
                    let mut subtitle = String::new();
                    subtitle.push_str(status);
                    if let Some(code) = exit {
                        subtitle.push_str(&format!(" exit {}", code));
                    }
                    if let Some(c) = &cwd {
                        subtitle.push_str(&format!("  —  {}", c));
                    }
                    items.push(PaletteItem {
                        key: format!("cmd:{}:{}", block.start_total_line, cmd),
                        title,
                        subtitle: if subtitle.is_empty() { None } else { Some(subtitle) },
                        entry: PaletteEntry::Command { cmd: cmd.clone(), cwd, exit },
                    });
                    added += 1;
                    if added >= 30 {
                        break;
                    }
                }
            }
        }

        // Recent files from MRU (top 20)
        if let Ok(cwd) = std::env::current_dir() {
            let recent = self.display.palette.recent_file_paths(20);
            for p in recent {
                let path = std::path::PathBuf::from(&p);
                if !path.exists() {
                    continue;
                }
                let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or(&p).to_string();
                let subtitle =
                    path.strip_prefix(&cwd).ok().map(|rel| rel.to_string_lossy().to_string());
                items.push(PaletteItem {
                    key: format!("file:{}", p),
                    title: format!("File: {}", file_name),
                    subtitle,
                    entry: PaletteEntry::File(p),
                });
            }
        }

        // Files from current working directory (basic, non-recursive, capped) for immediate
        // feedback
        if let Ok(cwd) = std::env::current_dir() {
            if let Ok(rd) = std::fs::read_dir(&cwd) {
                for (i, entry) in rd.flatten().enumerate() {
                    if i >= 100 {
                        break;
                    }
                    let path = entry.path();
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    let subtitle =
                        path.strip_prefix(&cwd).ok().map(|rel| rel.to_string_lossy().to_string());
                    items.push(PaletteItem {
                        key: format!("file:{}", path.to_string_lossy()),
                        title: format!("File: {}", file_name),
                        subtitle,
                        entry: PaletteEntry::File(path.to_string_lossy().to_string()),
                    });
                }
            }
        }

        // Open palette with assembled items
        self.display.palette.open(items);

        // Ask to append plugin commands if plugin system is enabled
        #[cfg(feature = "never")]
        {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::PaletteRequestPluginCommands,
                self.display.window.id(),
            ));
        }
        self.mark_dirty();
        if self.display.window.has_frame {
            self.display.window.request_redraw();
        }

        // Spawn background recursive scan of project files (repo root if available), append results once ready
        let proxy = self.event_proxy.clone();
        let window_id = self.display.window.id();
        let cwd = std::env::current_dir().ok();
        std::thread::spawn(move || {
            use std::collections::{HashSet, VecDeque};
            use std::fs;
            use std::path::{Path, PathBuf};

            fn find_repo_root(mut dir: PathBuf) -> PathBuf {
                let mut seen: usize = 0;
                while seen < 20 {
                    if dir.join(".git").is_dir() {
                        return dir;
                    }
                    if let Some(parent) = dir.parent() {
                        dir = parent.to_path_buf();
                        seen += 1;
                    } else {
                        break;
                    }
                }
                dir
            }

            fn read_gitignore(root: &Path) -> (HashSet<String>, Vec<String>) {
                let mut dirs = HashSet::new();
                let mut globs = Vec::new();
                let path = root.join(".gitignore");
                if let Ok(text) = fs::read_to_string(&path) {
                    for line in text.lines() {
                        let s = line.trim();
                        if s.is_empty() || s.starts_with('#') {
                            continue;
                        }
                        // Normalize simple directory entries and wildcard globs against leaf names
                        let entry = s.trim_start_matches("./").trim_start_matches('/');
                        if entry.ends_with('/') {
                            dirs.insert(entry.trim_end_matches('/').to_string());
                        } else if entry.contains('*') || entry.contains('?') {
                            globs.push(entry.to_string());
                        } else {
                            // bare name: ignore both files and dirs by leaf
                            dirs.insert(entry.to_string());
                        }
                    }
                }
                (dirs, globs)
            }

            fn name_matches_globs(name: &str, globs: &[String]) -> bool {
                // Very small wildcard matcher: supports '*' matching any chars and '?' matching one char
                fn matches(pat: &str, text: &str) -> bool {
                    let (mut pi, mut ti) = (0usize, 0usize);
                    let (mut star, mut mark) = (None, 0usize);
                    let p = pat.as_bytes();
                    let t = text.as_bytes();
                    while ti < t.len() {
                        if pi < p.len() && (p[pi] == b'?' || p[pi] == t[ti]) {
                            pi += 1;
                            ti += 1;
                            continue;
                        }
                        if pi < p.len() && p[pi] == b'*' {
                            star = Some(pi);
                            pi += 1;
                            mark = ti;
                            continue;
                        }
                        if let Some(si) = star {
                            pi = si + 1;
                            mark += 1;
                            ti = mark;
                            continue;
                        }
                        return false;
                    }
                    while pi < p.len() && p[pi] == b'*' {
                        pi += 1;
                    }
                    pi == p.len()
                }
                globs.iter().any(|g| matches(g, name))
            }

            let Some(cwd) = cwd else {
                return;
            };
            let root = find_repo_root(cwd);

            let default_ignore = [
                ".git",
                "target",
                "node_modules",
                "dist",
                "build",
                ".venv",
                "venv",
                ".cache",
                ".tox",
                ".mypy_cache",
                ".idea",
                ".vscode",
            ];
            let default_set: HashSet<&str> = default_ignore.iter().copied().collect();
            let (git_dirs, git_globs) = read_gitignore(&root);

            let mut out: Vec<String> = Vec::new();
            let max_files: usize = 5000;
            let mut queue: VecDeque<PathBuf> = VecDeque::new();
            queue.push_back(root.clone());

            while let Some(dir) = queue.pop_front() {
                let rd = match fs::read_dir(&dir) {
                    Ok(rd) => rd,
                    Err(_) => continue,
                };
                for entry in rd.flatten() {
                    if out.len() >= max_files {
                        break;
                    }
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    let ft = match entry.file_type() {
                        Ok(ft) => ft,
                        Err(_) => continue,
                    };
                    // Skip symlinks
                    #[cfg(unix)]
                    if ft.is_symlink() {
                        continue;
                    }
                    if ft.is_dir() {
                        // Skip default ignored dirs or .gitignore-listed dirs
                        if default_set.contains(name.as_str())
                            || git_dirs.contains(&name)
                            || name_matches_globs(&name, &git_globs)
                        {
                            continue;
                        }
                        // Skip hidden dot-directories except repo root
                        if name.starts_with('.') && !dir.as_path().eq(&root) {
                            continue;
                        }
                        queue.push_back(path);
                    } else if ft.is_file() {
                        // Skip ignored files by leaf name
                        if git_dirs.contains(&name) || name_matches_globs(&name, &git_globs) {
                            continue;
                        }
                        out.push(path.to_string_lossy().to_string());
                    }
                }
                if out.len() >= max_files {
                    break;
                }
            }

            if !out.is_empty() {
                let _ = proxy.send_event(Event::new(EventType::PaletteAppendFiles(out), window_id));
            }
        });
    }

    fn palette_active(&self) -> bool {
        self.display.palette.active()
    }

    fn palette_input(&mut self, c: char) {
        self.display.palette.push_filter_char(c);
        self.mark_dirty();
    }

    fn palette_backspace(&mut self) {
        self.display.palette.pop_filter_char();
        self.mark_dirty();
    }

    fn palette_move_selection(&mut self, delta: isize) {
        self.display.palette.move_selection(delta);
        self.mark_dirty();
    }

    fn palette_confirm_edit(&mut self) {
        // For commands: insert into composer for editing instead of running
        if let Some(PaletteEntry::Command { cmd, .. }) =
            self.display.palette.selected_entry().cloned()
        {
            self.display.composer_text = cmd;
            self.display.composer_cursor = self.display.composer_text.len();
            self.display.composer_sel_anchor = None;
            self.display.composer_focused = true;
            // Close palette
            self.display.palette.close();
            *self.dirty = true;
            if self.display.window.has_frame {
                self.display.window.request_redraw();
            }
        }
    }

    fn palette_confirm(&mut self) {
        use BindingAction as BA;
        // Note usage for MRU boost
        if let Some(key) = self.display.palette.selected_item_key().map(|s| s.to_string()) {
            self.display.palette.note_used(&key);
        }
        if let Some(entry) = self.display.palette.selected_entry().cloned() {
            match entry {
                PaletteEntry::Action(action) => {
                    match action {
                        BA::CreateTab => self.workspace_create_tab(),
                        BA::SplitVertical => self.workspace_split_vertical(),
                        BA::SplitHorizontal => self.workspace_split_horizontal(),
                        BA::FocusNextPane => self.workspace_focus_next_pane(),
                        BA::FocusPreviousPane => self.workspace_focus_previous_pane(),
                        BA::ToggleZoom => self.workspace_toggle_zoom(),
                        BA::OpenBlocksSearchPanel => {
                            if self.blocks_search_active() {
                                self.blocks_search_cancel()
                            } else {
                                self.open_blocks_search_panel()
                            }
                        }
                        BA::OpenWorkflowsPanel => {
                            if self.workflows_panel_active() {
                                self.workflows_panel_cancel()
                            } else {
                                self.open_workflows_panel()
                            }
                        }
                        BA::TogglePaneSync => {
                            self.workspace_toggle_sync();
                        }
                        _ => {
                            // Unsupported action here; fall back to sending a message
                        }
                    }
                }
                PaletteEntry::Workflow(name) => {
                    // Ask processor to execute via engine if available
                    #[cfg(feature = "workflow")]
                    {
                        let _ = self.event_proxy.send_event(Event::new(
                            EventType::WorkflowsExecuteByName(name),
                            self.display.window.id(),
                        ));
                    }
                    #[cfg(not(feature = "workflow"))]
                    {
                        let _ = name; // Use the variable to avoid warning
                        // Workflow feature not enabled; ignore selection or surface a small message
                    }
                }
                PaletteEntry::File(_path) => {
                    // Default action: open the File Tree overlay
                    self.open_file_tree_panel();
                }
                PaletteEntry::Command { cmd, cwd, .. } => {
                    if let Some(cwd) = cwd {
                        self.spawn_shell_command_in_cwd(cmd, cwd);
                    } else {
                        self.paste(&cmd, true);
                    }
                }
                #[cfg(feature = "never")]
                PaletteEntry::PluginCommand { plugin, command } => {
                    #[cfg(feature = "never")]
                    {
                        // Security Lens gating is enforced inside the plugin host when plugins execute external commands.
                        let _ = self.event_proxy.send_event(Event::new(
                            EventType::PluginsRunCommand { plugin, command },
                            self.display.window.id(),
                        ));
                    }
                    #[cfg(not(feature = "plugins"))]
                    {
                        let _ = (plugin, command);
                    }
                }
            }
        }
        // Start close animation before hiding
        self.display.palette.close();
        // Persist MRU on confirm
        self.display.palette.save_mru_to_config(self.config);
        self.display.palette_anim_opening = false;
        self.display.palette_anim_start = Some(std::time::Instant::now());
        let theme = self
            .config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.config.theme.resolve());
        self.display.palette_anim_duration_ms = if theme.ui.reduce_motion { 0 } else { 120 };
        self.mark_dirty();
    }

    fn palette_confirm_cd(&mut self) {
        // Alt+Enter: for file entries, paste "cd <dir>" into the prompt; otherwise fallback
        if let Some(PaletteEntry::File(path)) = self.display.palette.selected_entry().cloned() {
            // Mark usage
            if let Some(key) = self.display.palette.selected_item_key().map(|s| s.to_string()) {
                self.display.palette.note_used(&key);
            }
            let p = std::path::PathBuf::from(&path);
            let target_dir = if p.is_dir() {
                p
            } else {
                p.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from("."))
            };
            // Inline shell quote for POSIX
            let target_str = target_dir.to_string_lossy();
            let quoted = if target_str.is_empty() {
                "''".to_string()
            } else if target_str
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'/' | b'.' | b'-' | b'_' | b':'))
            {
                target_str.to_string()
            } else {
                format!("'{}'", target_str.replace("'", "'\\''"))
            };
            let cmd = format!("cd {}", quoted);
            self.paste(&cmd, true);
            // Close + persist MRU
            self.display.palette.close();
            self.display.palette.save_mru_to_config(self.config);
            self.display.palette_anim_opening = false;
            self.display.palette_anim_start = Some(std::time::Instant::now());
            let theme = self
                .config
                .resolved_theme
                .as_ref()
                .cloned()
                .unwrap_or_else(|| self.config.theme.resolve());
            self.display.palette_anim_duration_ms = if theme.ui.reduce_motion { 0 } else { 120 };
            self.mark_dirty();
            return;
        }
        // Fallback
        self.palette_confirm();
    }

    // Minimal single-quote shell escaping suitable for POSIX shells
    // Example: path O'Reilly -> 'O'\''Reilly'
    // For safety we also handle backslashes conservatively by leaving them as-is inside quotes
    // If locale contains Windows paths this is still harmless on Linux
    fn palette_cancel(&mut self) {
        // Persist MRU on cancel as well
        self.display.palette.save_mru_to_config(self.config);
        self.display.palette.close();
        // Trigger closing animation
        self.display.palette_anim_opening = false;
        self.display.palette_anim_start = Some(std::time::Instant::now());
        let theme = self
            .config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.config.theme.resolve());
        self.display.palette_anim_duration_ms = if theme.ui.reduce_motion { 0 } else { 120 };
        self.mark_dirty();
    }

    // Blocks Search panel controls
    #[cfg(feature = "never")]
    fn open_blocks_search_panel(&mut self) {
        if self.palette_active() {
            self.display.palette.save_mru_to_config(self.config);
            self.display.palette.close();
        }
        self.display.blocks_search.open();
        self.mark_dirty();
        self.send_user_event(EventType::BlocksSearchPerform(
            self.display.blocks_search.query.clone(),
        ));
    }

    #[cfg(feature = "never")]
    fn close_blocks_search_panel(&mut self) {
        self.display.blocks_search.close();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_active(&self) -> bool {
        self.display.blocks_search.active
    }

    #[cfg(feature = "never")]
    fn blocks_search_input(&mut self, c: char) {
        self.display.blocks_search.query.push(c);
        self.display.blocks_search.selected = 0;
        self.mark_dirty();
        // Debounce search
        let window_id = self.display.window.id();
        let timer_id = TimerId::new(Topic::BlocksSearchTyping, window_id);
        self.scheduler.unschedule(timer_id);
        let evt = Event::new(
            EventType::BlocksSearchPerform(self.display.blocks_search.query.clone()),
            window_id,
        );
        self.scheduler.schedule(evt, BLOCKS_SEARCH_DEBOUNCE, false, timer_id);
    }

    #[cfg(feature = "never")]
    fn blocks_search_backspace(&mut self) {
        self.display.blocks_search.query.pop();
        self.display.blocks_search.selected = 0;
        self.mark_dirty();
        // Debounce search
        let window_id = self.display.window.id();
        let timer_id = TimerId::new(Topic::BlocksSearchTyping, window_id);
        self.scheduler.unschedule(timer_id);
        let evt = Event::new(
            EventType::BlocksSearchPerform(self.display.blocks_search.query.clone()),
            window_id,
        );
        self.scheduler.schedule(evt, BLOCKS_SEARCH_DEBOUNCE, false, timer_id);
    }

    #[cfg(feature = "never")]
    fn blocks_search_move_selection(&mut self, delta: isize) {
        self.display.blocks_search.move_selection(delta);
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_confirm(&mut self) {
        if !self.display.blocks_search.results.is_empty() {
            let idx = self
                .display
                .blocks_search
                .selected
                .min(self.display.blocks_search.results.len() - 1);
            let cmd = self.display.blocks_search.results[idx].command.clone();
            self.paste(&cmd, true);
        }
        self.display.blocks_search.close();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_cancel(&mut self) {
        self.display.blocks_search.close();
        self.mark_dirty();
    }

    // Enhanced blocks search functionality
    #[cfg(feature = "never")]
    fn blocks_search_cycle_mode(&mut self) {
        self.display.blocks_search.cycle_search_mode();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_cycle_sort_field(&mut self) {
        self.display.blocks_search.cycle_sort_field();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_toggle_sort_order(&mut self) {
        self.display.blocks_search.toggle_sort_order();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_toggle_starred(&mut self) {
        self.display.blocks_search.toggle_starred_filter();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_clear_filters(&mut self) {
        self.display.blocks_search.clear_all_filters();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_next_page(&mut self) {
        self.display.blocks_search.next_page();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_prev_page(&mut self) {
        self.display.blocks_search.prev_page();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_toggle_star_selected(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            let block_id = item.id.clone();
            // Send event to toggle star status in storage
            self.send_user_event(EventType::BlocksToggleStar(block_id));
        }
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_show_actions(&mut self) {
        self.display.blocks_search.open_actions_menu();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_delete_selected(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            let block_id = item.id.clone();
            let title = format!("Delete block {}?", &block_id);
            let message = "This action cannot be undone.";
            self.send_user_event(EventType::ConfirmOpen {
                id: format!("delete_block_{}", block_id),
                title,
                body: message.to_string(),
                confirm_label: Some("Delete".to_string()),
                cancel_label: Some("Cancel".to_string()),
            });
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_copy_command(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            self.clipboard.store(ClipboardType::Clipboard, item.command.clone());
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_copy_output(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            if !item.output.is_empty() {
                self.clipboard.store(ClipboardType::Clipboard, item.output.clone());
            }
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_rerun_selected(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            let command = item.command.clone();
            self.paste(&command, true);
            // Also close the search panel
            self.display.blocks_search.close();
            self.mark_dirty();
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_insert_heredoc(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            if !item.output.is_empty() {
                let heredoc = crate::display::blocks_search_actions::generate_heredoc(&item.output);
                self.paste(&heredoc, true);
                self.display.blocks_search.close();
                self.mark_dirty();
            }
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_show_help(&mut self) {
        self.display.blocks_search.open_help();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_export_selected(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            let content = format!(
                "Command: {}\n\nOutput:\n{}",
                item.command,
                if item.output.is_empty() { "<no output>" } else { &item.output }
            );
            self.prompt_and_export_block_output(content);
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_toggle_tag(&mut self) {
        // Placeholder for tag functionality - would need tag management system
        // For now, just mark dirty to acknowledge the input
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_copy_both(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            let both = format!(
                "$ {}\n{}",
                item.command,
                if item.output.is_empty() { "<no output>" } else { &item.output }
            );
            self.clipboard.store(ClipboardType::Clipboard, both);
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_insert_command(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            let command = item.command.clone();
            self.paste(&command, false); // Don't close search panel
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_view_output(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            if !item.output.is_empty() {
                // Create a temporary viewer for the output
                // For now, copy to clipboard as fallback
                self.clipboard.store(ClipboardType::Clipboard, item.output.clone());
            }
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_share_block(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            let share_content = format!(
                "Command: {}\nOutput: {}",
                item.command,
                if item.output.is_empty() { "<no output>" } else { &item.output }
            );
            // For now, copy to clipboard as sharing mechanism
            self.clipboard.store(ClipboardType::Clipboard, share_content);
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_create_snippet(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            // Create snippet from command - would integrate with snippet system
            // For now, copy command to clipboard
            self.clipboard.store(ClipboardType::Clipboard, item.command.clone());
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_insert_heredoc_custom(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            if !item.output.is_empty() {
                // Use grep as default custom command (can be edited by user)
                let heredoc = crate::display::blocks_search_actions::generate_heredoc_with_command(
                    &item.output,
                    "grep ''",
                );
                self.paste(&heredoc, false); // Don't close panel to allow editing
                self.mark_dirty();
            }
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_insert_json_heredoc(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            if !item.output.is_empty() {
                let heredoc =
                    crate::display::blocks_search_actions::format_as_json_heredoc(&item.output);
                self.paste(&heredoc, true);
                self.display.blocks_search.close();
                self.mark_dirty();
            }
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_insert_shell_heredoc(&mut self) {
        if let Some(item) = self.display.blocks_search.get_selected_item() {
            if !item.output.is_empty() {
                let heredoc = crate::display::blocks_search_actions::format_heredoc_for_shell(
                    &item.output,
                    &item.shell,
                );
                self.paste(&heredoc, true);
                self.display.blocks_search.close();
                self.mark_dirty();
            }
        }
    }

    // Actions menu support
    #[cfg(feature = "never")]
    fn blocks_search_actions_menu_active(&self) -> bool {
        self.display.blocks_search.actions_menu_active()
    }

    #[cfg(feature = "never")]
    fn blocks_search_execute_action(&mut self) {
        if let Some(action) = self.display.blocks_search.get_selected_action() {
            use crate::display::blocks_search_actions::BlockAction;

            match action {
                #[cfg(feature = "ai")]
                BlockAction::ExplainError => {
                    if let Some(item) = self.display.blocks_search.get_selected_item() {
                        let text = if item.output.is_empty() {
                            item.command.clone()
                        } else {
                            item.output.clone()
                        };
                        self.send_user_event(EventType::AiExplain(Some(text)));
                    }
                }
                #[cfg(feature = "ai")]
                BlockAction::FixError => {
                    if let Some(item) = self.display.blocks_search.get_selected_item() {
                        let error_text = if item.output.is_empty() {
                            item.command.clone()
                        } else {
                            item.output.clone()
                        };
                        self.send_user_event(EventType::AiFix(Some(error_text)));
                    }
                }
                BlockAction::CopyCommand => self.blocks_search_copy_command(),
                BlockAction::CopyOutput => self.blocks_search_copy_output(),
                BlockAction::CopyBoth => self.blocks_search_copy_both(),
                BlockAction::InsertCommand => self.blocks_search_insert_command(),
                BlockAction::InsertAsHereDoc => self.blocks_search_insert_heredoc(),
                BlockAction::InsertAsHereDocCustom => self.blocks_search_insert_heredoc_custom(),
                BlockAction::InsertAsJsonHereDoc => self.blocks_search_insert_json_heredoc(),
                BlockAction::InsertAsShellHereDoc => self.blocks_search_insert_shell_heredoc(),
                BlockAction::RerunCommand => self.blocks_search_rerun_selected(),
                BlockAction::ToggleStar => self.blocks_search_toggle_star_selected(),
                BlockAction::EditTags => self.blocks_search_toggle_tag(),
                BlockAction::ExportBlock => self.blocks_search_export_selected(),
                BlockAction::ShareBlock => self.blocks_search_share_block(),
                BlockAction::DeleteBlock => self.blocks_search_delete_selected(),
                BlockAction::ViewFullOutput => self.blocks_search_view_output(),
                BlockAction::CreateSnippet => self.blocks_search_create_snippet(),
                // When AI feature is disabled at compile time, cover variants to satisfy match
                #[cfg(not(feature = "ai"))]
                _ => {}
            }
            // Close menu after action
            self.display.blocks_search.close_actions_menu();
        }
    }

    #[cfg(feature = "never")]
    fn blocks_search_close_actions_menu(&mut self) {
        self.display.blocks_search.close_actions_menu();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_move_actions_selection(&mut self, delta: isize) {
        self.display.blocks_search.move_actions_selection(delta);
        self.mark_dirty();
    }

    // Help overlay support
    #[cfg(feature = "never")]
    fn blocks_search_help_active(&self) -> bool {
        self.display.blocks_search.help_active()
    }

    #[cfg(feature = "never")]
    fn blocks_search_close_help(&mut self) {
        self.display.blocks_search.close_help();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn blocks_search_navigate_help(&mut self, forward: bool) {
        self.display.blocks_search.navigate_help(forward);
        self.mark_dirty();
    }

    // Workflows panel controls
    #[cfg(feature = "workflow")]
    fn open_workflows_panel(&mut self) {
        if self.palette_active() {
            self.display.palette.save_mru_to_config(self.config);
            self.display.palette.close();
        }
        self.display.workflows_panel.open();
        self.mark_dirty();
        // Trigger initial search with current query (may be empty)
        let q = self.display.workflows_panel.query.clone();
        let _ = self
            .event_proxy
            .send_event(Event::new(EventType::WorkflowsSearchPerform(q), self.display.window.id()));
    }

    #[cfg(feature = "never")]
    fn open_plugins_panel(&mut self) {
        if self.palette_active() {
            self.display.palette.save_mru_to_config(self.config);
            self.display.palette.close();
        }
        self.display.plugins_panel.open();
        self.mark_dirty();
        // Trigger initial plugins search
        let q = self.display.plugins_panel.query.clone();
        let _ = self
            .event_proxy
            .send_event(Event::new(EventType::PluginsSearchPerform(q), self.display.window.id()));
    }

    #[cfg(feature = "workflow")]
    fn workflows_panel_cancel(&mut self) {
        self.display.workflows_panel.close();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn plugins_panel_cancel(&mut self) {
        self.display.plugins_panel.close();
        self.mark_dirty();
    }

    #[cfg(feature = "workflow")]
    fn workflows_panel_active(&self) -> bool {
        self.display.workflows_panel.active
    }

    #[cfg(feature = "never")]
    fn plugins_panel_active(&self) -> bool {
        self.display.plugins_panel.active
    }

    #[cfg(feature = "workflow")]
    fn workflows_panel_input(&mut self, c: char) {
        self.display.workflows_panel.query.push(c);
        self.display.workflows_panel.selected = 0;
        self.mark_dirty();
        // Debounce search
        let window_id = self.display.window.id();
        let timer_id = TimerId::new(Topic::WorkflowsSearchTyping, window_id);
        self.scheduler.unschedule(timer_id);
        let q = self.display.workflows_panel.query.clone();
        let evt = Event::new(EventType::WorkflowsSearchPerform(q), window_id);
        self.scheduler.schedule(evt, WORKFLOWS_SEARCH_DEBOUNCE, false, timer_id);
    }

    #[cfg(feature = "never")]
    fn plugins_panel_input(&mut self, c: char) {
        self.display.plugins_panel.query.push(c);
        self.display.plugins_panel.selected = 0;
        self.mark_dirty();
        // Debounce search
        let window_id = self.display.window.id();
        let timer_id = TimerId::new(Topic::PluginsSearchTyping, window_id); // debounced plugin search
        self.scheduler.unschedule(timer_id);
        let q = self.display.plugins_panel.query.clone();
        let evt = Event::new(EventType::PluginsSearchPerform(q), window_id);
        self.scheduler.schedule(evt, WORKFLOWS_SEARCH_DEBOUNCE, false, timer_id);
    }

    #[cfg(feature = "workflow")]
    fn workflows_panel_backspace(&mut self) {
        self.display.workflows_panel.query.pop();
        self.display.workflows_panel.selected = 0;
        self.mark_dirty();
        // Debounce search
        let window_id = self.display.window.id();
        let timer_id = TimerId::new(Topic::WorkflowsSearchTyping, window_id);
        self.scheduler.unschedule(timer_id);
        let q = self.display.workflows_panel.query.clone();
        let evt = Event::new(EventType::WorkflowsSearchPerform(q), window_id);
        self.scheduler.schedule(evt, WORKFLOWS_SEARCH_DEBOUNCE, false, timer_id);
    }

    #[cfg(feature = "never")]
    fn plugins_panel_backspace(&mut self) {
        self.display.plugins_panel.query.pop();
        self.display.plugins_panel.selected = 0;
        self.mark_dirty();
        // Debounce search
        let window_id = self.display.window.id();
        let timer_id = TimerId::new(Topic::PluginsSearchTyping, window_id);
        self.scheduler.unschedule(timer_id);
        let q = self.display.plugins_panel.query.clone();
        let evt = Event::new(EventType::PluginsSearchPerform(q), window_id);
        self.scheduler.schedule(evt, WORKFLOWS_SEARCH_DEBOUNCE, false, timer_id);
    }

    #[cfg(feature = "workflow")]
    fn workflows_panel_move_selection(&mut self, delta: isize) {
        self.display.workflows_panel.move_selection(delta);
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn plugins_panel_move_selection(&mut self, delta: isize) {
        self.display.plugins_panel.move_selection(delta);
        self.mark_dirty();
    }

    #[cfg(feature = "workflow")]
    fn workflows_panel_confirm(&mut self) {
        if !self.display.workflows_panel.results.is_empty() {
            let idx = self
                .display
                .workflows_panel
                .selected
                .min(self.display.workflows_panel.results.len() - 1);
            let name = self.display.workflows_panel.results[idx].name.clone();
            // Ask processor to execute via engine if available, else fallback
            self.send_user_event(EventType::WorkflowsExecuteByName(name));
        }
        self.display.workflows_panel.close();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn plugins_panel_confirm(&mut self) {
        // If query looks like a URL, request install from URL
        let q = self.display.plugins_panel.query.trim().to_string();
        if q.starts_with("http://") || q.starts_with("https://") {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::PluginsInstallFromUrl { url: q },
                self.display.window.id(),
            ));
            return;
        }
        if self.display.plugins_panel.results.is_empty() {
            return;
        }
        let idx =
            self.display.plugins_panel.selected.min(self.display.plugins_panel.results.len() - 1);
        let item = self.display.plugins_panel.results[idx].clone();
        if item.loaded {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::PluginsUnloadByName(item.name),
                self.display.window.id(),
            ));
        } else if let Some(path) = item.path {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::PluginsLoadFromPath(path),
                self.display.window.id(),
            ));
        }
    }

    // Settings panel controls
    fn open_settings_panel(&mut self) {
        // Close palette if open
        if self.palette_active() {
            self.display.palette.save_mru_to_config(self.config);
            self.display.palette.close();
        }
        // Open with current config to preload values
        let cfg_ref: &UiConfig = self.config;
        self.display.settings_panel.open(cfg_ref);
        self.mark_dirty();
    }

    fn close_settings_panel(&mut self) {
        self.display.settings_panel.close();
        self.mark_dirty();
    }

    fn settings_panel_active(&self) -> bool {
        self.display.settings_panel.active
    }

    fn settings_panel_input(&mut self, c: char) {
        self.display.settings_panel.insert_char(c);
        self.mark_dirty();
    }

    fn settings_panel_backspace(&mut self) {
        self.display.settings_panel.backspace();
        self.mark_dirty();
    }

    fn settings_panel_next_field(&mut self) {
        self.display.settings_panel.next_field();
        self.mark_dirty();
    }

    fn settings_panel_prev_field(&mut self) {
        self.display.settings_panel.prev_field();
        self.mark_dirty();
    }

    fn settings_panel_cycle_provider(&mut self, forward: bool) {
        use crate::display::settings_panel::Field;
        match self.display.settings_panel.selected_field {
            Field::Provider => {
                self.display.settings_panel.cycle_provider(forward);
                self.mark_dirty();
            }
            Field::Routing => {
                self.display.settings_panel.cycle_routing(forward);
                self.mark_dirty();
            }
            Field::ApplyJoiner => {
                self.display.settings_panel.cycle_apply_joiner(forward);
                self.mark_dirty();
            }
            _ => {}
        }
    }

    fn settings_panel_switch_category(&mut self, forward: bool) {
        self.display.settings_panel.switch_category(forward);
        self.mark_dirty();
    }

    fn settings_panel_test_connection(&mut self) {
        let cfg_ref: &UiConfig = self.config;
        self.display.settings_panel.test_connection(cfg_ref);
        self.mark_dirty();
    }

    fn settings_panel_move_selection(&mut self, delta: isize) {
        self.display.settings_panel.move_kb_selection(delta);
        self.mark_dirty();
    }

    fn settings_panel_begin_capture(&mut self) {
        self.display.settings_panel.begin_kb_capture();
        self.mark_dirty();
    }

    fn settings_panel_cancel_capture(&mut self) {
        self.display.settings_panel.cancel_kb_capture();
        self.mark_dirty();
    }

    fn settings_panel_is_capturing(&self) -> bool {
        self.display.settings_panel.is_kb_capturing()
    }

    fn settings_panel_capture(
        &mut self,
        key: winit::keyboard::Key<String>,
        mods: winit::keyboard::ModifiersState,
    ) {
        let res = self.display.settings_panel.capture_kb_binding(self.config, key, mods);
        if res.is_ok() {
            // Reload config to apply keybinding
            let path = crate::config::installed_config("toml").unwrap_or_else(|| {
                let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                base.join("openagent-terminal").join("openagent-terminal.toml")
            });
            let _ = self
                .event_proxy
                .send_event(Event::new(EventType::ConfigReload(path), self.display.window.id()));
        }
        self.mark_dirty();
    }

    fn settings_panel_save(&mut self) {
        let need_reload;
        let category = self.display.settings_panel.category;
        let save_res = self.display.settings_panel.save();
        match save_res {
            Ok(()) => {
                // For most categories we should reload config to apply changes
                need_reload = matches!(
                    category,
                    crate::display::settings_panel::SettingsCategory::Ai
                        | crate::display::settings_panel::SettingsCategory::Theme
                        | crate::display::settings_panel::SettingsCategory::General
                );

                // Hot-apply AI provider changes: reconfigure runtime with new model/endpoint
                #[cfg(feature = "ai")]
                if matches!(category, crate::display::settings_panel::SettingsCategory::Ai) {
                    // Build provider selection and config without holding a mutable borrow
                    let name = self.display.settings_panel.provider.to_ascii_lowercase();
                    let prov_cfg = self
                        .config
                        .ai
                        .providers
                        .get(&name)
                        .cloned()
                        .or_else(|| {
                            crate::config::ai_providers::get_default_provider_configs()
                                .get(&name)
                                .cloned()
                        })
                        .unwrap_or_default();

                    // Reconfigure runtime in a limited scope and capture results
                    let (new_provider, new_model, err_opt) = if let Some(rt) = self.ai_runtime_mut()
                    {
                        rt.reconfigure_to(&name, &prov_cfg);
                        (
                            rt.ui.current_provider.clone(),
                            rt.ui.current_model.clone(),
                            rt.ui.error_message.clone(),
                        )
                    } else {
                        (String::new(), String::new(), None)
                    };

                    // Update composer badges from captured values (after mutable borrow ends)
                    if !new_provider.is_empty() {
                        self.display.ai_current_provider = new_provider;
                        self.display.ai_current_model = new_model;
                    }

                    // If an error occurred, surface it as a message
                    if let Some(err) = err_opt {
                        let message = crate::message_bar::Message::new(
                            format!("AI provider reconfigure failed: {}", err),
                            crate::message_bar::MessageType::Error,
                        );
                        let _ = self.event_proxy.send_event(Event::new(
                            EventType::Message(message),
                            self.display.window.id(),
                        ));
                    }
                }
            }
            Err(e) => {
                need_reload = false;
                self.display.settings_panel.message = Some(e);
            }
        }
        if need_reload {
            // Compute primary config path and request reload
            let path = crate::config::installed_config("toml").unwrap_or_else(|| {
                let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                base.join("openagent-terminal").join("openagent-terminal.toml")
            });
            let _ = self
                .event_proxy
                .send_event(Event::new(EventType::ConfigReload(path), self.display.window.id()));
        }
        self.mark_dirty();
    }

    // Workflows progress overlay controls
    #[cfg(feature = "workflow")]
    fn workflows_progress_active(&self) -> bool {
        self.display.workflows_progress.active
    }

    #[cfg(feature = "workflow")]
    fn workflows_progress_dismiss(&mut self) {
        if let Some(exec) = self.display.workflows_progress.execution_id.clone() {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::WorkflowsProgressClear(exec),
                self.display.window.id(),
            ));
        } else {
            // No execution id; clear directly
            self.display.workflows_progress.active = false;
            self.mark_dirty();
            if self.display.window.has_frame {
                self.display.window.request_redraw();
            }
        }
    }

    #[cfg(feature = "workflow")]
    fn workflows_progress_terminal(&self) -> bool {
        // Treat the workflow progress overlay as a terminal-level overlay only when
        // no other modal/panel UI has focus. This prevents Esc from dismissing the
        // workflow overlay while the user is interacting with higher-priority UI.
        !self.confirm_overlay_active()
            && !self.search_active()
            && !self.display.hint_state.active()
            && !self.blocks_search_active()
            && !self.workflows_panel_active()
            && !self.ai_active()
            && !self.palette_active()
    }

    // Confirm overlay controls
    fn confirm_overlay_active(&self) -> bool {
        self.display.confirm_overlay.active
    }

    fn confirm_overlay_confirm(&mut self) {
        if let Some(id) = self.display.confirm_overlay.id.clone() {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::ConfirmRespond { id, accepted: true },
                self.display.window.id(),
            ));
        }
    }

    fn confirm_overlay_cancel(&mut self) {
        if let Some(id) = self.display.confirm_overlay.id.clone() {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::ConfirmRespond { id, accepted: false },
                self.display.window.id(),
            ));
        }
    }

    // Notebooks panel controls (feature = "blocks")
        #[cfg(feature = "never")]
    fn open_notebooks_panel(&mut self) {
        if self.palette_active() {
            self.display.palette.save_mru_to_config(self.config);
            self.display.palette.close();
        }
        self.display.notebooks_panel.open();
        self.mark_dirty();
        // Kick off list load
        let _ = self
            .event_proxy
            .send_event(Event::new(EventType::NotebooksOpen, self.display.window.id()));
    }

    #[cfg(feature = "never")]
    fn notebooks_panel_active(&self) -> bool {
        self.display.notebooks_panel.active
    }

    #[cfg(feature = "never")]
    fn notebooks_panel_close(&mut self) {
        self.display.notebooks_panel.close();
        self.mark_dirty();
    }

    #[cfg(feature = "never")]
    fn notebooks_panel_move_selection(&mut self, delta: isize) {
        use std::cmp::min;
        // If a notebook is selected, move within cells; else move within notebooks
        // Decide based on focus
        if matches!(
            self.display.notebooks_panel.focus,
            crate::display::notebook_panel::FocusArea::Cells
        ) {
            // Move cell selection
            let len = self.display.notebooks_panel.cells.len();
            if len == 0 {
                return;
            }
            let cur = self.display.notebooks_panel.selected_cell.min(len.saturating_sub(1));
            let new_idx = if delta.is_negative() {
                cur.saturating_sub(delta.unsigned_abs())
            } else {
                min(cur.saturating_add(delta as usize), len.saturating_sub(1))
            };
            self.display.notebooks_panel.selected_cell = new_idx;
            self.mark_dirty();
        } else {
            // Move notebooks selection by changing selected_notebook id
            let len = self.display.notebooks_panel.notebooks.len();
            if len == 0 {
                return;
            }
            // Find current index as None => -1
            let cur_opt = self.display.notebooks_panel.selected_notebook.as_ref().and_then(|id| {
                self.display.notebooks_panel.notebooks.iter().position(|n| &n.id == id)
            });
            let cur = cur_opt.unwrap_or(0);
            let new_idx = if delta.is_negative() {
                cur.saturating_sub(delta.unsigned_abs())
            } else {
                min(cur.saturating_add(delta as usize), len.saturating_sub(1))
            };
            if let Some(nb) = self.display.notebooks_panel.notebooks.get(new_idx) {
                self.display.notebooks_panel.selected_notebook = Some(nb.id.clone());
                self.display.notebooks_panel.selected_cell = 0;
                // Request cells for this notebook
                let _ = self.event_proxy.send_event(Event::new(
                    EventType::NotebooksSelect(nb.id.clone()),
                    self.display.window.id(),
                ));
                self.mark_dirty();
            }
        }
    }

    #[cfg(feature = "never")]
    fn notebooks_panel_confirm(&mut self) {
        // If no notebook selected, treat as selecting the first notebook
        if self.display.notebooks_panel.selected_notebook.is_none() {
            if let Some(first) = self.display.notebooks_panel.notebooks.first() {
                self.display.notebooks_panel.selected_notebook = Some(first.id.clone());
                self.display.notebooks_panel.selected_cell = 0;
                let _ = self.event_proxy.send_event(Event::new(
                    EventType::NotebooksSelect(first.id.clone()),
                    self.display.window.id(),
                ));
                self.mark_dirty();
            }
            return;
        }
        // Run selected cell if it is a command cell
        let idx = self.display.notebooks_panel.selected_cell;
        if let Some(cell) = self.display.notebooks_panel.cells.get(idx) {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::NotebooksRunCell(cell.id.clone()),
                self.display.window.id(),
            ));
        }
    }

    #[cfg(feature = "never")]
    fn notebooks_panel_rerun_selected(&mut self) {
        if !self.display.notebooks_panel.active {
            return;
        }
        let idx = self.display.notebooks_panel.selected_cell;
        if let Some(cell) = self.display.notebooks_panel.cells.get(idx) {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::NotebooksRunCell(cell.id.clone()),
                self.display.window.id(),
            ));
        }
    }

    #[cfg(feature = "never")]
    fn notebooks_panel_add_command_cell(&mut self) {
        if let Some(ref nb_id) = self.display.notebooks_panel.selected_notebook {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::NotebooksAddCommand(nb_id.clone()),
                self.display.window.id(),
            ));
        }
    }

    #[cfg(feature = "never")]
    fn notebooks_panel_add_markdown_cell(&mut self) {
        if let Some(ref nb_id) = self.display.notebooks_panel.selected_notebook {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::NotebooksAddMarkdown(nb_id.clone()),
                self.display.window.id(),
            ));
        }
    }

    #[cfg(feature = "never")]
    fn notebooks_panel_run_all(&mut self) {
        if let Some(ref nb_id) = self.display.notebooks_panel.selected_notebook {
            let _ = self.event_proxy.send_event(Event::new(
                EventType::NotebooksRunNotebook(nb_id.clone()),
                self.display.window.id(),
            ));
        }
    }

    #[cfg(feature = "never")]
    fn notebooks_panel_focus_next(&mut self) {
        if self.display.notebooks_panel.active {
            self.display.notebooks_panel.toggle_focus();
            self.mark_dirty();
        }
    }

    #[cfg(feature = "never")]
    fn notebooks_panel_focus_prev(&mut self) {
        if self.display.notebooks_panel.active {
            self.display.notebooks_panel.toggle_focus();
            self.mark_dirty();
        }
    }

    #[inline]
    fn send_user_event(&self, event: crate::event::EventType) {
        let _ = self.event_proxy.send_event(Event::new(event, self.display.window.id()));
    }

    #[cfg(feature = "ai")]
    fn ai_runtime_mut(&mut self) -> Option<&mut crate::ai_runtime::AiRuntime> {
        self.ai_runtime.as_deref_mut()
    }

    #[cfg(feature = "ai")]
    fn ai_runtime_ref(&self) -> Option<&crate::ai_runtime::AiRuntime> {
        self.ai_runtime.as_deref()
    }

    #[inline]
    fn terminal(&self) -> &Term<T> {
        self.terminal
    }

    #[inline]
    fn terminal_mut(&mut self) -> &mut Term<T> {
        self.terminal
    }

    fn spawn_new_instance(&mut self) {
        let mut env_args = env::args();
        let binary = env_args.next().unwrap();

        let mut args: Vec<String> = Vec::new();

        // Reuse the arguments passed to OpenAgent Terminal for the new instance.
        #[allow(clippy::while_let_on_iterator)]
        while let Some(arg) = env_args.next() {
            // New instances shouldn't inherit command.
            if arg == "-e" || arg == "--command" {
                break;
            }

            // On unix, the working directory of the foreground shell is used by `start_daemon`.
            #[cfg(not(windows))]
            if arg == "--working-directory" {
                let _ = env_args.next();
                continue;
            }

            args.push(arg);
        }

        self.spawn_daemon(&binary, &args);
    }

    #[cfg(not(windows))]
    fn create_new_window(&mut self, #[cfg(target_os = "macos")] tabbing_id: Option<String>) {
        let mut options = WindowOptions::default();
        options.terminal_options.working_directory =
            foreground_process_path(self.master_fd, self.shell_pid).ok();

        #[cfg(target_os = "macos")]
        {
            options.window_tabbing_id = tabbing_id;
        }

        let _ = self.event_proxy.send_event(Event::new(EventType::CreateWindow(options), None));
    }

    #[cfg(windows)]
    fn create_new_window(&mut self) {
        let _ = self
            .event_proxy
            .send_event(Event::new(EventType::CreateWindow(WindowOptions::default()), None));
    }

    fn spawn_daemon<I, S>(&self, program: &str, args: I)
    where
        I: IntoIterator<Item = S> + Debug + Copy,
        S: AsRef<OsStr>,
    {
        #[cfg(not(windows))]
        let result = spawn_daemon(program, args, self.master_fd, self.shell_pid);
        #[cfg(windows)]
        let result = spawn_daemon(program, args);

        match result {
            Ok(_) => debug!("Launched {program} with args {args:?}"),
            Err(err) => warn!("Unable to launch {program} with args {args:?}: {err}"),
        }
    }

    fn change_font_size(&mut self, delta: f32) {
        // Round to pick integral px steps, since fonts look better on them.
        let new_size = self.display.font_size.as_px().round() + delta;
        self.display.font_size = FontSize::from_px(new_size);
        let font = self.config.font.clone().with_size(self.display.font_size);
        self.display.pending_update.set_font(font);
    }

    fn reset_font_size(&mut self) {
        let scale_factor = self.display.window.scale_factor as f32;
        self.display.font_size = self.config.font.size().scale(scale_factor);
        self.display
            .pending_update
            .set_font(self.config.font.clone().with_size(self.display.font_size));
    }

    #[inline]
    fn pop_message(&mut self) {
        if !self.message_buffer.is_empty() {
            self.display.pending_update.dirty = true;
            self.message_buffer.pop();
        }
    }

    #[inline]
    fn start_search(&mut self, direction: Direction) {
        // Only create new history entry if the previous regex wasn't empty.
        if self.search_state.history.front().map_or(true, |regex| !regex.is_empty()) {
            self.search_state.history.push_front(String::new());
            self.search_state.history.truncate(MAX_SEARCH_HISTORY_SIZE);
        }

        self.search_state.history_index = Some(0);
        self.search_state.direction = direction;
        self.search_state.focused_match = None;

        // Store original search position as origin and reset location.
        if self.terminal.mode().contains(TermMode::VI) {
            self.search_state.origin = self.terminal.vi_mode_cursor.point;
            self.search_state.display_offset_delta = 0;

            // Adjust origin for content moving upward on search start.
            if self.terminal.grid().cursor.point.line + 1 == self.terminal.screen_lines() {
                self.search_state.origin.line -= 1;
            }
        } else {
            let viewport_top = Line(-(self.terminal.grid().display_offset() as i32)) - 1;
            let viewport_bottom = viewport_top + self.terminal.bottommost_line();
            let last_column = self.terminal.last_column();
            self.search_state.origin = match direction {
                Direction::Right => Point::new(viewport_top, Column(0)),
                Direction::Left => Point::new(viewport_bottom, last_column),
            };
        }

        // Enable IME so we can input into the search bar with it if we were in Vi mode.
        self.window().set_ime_allowed(true);

        self.display.damage_tracker.frame().mark_fully_damaged();
        self.display.pending_update.dirty = true;
    }

    #[inline]
    fn start_seeded_search(&mut self, direction: Direction, text: String) {
        let origin = self.terminal.vi_mode_cursor.point;

        // Start new search.
        self.clear_selection();
        self.start_search(direction);

        // Enter initial selection text.
        for c in text.chars() {
            if let '$' | '('..='+' | '?' | '['..='^' | '{'..='}' = c {
                self.search_input('\\');
            }
            self.search_input(c);
        }

        // Leave search mode.
        self.confirm_search();

        if !self.terminal.mode().contains(TermMode::VI) {
            return;
        }

        // Find the target vi cursor point by going to the next match to the right of the origin,
        // then jump to the next search match in the target direction.
        let target = self.search_next(origin, Direction::Right, Side::Right).and_then(|rm| {
            let regex_match = match direction {
                Direction::Right => {
                    let origin = rm.end().add(self.terminal, Boundary::None, 1);
                    self.search_next(origin, Direction::Right, Side::Left)?
                }
                Direction::Left => {
                    let origin = rm.start().sub(self.terminal, Boundary::None, 1);
                    self.search_next(origin, Direction::Left, Side::Left)?
                }
            };
            Some(*regex_match.start())
        });

        // Move the vi cursor to the target position.
        if let Some(target) = target {
            self.terminal_mut().vi_goto_point(target);
            self.mark_dirty();
        }
    }

    #[inline]
    fn confirm_search(&mut self) {
        // Just cancel search when not in vi mode.
        if !self.terminal.mode().contains(TermMode::VI) {
            self.cancel_search();
            return;
        }

        // Force unlimited search if the previous one was interrupted.
        let timer_id = TimerId::new(Topic::DelayedSearch, self.display.window.id());
        if self.scheduler.scheduled(timer_id) {
            self.goto_match(None);
        }

        self.exit_search();
    }

    #[inline]
    fn cancel_search(&mut self) {
        if self.terminal.mode().contains(TermMode::VI) {
            // Recover pre-search state in vi mode.
            self.search_reset_state();
        } else if let Some(focused_match) = &self.search_state.focused_match {
            // Create a selection for the focused match.
            let start = *focused_match.start();
            let end = *focused_match.end();
            self.start_selection(SelectionType::Simple, start, Side::Left);
            self.update_selection(end, Side::Right);
            self.copy_selection(ClipboardType::Selection);
        }

        self.search_state.dfas = None;

        self.exit_search();
    }

    #[inline]
    fn search_input(&mut self, c: char) {
        match self.search_state.history_index {
            Some(0) => (),
            // When currently in history, replace active regex with history on change.
            Some(index) => {
                self.search_state.history[0] = self.search_state.history[index].clone();
                self.search_state.history_index = Some(0);
            }
            None => return,
        }
        let regex = &mut self.search_state.history[0];

        match c {
            // Handle backspace/ctrl+h.
            '\x08' | '\x7f' => {
                let _ = regex.pop();
            }
            // Add ascii and unicode text.
            ' '..='~' | '\u{a0}'..='\u{10ffff}' => regex.push(c),
            // Ignore non-printable characters.
            _ => return,
        }

        if !self.terminal.mode().contains(TermMode::VI) {
            // Clear selection so we do not obstruct any matches.
            self.terminal.selection = None;
        }

        self.update_search();
    }

    #[inline]
    fn search_pop_word(&mut self) {
        if let Some(regex) = self.search_state.regex_mut() {
            *regex = regex.trim_end().to_owned();
            regex.truncate(regex.rfind(' ').map_or(0, |i| i + 1));
            self.update_search();
        }
    }

    /// Go to the previous regex in the search history.
    #[inline]
    fn search_history_previous(&mut self) {
        let index = match &mut self.search_state.history_index {
            None => return,
            Some(index) if *index + 1 >= self.search_state.history.len() => return,
            Some(index) => index,
        };

        *index += 1;
        self.update_search();
    }

    /// Go to the previous regex in the search history.
    #[inline]
    fn search_history_next(&mut self) {
        let index = match &mut self.search_state.history_index {
            Some(0) | None => return,
            Some(index) => index,
        };

        *index -= 1;
        self.update_search();
    }

    #[inline]
    fn advance_search_origin(&mut self, direction: Direction) {
        // Use focused match as new search origin if available.
        if let Some(focused_match) = &self.search_state.focused_match {
            let new_origin = match direction {
                Direction::Right => focused_match.end().add(self.terminal, Boundary::None, 1),
                Direction::Left => focused_match.start().sub(self.terminal, Boundary::None, 1),
            };

            self.terminal.scroll_to_point(new_origin);

            self.search_state.display_offset_delta = 0;
            self.search_state.origin = new_origin;
        }

        // Search for the next match using the supplied direction.
        let search_direction = mem::replace(&mut self.search_state.direction, direction);
        self.goto_match(None);
        self.search_state.direction = search_direction;

        // If we found a match, we set the search origin right in front of it to make sure that
        // after modifications to the regex the search is started without moving the focused match
        // around.
        let focused_match = match &self.search_state.focused_match {
            Some(focused_match) => focused_match,
            None => return,
        };

        // Set new origin to the left/right of the match, depending on search direction.
        let new_origin = match self.search_state.direction {
            Direction::Right => *focused_match.start(),
            Direction::Left => *focused_match.end(),
        };

        // Store the search origin with display offset by checking how far we need to scroll to it.
        let old_display_offset = self.terminal.grid().display_offset() as i32;
        self.terminal.scroll_to_point(new_origin);
        let new_display_offset = self.terminal.grid().display_offset() as i32;
        self.search_state.display_offset_delta = new_display_offset - old_display_offset;

        // Store origin and scroll back to the match.
        self.terminal.scroll_display(Scroll::Delta(-self.search_state.display_offset_delta));
        self.search_state.origin = new_origin;
    }

    /// Find the next search match.
    fn search_next(&mut self, origin: Point, direction: Direction, side: Side) -> Option<Match> {
        self.search_state
            .dfas
            .as_mut()
            .and_then(|dfas| self.terminal.search_next(dfas, origin, direction, side, None))
    }

    #[inline]
    fn search_direction(&self) -> Direction {
        self.search_state.direction
    }

    #[inline]
    fn search_active(&self) -> bool {
        self.search_state.history_index.is_some()
    }

    /// Handle keyboard typing start.
    ///
    /// This will temporarily disable some features like terminal cursor blinking or the mouse
    /// cursor.
    ///
    /// All features are re-enabled again automatically.
    #[inline]
    fn on_typing_start(&mut self) {
        // Disable cursor blinking.
        let timer_id = TimerId::new(Topic::BlinkCursor, self.display.window.id());
        if self.scheduler.unschedule(timer_id).is_some() {
            self.schedule_blinking();

            // Mark the cursor as visible and queue redraw if the cursor was hidden.
            if mem::take(&mut self.display.cursor_hidden) {
                *self.dirty = true;
            }
        } else if *self.cursor_blink_timed_out {
            self.update_cursor_blinking();
        }

        // Hide mouse cursor.
        if self.config.mouse.hide_when_typing {
            self.display.window.set_mouse_visible(false);
        }
    }

    /// Process a new character for keyboard hints.
    fn hint_input(&mut self, c: char) {
        if let Some(hint) = self.display.hint_state.keyboard_input(self.terminal, c) {
            self.mouse.block_hint_launcher = false;
            self.trigger_hint(&hint);
        }
        *self.dirty = true;
    }

    /// Trigger a hint action.
    fn trigger_hint(&mut self, hint: &HintMatch) {
        if self.mouse.block_hint_launcher {
            return;
        }

        let hint_bounds = hint.bounds();
        let text = match hint.text(self.terminal) {
            Some(text) => text,
            None => return,
        };

        match &hint.action() {
            // Launch an external program.
            HintAction::Command(command) => {
                let mut args = command.args().to_vec();
                args.push(text.into());
                self.spawn_daemon(command.program(), &args);
            }
            // Copy the text to the clipboard.
            HintAction::Action(HintInternalAction::Copy) => {
                self.clipboard.store(ClipboardType::Clipboard, text);
            }
            // Write the text to the PTY/search.
            HintAction::Action(HintInternalAction::Paste) => self.paste(&text, true),
            // Select the text.
            HintAction::Action(HintInternalAction::Select) => {
                self.start_selection(SelectionType::Simple, *hint_bounds.start(), Side::Left);
                self.update_selection(*hint_bounds.end(), Side::Right);
                self.copy_selection(ClipboardType::Selection);
            }
            // Move the vi mode cursor.
            HintAction::Action(HintInternalAction::MoveViModeCursor) => {
                // Enter vi mode if we're not in it already.
                if !self.terminal.mode().contains(TermMode::VI) {
                    self.terminal.toggle_vi_mode();
                }

                self.terminal.vi_goto_point(*hint_bounds.start());
                self.mark_dirty();
            }
        }
    }

    /// Expand the selection to the current mouse cursor position.
    #[inline]
    fn expand_selection(&mut self) {
        let control = self.modifiers().state().control_key();
        let selection_type = match self.mouse().click_state {
            ClickState::None => return,
            _ if control => SelectionType::Block,
            ClickState::Click => SelectionType::Simple,
            ClickState::DoubleClick => SelectionType::Semantic,
            ClickState::TripleClick => SelectionType::Lines,
        };

        // Load mouse point, treating message bar and padding as the closest cell.
        let display_offset = self.terminal().grid().display_offset();
        let point = self.mouse().point(&self.size_info(), display_offset);

        let cell_side = self.mouse().cell_side;

        let selection = match &mut self.terminal_mut().selection {
            Some(selection) => selection,
            None => return,
        };

        selection.ty = selection_type;
        self.update_selection(point, cell_side);

        // Move vi mode cursor to mouse click position.
        if self.terminal().mode().contains(TermMode::VI) && !self.search_active() {
            self.terminal_mut().vi_mode_cursor.point = point;
        }
    }

    /// Get the semantic word at the specified point.
    fn semantic_word(&self, point: Point) -> String {
        let terminal = self.terminal();
        let grid = terminal.grid();

        // Find the next semantic word boundary to the right.
        let mut end = terminal.semantic_search_right(point);

        // Get point at which skipping over semantic characters has led us back to the
        // original character.
        let start_cell = &grid[point];
        let search_end = if start_cell.flags.intersects(Flags::LEADING_WIDE_CHAR_SPACER) {
            point.add(terminal, Boundary::None, 2)
        } else if start_cell.flags.intersects(Flags::WIDE_CHAR) {
            point.add(terminal, Boundary::None, 1)
        } else {
            point
        };

        // Keep moving until we're not on top of a semantic escape character.
        let semantic_chars = terminal.semantic_escape_chars();
        loop {
            let cell = &grid[end];

            // Get cell's character, taking wide characters into account.
            let c = if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                grid[end.sub(terminal, Boundary::None, 1)].c
            } else {
                cell.c
            };

            if !semantic_chars.contains(c) {
                break;
            }

            end = terminal.semantic_search_right(end.add(terminal, Boundary::None, 1));

            // Stop if the entire grid is only semantic escape characters.
            if end == search_end {
                return String::new();
            }
        }

        // Find the beginning of the semantic word.
        let start = terminal.semantic_search_left(end);

        terminal.bounds_to_string(start, end)
    }

    /// Handle beginning of terminal text input.
    fn on_terminal_input_start(&mut self) {
        self.on_typing_start();
        self.clear_selection();

        if self.terminal().grid().display_offset() != 0 {
            self.scroll(Scroll::Bottom);
        }
    }

    /// Paste a text into the terminal.
    fn paste(&mut self, text: &str, bracketed: bool) {
        if self.search_active() {
            for c in text.chars() {
                self.search_input(c);
            }
        } else if self.inline_search_state.char_pending {
            self.inline_search_input(text);
        } else if bracketed && self.terminal().mode().contains(TermMode::BRACKETED_PASTE) {
            self.on_terminal_input_start();

            self.write_to_pty(&b"\x1b[200~"[..]);

            // Write filtered escape sequences.
            //
            // We remove `\x1b` to ensure it's impossible for the pasted text to write the bracketed
            // paste end escape `\x1b[201~` and `\x03` since some shells incorrectly terminate
            // bracketed paste when they receive it.
            let filtered = text.replace(['\x1b', '\x03'], "");
            self.write_to_pty(filtered.into_bytes());

            self.write_to_pty(&b"\x1b[201~"[..]);
        } else {
            self.on_terminal_input_start();

            let payload = if bracketed {
                // In non-bracketed (ie: normal) mode, terminal applications cannot distinguish
                // pasted data from keystrokes.
                //
                // In theory, we should construct the keystrokes needed to produce the data we are
                // pasting... since that's neither practical nor sensible (and probably an
                // impossible task to solve in a general way), we'll just replace line breaks
                // (windows and unix style) with a single carriage return (\r, which is what the
                // Enter key produces).
                text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
            } else {
                // When we explicitly disable bracketed paste don't manipulate with the input,
                // so we pass user input as is.
                text.to_owned().into_bytes()
            };

            self.write_to_pty(payload);
        }
    }

    /// Toggle the vi mode status.
    #[inline]
    fn toggle_vi_mode(&mut self) {
        let was_in_vi_mode = self.terminal.mode().contains(TermMode::VI);
        if was_in_vi_mode {
            // If we had search running when leaving Vi mode we should mark terminal fully damaged
            // to cleanup highlighted results.
            if self.search_state.dfas.take().is_some() {
                self.display.damage_tracker.frame().mark_fully_damaged();
            }
        } else {
            self.clear_selection();
        }

        if self.search_active() {
            self.cancel_search();
        }

        // We don't want IME in Vi mode.
        self.window().set_ime_allowed(was_in_vi_mode);

        self.terminal.toggle_vi_mode();

        *self.dirty = true;
    }

    /// Get vi inline search state.
    fn inline_search_state(&mut self) -> &mut InlineSearchState {
        self.inline_search_state
    }

    /// Start vi mode inline search.
    fn start_inline_search(&mut self, direction: Direction, stop_short: bool) {
        self.inline_search_state.stop_short = stop_short;
        self.inline_search_state.direction = direction;
        self.inline_search_state.char_pending = true;
        self.inline_search_state.character = None;
    }

    /// Jump to the next matching character in the line.
    fn inline_search_next(&mut self) {
        let direction = self.inline_search_state.direction;
        self.inline_search(direction);
    }

    /// Jump to the next matching character in the line.
    fn inline_search_previous(&mut self) {
        let direction = self.inline_search_state.direction.opposite();
        self.inline_search(direction);
    }

    /// Process input during inline search.
    fn inline_search_input(&mut self, text: &str) {
        // Ignore input with empty text, like modifier keys.
        let c = match text.chars().next() {
            Some(c) => c,
            None => return,
        };

        self.inline_search_state.char_pending = false;
        self.inline_search_state.character = Some(c);
        self.window().set_ime_allowed(false);

        // Immediately move to the captured character.
        self.inline_search_next();
    }

    fn message(&self) -> Option<&Message> {
        self.message_buffer.message()
    }

    fn config(&self) -> &UiConfig {
        self.config
    }

    #[cfg(target_os = "macos")]
    fn event_loop(&self) -> &ActiveEventLoop {
        self.event_loop
    }

    fn clipboard_mut(&mut self) -> &mut Clipboard {
        self.clipboard
    }

    fn scheduler_mut(&mut self) -> &mut Scheduler {
        self.scheduler
    }

    fn run_workflow_by_name(&mut self, name: &str) {
        // Try user-defined workflows from config
        if let Some(wf) = self.config.workflows.iter().find(|w| w.name == name) {
            // Expand simple {param} placeholders using defaults if any
            let mut cmd = wf.command.clone();
            for p in &wf.params {
                let placeholder = format!("{{{}}}", p.name);
                let val = p.default.clone().unwrap_or_default();
                cmd = cmd.replace(&placeholder, &val);
            }
            // Paste to terminal; user can review/edit and hit Enter
            self.paste(&cmd, true);
            return;
        }
        // Not found: inform user
        let msg = Message::new(
            format!("Workflow not found: {}", name),
            crate::message_bar::MessageType::Warning,
        );
        let _ = self
            .event_proxy
            .send_event(Event::new(EventType::Message(msg), self.display.window.id()));
        self.display.pending_update.dirty = true;
    }

    // Workspace / panes: wire to real WorkspaceManager
    fn workspace_split_horizontal(&mut self) {
        // Prefer Warp integration when available to ensure PTY lifecycle and session persistence
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::SplitRight)
                .unwrap_or(false);
            let msg = if ok {
                "Split pane horizontally (Warp)".into()
            } else {
                "Split pane horizontally failed".into()
            };
            self.message_buffer.push(Message::new(msg, crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        // Fallback: native split manager without PTY wiring
        let ratio = self.config.workspace.splits.default_ratio;
        let res = self.workspace.split_horizontal(ratio);
        let msg = if let Some(id) = res {
            format!("Split pane horizontally, new pane id {:?}", id)
        } else {
            "Split pane horizontally failed".into()
        };
        self.message_buffer.push(Message::new(msg, crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_split_vertical(&mut self) {
        // Prefer Warp integration when available to ensure PTY lifecycle and session persistence
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::SplitDown)
                .unwrap_or(false);
            let msg = if ok {
                "Split pane vertically (Warp)".into()
            } else {
                "Split pane vertically failed".into()
            };
            self.message_buffer.push(Message::new(msg, crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        // Fallback: native split manager without PTY wiring
        let ratio = self.config.workspace.splits.default_ratio;
        let res = self.workspace.split_vertical(ratio);
        let msg = if let Some(id) = res {
            format!("Split pane vertically, new pane id {:?}", id)
        } else {
            "Split pane vertically failed".into()
        };
        self.message_buffer.push(Message::new(msg, crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_focus_next_pane(&mut self) {
        // Prefer Warp directional navigation when available
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::NavigatePane(
                    crate::workspace::warp_split_manager::WarpNavDirection::Right,
                ))
                .unwrap_or(false);
            let msg = if ok { "Focused next pane (Warp)" } else { "Focus next pane failed" };
            self.message_buffer
                .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        // Fallback: native cycling
        let ok = self.workspace.focus_next_pane();
        let msg = if ok { "Focused next pane" } else { "Focus next pane failed" };
        self.message_buffer
            .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_focus_previous_pane(&mut self) {
        // Prefer Warp directional navigation when available
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::NavigatePane(
                    crate::workspace::warp_split_manager::WarpNavDirection::Left,
                ))
                .unwrap_or(false);
            let msg =
                if ok { "Focused previous pane (Warp)" } else { "Focus previous pane failed" };
            self.message_buffer
                .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        // Fallback: native cycling
        let ok = self.workspace.focus_previous_pane();
        let msg = if ok { "Focused previous pane" } else { "Focus previous pane failed" };
        self.message_buffer
            .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_close_pane(&mut self) {
        // Prefer Warp integration for cleanup guarantees
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::ClosePane)
                .unwrap_or(false);
            let msg = if ok { "Closed pane (Warp)" } else { "Close pane failed" };
            self.message_buffer
                .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        // Fallback
        let ok = self.workspace.close_pane();
        let msg = if ok { "Closed pane" } else { "Close pane failed" };
        self.message_buffer
            .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_resize_left(&mut self) {
        // Prefer Warp integration for consistent ratio logic
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::ResizePane(
                    crate::workspace::warp_split_manager::WarpResizeDirection::ExpandLeft,
                ))
                .unwrap_or(false);
            let msg = if ok { "Resized pane left (Warp)" } else { "Resize left failed" };
            self.message_buffer
                .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        let ok = self.workspace.resize_left();
        let msg = if ok { "Resized pane left" } else { "Resize left failed" };
        self.message_buffer
            .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_resize_right(&mut self) {
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::ResizePane(
                    crate::workspace::warp_split_manager::WarpResizeDirection::ExpandRight,
                ))
                .unwrap_or(false);
            let msg = if ok { "Resized pane right (Warp)" } else { "Resize right failed" };
            self.message_buffer
                .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        let ok = self.workspace.resize_right();
        let msg = if ok { "Resized pane right" } else { "Resize right failed" };
        self.message_buffer
            .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_resize_up(&mut self) {
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::ResizePane(
                    crate::workspace::warp_split_manager::WarpResizeDirection::ExpandUp,
                ))
                .unwrap_or(false);
            let msg = if ok { "Resized pane up (Warp)" } else { "Resize up failed" };
            self.message_buffer
                .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        let ok = self.workspace.resize_up();
        let msg = if ok { "Resized pane up" } else { "Resize up failed" };
        self.message_buffer
            .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_resize_down(&mut self) {
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::ResizePane(
                    crate::workspace::warp_split_manager::WarpResizeDirection::ExpandDown,
                ))
                .unwrap_or(false);
            let msg = if ok { "Resized pane down (Warp)" } else { "Resize down failed" };
            self.message_buffer
                .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        let ok = self.workspace.resize_down();
        let msg = if ok { "Resized pane down" } else { "Resize down failed" };
        self.message_buffer
            .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_focus_pane_left(&mut self) {
        // Prefer Warp directional navigation when available
        if self.workspace.has_warp() {
            let _ =
                self.workspace.execute_warp_action(&crate::workspace::WarpAction::NavigatePane(
                    crate::workspace::warp_split_manager::WarpNavDirection::Left,
                ));
        } else {
            let _ = self.workspace.focus_pane_left();
        }
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_focus_pane_right(&mut self) {
        if self.workspace.has_warp() {
            let _ =
                self.workspace.execute_warp_action(&crate::workspace::WarpAction::NavigatePane(
                    crate::workspace::warp_split_manager::WarpNavDirection::Right,
                ));
        } else {
            let _ = self.workspace.focus_pane_right();
        }
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_focus_pane_up(&mut self) {
        if self.workspace.has_warp() {
            let _ =
                self.workspace.execute_warp_action(&crate::workspace::WarpAction::NavigatePane(
                    crate::workspace::warp_split_manager::WarpNavDirection::Up,
                ));
        } else {
            let _ = self.workspace.focus_pane_up();
        }
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_focus_pane_down(&mut self) {
        if self.workspace.has_warp() {
            let _ =
                self.workspace.execute_warp_action(&crate::workspace::WarpAction::NavigatePane(
                    crate::workspace::warp_split_manager::WarpNavDirection::Down,
                ));
        } else {
            let _ = self.workspace.focus_pane_down();
        }
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_create_tab(&mut self) {
        // Prefer Warp integration for tab creation to spawn PTY/Term immediately
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::CreateTab)
                .unwrap_or(false);
            let msg = if ok { "Created new tab (Warp)".into() } else { "Create tab failed".into() };
            self.message_buffer.push(Message::new(msg, crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        let title = format!("Tab {}", self.workspace.tab_count() + 1);
        let working_dir = std::env::current_dir().ok();
        let tab_id = self.workspace.create_tab(title.clone(), working_dir);
        let msg = format!("Created new tab '{}' with ID {:?}", title, tab_id);
        self.message_buffer.push(Message::new(msg, crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_close_tab(&mut self) {
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::CloseTab)
                .unwrap_or(false);
            let msg = if ok { "Closed tab (Warp)".into() } else { "Close tab failed".into() };
            self.message_buffer.push(Message::new(msg, crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        if let Some(active_tab) = self.workspace.active_tab() {
            let tab_id = active_tab.id;
            let tab_title = active_tab.title.clone();
            let ok = self.workspace.close_tab(tab_id);
            let msg =
                if ok { format!("Closed tab '{}'", tab_title) } else { "Close tab failed".into() };
            self.message_buffer.push(Message::new(msg, crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
        }
    }

    fn workspace_next_tab(&mut self) {
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::NextTab)
                .unwrap_or(false);
            let msg = if ok { "Switched to next tab (Warp)" } else { "Switch to next tab failed" };
            self.message_buffer
                .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        let ok = self.workspace.next_tab();
        let msg = if ok { "Switched to next tab" } else { "Switch to next tab failed" };
        self.message_buffer
            .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_previous_tab(&mut self) {
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::PreviousTab)
                .unwrap_or(false);
            let msg = if ok {
                "Switched to previous tab (Warp)"
            } else {
                "Switch to previous tab failed"
            };
            self.message_buffer
                .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        let ok = self.workspace.previous_tab();
        let msg = if ok { "Switched to previous tab" } else { "Switch to previous tab failed" };
        self.message_buffer
            .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_switch_to_tab(&mut self, tab_id: crate::workspace::TabId) {
        let ok = self.workspace.switch_to_tab(tab_id);
        let msg = if ok {
            format!("Switched to tab {:?}", tab_id)
        } else {
            "Switch to tab failed".into()
        };
        self.message_buffer.push(Message::new(msg, crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_toggle_zoom(&mut self) {
        if self.workspace.has_warp() {
            let ok = self
                .workspace
                .execute_warp_action(&crate::workspace::WarpAction::ZoomPane)
                .unwrap_or(false);
            let msg = if ok { "Toggled pane zoom (Warp)" } else { "Toggle zoom failed" };
            self.message_buffer
                .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            return;
        }

        let ok = self.workspace.toggle_zoom();
        let msg = if ok { "Toggled pane zoom" } else { "Toggle zoom failed" };
        self.message_buffer
            .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_mark_active_tab_error(&mut self, non_zero: bool) {
        self.workspace.mark_active_tab_error(non_zero);
        *self.dirty = true;
    }

    fn workspace_toggle_sync(&mut self) {
        let ok = self.workspace.toggle_active_tab_sync();
        let msg = if ok { "Toggled pane sync" } else { "Toggle pane sync failed" };
        self.message_buffer
            .push(Message::new(msg.into(), crate::message_bar::MessageType::Warning));
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_split_hit(
        &mut self,
        mouse_x_px: f32,
        mouse_y_px: f32,
        tolerance_px: f32,
    ) -> Option<crate::workspace::split_manager::SplitDividerHit> {
        self.workspace.hit_test_split_divider(mouse_x_px, mouse_y_px, tolerance_px)
    }

    fn workspace_set_split_ratio_at_path(
        &mut self,
        path: Vec<crate::workspace::split_manager::SplitChild>,
        axis: crate::workspace::split_manager::SplitAxis,
        new_ratio: f32,
    ) {
        let _ = self.workspace.set_split_ratio_at_path(&path, axis, new_ratio);
        self.display.pending_update.dirty = true;
        *self.dirty = true;
    }

    fn workspace_tab_bar_click(
        &mut self,
        mouse_x_px: usize,
        mouse_y_px: usize,
    ) -> Option<crate::display::warp_ui::TabBarAction> {
        let position = self.config.workspace.tab_bar.position;
        self.display.handle_tab_bar_click(
            self.config,
            &self.workspace.tabs,
            position,
            mouse_x_px,
            mouse_y_px,
        )
    }

    fn workspace_tab_bar_drag_press(
        &mut self,
        mouse_x_px: usize,
        mouse_y_px: usize,
        button: MouseButton,
    ) -> bool {
        let position = self.config.workspace.tab_bar.position;
        if let Some(action) = self.display.handle_tab_bar_mouse_press(
            self.config,
            &self.workspace.tabs,
            position,
            mouse_x_px,
            mouse_y_px,
            button,
        ) {
            use crate::display::warp_ui::TabBarAction as TBA;
            match action {
                TBA::SelectTab(id) => {
                    self.workspace_switch_to_tab(id);
                    return true;
                }
                TBA::CloseTab(id) => {
                    self.workspace_switch_to_tab(id);
                    self.workspace_close_tab();
                    return true;
                }
                TBA::CreateTab => {
                    self.workspace_create_tab();
                    return true;
                }
                TBA::OpenSettings => {
                    self.open_settings_panel();
                    return true;
                }
                // Drag-related actions are handled by dedicated handlers; treat as handled here
                TBA::BeginDrag(_) | TBA::DragMove(..) | TBA::EndDrag(_) | TBA::CancelDrag(_) => {
                    // Mark dirty for visuals; actual drag is handled elsewhere
                    self.display.pending_update.dirty = true;
                    *self.dirty = true;
                    return true;
                }
            }
        }
        false
    }

    fn workspace_tab_bar_drag_move(&mut self, mouse_x_px: usize, mouse_y_px: usize) -> bool {
        if let Some(action) =
            self.display.handle_tab_bar_mouse_move(&self.workspace.tabs, mouse_x_px, mouse_y_px)
        {
            use crate::display::warp_ui::TabBarAction as TBA;
            if let TBA::DragMove(tab_id, new_pos) = action {
                let moved = self.workspace.tabs.move_tab(tab_id, new_pos);
                if moved {
                    // Damage the tab bar line to refresh visuals
                    let line = match self.config.workspace.tab_bar.position {
                        crate::workspace::TabBarPosition::Top => 0,
                        crate::workspace::TabBarPosition::Bottom => {
                            self.display.size_info.screen_lines().saturating_sub(1)
                        }
                        crate::workspace::TabBarPosition::Hidden => 0,
                    };
                    let cols = self.display.size_info.columns();
                    self.display.damage_tracker.frame().damage_line(
                        openagent_terminal_core::term::LineDamageBounds::new(line, 0, cols),
                    );
                    self.display.pending_update.dirty = true;
                    *self.dirty = true;
                }
                return true;
            }
        }
        false
    }

    fn workspace_tab_bar_drag_release(&mut self, button: MouseButton) -> bool {
        if let Some(action) = self.display.handle_tab_bar_mouse_release(button) {
            use crate::display::warp_ui::TabBarAction as TBA;
            match action {
                TBA::EndDrag(_) => {
                    self.display.pending_update.dirty = true;
                    *self.dirty = true;
                    return true;
                }
                TBA::SelectTab(id) => {
                    self.workspace_switch_to_tab(id);
                    return true;
                }
                TBA::CloseTab(id) => {
                    self.workspace_switch_to_tab(id);
                    self.workspace_close_tab();
                    return true;
                }
                TBA::CreateTab => {
                    self.workspace_create_tab();
                    return true;
                }
                TBA::OpenSettings => {
                    self.open_settings_panel();
                    return true;
                }
                TBA::BeginDrag(_) | TBA::DragMove(..) | TBA::CancelDrag(_) => {
                    // Shouldn't happen on release; ignore
                    return false;
                }
            }
        }
        false
    }

    fn copy_to_clipboard(&mut self, text: String) {
        self.clipboard.store(ClipboardType::Clipboard, text);
    }

    // --- Pane drag-and-drop integration (Alt+Left Drag) ---
    fn workspace_pane_drag_press(
        &mut self,
        mouse_x_px: f32,
        mouse_y_px: f32,
        button: MouseButton,
    ) -> bool {
        use crate::display::pane_drag_drop::PaneDragType;
        if button != MouseButton::Left {
            return false;
        }
        // Require an active tab/pane
        let (tab_id, pane_id) = match self.workspace.active_tab() {
            Some(tab) => (tab.id, tab.active_pane),
            None => return false,
        };
        // Start drag animation state
        self.display.pane_drag_manager.start_drag(
            tab_id,
            pane_id,
            (mouse_x_px, mouse_y_px),
            PaneDragType::MoveToTab,
        );
        self.display.pending_update.dirty = true;
        *self.dirty = true;
        true
    }

    fn workspace_pane_drag_move(&mut self, mouse_x_px: f32, mouse_y_px: f32) -> bool {
        // Need active tab and its layout to compute split areas
        let (tab_id, split_layout) = match self.workspace.active_tab() {
            Some(tab) => (tab.id, &tab.split_layout),
            None => return false,
        };

        // Compute tab positions: prefer cached precise bounds from draw pass
        let mut tab_positions: Vec<(crate::workspace::TabId, f32, f32)> =
            self.display.tab_bounds_px.clone();
        // Fallback to even widths if cache is empty
        if tab_positions.is_empty() {
            let tm = &self.workspace.tabs;
            let order = tm.tab_order();
            if !order.is_empty() {
                let w = self.display.size_info.width();
                let tab_w = (w / order.len() as f32).max(1.0);
                let mut x = 0.0f32;
                for &tid in order {
                    let width = (tab_w).min((w - x).max(0.0));
                    tab_positions.push((tid, x, width));
                    x += tab_w;
                }
            }
        }

        // Compute split areas for the active tab
        let si = self.display.size_info;
        // Content container with padding and reserved tab bar row (when effectively visible)
        let x0 = si.padding_x();
        let mut y0 = si.padding_y();
        let cw = si.width() - 2.0 * si.padding_x();
        let mut ch = si.height() - 2.0 * si.padding_y();
        let tab_cfg = &self.config.workspace.tab_bar;
        let is_fs = self.display.window.is_fullscreen();
        let effective_visibility = match tab_cfg.visibility {
            crate::config::workspace::TabBarVisibility::Always => {
                crate::config::workspace::TabBarVisibility::Always
            }
            crate::config::workspace::TabBarVisibility::Hover => {
                crate::config::workspace::TabBarVisibility::Hover
            }
            crate::config::workspace::TabBarVisibility::Auto => {
                if is_fs {
                    crate::config::workspace::TabBarVisibility::Hover
                } else {
                    crate::config::workspace::TabBarVisibility::Always
                }
            }
        };
        if tab_cfg.show
            && !self.config.workspace.warp_overlay_only
            && matches!(effective_visibility, crate::config::workspace::TabBarVisibility::Always)
            && tab_cfg.position != crate::workspace::TabBarPosition::Hidden
        {
            let ch_row = si.cell_height();
            match tab_cfg.position {
                crate::workspace::TabBarPosition::Top => {
                    y0 += ch_row;
                    ch = (ch - ch_row).max(0.0);
                }
                crate::workspace::TabBarPosition::Bottom => {
                    ch = (ch - ch_row).max(0.0);
                }
                crate::workspace::TabBarPosition::Hidden => {}
            }
        }
        let container = crate::workspace::split_manager::PaneRect::new(x0, y0, cw, ch);
        let split_areas_raw = self.workspace.splits.calculate_pane_rects(split_layout, container);
        let mut split_areas: Vec<(
            crate::workspace::TabId,
            crate::workspace::PaneId,
            f32,
            f32,
            f32,
            f32,
        )> = Vec::with_capacity(split_areas_raw.len());
        for (pid, rect) in split_areas_raw {
            split_areas.push((tab_id, pid, rect.x, rect.y, rect.width, rect.height));
        }

        // Determine if the cursor is inside the tab bar band (supports Top/Bottom)
        let style = crate::display::warp_ui::WarpTabStyle::from_theme(self.config);
        let consider_tabs =
            tab_cfg.show && tab_cfg.position != crate::workspace::TabBarPosition::Hidden;
        let bar_y = match tab_cfg.position {
            crate::workspace::TabBarPosition::Top => 0.0,
            crate::workspace::TabBarPosition::Bottom => si.height() - style.tab_height,
            crate::workspace::TabBarPosition::Hidden => -1.0,
        };
        // Apply vertical snap margin when deciding if we're inside the tab bar band
        let snap_px = self.config.workspace.drag.tab_drop_snap_px.max(0.0);
        let bar_top = (bar_y - snap_px).max(0.0);
        let bar_bottom = (bar_y + style.tab_height + snap_px).min(si.height());
        let in_tab_bar_band = consider_tabs && mouse_y_px >= bar_top && mouse_y_px < bar_bottom;

        // Only allow tab drop targeting when actually hovering (or snapped to) the tab bar band
        let empty_tabs: Vec<(crate::workspace::TabId, f32, f32)> = Vec::new();
        let tabs_for_hit = if in_tab_bar_band { &tab_positions } else { &empty_tabs };

        // Compute current drop zone and update animations
        let mut dz = self.display.pane_drag_manager.calculate_drop_zone(
            (mouse_x_px, mouse_y_px),
            tabs_for_hit,
            &split_areas,
        );

        // If in the tab bar band and not over any existing tab, allow creating a new tab by dropping
        if dz.is_none() && in_tab_bar_band {
            let max_tab_x = tab_positions.iter().map(|(_, x, w)| x + w).fold(0.0, f32::max);
            let extra = self.config.workspace.drag.new_tab_snap_extra_px.max(0.0);
            if mouse_x_px >= (max_tab_x - extra).max(0.0) {
                dz = Some(crate::display::pane_drag_drop::PaneDropZone::NewTab {
                    position: tab_positions.len(),
                });
            }
        }

        let updated = self.display.pane_drag_manager.update_drag((mouse_x_px, mouse_y_px), dz);
        if updated {
            self.display.pending_update.dirty = true;
            *self.dirty = true;
            if self.display.window.has_frame {
                self.display.window.request_redraw();
            }
        }
        updated
    }

    fn workspace_pane_drag_release(&mut self, button: MouseButton) -> bool {
        if button != MouseButton::Left {
            return false;
        }
        if let Some((drag_state, drop_zone)) = self.display.pane_drag_manager.end_drag() {
            use crate::display::pane_drag_drop::SplitDirection;
            match drop_zone {
                Some(crate::display::pane_drag_drop::PaneDropZone::Split {
                    tab_id: _tab_target,
                    direction,
                    target_split: target_opt,
                    before,
                }) => {
                    // If Shift is held, attempt a fast swap of adjacent panes
                    let shift_down = self.modifiers.state().shift_key();
                    if shift_down {
                        if let Some(target_pid) = target_opt {
                            if let Some(active_tab) = self.workspace.active_tab_mut() {
                                let sm = crate::workspace::SplitManager::new();
                                let swapped = sm.swap_adjacent_panes(
                                    &mut active_tab.split_layout,
                                    drag_state.source_split,
                                    target_pid,
                                );
                                if swapped {
                                    // Keep focus on the dragged (now swapped) pane
                                    active_tab.active_pane = drag_state.source_split;
                                    self.display.pending_update.dirty = true;
                                    *self.dirty = true;
                                    return true;
                                }
                            }
                        }
                    }
                    // Otherwise: Move semantics within the active tab: re-parent the dragged pane next to target
                    if let Some(target_pid) = target_opt {
                        if let Some(active_tab) = self.workspace.active_tab_mut() {
                            // Map SplitDirection to SplitAxis
                            let axis = match direction {
                                SplitDirection::Vertical => {
                                    crate::workspace::split_manager::SplitAxis::Horizontal
                                }
                                SplitDirection::Horizontal => {
                                    crate::workspace::split_manager::SplitAxis::Vertical
                                }
                            };
                            let mut sm = crate::workspace::SplitManager::new();
                            let _ok = sm.move_pane_to_split(
                                &mut active_tab.split_layout,
                                drag_state.source_split,
                                target_pid,
                                axis,
                                before,
                            );
                            // Normalize and focus the moved pane
                            sm.normalize(&mut active_tab.split_layout);
                            active_tab.active_pane = drag_state.source_split;
                            self.display.pending_update.dirty = true;
                            *self.dirty = true;
                            return true;
                        }
                    }
                }
                Some(crate::display::pane_drag_drop::PaneDropZone::Tab { tab_id, .. }) => {
                    // Cross-tab move: remove from source tab and insert next to the destination
                    // tab's active pane
                    // 1) Remove from the source tab's layout (from drag_state.source_tab)
                    if let Some(src_tab) = self.workspace.tabs.get_tab_mut(drag_state.source_tab) {
                        let mut sm = crate::workspace::SplitManager::new();
                        let _ = sm.close_pane(&mut src_tab.split_layout, drag_state.source_split);
                    }

                    // 2) Move PaneContext across tabs (if present)
                    let _ = self.workspace.tabs.move_pane_context(
                        drag_state.source_tab,
                        tab_id,
                        drag_state.source_split,
                    );

                    // 3) Insert into destination tab around its active pane and update active pane
                    if let Some(dest_tab) = self.workspace.tabs.get_tab_mut(tab_id) {
                        let axis = crate::workspace::split_manager::SplitAxis::Horizontal;
                        let before = true;
                        let sm = crate::workspace::SplitManager::new();
                        let _ = sm.insert_pane_with_split(
                            &mut dest_tab.split_layout,
                            dest_tab.active_pane,
                            drag_state.source_split,
                            axis,
                            before,
                        );
                        // Normalize destination layout
                        sm.normalize(&mut dest_tab.split_layout);
                        // Focus moved pane
                        dest_tab.active_pane = drag_state.source_split;
                    }

                    // 4) Switch focus to destination tab
                    self.workspace_switch_to_tab(tab_id);
                    self.display.pending_update.dirty = true;
                    *self.dirty = true;
                    return true;
                }
                Some(crate::display::pane_drag_drop::PaneDropZone::NewTab { position: _ }) => {
                    // Move to a new tab: create tab and set its layout to the moved pane
                    let new_id = self.workspace.create_tab("New Tab".into(), None);

                    // Remove from source tab's layout (by ID from drag state)
                    if let Some(src_tab) = self.workspace.tabs.get_tab_mut(drag_state.source_tab) {
                        let mut sm = crate::workspace::SplitManager::new();
                        let _ = sm.close_pane(&mut src_tab.split_layout, drag_state.source_split);
                    }

                    // Move PaneContext across tabs (if present)
                    let _ = self.workspace.tabs.move_pane_context(
                        drag_state.source_tab,
                        new_id,
                        drag_state.source_split,
                    );

                    // Initialize new tab layout and focus
                    if let Some(new_tab) = self.workspace.tabs.get_tab_mut(new_id) {
                        new_tab.split_layout = crate::workspace::split_manager::SplitLayout::Single(
                            drag_state.source_split,
                        );
                        // Normalize (trivial here but keeps consistency)
                        let sm = crate::workspace::SplitManager::new();
                        sm.normalize(&mut new_tab.split_layout);
                        new_tab.active_pane = drag_state.source_split;
                    }

                    self.workspace_switch_to_tab(new_id);
                    self.display.pending_update.dirty = true;
                    *self.dirty = true;
                    return true;
                }
                None => {
                    // No drop target: just end drag
                    return true;
                }
            }
        }
        false
    }

    fn spawn_shell_command_in_cwd(&mut self, cmd: String, cwd: String) {
        // Use the shell to run the command in the specified working directory
        let shell_cmd = if cfg!(windows) {
            format!("cd /d {} && {}", cwd, cmd)
        } else {
            format!("cd {} && {}", cwd, cmd)
        };

        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        let shell_args =
            if cfg!(windows) { vec!["/c", &shell_cmd] } else { vec!["-c", &shell_cmd] };

        self.spawn_daemon(shell, &shell_args);
    }

    fn prompt_and_export_block_output(&mut self, text: String) {
        // For now, we'll just copy to clipboard and show a message
        // In a full implementation, this would prompt the user for a file path
        self.copy_to_clipboard(text.clone());

        // Add a message to inform the user
        let message = Message::new(
            format!("Block output copied to clipboard ({} chars)", text.len()),
            crate::message_bar::MessageType::Warning,
        );
        let _ = self.event_proxy.send_event(Event::new(EventType::Message(message), None));
    }

    // Inline AI suggestions integration
    #[cfg(feature = "ai")]
    fn inline_suggestion_visible(&self) -> bool {
        self.ai_runtime.as_ref().map(|rt| rt.ui.inline_suggestion.is_some()).unwrap_or(false)
    }

    #[cfg(feature = "ai")]
    fn accept_inline_suggestion(&mut self) {
        if let Some(rt) = &mut self.ai_runtime {
            if let Some(text) = rt.ui.inline_suggestion.take() {
                // Insert suggestion into the prompt via paste
                self.paste(&text, true);
                *self.dirty = true;
            }
        }
    }

    #[cfg(feature = "ai")]
    fn accept_inline_suggestion_word(&mut self) {
        let suggestion_data =
            if let Some(rt) = &mut self.ai_runtime { rt.ui.inline_suggestion.take() } else { None };

        if let Some(suf) = suggestion_data {
            let (accept, rest) = next_word(&suf);
            if !accept.is_empty() {
                self.paste(&accept, true);
            }
            if let Some(rt) = &mut self.ai_runtime {
                rt.ui.inline_suggestion = if rest.is_empty() { None } else { Some(rest) };
            }
            *self.dirty = true;
        }

        // Extract next word from suffix up to whitespace boundary
        fn next_word(s: &str) -> (String, String) {
            let mut iter = s.char_indices();
            let mut end = 0usize;
            for (i, ch) in &mut iter {
                end = i + ch.len_utf8();
                if ch.is_whitespace() {
                    break;
                }
            }
            // If first segment is empty (starts with whitespace), accept that whitespace first
            if s.chars().next().is_some_and(|c| c.is_whitespace()) {
                let mut j = 0usize;
                for (i, ch) in s.char_indices() {
                    j = i + ch.len_utf8();
                    if !ch.is_whitespace() {
                        break;
                    }
                }
                let (a, r) = s.split_at(j);
                return (a.to_string(), r.to_string());
            }
            let (a, r) = s.split_at(end.min(s.len()));
            (a.to_string(), r.to_string())
        }
    }

    #[cfg(feature = "ai")]
    fn accept_inline_suggestion_char(&mut self) {
        let suggestion_data =
            if let Some(rt) = &mut self.ai_runtime { rt.ui.inline_suggestion.take() } else { None };

        if let Some(mut suf) = suggestion_data {
            if let Some(first) = suf.chars().next() {
                let mut buf = [0u8; 4];
                let s = first.encode_utf8(&mut buf);
                self.paste(s, true);
                suf.drain(..first.len_utf8());
                if let Some(rt) = &mut self.ai_runtime {
                    rt.ui.inline_suggestion = if suf.is_empty() { None } else { Some(suf) };
                }
                *self.dirty = true;
            }
        }
    }

    #[cfg(feature = "ai")]
    fn dismiss_inline_suggestion(&mut self) {
        if let Some(rt) = &mut self.ai_runtime {
            if rt.ui.inline_suggestion.take().is_some() {
                *self.dirty = true;
            }
        }
    }

    #[cfg(feature = "ai")]
    fn schedule_inline_suggest(&mut self) {
        // Debounce scheduling
        if !(self.config.ai.enabled && self.config.ai.inline_suggestions) {
            return;
        }
        let window_id = self.display.window.id();
        let timer_id = TimerId::new(Topic::AiInlineTyping, window_id);
        self.scheduler.unschedule(timer_id);
        let evt = Event::new(EventType::AiInlineDebounced, window_id);
        self.scheduler.schedule(evt, AI_INLINE_SUGGEST_DEBOUNCE, false, timer_id);
    }

    #[cfg(feature = "ai")]
    fn clear_inline_suggestion(&mut self) {
        if let Some(rt) = &mut self.ai_runtime {
            if rt.ui.inline_suggestion.take().is_some() {
                *self.dirty = true;
            }
        }
    }

    // Command palette has been removed; placeholder methods were deleted.

    #[cfg(feature = "ai")]
    fn open_ai_panel(&mut self) {
        if let Some(runtime) = &mut self.ai_runtime {
            runtime.toggle_panel();
            *self.dirty = true;
        }
    }

    #[cfg(feature = "ai")]
    fn close_ai_panel(&mut self) {
        if let Some(runtime) = &mut self.ai_runtime {
            runtime.cancel();
            runtime.ui.active = false;
            *self.dirty = true;
        }
    }

    #[cfg(feature = "ai")]
    fn ai_active(&self) -> bool {
        if let Some(runtime) = &self.ai_runtime {
            runtime.ui.active
        } else {
            false
        }
    }

    #[cfg(feature = "ai")]
    fn ai_input(&mut self, c: char) {
        if let Some(runtime) = &mut self.ai_runtime {
            let mut buf = [0; 4];
            let s = c.encode_utf8(&mut buf);
            runtime.insert_text(s);
            *self.dirty = true;
        }
    }

    #[cfg(feature = "ai")]
    fn ai_backspace(&mut self) {
        if let Some(runtime) = &mut self.ai_runtime {
            runtime.backspace();
            *self.dirty = true;
        }
    }

    #[cfg(feature = "ai")]
    fn ai_propose(&mut self) {
        if let Some(runtime) = &mut self.ai_runtime {
            let proxy = self.event_proxy.clone();
            let window_id = self.display.window.id();
            // Prefer full PTY/workspace context when available (Warp parity)
            let ai_ctx = self.workspace.warp.as_ref().and_then(|w| w.get_current_ai_context());
            runtime.start_propose_stream_with_context(ai_ctx, proxy, window_id);
            *self.dirty = true;
        }
    }

    fn ai_try_handle_header_click(&mut self) -> bool {
        #[cfg(feature = "ai")]
        {
            // Precompute values requiring only &self to avoid borrow conflicts.
            let display_offset = self.terminal.grid().display_offset();
            let grid_point = self.mouse.point(&self.size_info(), display_offset);
            let vpoint = match openagent_terminal_core::term::point_to_viewport(
                display_offset,
                grid_point,
            ) {
                Some(p) => p,
                None => return false,
            };

            // Borrow AI runtime mutably only after computing vpoint.
            let runtime = match &mut self.ai_runtime {
                Some(rt) => rt,
                None => return false,
            };

            // Compute geometry; None means fully hidden
            let geom = match self.display.ai_panel_geometry(self.config, &runtime.ui) {
                Some(g) => g,
                None => return false,
            };

            // Only header line is interactive
            if vpoint.line != geom.header_line {
                return false;
            }

            // Provider selection has moved to the bottom composer; header contains only controls now.

            // Fallback: header controls region on the right if the click wasn't on a chip
            let col = vpoint.column.0;
            if col < geom.controls_col_start || col > geom.controls_col_end {
                return false;
            }

            // Controls are laid out as: [⏹][space][⟳][space][✖]
            let stop_col = geom.controls_col_start;
            let regen_col = geom.controls_col_start + 2;
            let close_col = geom.controls_col_start + 4;

            let streaming = runtime.ui.streaming_active || runtime.ui.is_loading;
            let stop_enabled = streaming;
            let regen_enabled = !streaming;

            // Close is a single-column target; Stop/Regenerate extend into their right space
            if col == close_col {
                self.close_ai_panel();
                *self.dirty = true;
                return true;
            } else if (col == regen_col || col == regen_col + 1) && regen_enabled {
                let _ = self
                    .event_proxy
                    .send_event(Event::new(EventType::AiRegenerate, self.display.window.id()));
                *self.dirty = true;
                return true;
            } else if (col == stop_col || col == stop_col + 1) && stop_enabled {
                let _ = self
                    .event_proxy
                    .send_event(Event::new(EventType::AiStop, self.display.window.id()));
                *self.dirty = true;
                return true;
            }
            false
        }
        #[cfg(not(feature = "ai"))]
        {
            false
        }
    }

    fn ai_update_hover_header(&mut self) -> bool {
        #[cfg(feature = "ai")]
        {
            // Default to no hover
            let mut hovered: Option<crate::display::ai_panel::AiHeaderControl> = None;

            // Precompute mapping requiring only &self to avoid borrow conflicts
            let display_offset = self.terminal.grid().display_offset();
            let grid_point = self.mouse.point(&self.size_info(), display_offset);
            let vpoint = match openagent_terminal_core::term::point_to_viewport(
                display_offset,
                grid_point,
            ) {
                Some(p) => p,
                None => {
                    if self.display.ai_hover_control.take().is_some() {
                        *self.dirty = true;
                    }
                    return false;
                }
            };

            let runtime = match &mut self.ai_runtime {
                Some(rt) => rt,
                None => {
                    if self.display.ai_hover_control.take().is_some() {
                        *self.dirty = true;
                    }
                    return false;
                }
            };

            let geom = match self.display.ai_panel_geometry(self.config, &runtime.ui) {
                Some(g) => g,
                None => {
                    if self.display.ai_hover_control.take().is_some() {
                        *self.dirty = true;
                    }
                    return false;
                }
            };

            // Only on header line and within controls band
            if vpoint.line == geom.header_line {
                let col = vpoint.column.0;
                if col >= geom.controls_col_start && col <= geom.controls_col_end {
                    // Controls at columns: start, start+2, start+4
                    let stop_col = geom.controls_col_start;
                    let regen_col = geom.controls_col_start + 2;
                    let close_col = geom.controls_col_start + 4;

                    let streaming = runtime.ui.streaming_active || runtime.ui.is_loading;
                    let stop_enabled = streaming;
                    let regen_enabled = !streaming;

                    // Close is a single-column target; Stop/Regenerate extend into their right
                    // space
                    if col == close_col {
                        hovered = Some(crate::display::ai_panel::AiHeaderControl::Close);
                    } else if (col == regen_col || col == regen_col + 1) && regen_enabled {
                        hovered = Some(crate::display::ai_panel::AiHeaderControl::Regenerate);
                    } else if (col == stop_col || col == stop_col + 1) && stop_enabled {
                        hovered = Some(crate::display::ai_panel::AiHeaderControl::Stop);
                    } else {
                        hovered = None;
                    }
                }
            }

            // Update hover state and damage relevant lines if changed
            if self.display.ai_hover_control != hovered {
                self.display.ai_hover_control = hovered;
                *self.dirty = true;
                let cols = self.display.size_info.columns();
                // Damage header and the actions/tooltip line beneath it
                let header = geom.header_line;
                let actions_line = header.saturating_add(1);
                self.display.damage_tracker.frame().damage_line(
                    openagent_terminal_core::term::LineDamageBounds::new(header, 0, cols),
                );
                if actions_line <= self.display.size_info.screen_lines().saturating_sub(1) {
                    self.display.damage_tracker.frame().damage_line(
                        openagent_terminal_core::term::LineDamageBounds::new(actions_line, 0, cols),
                    );
                }
            }

            hovered.is_some()
        }
        #[cfg(not(feature = "ai"))]
        {
            false
        }
    }
}

impl<'a, N: Notify + 'a, T: EventListener> ActionContext<'a, N, T> {
    fn update_search(&mut self) {
        let regex = match self.search_state.regex() {
            Some(regex) => regex,
            None => return,
        };

        // Hide cursor while typing into the search bar.
        if self.config.mouse.hide_when_typing {
            self.display.window.set_mouse_visible(false);
        }

        if regex.is_empty() {
            // Stop search if there's nothing to search for.
            self.search_reset_state();
            self.search_state.dfas = None;
        } else {
            // Create search dfas for the new regex string.
            self.search_state.dfas = RegexSearch::new(regex).ok();

            // Update search highlighting.
            self.goto_match(MAX_SEARCH_WHILE_TYPING);
        }

        *self.dirty = true;
    }

    /// Reset terminal to the state before search was started.
    fn search_reset_state(&mut self) {
        // Unschedule pending timers.
        let timer_id = TimerId::new(Topic::DelayedSearch, self.display.window.id());
        self.scheduler.unschedule(timer_id);

        // Clear focused match.
        self.search_state.focused_match = None;

        // The viewport reset logic is only needed for vi mode, since without it our origin is
        // always at the current display offset instead of at the vi cursor position which we need
        // to recover to.
        if !self.terminal.mode().contains(TermMode::VI) {
            return;
        }

        // Reset display offset and cursor position.
        self.terminal.vi_mode_cursor.point = self.search_state.origin;
        self.terminal.scroll_display(Scroll::Delta(self.search_state.display_offset_delta));
        self.search_state.display_offset_delta = 0;

        *self.dirty = true;
    }

    /// Jump to the first regex match from the search origin.
    fn goto_match(&mut self, mut limit: Option<usize>) {
        let dfas = match &mut self.search_state.dfas {
            Some(dfas) => dfas,
            None => return,
        };

        // Limit search only when enough lines are available to run into the limit.
        limit = limit.filter(|&limit| limit <= self.terminal.total_lines());

        // Jump to the next match.
        let direction = self.search_state.direction;
        let clamped_origin = self.search_state.origin.grid_clamp(self.terminal, Boundary::Grid);
        match self.terminal.search_next(dfas, clamped_origin, direction, Side::Left, limit) {
            Some(regex_match) => {
                let old_offset = self.terminal.grid().display_offset() as i32;

                if self.terminal.mode().contains(TermMode::VI) {
                    // Move vi cursor to the start of the match.
                    self.terminal.vi_goto_point(*regex_match.start());
                } else {
                    // Select the match when vi mode is not active.
                    self.terminal.scroll_to_point(*regex_match.start());
                }

                // Update the focused match.
                self.search_state.focused_match = Some(regex_match);

                // Store number of lines the viewport had to be moved.
                let display_offset = self.terminal.grid().display_offset();
                self.search_state.display_offset_delta += old_offset - display_offset as i32;

                // Since we found a result, we require no delayed re-search.
                let timer_id = TimerId::new(Topic::DelayedSearch, self.display.window.id());
                self.scheduler.unschedule(timer_id);
            }
            // Reset viewport only when we know there is no match, to prevent unnecessary jumping.
            None if limit.is_none() => self.search_reset_state(),
            None => {
                // Schedule delayed search if we ran into our search limit.
                let timer_id = TimerId::new(Topic::DelayedSearch, self.display.window.id());
                if !self.scheduler.scheduled(timer_id) {
                    let event = Event::new(EventType::SearchNext, self.display.window.id());
                    self.scheduler.schedule(event, TYPING_SEARCH_DELAY, false, timer_id);
                }

                // Clear focused match.
                self.search_state.focused_match = None;
            }
        }

        *self.dirty = true;
    }

    /// Cleanup the search state.
    fn exit_search(&mut self) {
        let vi_mode = self.terminal.mode().contains(TermMode::VI);
        self.window().set_ime_allowed(!vi_mode);

        self.display.damage_tracker.frame().mark_fully_damaged();
        self.display.pending_update.dirty = true;
        self.search_state.history_index = None;

        // Clear focused match.
        self.search_state.focused_match = None;
    }

    /// Update the cursor blinking state.
    fn update_cursor_blinking(&mut self) {
        // Get config cursor style.
        let mut cursor_style = self.config.cursor.style;
        let vi_mode = self.terminal.mode().contains(TermMode::VI);
        if vi_mode {
            cursor_style = self.config.cursor.vi_mode_style.unwrap_or(cursor_style);
        }

        // Check terminal cursor style.
        let terminal_blinking = self.terminal.cursor_style().blinking;
        let mut blinking = cursor_style.blinking_override().unwrap_or(terminal_blinking);
        blinking &= (vi_mode || self.terminal().mode().contains(TermMode::SHOW_CURSOR))
            && self.display().ime.preedit().is_none();

        // Update cursor blinking state.
        let window_id = self.display.window.id();
        self.scheduler.unschedule(TimerId::new(Topic::BlinkCursor, window_id));
        self.scheduler.unschedule(TimerId::new(Topic::BlinkTimeout, window_id));

        // Reset blinking timeout.
        *self.cursor_blink_timed_out = false;

        if blinking && self.terminal.is_focused {
            self.schedule_blinking();
            self.schedule_blinking_timeout();
        } else {
            self.display.cursor_hidden = false;
            *self.dirty = true;
        }
    }

    fn schedule_blinking(&mut self) {
        let window_id = self.display.window.id();
        let timer_id = TimerId::new(Topic::BlinkCursor, window_id);
        let event = Event::new(EventType::BlinkCursor, window_id);
        let blinking_interval = Duration::from_millis(self.config.cursor.blink_interval());
        // Coalesce any pending blink timer to avoid duplicates
        let _ = self.scheduler.unschedule(timer_id);
        self.scheduler.schedule(event, blinking_interval, true, timer_id);
    }

    fn schedule_blinking_timeout(&mut self) {
        let blinking_timeout = self.config.cursor.blink_timeout();
        if blinking_timeout == Duration::ZERO {
            return;
        }

        let window_id = self.display.window.id();
        let event = Event::new(EventType::BlinkCursorTimeout, window_id);
        let timer_id = TimerId::new(Topic::BlinkTimeout, window_id);

        self.scheduler.schedule(event, blinking_timeout, false, timer_id);
    }

    /// Perform vi mode inline search in the specified direction.
    fn inline_search(&mut self, direction: Direction) {
        let c = match self.inline_search_state.character {
            Some(c) => c,
            None => return,
        };
        let mut buf = [0; 4];
        let search_character = c.encode_utf8(&mut buf);

        // Find next match in this line.
        let vi_point = self.terminal.vi_mode_cursor.point;
        let point = match direction {
            Direction::Right => self.terminal.inline_search_right(vi_point, search_character),
            Direction::Left => self.terminal.inline_search_left(vi_point, search_character),
        };

        // Jump to point if there's a match.
        if let Ok(mut point) = point {
            if self.inline_search_state.stop_short {
                let grid = self.terminal.grid();
                point = match direction {
                    Direction::Right => {
                        grid.iter_from(point).prev().map_or(point, |cell| cell.point)
                    }
                    Direction::Left => {
                        grid.iter_from(point).next().map_or(point, |cell| cell.point)
                    }
                };
            }

            self.terminal.vi_goto_point(point);
            self.mark_dirty();
        }
    }
}

/// Identified purpose of the touch input.
#[derive(Debug, Default)]
pub enum TouchPurpose {
    #[default]
    None,
    Select(TouchEvent),
    Scroll(TouchEvent),
    Zoom(TouchZoom),
    ZoomPendingSlot(TouchEvent),
    Tap(TouchEvent),
    Invalid(HashSet<u64, RandomState>),
}

/// Touch zooming state.
#[derive(Debug)]
pub struct TouchZoom {
    slots: (TouchEvent, TouchEvent),
    fractions: f32,
}

impl TouchZoom {
    pub fn new(slots: (TouchEvent, TouchEvent)) -> Self {
        Self { slots, fractions: Default::default() }
    }

    /// Get slot distance change since last update.
    pub fn font_delta(&mut self, slot: TouchEvent) -> f32 {
        let old_distance = self.distance();

        // Update touch slots.
        if slot.id == self.slots.0.id {
            self.slots.0 = slot;
        } else {
            self.slots.1 = slot;
        }

        // Calculate font change in `FONT_SIZE_STEP` increments.
        let delta = (self.distance() - old_distance) * TOUCH_ZOOM_FACTOR + self.fractions;
        let font_delta = (delta.abs() / FONT_SIZE_STEP).floor() * FONT_SIZE_STEP * delta.signum();
        self.fractions = delta - font_delta;

        font_delta
    }

    /// Get active touch slots.
    pub fn slots(&self) -> (TouchEvent, TouchEvent) {
        self.slots
    }

    /// Calculate distance between slots.
    fn distance(&self) -> f32 {
        let delta_x = self.slots.0.location.x - self.slots.1.location.x;
        let delta_y = self.slots.0.location.y - self.slots.1.location.y;
        delta_x.hypot(delta_y) as f32
    }
}

/// State of the mouse.
#[derive(Debug)]
pub struct Mouse {
    pub left_button_state: ElementState,
    pub middle_button_state: ElementState,
    pub right_button_state: ElementState,
    pub last_click_timestamp: Instant,
    pub last_click_button: MouseButton,
    pub click_state: ClickState,
    pub accumulated_scroll: AccumulatedScroll,
    pub cell_side: Side,
    pub block_hint_launcher: bool,
    pub hint_highlight_dirty: bool,
    pub inside_text_area: bool,
    pub x: usize,
    pub y: usize,
}

impl Default for Mouse {
    fn default() -> Mouse {
        Mouse {
            last_click_timestamp: Instant::now(),
            last_click_button: MouseButton::Left,
            left_button_state: ElementState::Released,
            middle_button_state: ElementState::Released,
            right_button_state: ElementState::Released,
            click_state: ClickState::None,
            cell_side: Side::Left,
            hint_highlight_dirty: Default::default(),
            block_hint_launcher: Default::default(),
            inside_text_area: Default::default(),
            accumulated_scroll: Default::default(),
            x: Default::default(),
            y: Default::default(),
        }
    }
}

impl Mouse {
    /// Convert mouse pixel coordinates to viewport point.
    ///
    /// If the coordinates are outside of the terminal grid, like positions inside the padding, the
    /// coordinates will be clamped to the closest grid coordinates.
    #[inline]
    pub fn point(&self, size: &SizeInfo, display_offset: usize) -> Point {
        let col = self.x.saturating_sub(size.padding_x() as usize) / (size.cell_width() as usize);
        let col = min(Column(col), size.last_column());

        let line = self.y.saturating_sub(size.padding_y() as usize) / (size.cell_height() as usize);
        let line = min(line, size.bottommost_line().0 as usize);

        term::viewport_to_point(display_offset, Point::new(line, col))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ClickState {
    None,
    Click,
    DoubleClick,
    TripleClick,
}

/// The amount of scroll accumulated from the pointer events.
#[derive(Default, Debug)]
pub struct AccumulatedScroll {
    /// Scroll we should perform along `x` axis.
    pub x: f64,

    /// Scroll we should perform along `y` axis.
    pub y: f64,
}

impl input::Processor<EventProxy, ActionContext<'_, Notifier, EventProxy>> {
    /// Sanitize terminal output for safe suggestion display: strip ANSI and redact secrets.
    fn sanitize_error_output(text: &str, max_chars: usize) -> String {
        fn strip_ansi(input: &str) -> String {
            let mut out = String::with_capacity(input.len());
            let bytes = input.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == 0x1B {
                    if i + 1 < bytes.len() {
                        let next = bytes[i + 1];
                        if next == b'[' {
                            i += 2;
                            while i < bytes.len() {
                                let b = bytes[i];
                                if (0x40..=0x7E).contains(&b) {
                                    i += 1;
                                    break;
                                }
                                i += 1;
                            }
                            continue;
                        } else if next == b']' {
                            i += 2;
                            while i < bytes.len() {
                                if bytes[i] == 0x07 {
                                    i += 1;
                                    break;
                                }
                                if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'\\'
                                {
                                    i += 2;
                                    break;
                                }
                                i += 1;
                            }
                            continue;
                        }
                    }
                    i += 1;
                    continue;
                }
                out.push(bytes[i] as char);
                i += 1;
            }
            out
        }
        fn redact_line(mut line: String) -> String {
            let lower = line.to_lowercase();
            let keywords = [
                "api_key",
                "apikey",
                "token",
                "secret",
                "password",
                "passwd",
                "authorization",
                "auth",
            ];
            // Authorization: Bearer ...
            if let Some(pos) = lower.find("bearer ") {
                let cut = pos + "bearer ".len();
                if cut <= line.len() {
                    line.replace_range(cut.., "{{REDACTED}}");
                    return line;
                }
            }
            let extended = Processor::privacy_extended_flag().unwrap_or(false);
            if extended {
                if let Some(idx) = lower.find("akia") {
                    let start = idx;
                    let end = (start + 20).min(line.len());
                    line.replace_range(start..end, "{{REDACTED_AWS_KEY}}");
                    return line;
                }
                if let Some(idx) = lower.find("asia") {
                    let start = idx;
                    let end = (start + 20).min(line.len());
                    line.replace_range(start..end, "{{REDACTED_AWS_KEY}}");
                    return line;
                }
                if let Some(first_dot) = line.find('.') {
                    if line[first_dot + 1..].find('.').is_some() {
                        let mid_start = first_dot + 1;
                        line.replace_range(mid_start..line.len(), "{{REDACTED_JWT}}");
                        return line;
                    }
                }
            }
            for kw in keywords.iter() {
                if let Some(kw_pos) = lower.find(kw) {
                    let after_kw = kw_pos + kw.len();
                    let mut sep_idx: Option<usize> = None;
                    for (i, ch) in line.char_indices().skip(after_kw) {
                        if ch == ':' || ch == '=' {
                            sep_idx = Some(i);
                            break;
                        }
                    }
                    if let Some(sep) = sep_idx {
                        let mut val_start = sep + 1;
                        while val_start < line.len() {
                            if let Some(c) = line[val_start..].chars().next() {
                                if c.is_whitespace() {
                                    val_start += c.len_utf8();
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        if val_start < line.len() {
                            line.replace_range(val_start.., "{{REDACTED}}");
                            return line;
                        }
                    }
                }
            }
            line
        }
        let mut clean = strip_ansi(text);
        clean = clean.replace("\r\n", "\n");
        let mut out = String::new();
        for raw in clean.lines() {
            let red = redact_line(raw.to_string());
            if out.len() + red.len() + 1 > max_chars {
                break;
            }
            out.push_str(&red);
            out.push('\n');
        }
        if out.ends_with('\n') {
            let _ = out.pop();
        }
        out
    }

    /// Handle events from winit.
    pub fn handle_event(&mut self, event: WinitEvent<Event>) {
        match event {
            WinitEvent::UserEvent(Event { payload, .. }) => match payload {
                EventType::WorkspaceHoverFocusRequest { .. } => { /* handled via direct ActionContext call; ignore */
                }
                EventType::ComponentsInitialized(_) => (),
                EventType::SearchNext => self.ctx.goto_match(None),
                EventType::Scroll(scroll) => self.ctx.scroll(scroll),
                EventType::BlinkCursor => {
                    // Only change state when timeout isn't reached, since we could get
                    // BlinkCursor and BlinkCursorTimeout events at the same time.
                    if !*self.ctx.cursor_blink_timed_out {
                        self.ctx.display.cursor_hidden ^= true;
                        *self.ctx.dirty = true;
                    }
                }
                EventType::BlinkCursorTimeout => {
                    // Disable blinking after timeout reached.
                    let timer_id = TimerId::new(Topic::BlinkCursor, self.ctx.display.window.id());
                    self.ctx.scheduler.unschedule(timer_id);
                    *self.ctx.cursor_blink_timed_out = true;
                    self.ctx.display.cursor_hidden = false;
                    *self.ctx.dirty = true;
                }
                // Add message only if it's not already queued.
                EventType::Message(message) if !self.ctx.message_buffer.is_queued(&message) => {
                    self.ctx.message_buffer.push(message);
                    self.ctx.display.pending_update.dirty = true;
                }
                EventType::Terminal(event) => match event {
                    TerminalEvent::Title(title) => {
                        if !self.ctx.preserve_title && self.ctx.config.window.dynamic_title {
                            self.ctx.window().set_title(title);
                        }
                    }
                    TerminalEvent::CommandBlock(ev) => {
                        // Enable blocks manager on first event and update index.
                        self.ctx.display().blocks.enabled = true;
                        let total_lines = { self.ctx.terminal().grid().total_lines() };
                        // Feed structured event directly to blocks manager
                        self.ctx.display().blocks.on_event(total_lines, &ev);

                        // Track last started command for IDE error suggestions
                        if let CoreCommandBlockEvent::CommandStart { cmd } = &ev {
                            if let Some(c) = cmd.clone() {
                                *self.ctx.last_command = Some(c);
                            }
                        }

                        // Update active tab error indicator when a command ends
                        if let CoreCommandBlockEvent::CommandEnd { exit, .. } = ev {
                            let non_zero = exit.map(|c| c != 0).unwrap_or(false);
                            self.ctx.workspace_mark_active_tab_error(non_zero);

                            // IDE: surface basic suggestions for non-zero exits using command and output preview
                            if let Some(code) = exit {
                                if code != 0 {
                                    // Try to extract the output for the last block range
                                    let mut output_preview = String::new();
                                    // Extract start/end total lines of the last block in a short mutable borrow scope
                                    let (start_opt, end_opt) = {
                                        let blocks = &self.ctx.display().blocks.blocks;
                                        if let Some(last_block) = blocks.last() {
                                            (
                                                Some(last_block.start_total_line),
                                                last_block.end_total_line,
                                            )
                                        } else {
                                            (None, None)
                                        }
                                    };
                                    let grid = self.ctx.terminal().grid();
                                    if let (Some(start), Some(end)) = (start_opt, end_opt) {
                                        // Collect up to the last 200 visible lines of the block to limit cost
                                        let max_collect = 200usize;
                                        let start_collect =
                                            end.saturating_sub(max_collect).max(start);
                                        let top = grid.topmost_line();
                                        for abs in start_collect..=end {
                                            let line = top + (abs as i32);
                                            if line < grid.topmost_line()
                                                || line > grid.bottommost_line()
                                            {
                                                continue;
                                            }
                                            let row = &grid[line];
                                            let len = row.line_length().0.min(grid.columns());
                                            if len == 0 {
                                                output_preview.push('\n');
                                                continue;
                                            }
                                            for col in 0..len {
                                                let ch = row
                                                    [openagent_terminal_core::index::Column(col)]
                                                .c;
                                                output_preview.push(ch);
                                            }
                                            output_preview.push('\n');
                                        }
                                        // Trim very long previews
                                        if output_preview.len() > 4000 {
                                            output_preview.truncate(4000);
                                        }
                                    }
                                    // Produce a suggestion using the native IDE manager with the last started command
                                    let cmd = self.ctx.last_command.as_deref().unwrap_or("");
                                    // Sanitize the output preview before passing to IDE (remove ANSI, redact secrets)
                                    let sanitized =
                                        Self::sanitize_error_output(&output_preview, 4000);
                                    let suggestions =
                                        self.ctx.ide.handle_error(cmd, code, &sanitized);
                                    if let Some(s) = suggestions.first() {
                                        let msg = crate::message_bar::Message::new(
                                            format!(
                                                "Command failed (exit {}). Suggestion: {}",
                                                code, s.text
                                            ),
                                            crate::message_bar::MessageType::Warning,
                                        );
                                        let _ = self.ctx.event_proxy.send_event(Event::new(
                                            EventType::Message(msg),
                                            self.ctx.display.window.id(),
                                        ));
                                    }
                                }
                            }
                        }

                        // Broadcast plugin hooks for pre/post command and directory changes.
                        #[cfg(feature = "never")]
                        if let Some(components) = &self.ctx.components {
                            if let Some(pm) = &components.plugin_manager {
                                let pm = pm.clone();
                                let rt = components.runtime.clone();
                                let ts = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();

                                match &ev {
                                    CoreCommandBlockEvent::CommandStart { cmd } => {
                                        let cwd = std::env::current_dir()
                                            .ok()
                                            .map(|p| p.to_string_lossy().to_string());
                                        let evt = PluginEvent {
                                            event_type: "pre_command".into(),
                                            data: json!({
                                                "cmd": cmd,
                                                "cwd": cwd,
                                            }),
                                            timestamp: ts,
                                        };
                                        rt.spawn(async move {
                                            let _ = pm.broadcast_event(&evt).await;
                                        });
                                    }
                                    CoreCommandBlockEvent::CommandEnd { exit, cwd } => {
                                        // Post-command hook
                                        let evt = PluginEvent {
                                            event_type: "post_command".into(),
                                            data: json!({
                                                "exit_code": exit,
                                                "cwd": cwd,
                                            }),
                                            timestamp: ts,
                                        };
                                        let pm_clone = pm.clone();
                                        rt.spawn(async move {
                                            let _ = pm_clone.broadcast_event(&evt).await;
                                        });

                                        // Directory change notification if provided by shell integration
                                        if let Some(new_cwd) = cwd.clone() {
                                            let evt_dir = PluginEvent {
                                                event_type: "directory_change".into(),
                                                data: json!({ "cwd": new_cwd }),
                                                timestamp: ts,
                                            };
                                            let pm_clone2 = pm.clone();
                                            rt.spawn(async move {
                                                let _ = pm_clone2.broadcast_event(&evt_dir).await;
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }

                        *self.ctx.dirty = true;
                    }
                    TerminalEvent::ResetTitle => {
                        let window_config = &self.ctx.config.window;
                        if !self.ctx.preserve_title && window_config.dynamic_title {
                            self.ctx.display.window.set_title(window_config.identity.title.clone());
                        }
                    }
                    TerminalEvent::Bell => {
                        // Set window urgency hint when window is not focused.
                        let focused = self.ctx.terminal.is_focused;
                        if !focused && self.ctx.terminal.mode().contains(TermMode::URGENCY_HINTS) {
                            self.ctx.window().set_urgent(true);
                        }

                        // Ring visual bell.
                        self.ctx.display.visual_bell.ring();

                        // Execute bell command.
                        if let Some(bell_command) = &self.ctx.config.bell.command {
                            if self
                                .ctx
                                .prev_bell_cmd
                                .as_ref()
                                .map_or(true, |i| i.elapsed() >= BELL_CMD_COOLDOWN)
                            {
                                self.ctx.spawn_daemon(bell_command.program(), bell_command.args());

                                *self.ctx.prev_bell_cmd = Some(Instant::now());
                            }
                        }
                    }
                    TerminalEvent::ClipboardStore(clipboard_type, content) => {
                        if self.ctx.terminal.is_focused {
                            self.ctx.clipboard.store(clipboard_type, content);
                        }
                    }
                    TerminalEvent::ClipboardLoad(clipboard_type, format) => {
                        if self.ctx.terminal.is_focused {
                            let text = format(self.ctx.clipboard.load(clipboard_type).as_str());
                            self.ctx.write_to_pty(text.into_bytes());
                        }
                    }
                    TerminalEvent::ColorRequest(index, format) => {
                        let color = match self.ctx.terminal().colors()[index] {
                            Some(color) => Rgb(color),
                            // Ignore cursor color requests unless it was changed.
                            None if index == NamedColor::Cursor as usize => return,
                            None => self.ctx.display.colors[index],
                        };
                        self.ctx.write_to_pty(format(color.0).into_bytes());
                    }
                    TerminalEvent::TextAreaSizeRequest(format) => {
                        let text = format(self.ctx.size_info().into());
                        self.ctx.write_to_pty(text.into_bytes());
                    }
                    TerminalEvent::PtyWrite(text) => self.ctx.write_to_pty(text.into_bytes()),
                    TerminalEvent::MouseCursorDirty => self.reset_mouse_cursor(),
                    TerminalEvent::CursorBlinkingChange => self.ctx.update_cursor_blinking(),
                    TerminalEvent::Exit | TerminalEvent::ChildExit(_) | TerminalEvent::Wakeup => (),
                },
                #[cfg(unix)]
                EventType::IpcConfig(_) | EventType::IpcGetConfig(..) => (),
                #[cfg(all(unix, feature = "sync"))]
                EventType::IpcSync(..) => (),
                EventType::Message(_)
                | EventType::ConfigReload(_)
                | EventType::CreateWindow(_)
                | EventType::Frame => (),
                EventType::PasteCommand(text) => {
                    // Legacy direct paste path (may be gated in Processor before reaching here)
                    self.ctx.paste(&text, true);
                    *self.ctx.dirty = true;
                }
                EventType::PasteCommandChecked(text) => {
                    // Paste content that already passed Security Lens gating
                    self.ctx.paste(&text, true);
                    *self.ctx.dirty = true;
                }
                // Palette: append results from async file scan
                EventType::PaletteAppendFiles(paths) => {
                    // Build PaletteItems from paths on the main thread
                    let cwd = std::env::current_dir().ok();
                    let items: Vec<crate::display::palette::PaletteItem> = paths
                        .into_iter()
                        .map(|p| {
                            let path = std::path::PathBuf::from(&p);
                            let file_name = path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or(p.as_str())
                                .to_string();
                            let subtitle = cwd
                                .as_ref()
                                .and_then(|cwd| path.strip_prefix(cwd).ok())
                                .map(|rel| rel.to_string_lossy().to_string());
                            crate::display::palette::PaletteItem {
                                key: format!("file:{}", p),
                                title: format!("File: {}", file_name),
                                subtitle,
                                entry: crate::display::palette::PaletteEntry::File(p),
                            }
                        })
                        .collect();
                    self.ctx.display.palette.add_items_unique(items);
                    *self.ctx.dirty = true;
                }
                #[cfg(feature = "never")]
                EventType::PaletteAppendPluginCommands(entries) => {
                    // Convert plugin entries to palette items
                    let items: Vec<PaletteItem> = entries
                        .into_iter()
                        .map(|(plugin, command, subtitle)| PaletteItem {
                            key: format!("plugin:{}:{}", plugin, command),
                            title: format!("{}: {}", plugin, command),
                            subtitle,
                            entry: PaletteEntry::PluginCommand { plugin, command },
                        })
                        .collect();
                    self.ctx.display.palette.add_items_unique(items);
                    *self.ctx.dirty = true;
                }
                // Warp-style workspace events
                EventType::WarpUiUpdate(update_type) => {
                    match update_type {
                        crate::workspace::WarpUiUpdateType::SessionAutoSave => {
                            // Trigger session autosave when Warp is enabled and due
                            if self.ctx.config.workspace.sessions.enabled
                                && self.ctx.workspace.has_warp()
                            {
                                if let Some(warp) = &mut self.ctx.workspace.warp {
                                    if warp.should_auto_save() {
                                        let _ = warp.execute_warp_action(
                                            &crate::workspace::WarpAction::SaveSession,
                                        );
                                    }
                                }
                            }
                            *self.ctx.dirty = true;
                        }
                        // Other UI updates just mark dirty for redraw
                        _ => {
                            *self.ctx.dirty = true;
                        }
                    }
                }
                // Confirmation overlay events are handled at the window-processor level
                EventType::ConfirmOpen { .. }
                | EventType::ConfirmRespond { .. }
                | EventType::ConfirmResolved { .. } => (),
                #[cfg(feature = "never")]
                EventType::BlocksSearchPerform(_) | EventType::BlocksSearchResults(_) => (),
                #[cfg(feature = "never")]
                EventType::BlocksToggleStar(_block_id) => {
                    // Star toggling is handled at the processor level, not in input processor
                    // This event should already be processed there
                }
                #[cfg(feature = "never")]
                EventType::NotebooksOpen
                | EventType::NotebooksList(_)
                | EventType::NotebooksSelect(_)
                | EventType::NotebooksCells(_)
                | EventType::NotebooksRunCell(_)
                | EventType::NotebooksRunNotebook(_)
                | EventType::NotebooksAddCommand(_)
                | EventType::NotebooksAddMarkdown(_)
                | EventType::NotebooksDeleteCell(_)
                | EventType::NotebooksConvertCellToMarkdown(_)
                | EventType::NotebooksConvertCellToCommand(_)
                | EventType::NotebooksExportNotebook(_)
                | EventType::NotebooksEditApply { .. } => {
                    // Notebooks UI events are handled at the processor level
                    *self.ctx.dirty = true;
                }
                // Blocks quick actions
                EventType::BlocksToggleFoldUnderCursor => {
                    let display_offset = self.ctx.terminal().grid().display_offset();
                    let grid_point = self.ctx.mouse().point(&self.ctx.size_info(), display_offset);
                    if let Some(vp) =
                        openagent_terminal_core::term::point_to_viewport(display_offset, grid_point)
                    {
                        if self
                            .ctx
                            .display()
                            .blocks
                            .toggle_fold_header_at_viewport_line(display_offset, vp.line.into())
                        {
                            self.ctx.display.damage_tracker.frame().mark_fully_damaged();
                            *self.ctx.dirty = true;
                        }
                    }
                }
                EventType::BlocksCopyHeaderUnderCursor => {
                    // Copy full block output under cursor (excluding header line)
                    let display_offset = self.ctx.terminal().grid().display_offset();
                    let grid_point = self.ctx.mouse().point(&self.ctx.size_info(), display_offset);
                    if let Some(vp) =
                        openagent_terminal_core::term::point_to_viewport(display_offset, grid_point)
                    {
                        // Limit borrow to extract range
                        let (start_opt, end_opt) = {
                            let display = self.ctx.display();
                            if let Some(block) = display
                                .blocks
                                .block_at_header_viewport_line(display_offset, vp.line.into())
                            {
                                let start = block.start_total_line.saturating_add(1); // skip header
                                let end = block.end_total_line.unwrap_or_else(|| {
                                    // If still running, include until last visible line
                                    let grid = self.ctx.terminal().grid();
                                    grid.total_lines().saturating_sub(1)
                                });
                                (Some(start), Some(end))
                            } else {
                                (None, None)
                            }
                        };
                        if let (Some(start), Some(end)) = (start_opt, end_opt) {
                            let grid = self.ctx.terminal().grid();
                            let top = grid.topmost_line();
                            let mut out = String::new();
                            for abs in start..=end {
                                let line = top + (abs as i32);
                                if line < grid.topmost_line() || line > grid.bottommost_line() {
                                    continue;
                                }
                                let row = &grid[line];
                                let len = row.line_length().0.min(grid.columns());
                                if len == 0 {
                                    out.push('\n');
                                    continue;
                                }
                                for col in 0..len {
                                    let ch = row[openagent_terminal_core::index::Column(col)].c;
                                    out.push(ch);
                                }
                                out.push('\n');
                            }
                            // Trim a trailing newline for cleaner copy
                            if out.ends_with('\n') {
                                let _ = out.pop();
                            }
                            self.ctx.copy_to_clipboard(out);
                        }
                    }
                }
                EventType::BlocksExportHeaderUnderCursor => {
                    // Export full block output under cursor (excluding header line)
                    let display_offset = self.ctx.terminal().grid().display_offset();
                    let grid_point = self.ctx.mouse().point(&self.ctx.size_info(), display_offset);
                    if let Some(vp) =
                        openagent_terminal_core::term::point_to_viewport(display_offset, grid_point)
                    {
                        let (start_opt, end_opt) = {
                            let display = self.ctx.display();
                            if let Some(block) = display
                                .blocks
                                .block_at_header_viewport_line(display_offset, vp.line.into())
                            {
                                let start = block.start_total_line.saturating_add(1);
                                let end = block.end_total_line.unwrap_or_else(|| {
                                    let grid = self.ctx.terminal().grid();
                                    grid.total_lines().saturating_sub(1)
                                });
                                (Some(start), Some(end))
                            } else {
                                (None, None)
                            }
                        };
                        if let (Some(start), Some(end)) = (start_opt, end_opt) {
                            let grid = self.ctx.terminal().grid();
                            let top = grid.topmost_line();
                            let mut out = String::new();
                            for abs in start..=end {
                                let line = top + (abs as i32);
                                if line < grid.topmost_line() || line > grid.bottommost_line() {
                                    continue;
                                }
                                let row = &grid[line];
                                let len = row.line_length().0.min(grid.columns());
                                if len == 0 {
                                    out.push('\n');
                                    continue;
                                }
                                for col in 0..len {
                                    let ch = row[openagent_terminal_core::index::Column(col)].c;
                                    out.push(ch);
                                }
                                out.push('\n');
                            }
                            if out.ends_with('\n') {
                                let _ = out.pop();
                            }
                            self.ctx.prompt_and_export_block_output(out);
                        }
                    }
                }
                EventType::BlocksRerunUnderCursor => {
                    // Find the block at the header under cursor and rerun its command in its cwd
                    let display_offset = self.ctx.terminal().grid().display_offset();
                    let grid_point = self.ctx.mouse().point(&self.ctx.size_info(), display_offset);
                    if let Some(vp) =
                        openagent_terminal_core::term::point_to_viewport(display_offset, grid_point)
                    {
                        // Extract cmd/cwd under a limited borrow scope
                        let (cmd_opt, cwd_opt) = {
                            let display = self.ctx.display();
                            if let Some(block) = display
                                .blocks
                                .block_at_header_viewport_line(display_offset, vp.line.into())
                            {
                                if let Some(cmd) = block.cmd.clone() {
                                    let cwd = block.cwd.clone().unwrap_or_else(|| {
                                        std::env::current_dir()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_string()
                                    });
                                    (Some(cmd), Some(cwd))
                                } else {
                                    (None, None)
                                }
                            } else {
                                (None, None)
                            }
                        };
                        if let (Some(cmd), Some(cwd)) = (cmd_opt, cwd_opt) {
                            self.ctx.spawn_shell_command_in_cwd(cmd, cwd);
                        }
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiStreamChunk(chunk) => {
                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        let prev = runtime.ui.streaming_text.len();
                        let new = chunk.len();
                        if matches!(
                            std::env::var("OPENAGENT_AI_LOG_VERBOSITY").ok().as_deref(),
                            Some("verbose")
                        ) {
                            log::debug!(
                                "ai_event_chunk_append prev={} add={} total={}",
                                prev,
                                new,
                                prev + new
                            );
                        }
                        runtime.ui.streaming_text.push_str(&chunk);
                        // Cap in-memory streaming buffer to avoid unbounded growth in long sessions
                        let cap_bytes: usize = std::env::var("OPENAGENT_AI_STREAM_MAX_BYTES")
                            .ok()
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(512 * 1024);
                        let cur_len = runtime.ui.streaming_text.len();
                        if cur_len > cap_bytes {
                            let drop = cur_len - cap_bytes;
                            // Drop from the front to keep the most recent content visible
                            runtime.ui.streaming_text.drain(0..drop);
                        }

                        // Redraw throttling: only mark dirty if enough time elapsed since last redraw
                        let throttle_ms = std::env::var("OPENAGENT_AI_STREAM_REDRAW_MS")
                            .ok()
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(16);
                        let now = std::time::Instant::now();
                        let should_redraw = match runtime.ui.streaming_last_redraw {
                            None => true,
                            Some(last) => {
                                now.saturating_duration_since(last).as_millis() as u64
                                    >= throttle_ms
                            }
                        };
                        if should_redraw {
                            runtime.ui.streaming_last_redraw = Some(now);
                            *self.ctx.dirty = true;
                        }
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiStreamFinished => {
                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        if matches!(
                            std::env::var("OPENAGENT_AI_LOG_VERBOSITY").ok().as_deref(),
                            Some("summary") | Some("verbose")
                        ) {
                            log::info!(
                                "ai_event_stream_finished total_len={}",
                                runtime.ui.streaming_text.len()
                            );
                        }
                        runtime.ui.streaming_active = false;
                        runtime.ui.is_loading = false;
                        *self.ctx.dirty = true;
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiStreamError(err) => {
                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        log::error!("ai_event_stream_error err={}", err);
                        runtime.ui.streaming_active = false;
                        runtime.ui.is_loading = false;
                        runtime.ui.error_message = Some(err);
                        *self.ctx.dirty = true;
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiProposals(props) => {
                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        if matches!(
                            std::env::var("OPENAGENT_AI_LOG_VERBOSITY").ok().as_deref(),
                            Some("summary") | Some("verbose")
                        ) {
                            log::info!("ai_event_blocking_proposals proposals={}", props.len());
                        }
                        runtime.ui.streaming_active = false;
                        runtime.ui.is_loading = false;
                        runtime.ui.proposals = props;
                        *self.ctx.dirty = true;
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiRegenerate => {
                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        let proxy = self.ctx.event_proxy.clone();
                        let window_id = self.ctx.display.window.id();
                        runtime.regenerate(proxy, window_id);
                        *self.ctx.dirty = true;
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiStop => {
                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        runtime.cancel();
                        *self.ctx.dirty = true;
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiInsertToPrompt(text) => {
                    // Insert generated content into the shell prompt via paste
                    self.ctx.paste(&text, true);
                    *self.ctx.dirty = true;
                }
                #[cfg(feature = "ai")]
                EventType::AiApplyAsCommand { command, dry_run } => {
                    // Route through Security Lens check
                    let _ = self.ctx.event_proxy.send_event(Event::new(
                        EventType::SecurityCheckAiApply { command, dry_run },
                        self.ctx.display.window.id(),
                    ));
                    *self.ctx.dirty = true;
                }
                #[cfg(feature = "ai")]
                EventType::SecurityCheckAiApply { .. } => {
                    // Handled at Processor level to avoid duplicate prompts
                }
                #[cfg(feature = "ai")]
                EventType::AiApplyAsCommandChecked { command, dry_run } => {
                    if dry_run {
                        // Dry-run should not execute; just paste annotated content
                        self.ctx.paste(&command, true);
                    } else {
                        // Confirmed or safe; paste to prompt and execute with Enter (Warp-like Ctrl+Enter)
                        self.ctx.paste(&command, true);
                        // Send newline to execute the pasted command
                        self.ctx.write_to_pty("\n".as_bytes());
                    }
                    *self.ctx.dirty = true;
                }
                #[cfg(feature = "ai")]
                EventType::AiCopyOutput { format } => {
                    if let Some(runtime) = &self.ctx.ai_runtime {
                        if let Some(text) = runtime.copy_output(format) {
                            self.ctx.copy_to_clipboard(text);
                        }
                    }
                }
                // New AI panel events
                #[cfg(feature = "ai")]
                EventType::AiToggle => {
                    self.ctx.open_ai_panel();
                }
                #[cfg(feature = "ai")]
                EventType::AiSubmit => {
                    self.ctx.ai_propose();
                }
                #[cfg(feature = "ai")]
                EventType::AiClose => {
                    self.ctx.close_ai_panel();
                }
                #[cfg(feature = "ai")]
                EventType::AiSelectNext => {
                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        runtime.next_proposal();
                        *self.ctx.dirty = true;
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiSelectPrev => {
                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        runtime.previous_proposal();
                        *self.ctx.dirty = true;
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiApplyDryRun => {
                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        if let Some((cmd, _)) = runtime.apply_command(true) {
                            // Show dry run output in confirmation overlay
                            let id = format!(
                                "ai_dry_run_{}",
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis()
                            );
                            let _ = self.ctx.event_proxy.send_event(Event::new(
                                EventType::ConfirmOpen {
                                    id,
                                    title: "Security Lens - Dry Run".to_string(),
                                    body: cmd,
                                    confirm_label: Some("Copy".to_string()),
                                    cancel_label: Some("Close".to_string()),
                                },
                                self.ctx.display.window.id(),
                            ));
                        }
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiCopyCode => {
                    if let Some(runtime) = &self.ctx.ai_runtime {
                        if let Some(text) = runtime.copy_output(crate::event::AiCopyFormat::Code) {
                            self.ctx.copy_to_clipboard(text);
                        }
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiCopyAll => {
                    if let Some(runtime) = &self.ctx.ai_runtime {
                        if let Some(text) = runtime.copy_output(crate::event::AiCopyFormat::Text) {
                            self.ctx.copy_to_clipboard(text);
                        }
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiSwitchProvider(provider_name) => {
                    // Warp-like: hot-swap runtime provider immediately, preserve scratch/cursor.
                    let name = provider_name.to_ascii_lowercase();
                    // Resolve provider config from user config or defaults without holding mutable borrows
                    let prov_cfg = {
                        let cfg = self.ctx.config();
                        cfg.ai
                            .providers
                            .get(&name)
                            .cloned()
                            .or_else(|| {
                                crate::config::ai_providers::get_default_provider_configs()
                                    .get(&name)
                                    .cloned()
                            })
                            .unwrap_or_default()
                    };

                    if let Some(rt) = &mut self.ctx.ai_runtime {
                        // Reconfigure using precomputed config
                        rt.reconfigure_to(&name, &prov_cfg);
                        let new_provider = rt.ui.current_provider.clone();
                        let new_model = rt.ui.current_model.clone();
                        let err_opt = rt.ui.error_message.clone();
                        // Update composer display cache after mutable borrow ends
                        self.ctx.display.ai_current_provider = new_provider;
                        self.ctx.display.ai_current_model = new_model;

                        if let Some(err) = err_opt {
                            // Surface a toast/message-bar notification on failure
                            let message = crate::message_bar::Message::new(
                                format!("AI provider switch failed: {}", err),
                                crate::message_bar::MessageType::Error,
                            );
                            let _ = self.ctx.event_proxy.send_event(Event::new(
                                EventType::Message(message),
                                self.ctx.display.window.id(),
                            ));
                        } else {
                            // Persist selection to config file (best-effort)
                            if let Some(path) = self.ctx.config().config_paths.first() {
                                let mut doc: toml::Value = if path.exists() {
                                    match std::fs::read_to_string(path) {
                                        Ok(s) => toml::from_str(&s).unwrap_or_else(|_| {
                                            toml::Value::Table(toml::map::Map::new())
                                        }),
                                        Err(_) => toml::Value::Table(toml::map::Map::new()),
                                    }
                                } else {
                                    toml::Value::Table(toml::map::Map::new())
                                };
                                if !doc.is_table() {
                                    doc = toml::Value::Table(toml::map::Map::new());
                                }
                                let tbl = doc.as_table_mut().unwrap();
                                let ai = tbl
                                    .entry("ai")
                                    .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                                if !ai.is_table() {
                                    *ai = toml::Value::Table(toml::map::Map::new());
                                }
                                let ai_tbl = ai.as_table_mut().unwrap();
                                ai_tbl.insert(
                                    "provider".to_string(),
                                    toml::Value::String(name.clone()),
                                );
                                if let Some(parent) = path.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                if let Ok(s) = toml::to_string_pretty(&doc) {
                                    let _ = std::fs::write(path, s);
                                }
                            }
                        }
                    } else {
                        // If runtime wasn't initialized, just update the display; runtime will be created when AI opens
                        self.ctx.display.ai_current_provider = name.clone();
                        self.ctx.display.ai_current_model =
                            prov_cfg.default_model.unwrap_or_default();
                        // Persist selection best-effort when runtime is not initialized yet
                        if let Some(path) = self.ctx.config().config_paths.first() {
                            let mut doc: toml::Value = if path.exists() {
                                std::fs::read_to_string(path)
                                    .ok()
                                    .and_then(|s| toml::from_str(&s).ok())
                                    .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()))
                            } else {
                                toml::Value::Table(toml::map::Map::new())
                            };
                            if !doc.is_table() {
                                doc = toml::Value::Table(toml::map::Map::new());
                            }
                            let tbl = doc.as_table_mut().unwrap();
                            let ai = tbl
                                .entry("ai")
                                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                            if !ai.is_table() {
                                *ai = toml::Value::Table(toml::map::Map::new());
                            }
                            let ai_tbl = ai.as_table_mut().unwrap();
                            ai_tbl
                                .insert("provider".to_string(), toml::Value::String(name.clone()));
                            if let Some(parent) = path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Ok(s) = toml::to_string_pretty(&doc) {
                                let _ = std::fs::write(path, s);
                            }
                        }
                    }
                    *self.ctx.dirty = true;
                }
                #[cfg(feature = "ai")]
                EventType::AiInlineDebounced => {
                    // Only compute when inline suggestions are enabled and panel is not active
                    let can_offer = self.ctx.config.ai.enabled
                        && self.ctx.config.ai.inline_suggestions
                        && !self.ctx.ai_active()
                        && !self.ctx.search_active()
                        && !self.ctx.palette_active()
                        && !self.ctx.confirm_overlay_active()
                        && {
                            #[cfg(feature = "workflow")]
                            {
                                !self.ctx.workflows_panel_active()
                            }
                            #[cfg(not(feature = "workflow"))]
                            {
                                true
                            }
                        };

                    // Extract all terminal data before taking mutable borrow
                    let (not_altscreen, ime_off, prefix) = {
                        let term = self.ctx.terminal();
                        let not_altscreen = !term
                            .mode()
                            .contains(openagent_terminal_core::term::TermMode::ALT_SCREEN);
                        let ime_off = self.ctx.display.ime.preedit().is_none();

                        // Extract current line prefix up to the cursor
                        let point = term.grid().cursor.point;
                        // Collect characters from start of line to cursor (skipping spacer flags)
                        use openagent_terminal_core::index::Column as Col;
                        use openagent_terminal_core::term::cell::Flags as CellFlags;
                        let row = &term.grid()[point.line];
                        let mut prefix = String::new();
                        for x in 0..point.column.0 {
                            let cell = &row[Col(x)];
                            if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                                continue;
                            }
                            let ch = cell.c;
                            if ch != '\u{0}' {
                                prefix.push(ch);
                            }
                        }

                        (not_altscreen, ime_off, prefix)
                    };

                    if can_offer && not_altscreen && ime_off {
                        if let Some(runtime) = &mut self.ctx.ai_runtime {
                            let proxy = self.ctx.event_proxy.clone();
                            let window_id = self.ctx.display.window.id();
                            runtime.start_inline_suggest(prefix, proxy, window_id);
                            *self.ctx.dirty = true;
                        }
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiInlineSuggestionReady(suffix) => {
                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        if suffix.trim().is_empty() {
                            runtime.ui.inline_suggestion = None;
                        } else {
                            runtime.ui.inline_suggestion = Some(suffix);
                        }
                        *self.ctx.dirty = true;
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiExplain(target) => {
                    // Extract selection before mutable borrow
                    let text_to_explain = target.clone().unwrap_or_else(|| {
                        self.ctx
                            .terminal()
                            .selection_to_string()
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| "Explain the last command output".to_string())
                    });

                    // Gather AI context from the active workspace/pane if available
                    let (working_directory, shell_kind) = {
                        let ai_ctx = self
                            .ctx
                            .workspace
                            .warp
                            .as_ref()
                            .and_then(|w| w.get_current_ai_context());
                        crate::ai_context_provider::context_to_ai_params(&ai_ctx)
                    };

                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        runtime.propose_explain(text_to_explain, working_directory, shell_kind);
                        *self.ctx.dirty = true;
                    }
                }
                #[cfg(feature = "ai")]
                EventType::AiFix(target) => {
                    // Extract selection before mutable borrow
                    let error_text = target.clone().unwrap_or_else(|| {
                        self.ctx
                            .terminal()
                            .selection_to_string()
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| {
                                "Please suggest a fix for the recent error".to_string()
                            })
                    });

                    // Gather AI context from the active workspace/pane if available
                    let (working_directory, shell_kind, last_command) = {
                        if let Some(ai_ctx) = self
                            .ctx
                            .workspace
                            .warp
                            .as_ref()
                            .and_then(|w| w.get_current_ai_context())
                        {
                            let (wd, sk) = crate::ai_context_provider::context_to_ai_params(&Some(
                                ai_ctx.clone(),
                            ));
                            (wd, sk, ai_ctx.last_command)
                        } else {
                            (None, None, None)
                        }
                    };

                    if let Some(runtime) = &mut self.ctx.ai_runtime {
                        runtime.propose_fix(
                            error_text,
                            last_command,
                            working_directory,
                            shell_kind,
                        );
                        *self.ctx.dirty = true;
                    }
                }
                // Workflow panel events
                #[cfg(feature = "workflow")]
                EventType::WorkflowsSearchPerform(_) => {
                    // Workflow search is handled at the processor level
                }
                #[cfg(feature = "workflow")]
                EventType::WorkflowsSearchResults(_) => {
                    // Workflow search results are handled at the processor level
                }
                #[cfg(feature = "workflow")]
                EventType::WorkflowsExecuteByName(_) => {
                    // Workflow execution is handled at the processor level
                }
                #[cfg(feature = "workflow")]
                EventType::WorkflowsProgressUpdate { .. } => {
                    // Workflow progress updates are handled at the processor level
                    *self.ctx.dirty = true;
                }
                #[cfg(feature = "workflow")]
                EventType::WorkflowsProgressClear(_) => {
                    // Workflow progress clearing is handled at the processor level
                    *self.ctx.dirty = true;
                }
                #[cfg(feature = "workflow")]
                EventType::WorkflowsOpenParamsForm { .. }
                | EventType::WorkflowsSubmitParams { .. }
                | EventType::WorkflowsCancelParams => {
                    // Params form open/submit/cancel handled at processor level
                }
                #[cfg(feature = "never")]
                EventType::PaletteRequestPluginCommands | EventType::PluginsRunCommand { .. } => {
                    // Already handled at Processor level; ignore here
                }
                #[cfg(feature = "never")]
                EventType::PluginsSearchPerform(_)
                | EventType::PluginsSearchResults(_)
                | EventType::PluginsLoadFromPath(_)
                | EventType::PluginsUnloadByName(_)
                | EventType::PluginsInstallFromUrl { .. } => {
                    // Plugins panel events are handled in the window context processor; nothing to do here
                }
                // Notebook events (feature = "never") - stub handling
                EventType::NotebooksDeleteCell(_)
                | EventType::NotebooksConvertCellToMarkdown(_)
                | EventType::NotebooksConvertCellToCommand(_)
                | EventType::NotebooksExportNotebook(_)
                | EventType::NotebooksEditApply { .. } => {
                    // Notebook events are handled at processor level; ignore here
                }
            },
            WinitEvent::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::CloseRequested => {
                        // User asked to close the window, so no need to hold it.
                        self.ctx.window().hold = false;
                        self.ctx.terminal.exit();
                    }
                    WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                        let old_scale_factor =
                            mem::replace(&mut self.ctx.window().scale_factor, scale_factor);

                        let display_update_pending = &mut self.ctx.display.pending_update;

                        // Rescale font size for the new factor.
                        let font_scale = scale_factor as f32 / old_scale_factor as f32;
                        self.ctx.display.font_size = self.ctx.display.font_size.scale(font_scale);

                        let font = self.ctx.config.font.clone();
                        display_update_pending.set_font(font.with_size(self.ctx.display.font_size));
                    }
                    WindowEvent::Resized(size) => {
                        // Ignore resize events to zero in any dimension, to avoid issues with Winit
                        // and the ConPTY. A 0x0 resize will also occur when the window is minimized
                        // on Windows.
                        if size.width == 0 || size.height == 0 {
                            return;
                        }

                        self.ctx.display.pending_update.set_dimensions(size);
                    }
                    WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => {
                        self.key_input(event);
                    }
                    WindowEvent::ModifiersChanged(modifiers) => self.modifiers_input(modifiers),
                    WindowEvent::MouseInput { state, button, .. } => {
                        self.ctx.window().set_mouse_visible(true);
                        self.mouse_input(state, button);
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        self.ctx.window().set_mouse_visible(true);
                        self.mouse_moved(position);
                    }
                    WindowEvent::MouseWheel { delta, phase, .. } => {
                        self.ctx.window().set_mouse_visible(true);
                        self.mouse_wheel_input(delta, phase);
                    }
                    WindowEvent::Touch(touch) => self.touch(touch),
                    WindowEvent::Focused(is_focused) => {
                        self.ctx.terminal.is_focused = is_focused;

                        // Cancel any active pane drag on focus loss to avoid stuck overlays.
                        if !is_focused {
                            self.ctx.display.pane_drag_manager.cancel_drag();
                            self.ctx.display.pending_update.dirty = true;
                            *self.ctx.dirty = true;
                        }

                        // When the unfocused hollow is used we must redraw on focus change.
                        if self.ctx.config.cursor.unfocused_hollow {
                            *self.ctx.dirty = true;
                        }

                        // Reset the urgency hint when gaining focus.
                        if is_focused {
                            self.ctx.window().set_urgent(false);
                        }

                        self.ctx.update_cursor_blinking();
                        self.on_focus_change(is_focused);
                    }
                    WindowEvent::Occluded(occluded) => {
                        *self.ctx.occluded = occluded;
                    }
                    WindowEvent::DroppedFile(path) => {
                        let path: String = path.to_string_lossy().into();
                        self.ctx.paste(&(path + " "), true);
                    }
                    WindowEvent::CursorLeft { .. } => {
                        self.ctx.mouse.inside_text_area = false;

                        // Cancel any active pane drag when cursor leaves the window.
                        self.ctx.display.pane_drag_manager.cancel_drag();
                        self.ctx.display.pending_update.dirty = true;
                        *self.ctx.dirty = true;

                        if self.ctx.display().highlighted_hint.is_some() {
                            *self.ctx.dirty = true;
                        }
                    }
                    WindowEvent::Ime(ime) => match ime {
                        Ime::Commit(text) => {
                            // If composer is focused, route IME commit according to
                            // composer_open_mode
                            let theme = self
                                .ctx
                                .config
                                .resolved_theme
                                .as_ref()
                                .cloned()
                                .unwrap_or_else(|| self.ctx.config.theme.resolve());
                            let open_mode = theme.ui.composer_open_mode.clone();
                            if self.ctx.display.composer_focused && !self.ctx.palette_active() {
                                match open_mode {
                                    crate::config::theme::ComposerOpenMode::Instant => {
                                        #[cfg(feature = "ai")]
                                        {
                                            self.ctx.open_ai_panel();
                                            if let Some(runtime) = &mut self.ctx.ai_runtime {
                                                runtime.ui.scratch = text.clone();
                                                runtime.ui.cursor_position =
                                                    runtime.ui.scratch.len();
                                            }
                                        }
                                        // Reset composer state
                                        self.ctx.display.composer_text.clear();
                                        self.ctx.display.composer_cursor = 0;
                                        self.ctx.display.composer_sel_anchor = None;
                                        self.ctx.display.composer_view_col_offset = 0;
                                        self.ctx.display.composer_focused = false;
                                        *self.ctx.dirty = true;
                                        self.ctx.update_cursor_blinking();
                                        return;
                                    }
                                    crate::config::theme::ComposerOpenMode::Commit => {
                                        // Insert committed text into composer buffer at cursor
                                        let cur = self.ctx.display.composer_cursor;
                                        self.ctx.display.composer_text.insert_str(cur, &text);
                                        self.ctx.display.composer_cursor = cur + text.len();
                                        *self.ctx.dirty = true;
                                        self.ctx.update_cursor_blinking();
                                        return;
                                    }
                                }
                            }
                            *self.ctx.dirty = true;
                            // Don't use bracketed paste for single char input.
                            self.ctx.paste(&text, text.chars().count() > 1);
                            self.ctx.update_cursor_blinking();
                        }
                        Ime::Preedit(text, cursor_offset) => {
                            let preedit =
                                (!text.is_empty()).then(|| Preedit::new(text, cursor_offset));

                            if self.ctx.display.ime.preedit() != preedit.as_ref() {
                                self.ctx.display.ime.set_preedit(preedit);
                                self.ctx.update_cursor_blinking();
                                *self.ctx.dirty = true;
                            }
                        }
                        Ime::Enabled => {
                            self.ctx.display.ime.set_enabled(true);
                            *self.ctx.dirty = true;
                        }
                        Ime::Disabled => {
                            self.ctx.display.ime.set_enabled(false);
                            *self.ctx.dirty = true;
                        }
                    },
                    WindowEvent::KeyboardInput { is_synthetic: true, .. }
                    | WindowEvent::ActivationTokenDone { .. }
                    | WindowEvent::DoubleTapGesture { .. }
                    | WindowEvent::TouchpadPressure { .. }
                    | WindowEvent::RotationGesture { .. }
                    | WindowEvent::CursorEntered { .. }
                    | WindowEvent::PinchGesture { .. }
                    | WindowEvent::AxisMotion { .. }
                    | WindowEvent::PanGesture { .. }
                    | WindowEvent::HoveredFileCancelled
                    | WindowEvent::Destroyed
                    | WindowEvent::ThemeChanged(_)
                    | WindowEvent::HoveredFile(_)
                    | WindowEvent::RedrawRequested
                    | WindowEvent::Moved(_) => (),
                }
            }
            WinitEvent::Suspended
            | WinitEvent::NewEvents { .. }
            | WinitEvent::DeviceEvent { .. }
            | WinitEvent::LoopExiting
            | WinitEvent::Resumed
            | WinitEvent::MemoryWarning
            | WinitEvent::AboutToWait => (),
        }
    }
}

// End impl ApplicationHandler<Event> for Processor

#[derive(Debug, Clone)]
pub struct EventProxy {
    proxy: EventLoopProxy<Event>,
    window_id: WindowId,
}

impl EventProxy {
    pub fn new(proxy: EventLoopProxy<Event>, window_id: WindowId) -> Self {
        Self { proxy, window_id }
    }

    /// Send an event to the event loop.
    pub fn send_event(&self, event: EventType) {
        let _ = self.proxy.send_event(Event::new(event, self.window_id));
    }
}

impl EventListener for EventProxy {
    fn send_event(&self, event: TerminalEvent) {
        let _ = self.proxy.send_event(Event::new(event.into(), self.window_id));
    }
}

#[cfg(test)]
pub(crate) mod test_posted_events {
    use super::*;
    #[allow(dead_code)] // Only used in tests
    static SENT: once_cell::sync::Lazy<std::sync::Mutex<Vec<EventType>>> =
        once_cell::sync::Lazy::new(|| std::sync::Mutex::new(Vec::new()));

    #[allow(dead_code)] // Only used in tests
    pub fn record(ev: EventType) {
        let mut g = SENT.lock().unwrap();
        g.push(ev);
    }

    #[allow(dead_code)] // Only used in tests
    pub fn take() -> Vec<EventType> {
        let mut g = SENT.lock().unwrap();
        let v = g.clone();
        g.clear();
        v
    }

    #[allow(dead_code)] // Only used in tests
    pub fn clear() {
        SENT.lock().unwrap().clear();
    }
}

#[cfg(all(test, feature = "never"))]
mod test_blocks_search_cancel {
    /// Lightweight event delivery helper for tests to emulate ApplicationHandler::user_event
    /// without requiring an ActiveEventLoop.
    pub(crate) fn handle_user_event_for_test(&mut self, event: Event) {
        match (event.payload, event.window_id) {
            #[cfg(feature = "never")]
            (EventType::BlocksSearchPerform(query), Some(window_id)) => {
                self.process_blocks_search_perform(query, window_id);
            }
            _ => {}
        }
    }
}



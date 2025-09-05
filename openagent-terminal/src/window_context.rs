//! Terminal window context.

use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::mem;
#[cfg(not(windows))]
use std::os::unix::io::{AsRawFd, RawFd};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use glutin::config::Config as GlutinConfig;
use glutin::display::GetGlDisplay;
#[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
use glutin::platform::x11::X11GlConfigExt;
use log::info;
use serde_json as json;
use winit::event::{Event as WinitEvent, Modifiers, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::raw_window_handle::HasDisplayHandle;
use winit::window::WindowId;

use openagent_terminal_core::event::Event as TerminalEvent;
use openagent_terminal_core::event_loop::{Notifier, EventLoopSender};
use openagent_terminal_core::grid::{Dimensions, Scroll};
use openagent_terminal_core::index::Direction;
use openagent_terminal_core::sync::FairMutex;
use openagent_terminal_core::term::test::TermSize;
use openagent_terminal_core::term::{Term, TermMode};
use openagent_terminal_core::tty;

use crate::cli::{ParsedOptions, WindowOptions};
use crate::clipboard::Clipboard;
use crate::components_init::InitializedComponents;
use crate::config::UiConfig;
use crate::display::window::Window;
use crate::display::Display;
use crate::multiplexer::PaneManager;
use crate::event::{
    ActionContext, Event, EventProxy, InlineSearchState, Mouse, SearchState, TouchPurpose,
};
#[cfg(unix)]
use crate::logging::LOG_TARGET_IPC_CONFIG;
use crate::message_bar::MessageBuffer;
use crate::scheduler::Scheduler;
use crate::{input, renderer};

/// Adapter wrapping a PTY event loop sender to satisfy Notify + OnResize without
/// borrowing WindowContext. This avoids borrow checker conflicts when building
/// ActionContext.
pub(crate) struct NotifierAdapter {
    pub sender: EventLoopSender,
}

impl openagent_terminal_core::event::Notify for NotifierAdapter {
    fn notify<B>(&self, bytes: B)
    where
        B: Into<std::borrow::Cow<'static, [u8]>>,
    {
        use openagent_terminal_core::event_loop::Msg;
        let _ = self.sender.send(Msg::Input(bytes.into()));
    }
}

impl openagent_terminal_core::event::OnResize for NotifierAdapter {
    fn on_resize(&mut self, window_size: openagent_terminal_core::event::WindowSize) {
        use openagent_terminal_core::event_loop::Msg;
        let _ = self.sender.send(Msg::Resize(window_size));
    }
}

/// Event context for one individual OpenAgent Terminal window.
pub struct WindowContext {
    pub message_buffer: MessageBuffer,
    pub display: Display,
    pub dirty: bool,
    event_queue: Vec<winit::event::Event<Event>>,
    /// Pending confirmation overlays for tab close: id -> (tab_id, window_id)
    pending_tab_close_confirms: std::collections::HashMap<String, (crate::workspace::TabId, WindowId)>,
    /// Active pane's terminal (for compatibility with single-pane code paths)
    terminal: Arc<FairMutex<Term<EventProxy>>>,
    cursor_blink_timed_out: bool,
    prev_bell_cmd: Option<Instant>,
    modifiers: Modifiers,
    inline_search_state: InlineSearchState,
    search_state: SearchState,
    /// Deprecated single-PTY notifier; we now route per-pane notifiers from pane_manager
    /// and only use a temporary mutable reference to the active pane's notifier when needed.
    mouse: Mouse,
    touch: TouchPurpose,
    occluded: bool,
    preserve_title: bool,
    /// Multi-pane manager containing one Term+PTY per pane
    pane_manager: PaneManager,
    #[cfg(not(windows))]
    master_fd: RawFd,
    #[cfg(not(windows))]
    shell_pid: u32,
    window_config: ParsedOptions,
    config: Rc<UiConfig>,
    components: Option<Arc<InitializedComponents>>,
    /// Workspace manager for tabs and split panes
    pub workspace: crate::workspace::WorkspaceManager,
    #[cfg(feature = "ai")]
    pub ai_runtime: Option<crate::ai_runtime::AiRuntime>,
}

impl WindowContext {
    /// Create initial window context that does bootstrapping the graphics API we're going to use.
    pub fn initial(
        event_loop: &ActiveEventLoop,
        proxy: EventLoopProxy<Event>,
        config: Rc<UiConfig>,
        mut options: WindowOptions,
    ) -> Result<Self, Box<dyn Error>> {
        let raw_display_handle = event_loop.display_handle().unwrap().as_raw();

        let mut identity = config.window.identity.clone();
        options.window_identity.override_identity_config(&mut identity);

        // Optional WGPU path (experimental): prefer WGPU if requested.
        if config.debug.prefer_wgpu {
            #[cfg(feature = "wgpu")]
            {
                log::info!(
                    "prefer_wgpu is enabled; WGPU backend is not yet implemented — falling back to OpenGL"
                );
            }
            #[cfg(not(feature = "wgpu"))]
            {
                log::info!(
                    "prefer_wgpu is enabled but the 'wgpu' feature is not built; using OpenGL"
                );
            }
        }

        // Windows has different order of GL platform initialization compared to any other platform;
        // it requires the window first.
        #[cfg(windows)]
        let mut window_opt = Some(Window::new(event_loop, &config, &identity, &mut options)?);
        #[cfg(windows)]
        let mut raw_window_handle = window_opt.as_ref().map(|w| w.raw_window_handle());

        // On Windows, prefer WGPU if requested (feature-gated); we can probe here before GL display setup.
        #[cfg(all(feature = "wgpu", windows))]
        if config.debug.prefer_wgpu {
            if let Some(ref win_ref) = window_opt {
                match pollster::block_on(crate::renderer::wgpu::WgpuRenderer::new(
                    win_ref.winit_window(),
                    win_ref.inner_size(),
                    config.debug.renderer,
                    config.debug.srgb_swapchain,
                    config.debug.subpixel_text,
                    config.debug.zero_evicted_atlas_layer,
                    config.debug.atlas_eviction_policy,
                    config.debug.atlas_report_interval_frames,
                )) {
                    Ok(_) => {
                        // Move the window into the WGPU display creation.
                        let window_for_wgpu = window_opt.take().unwrap();
                        match Display::new_wgpu(window_for_wgpu, &config, false) {
                            Ok(display) => return Self::new(display, config, options, proxy),
                            Err(err) => {
                                log::info!(
                                    "WGPU initialization failed after successful probe, falling back to OpenGL: {err}"
                                );
                                // Recreate the window for OpenGL fallback since it was moved.
                                window_opt = Some(Window::new(
                                    event_loop,
                                    &config,
                                    &identity,
                                    &mut options,
                                )?);
                                raw_window_handle =
                                    window_opt.as_ref().map(|w| w.raw_window_handle());
                            },
                        }
                    },
                    Err(err) => log::info!("WGPU probe failed, using OpenGL: {:?}", err),
                }
            }
        }

        #[cfg(not(windows))]
        let raw_window_handle = None;

        let gl_display = renderer::platform::create_gl_display(
            raw_display_handle,
            raw_window_handle,
            config.debug.prefer_egl,
        )?;
        let gl_config = renderer::platform::pick_gl_config(&gl_display, raw_window_handle)?;

        #[cfg(not(windows))]
        let mut window_opt = Some(Window::new(
            event_loop,
            &config,
            &identity,
            &mut options,
            #[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
            gl_config.x11_visual(),
        )?);

        // Prefer WGPU if enabled and feature is built. We probe initialization first and then build the Display.
        #[cfg(feature = "wgpu")]
        if config.debug.prefer_wgpu {
            if let Some(ref win_ref) = window_opt {
                let wgpu_probe = pollster::block_on(crate::renderer::wgpu::WgpuRenderer::new(
                    win_ref.winit_window(),
                    win_ref.inner_size(),
                    config.debug.renderer,
                    config.debug.srgb_swapchain,
                    config.debug.subpixel_text,
                    config.debug.zero_evicted_atlas_layer,
                    config.debug.atlas_eviction_policy,
                    config.debug.atlas_report_interval_frames,
                ));
                if wgpu_probe.is_ok() {
                    let window_for_wgpu = window_opt.take().unwrap();
                    match Display::new_wgpu(window_for_wgpu, &config, false) {
                        Ok(display) => return Self::new(display, config, options, proxy),
                        Err(err) => {
                            log::info!(
                                "WGPU initialization failed after successful probe, falling back to OpenGL: {err}"
                            );
                            // Recreate the window for OpenGL fallback since it was moved into the WGPU attempt.
                            window_opt = Some(Window::new(
                                event_loop,
                                &config,
                                &identity,
                                &mut options,
                                #[cfg(all(
                                    feature = "x11",
                                    not(any(target_os = "macos", windows))
                                ))]
                                gl_config.x11_visual(),
                            )?);
                        },
                    }
                }
            }
        }

        // Create context.
        let gl_context =
            renderer::platform::create_gl_context(&gl_display, &gl_config, raw_window_handle)?;

        let window = window_opt.take().expect("window should be available for GL path");
        let display = Display::new(window, gl_context, &config, false)?;

        Self::new(display, config, options, proxy)
    }

    /// Create additional context with the graphics platform other windows are using.
    pub fn additional(
        gl_config: &GlutinConfig,
        event_loop: &ActiveEventLoop,
        proxy: EventLoopProxy<Event>,
        config: Rc<UiConfig>,
        mut options: WindowOptions,
        config_overrides: ParsedOptions,
    ) -> Result<Self, Box<dyn Error>> {
        let gl_display = gl_config.display();

        let mut identity = config.window.identity.clone();
        options.window_identity.override_identity_config(&mut identity);

        // Check if new window will be opened as a tab.
        // This must be done before `Window::new()`, which unsets `window_tabbing_id`.
        #[cfg(target_os = "macos")]
        let tabbed = options.window_tabbing_id.is_some();
        #[cfg(not(target_os = "macos"))]
        let tabbed = false;

        let window = Window::new(
            event_loop,
            &config,
            &identity,
            &mut options,
            #[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
            gl_config.x11_visual(),
        )?;

        // Create context.
        let raw_window_handle = window.raw_window_handle();
        let gl_context =
            renderer::platform::create_gl_context(&gl_display, gl_config, Some(raw_window_handle))?;

        let display = Display::new(window, gl_context, &config, tabbed)?;

        let mut window_context = Self::new(display, config, options, proxy)?;

        // Set the config overrides at startup.
        //
        // These are already applied to `config`, so no update is necessary.
        window_context.window_config = config_overrides;

        Ok(window_context)
    }

    /// Create a new terminal window context.
    fn new(
        display: Display,
        config: Rc<UiConfig>,
        options: WindowOptions,
        proxy: EventLoopProxy<Event>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut pty_config = config.pty_config();
        options.terminal_options.override_pty_config(&mut pty_config);

        let preserve_title = options.window_identity.title.is_some();

        info!(
            "PTY dimensions: {:?} x {:?}",
            display.size_info.screen_lines(),
            display.size_info.columns()
        );

        let event_proxy = EventProxy::new(proxy.clone(), display.window.id());

        // Initialize the multi-pane manager and create the initial pane (Term + PTY).
        let mut pane_manager = PaneManager::new(config.clone(), display.size_info, display.window.id(), event_proxy.clone());
        // Use working directory from options if provided
        let initial_title = config.window.identity.title.clone();
        let initial_pane_id = pane_manager
            .create_pane(options.terminal_options.working_directory.clone(), initial_title, Some(display.size_info))?;

        // Set compatibility terminal pointer to the active pane
        let terminal = pane_manager
            .get_pane(initial_pane_id)
            .expect("initial pane must exist")
            .terminal
            .clone();

        // Start cursor blinking, in case `Focused` isn't sent on startup.
        if config.cursor.style().blinking {
            event_proxy.send_event(TerminalEvent::CursorBlinkingChange.into());
        }

        // Initialize workspace manager for this window (tabs and panes)
        let session_file_path = if config.workspace.sessions.enabled {
            config
                .workspace
                .sessions
                .file_path
                .clone()
                .or(config.workspace.warp_session_file.clone())
                .or_else(|| {
                    dirs::config_dir()
                        .map(|p| p.join("openagent-terminal").join("warp-session.json"))
                })
        } else {
            None
        };

        let mut workspace = if config.workspace.warp_style {
            crate::workspace::WorkspaceManager::with_warp(
                crate::workspace::WorkspaceId(0),
                config.clone(),
                display.size_info,
                session_file_path,
            )
        } else {
            crate::workspace::WorkspaceManager::new(
                crate::workspace::WorkspaceId(0),
                config.clone(),
                display.size_info,
            )
        };

        // Initialize Warp functionality immediately (creates default tab or restores session)
        if config.workspace.warp_style {
            // Detect previous crash via session running marker
            let session_file_path = if config.workspace.sessions.enabled {
                config
                    .workspace
                    .sessions
                    .file_path
                    .clone()
                    .or(config.workspace.warp_session_file.clone())
                    .or_else(|| {
                        dirs::config_dir()
                            .map(|p| p.join("openagent-terminal").join("warp-session.json"))
                    })
            } else {
                None
            };
            let mut restore_on_startup = config.workspace.sessions.restore_on_startup;
            if let Some(ref session_path) = session_file_path {
                let marker = session_path.with_extension("running");
                if marker.exists() && session_path.exists() {
                    // Previous run did not cleanly remove the marker; prompt later instead of auto-restore
                    restore_on_startup = false;
                }
            }
            let _ = workspace.initialize_warp(
                display.window.id(),
                proxy.clone(),
                restore_on_startup,
            );
        }

        // For non-Warp mode, create an initial tab
        if !config.workspace.warp_style {
            let _initial_tab = workspace.create_tab(
                config.window.identity.title.clone(),
                options.terminal_options.working_directory.clone(),
            );
        }
        // Warp mode will create its initial tab during initialization

        // Create context for the OpenAgent Terminal window.
        let window_context = WindowContext {
            preserve_title,
            terminal,
            display,
            pending_tab_close_confirms: std::collections::HashMap::new(),
            #[cfg(not(windows))]
            master_fd: {
                let active = pane_manager
                    .get_pane(initial_pane_id)
                    .expect("initial pane must exist");
                active.master_fd
            },
            #[cfg(not(windows))]
            shell_pid: {
                let active = pane_manager
                    .get_pane(initial_pane_id)
                    .expect("initial pane must exist");
                active.shell_pid
            },
            config: config.clone(),
            components: None, // Will be initialized later by the event processor
            cursor_blink_timed_out: Default::default(),
            prev_bell_cmd: Default::default(),
            inline_search_state: Default::default(),
            message_buffer: Default::default(),
            window_config: Default::default(),
            search_state: Default::default(),
            event_queue: Default::default(),
            modifiers: Default::default(),
            occluded: Default::default(),
            mouse: Default::default(),
            touch: Default::default(),
            dirty: Default::default(),
            workspace,
            pane_manager,
            #[cfg(feature = "ai")]
            ai_runtime: {
                #[cfg(feature = "ai")]
                {
                    if config.ai.enabled {
                        // Export AI log verbosity for provider/runtime side
                        std::env::set_var(
                            "OPENAGENT_AI_LOG_VERBOSITY",
                            config.ai.log_verbosity.to_string(),
                        );
                        Some(crate::ai_runtime::AiRuntime::from_config(
                            config.ai.provider.as_deref(),
                            config.ai.endpoint_env.as_deref(),
                            config.ai.api_key_env.as_deref(),
                            config.ai.model_env.as_deref(),
                        ))
                    } else {
                        None
                    }
                }
                #[cfg(not(feature = "ai"))]
                {
                    None
                }
            },
        };

        // Note: Warp functionality will be initialized later in the event processor
        // after the WindowContext is fully set up and Arc-wrapped

        Ok(window_context)
    }

    /// Update the terminal window to the latest config.
    pub fn update_config(&mut self, new_config: Rc<UiConfig>) {
        let old_config = mem::replace(&mut self.config, new_config);

        // Apply ipc config if there are overrides.
        self.config = self.window_config.override_config_rc(self.config.clone());

        self.display.update_config(&self.config);
        self.terminal.lock().set_options(self.config.term_options());

        // Propagate reduce-motion from resolved theme into display animations (Warp-like behavior)
        let theme = self
            .config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.config.theme.resolve());
        self.display.set_reduce_motion(theme.ui.reduce_motion);

        // Reload cursor if its thickness has changed.
        if (old_config.cursor.thickness() - self.config.cursor.thickness()).abs() > f32::EPSILON {
            self.display.pending_update.set_cursor_dirty();
        }

        if old_config.font != self.config.font {
            let scale_factor = self.display.window.scale_factor as f32;
            // Do not update font size if it has been changed at runtime.
            if self.display.font_size == old_config.font.size().scale(scale_factor) {
                self.display.font_size = self.config.font.size().scale(scale_factor);
            }

            let font = self.config.font.clone().with_size(self.display.font_size);
            self.display.pending_update.set_font(font);
        }

        // Always reload the theme to account for auto-theme switching.
        self.display.window.set_theme(self.config.window.theme());

        // Update display if either padding options or resize increments were changed.
        let window_config = &old_config.window;
        if window_config.padding(1.) != self.config.window.padding(1.)
            || window_config.dynamic_padding != self.config.window.dynamic_padding
            || window_config.resize_increments != self.config.window.resize_increments
        {
            self.display.pending_update.dirty = true;
        }

        // Update title on config reload according to the following table.
        //
        // │cli │ dynamic_title │ current_title == old_config ││ set_title │
        // │ Y  │       _       │              _              ││     N     │
        // │ N  │       Y       │              Y              ││     Y     │
        // │ N  │       Y       │              N              ││     N     │
        // │ N  │       N       │              _              ││     Y     │
        if !self.preserve_title
            && (!self.config.window.dynamic_title
                || self.display.window.title() == old_config.window.identity.title)
        {
            self.display.window.set_title(self.config.window.identity.title.clone());
        }

        let opaque = self.config.window_opacity() >= 1.;

        // Disable shadows for transparent windows on macOS.
        #[cfg(target_os = "macos")]
        self.display.window.set_has_shadow(opaque);

        #[cfg(target_os = "macos")]
        self.display.window.set_option_as_alt(self.config.window.option_as_alt());

        // Change opacity and blur state.
        self.display.window.set_transparent(!opaque);
        self.display.window.set_blur(self.config.window.blur);

        // Update hint keys.
        self.display.hint_state.update_alphabet(self.config.hints.alphabet());

        // Update cursor blinking.
        let event = Event::new(TerminalEvent::CursorBlinkingChange.into(), None);
        self.event_queue.push(event.into());

        self.dirty = true;
    }

    /// Get reference to the window's configuration.
    #[cfg(unix)]
    pub fn config(&self) -> &UiConfig {
        &self.config
    }

    /// Clear the window config overrides.
    #[cfg(unix)]
    pub fn reset_window_config(&mut self, config: Rc<UiConfig>) {
        // Clear previous window errors.
        self.message_buffer.remove_target(LOG_TARGET_IPC_CONFIG);

        self.window_config.clear();

        // Reload current config to pull new IPC config.
        self.update_config(config);
    }

    /// Add new window config overrides.
    #[cfg(unix)]
    pub fn add_window_config(&mut self, config: Rc<UiConfig>, options: &ParsedOptions) {
        // Clear previous window errors.
        self.message_buffer.remove_target(LOG_TARGET_IPC_CONFIG);

        self.window_config.extend_from_slice(options);

        // Reload current config to pull new IPC config.
        self.update_config(config);
    }

    /// Draw the window.
    pub fn draw(&mut self, scheduler: &mut Scheduler) {
        self.display.window.requested_redraw = false;

        if self.occluded {
            return;
        }

        self.dirty = false;

        // Force the display to process any pending display update.
        self.display.process_renderer_update();

        // Request immediate re-draw if visual bell animation is not finished yet.
        if !self.display.visual_bell.completed() {
            // We can get an OS redraw which bypasses OpenAgent Terminal's frame throttling, thus
            // marking the window as dirty when we don't have frame yet.
            if self.display.window.has_frame {
                self.display.window.request_redraw();
            } else {
                self.dirty = true;
            }
        }

        // Redraw the window.
        // Ensure pane set matches the workspace layout (create/destroy panes as needed)
        self.sync_panes_with_layout();

        // If we have a split layout with more than one pane, draw all panes; otherwise use single-pane draw
        let draw_all = if let Some(tab) = self.workspace.active_tab() {
            crate::workspace::SplitManager::pane_count_static(&tab.split_layout) >= 2
        } else {
            false
        };


        if draw_all {
            // Compute content container (subtract reserved tab bar row if Always and reserve_row)
            let si = self.display.size_info;
            let config = &self.config;
            let mut x0 = si.padding_x();
            let mut y0 = si.padding_y();
            let mut w = si.width() - 2.0 * si.padding_x();
            let mut h = si.height() - 2.0 * si.padding_y();
            if let Some(tab) = self.workspace.active_tab() {
                if config.workspace.tab_bar.show
                    && config.workspace.tab_bar.reserve_row
                    && config.workspace.tab_bar.position
                        != crate::config::workspace::TabBarPosition::Hidden
                {
                    let is_fs = self.display.window.is_fullscreen();
                    let eff_vis = match config.workspace.tab_bar.visibility {
                        crate::config::workspace::TabBarVisibility::Always => {
                            crate::config::workspace::TabBarVisibility::Always
                        },
                        crate::config::workspace::TabBarVisibility::Hover => {
                            crate::config::workspace::TabBarVisibility::Hover
                        },
                        crate::config::workspace::TabBarVisibility::Auto => {
                            if is_fs {
                                crate::config::workspace::TabBarVisibility::Hover
                            } else {
                                crate::config::workspace::TabBarVisibility::Always
                            }
                        },
                    };
                    if matches!(eff_vis, crate::config::workspace::TabBarVisibility::Always) {
                        let ch = si.cell_height();
                        match config.workspace.tab_bar.position {
                            crate::config::workspace::TabBarPosition::Top => {
                                y0 += ch;
                                h = (h - ch).max(0.0);
                            },
                            crate::config::workspace::TabBarPosition::Bottom => {
                                h = (h - ch).max(0.0);
                            },
                            _ => {},
                        }
                    }
                }

                let container = crate::workspace::split_manager::PaneRect::new(x0, y0, w, h);
                let rects = crate::multiplexer::compute_pane_rectangles(&tab.split_layout, container);

                // Resize each pane terminal to match its pane rectangle (columns/lines)
                let cw = self.display.size_info.cell_width();
                let ch = self.display.size_info.cell_height();
                for (pid, rect) in &rects {
                    let pane_term_size = crate::display::SizeInfo::new(
                        rect.width,
                        rect.height,
                        cw,
                        ch,
                        0.0,
                        0.0,
                        false,
                    );
                    let _ = self.pane_manager.resize_pane(*pid, pane_term_size);
                }

                // Draw all panes in one frame
                let tmap = self.pane_manager.get_all_terminals();
                let focused = Some(tab.active_pane);
                #[cfg(feature = "ai")]
                let ai_state_opt = self.ai_runtime.as_ref().map(|r| &r.ui);
                self.display.draw_multipane_frame(
                    &tmap,
                    &rects,
                    focused,
                    &self.config,
                    &mut self.search_state,
                    scheduler,
                    &self.message_buffer,
                    #[cfg(feature = "ai")]
                    ai_state_opt,
                    Some(&self.workspace.tabs),
                );
            }
        } else {
            let active_terminal = self.terminal.lock();
            #[cfg(feature = "ai")]
            let ai_state_opt = self.ai_runtime.as_ref().map(|r| &r.ui);
            self.display.draw(
                active_terminal,
                scheduler,
                &self.message_buffer,
                &self.config,
                &mut self.search_state,
                #[cfg(feature = "ai")]
                ai_state_opt,
                Some(&self.workspace.tabs),
            );
        }
    }

    /// Process events for this terminal window.
    pub fn handle_event(
        &mut self,
        #[cfg(target_os = "macos")] event_loop: &ActiveEventLoop,
        event_proxy: &EventLoopProxy<Event>,
        clipboard: &mut Clipboard,
        scheduler: &mut Scheduler,
        event: WinitEvent<Event>,
    ) {
        match event {
            WinitEvent::AboutToWait
            | WinitEvent::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                // Skip further event handling with no staged updates.
                if self.event_queue.is_empty() {
                    return;
                }

                // Continue to process all pending events.
            },
            event => {
                self.event_queue.push(event);
                return;
            },
        }

        // Route events to the active pane's terminal and PTY notifier
        self.sync_panes_with_layout();
        let mut terminal = self.terminal.lock();

        let old_is_searching = self.search_state.history_index.is_some();

        // Helper to build a notifier adapter for the active pane
        let make_active_adapter = |this: &WindowContext| {
            // Determine active pane
            let active_pane_id = this
                .workspace
                .active_tab()
                .map(|t| t.active_pane)
                .expect("active tab/pane required");
            let pane = this
                .pane_manager
                .get_pane(active_pane_id)
                .expect("active pane must exist");
            let sender = pane.pty_notifier.0.clone();
            NotifierAdapter { sender }
        };

        let mut adapter = make_active_adapter(self);
        let context = ActionContext {
            cursor_blink_timed_out: &mut self.cursor_blink_timed_out,
            prev_bell_cmd: &mut self.prev_bell_cmd,
            message_buffer: &mut self.message_buffer,
            inline_search_state: &mut self.inline_search_state,
            search_state: &mut self.search_state,
            modifiers: &mut self.modifiers,
            // Use the active pane's PTY notifier via adapter
            notifier: &mut adapter,
            display: &mut self.display,
            mouse: &mut self.mouse,
            touch: &mut self.touch,
            dirty: &mut self.dirty,
            occluded: &mut self.occluded,
            terminal: &mut terminal,
            #[cfg(not(windows))]
            master_fd: self.master_fd,
            #[cfg(not(windows))]
            shell_pid: self.shell_pid,
            preserve_title: self.preserve_title,
            config: &self.config,
            event_proxy,
            #[cfg(target_os = "macos")]
            event_loop,
            clipboard,
            scheduler,
            #[cfg(feature = "ai")]
            ai_runtime: self.ai_runtime.as_mut(),
            workspace: &mut self.workspace,
            pending_tab_close_confirms: &mut self.pending_tab_close_confirms,
        };
        let mut processor = input::Processor::new(context);

        for event in self.event_queue.drain(..) {
            processor.handle_event(event);
        }

        // Process DisplayUpdate events.
        if self.display.pending_update.dirty {
            let mut adapter2 = make_active_adapter(self);
            Self::submit_display_update(
                &mut terminal,
                &mut self.display,
                &mut adapter2,
                &self.message_buffer,
                &mut self.search_state,
                old_is_searching,
                &self.config,
            );
            self.dirty = true;
        }

        if self.dirty || self.mouse.hint_highlight_dirty {
            self.dirty |= self.display.update_highlighted_hints(
                &terminal,
                &self.config,
                &self.mouse,
                self.modifiers.state(),
            );
            self.mouse.hint_highlight_dirty = false;
        }

        // Don't call `request_redraw` when event is `RedrawRequested` since the `dirty` flag
        // represents the current frame, but redraw is for the next frame.
        if self.dirty
            && self.display.window.has_frame
            && !self.occluded
            && !matches!(event, WinitEvent::WindowEvent { event: WindowEvent::RedrawRequested, .. })
        {
            self.display.window.request_redraw();
        }
    }

    /// ID of this terminal context.
    pub fn id(&self) -> WindowId {
        self.display.window.id()
    }

    /// Set the initialized components for this window context.
    pub fn set_components(&mut self, components: Arc<InitializedComponents>) {
        self.components = Some(components);
    }

    /// Initialize Warp functionality for this window context.
    /// This should be called after the window context is stored in the processor.
    #[allow(dead_code)]
    pub fn initialize_warp_if_enabled(
        &mut self,
        window_context_weak: std::sync::Weak<std::sync::Mutex<WindowContext>>,
        _event_proxy: EventLoopProxy<Event>,
    ) -> Result<(), Box<dyn Error>> {
        if self.config.workspace.warp_style {
            // Create a temporary Arc to pass to the Warp initialization
            // In a real implementation, we'd need a better approach to avoid circular references
            if let Some(_window_context_arc) = window_context_weak.upgrade() {
                // This is a simplified approach - in production code we'd need a different pattern
                // to avoid the Arc<Mutex<WindowContext>> dependency
                info!("Warp-style workspace enabled but initialization requires architectural changes");
                info!("Warp session file: {:?}", self.config.workspace.warp_session_file);
            }
        }
        Ok(())
    }

    /// Write the ref test results to the disk.
    pub fn write_ref_test_results(&self) {
        // Dump grid state.
        let mut grid = self.terminal.lock().grid().clone();
        grid.initialize_all();
        grid.truncate();

        let serialized_grid = json::to_string(&grid).expect("serialize grid");

        let size_info = &self.display.size_info;
        let size = TermSize::new(size_info.columns(), size_info.screen_lines());
        let serialized_size = json::to_string(&size).expect("serialize size");

        let serialized_config = format!("{{\"history_size\":{}}}", grid.history_size());

        File::create("./grid.json")
            .and_then(|mut f| f.write_all(serialized_grid.as_bytes()))
            .expect("write grid.json");

        File::create("./size.json")
            .and_then(|mut f| f.write_all(serialized_size.as_bytes()))
            .expect("write size.json");

        File::create("./config.json")
            .and_then(|mut f| f.write_all(serialized_config.as_bytes()))
            .expect("write config.json");
    }

    /// Submit the pending changes to the `Display`.
    fn submit_display_update(
        terminal: &mut Term<EventProxy>,
        display: &mut Display,
        pty_resize_handle: &mut dyn openagent_terminal_core::event::OnResize,
        message_buffer: &MessageBuffer,
        search_state: &mut SearchState,
        old_is_searching: bool,
        config: &UiConfig,
    ) {
        // Compute cursor positions before resize.
        let num_lines = terminal.screen_lines();
        let cursor_at_bottom = terminal.grid().cursor.point.line + 1 == num_lines;
        let origin_at_bottom = if terminal.mode().contains(TermMode::VI) {
            terminal.vi_mode_cursor.point.line == num_lines - 1
        } else {
            search_state.direction == Direction::Left
        };

        display.handle_update(terminal, pty_resize_handle, message_buffer, search_state, config);

        let new_is_searching = search_state.history_index.is_some();
        if !old_is_searching && new_is_searching {
            // Scroll on search start to make sure origin is visible with minimal viewport motion.
            let display_offset = terminal.grid().display_offset();
            if display_offset == 0 && cursor_at_bottom && !origin_at_bottom {
                terminal.scroll_display(Scroll::Delta(1));
            } else if display_offset != 0 && origin_at_bottom {
                terminal.scroll_display(Scroll::Delta(-1));
            }
        }
    }

    /// Return a mutable reference to the active pane's PTY notifier.
    fn active_pane_notifier_mut(&mut self) -> &mut Notifier {
        // Determine active pane from workspace
        let active_pane_id = self
            .workspace
            .active_tab()
            .map(|t| t.active_pane)
            .expect("active tab/pane required");
        self.pane_manager
            .get_pane_mut(active_pane_id)
            .map(|p| &mut p.pty_notifier)
            .expect("active pane must exist")
    }

    /// Ensure the pane manager matches the workspace split layout (create/destroy/focus panes).
    fn sync_panes_with_layout(&mut self) {
        // Only proceed if we have an active tab
        let Some(active_tab) = self.workspace.active_tab() else { return; };

        // Compute required pane IDs from the layout
        let required_ids = active_tab.split_layout.collect_pane_ids();

        // Add missing panes
        for pid in &required_ids {
            if !self.pane_manager.pane_ids().any(|id| id == *pid) {
                let _ = self
                    .pane_manager
                    .create_pane_with_id(*pid, Some(active_tab.working_directory.clone()), active_tab.title.clone(), None);
            }
        }

        // Remove panes that are no longer present in the layout
        let existing: Vec<_> = self.pane_manager.pane_ids().collect();
        for pid in existing {
            if !required_ids.contains(&pid) {
                self.pane_manager.remove_pane(pid);
            }
        }

        // Focus the active pane according to the tab
        let _ = self.pane_manager.focus_pane(active_tab.active_pane);

        // Update compatibility terminal pointer and OS-specific fields to the active pane
        if let Some(active) = self.pane_manager.get_pane(active_tab.active_pane) {
            self.terminal = active.terminal.clone();
            #[cfg(not(windows))]
            {
                self.master_fd = active.master_fd;
                self.shell_pid = active.shell_pid;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagent_terminal_core::event::{Notify, OnResize};

    #[test]
    fn notifier_adapter_implements_traits() {
        fn assert_traits<T: Notify + OnResize>() {}
        assert_traits::<NotifierAdapter>();
    }

    // Compile-time acceptance test: ensure Processor can be instantiated with ActionContext
    // using NotifierAdapter as the notifier type (no runtime execution).
    #[allow(dead_code)]
    fn _assert_processor_accepts_adapter<'a>(
        _p: &mut crate::input::Processor<
            crate::event::EventProxy,
            crate::event::ActionContext<'a, NotifierAdapter, crate::event::EventProxy>,
        >,
    ) {
    }
}

impl Drop for WindowContext {
    fn drop(&mut self) {
        // Ensure all panes are shutdown cleanly.
        self.pane_manager.shutdown_all();
        // Best-effort: remove running marker to indicate clean shutdown
        if self.config.workspace.sessions.enabled && self.config.workspace.warp_style {
            let session_file_path = self
                .config
                .workspace
                .sessions
                .file_path
                .clone()
                .or(self.config.workspace.warp_session_file.clone())
                .or_else(|| dirs::config_dir().map(|p| p.join("openagent-terminal").join("warp-session.json")));
            if let Some(session_path) = session_file_path {
                let marker = session_path.with_extension("running");
                let _ = std::fs::remove_file(marker);
            }
        }
    }
}

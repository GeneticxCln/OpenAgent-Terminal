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

#[cfg(feature = "gl-backend")]
use glutin::config::Config as GlutinConfig;
#[cfg(feature = "gl-backend")]
use glutin::display::GetGlDisplay;
#[cfg(all(feature = "gl-backend", feature = "x11", not(any(target_os = "macos", windows))))]
use glutin::platform::x11::X11GlConfigExt;
use log::info;
use serde_json as json;
use winit::event::{Event as WinitEvent, Modifiers, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::raw_window_handle::HasDisplayHandle;
use winit::window::WindowId;

use openagent_terminal_core::event::Event as TerminalEvent;
use openagent_terminal_core::event_loop::{EventLoop as PtyEventLoop, Msg, Notifier};
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
use crate::event::{
    ActionContext, Event, EventProxy, InlineSearchState, Mouse, SearchState, TouchPurpose,
};
use crate::input;
#[cfg(unix)]
use crate::logging::LOG_TARGET_IPC_CONFIG;
use crate::message_bar::MessageBuffer;
#[cfg(feature = "gl-backend")]
use crate::renderer;
use crate::scheduler::Scheduler;

/// Event context for one individual OpenAgent Terminal window.
pub struct WindowContext {
    pub message_buffer: MessageBuffer,
    pub display: Display,
    pub dirty: bool,
    event_queue: Vec<winit::event::Event<Event>>,
    terminal: Arc<FairMutex<Term<EventProxy>>>,
    cursor_blink_timed_out: bool,
    prev_bell_cmd: Option<Instant>,
    modifiers: Modifiers,
    inline_search_state: InlineSearchState,
    search_state: SearchState,
    notifier: Notifier,
    mouse: Mouse,
    touch: TouchPurpose,
    occluded: bool,
    preserve_title: bool,
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
        let _raw_display_handle = event_loop
            .display_handle()
            .expect("display handle not available from event loop")
            .as_raw();

        let mut identity = config.window.identity.clone();
        options.window_identity.override_identity_config(&mut identity);

        // Force WGPU-only mode, no OpenGL fallback.
        #[cfg(feature = "wgpu")]
        {
            // Prefer X11 backend on Unix if not explicitly set.
            #[cfg(all(unix, not(target_os = "macos")))]
            if std::env::var("WINIT_UNIX_BACKEND").is_err() {
                std::env::set_var("WINIT_UNIX_BACKEND", "x11");
            }

            // First attempt: WGPU
            #[cfg(windows)]
            let window_wgpu = Window::new(event_loop, &config, &identity, &mut options)?;
            #[cfg(not(windows))]
            let window_wgpu = Window::new(
                event_loop,
                &config,
                &identity,
                &mut options,
                #[cfg(all(
                    feature = "gl-backend",
                    feature = "x11",
                    not(any(target_os = "macos", windows))
                ))]
                None,
            )?;

            tracing::info!("Initializing WGPU backend…");
            let display = Display::new_wgpu(window_wgpu, &config, false)?;
            tracing::info!("Render backend selected: WGPU");
            Self::new(display, config, options, proxy)
        }

        // If the build does not include the `wgpu` feature, error clearly.
        #[cfg(not(feature = "wgpu"))]
        {
            return Err("This build requires WGPU. Rebuild with --features=wgpu".into());
        }
    }

    /// Create additional context with the graphics platform other windows are using.
    #[allow(dead_code)]
    #[cfg(feature = "gl-backend")]
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

    /// Create additional context using the WGPU backend.
    #[cfg(feature = "wgpu")]
    pub fn additional_wgpu(
        event_loop: &ActiveEventLoop,
        proxy: EventLoopProxy<Event>,
        config: Rc<UiConfig>,
        mut options: WindowOptions,
        config_overrides: ParsedOptions,
    ) -> Result<Self, Box<dyn Error>> {
        let mut identity = config.window.identity.clone();
        options.window_identity.override_identity_config(&mut identity);

        let window = {
            #[cfg(windows)]
            {
                Window::new(event_loop, &config, &identity, &mut options)?
            }
            #[cfg(not(windows))]
            {
                Window::new(
                    event_loop,
                    &config,
                    &identity,
                    &mut options,
                    #[cfg(all(
                        feature = "gl-backend",
                        feature = "x11",
                        not(any(target_os = "macos", windows))
                    ))]
                    None,
                )?
            }
        };

        let display = Display::new_wgpu(window, &config, false)?;

        let mut window_context = Self::new(display, config, options, proxy)?;
        // Apply overrides already reflected in `config`.
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

        // Create the terminal.
        //
        // This object contains all of the state about what's being displayed. It's
        // wrapped in a clonable mutex since both the I/O loop and display need to
        // access it.
        let terminal = Term::new(config.term_options(), &display.size_info, event_proxy.clone());
        let terminal = Arc::new(FairMutex::new(terminal));

        // Create the PTY.
        //
        // The PTY forks a process to run the shell on the slave side of the
        // pseudoterminal. A file descriptor for the master side is retained for
        // reading/writing to the shell.
        let pty = tty::new(&pty_config, display.size_info.into(), display.window.id().into())?;

        #[cfg(not(windows))]
        let master_fd = pty.file().as_raw_fd();
        #[cfg(not(windows))]
        let shell_pid = pty.child().id();

        // Create the pseudoterminal I/O loop.
        //
        // PTY I/O is ran on another thread as to not occupy cycles used by the
        // renderer and input processing. Note that access to the terminal state is
        // synchronized since the I/O loop updates the state, and the display
        // consumes it periodically.
        let event_loop = PtyEventLoop::new(
            Arc::clone(&terminal),
            event_proxy.clone(),
            pty,
            pty_config.drain_on_exit,
            config.debug.ref_test,
        )?;

        // The event loop channel allows write requests from the event processor
        // to be sent to the pty loop and ultimately written to the pty.
        let loop_tx = event_loop.channel();

        // Kick off the I/O thread.
        let _io_thread = event_loop.spawn();

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
            let _ = workspace.initialize_warp(
                display.window.id(),
                proxy.clone(),
                config.workspace.sessions.restore_on_startup,
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
        let mut window_context = WindowContext {
            preserve_title,
            terminal,
            display,
            #[cfg(not(windows))]
            master_fd,
            #[cfg(not(windows))]
            shell_pid,
            config: config.clone(),
            components: None, // Will be initialized later by the event processor
            notifier: Notifier(loop_tx),
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
                        let provider_name = config.ai.provider.as_deref().unwrap_or("null");
                        // Prefer user-configured provider entry; fall back to defaults
                        let prov_cfg = config
                            .ai
                            .providers
                            .get(provider_name)
                            .cloned()
                            .or_else(|| {
                                crate::config::ai_providers::get_default_provider_configs()
                                    .get(provider_name)
                                    .cloned()
                            })
                            .unwrap_or_default();
                        Some(crate::ai_runtime::AiRuntime::from_secure_config(
                            provider_name,
                            &prov_cfg,
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

        // Apply effective reduce-motion preference from config: override takes precedence over theme
        let effective_reduce_motion =
            window_context.config.reduce_motion_override.unwrap_or_else(|| {
                window_context
                    .config
                    .resolved_theme
                    .as_ref()
                    .map(|t| t.ui.reduce_motion)
                    .unwrap_or(false)
            });
        window_context.display.set_reduce_motion(effective_reduce_motion);

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

        // Apply effective reduce-motion preference from config: override takes precedence over theme
        let effective_reduce_motion = self.config.reduce_motion_override.unwrap_or_else(|| {
            self.config.resolved_theme.as_ref().map(|t| t.ui.reduce_motion).unwrap_or(false)
        });
        self.display.set_reduce_motion(effective_reduce_motion);

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

        // Rebuild AI runtime on config update to apply provider changes immediately (Warp parity)
        #[cfg(feature = "ai")]
        {
            if self.config.ai.enabled {
                // Export AI log verbosity for provider/runtime side
                std::env::set_var(
                    "OPENAGENT_AI_LOG_VERBOSITY",
                    self.config.ai.log_verbosity.to_string(),
                );

                let provider_name = self.config.ai.provider.as_deref().unwrap_or("null");
                // Prefer user-configured provider entry; fall back to defaults
                let prov_cfg = self
                    .config
                    .ai
                    .providers
                    .get(provider_name)
                    .cloned()
                    .or_else(|| {
                        crate::config::ai_providers::get_default_provider_configs()
                            .get(provider_name)
                            .cloned()
                    })
                    .unwrap_or_default();

                self.ai_runtime = Some(crate::ai_runtime::AiRuntime::from_secure_config(
                    provider_name,
                    &prov_cfg,
                ));
            } else {
                self.ai_runtime = None;
            }
        }

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

        // Update workspace animations; request another frame while animations are active
        if self.display.update_workspace_animations() && self.display.window.has_frame {
            self.display.window.request_redraw();
        }

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
        let terminal = self.terminal.lock();
        #[cfg(feature = "ai")]
        let ai_state_opt = self.ai_runtime.as_ref().map(|r| &r.ui);
        self.display.draw(
            terminal,
            scheduler,
            &self.message_buffer,
            &self.config,
            &mut self.search_state,
            #[cfg(feature = "ai")]
            ai_state_opt,
            Some(&self.workspace.tabs),
        );
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

        let mut terminal = self.terminal.lock();

        let old_is_searching = self.search_state.history_index.is_some();

        let context = ActionContext {
            cursor_blink_timed_out: &mut self.cursor_blink_timed_out,
            prev_bell_cmd: &mut self.prev_bell_cmd,
            message_buffer: &mut self.message_buffer,
            inline_search_state: &mut self.inline_search_state,
            search_state: &mut self.search_state,
            modifiers: &mut self.modifiers,
            notifier: &mut self.notifier,
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
        };
        let mut processor = input::Processor::new(context);

        for event in self.event_queue.drain(..) {
            processor.handle_event(event);
        }

        // Process DisplayUpdate events.
        if self.display.pending_update.dirty {
            Self::submit_display_update(
                &mut terminal,
                &mut self.display,
                &mut self.notifier,
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
        notifier: &mut Notifier,
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

        display.handle_update(terminal, notifier, message_buffer, search_state, config);

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
}

impl Drop for WindowContext {
    fn drop(&mut self) {
        // Shutdown the terminal's PTY.
        let _ = self.notifier.0.send(Msg::Shutdown);
    }
}

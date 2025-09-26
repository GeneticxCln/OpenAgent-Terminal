//! OpenAgent Terminal - The GPU Enhanced Terminal.

#![warn(rust_2018_idioms, future_incompatible)]
#![warn(clippy::all, clippy::if_not_else, clippy::enum_glob_use)]
#![allow(
    clippy::pedantic,
    clippy::similar_names,
    clippy::unnested_or_patterns,
    clippy::needless_raw_string_hashes,
    clippy::unreadable_literal,
    clippy::redundant_else,
    clippy::many_single_char_names
)]
// During development, keep warnings as warnings to allow feature-gated code paths without breaking
// clippy runs. With the default subsystem, 'console', windows creates an additional console
// window for the program.
// This is silently ignored on non-windows systems.
// See https://msdn.microsoft.com/en-us/library/4cc7ya5b.aspx for more details.
#![windows_subsystem = "windows"]

#[cfg(not(any(feature = "x11", feature = "wayland", target_os = "macos", windows)))]
compile_error!(r#"at least one of the "x11"/"wayland" features must be enabled"#);

use std::error::Error;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::PathBuf;
use std::{env, fs};

use tracing::info;
#[cfg(windows)]
use windows_sys::Win32::System::Console::{AttachConsole, FreeConsole, ATTACH_PARENT_PROCESS};
use winit::event_loop::EventLoop;
#[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

use openagent_terminal_core::tty;

// Re-export SerdeReplace at crate root so config derive macros can refer to `crate::SerdeReplace`.
pub use openagent_terminal_config::SerdeReplace;

mod ai_context_provider;
mod ai_runtime;
mod cli;
mod clipboard;
mod config;
mod daemon;
mod display;
mod event;
mod ide;
mod input;
#[cfg(unix)]
mod ipc;
mod logging;
mod logging_v2;
#[cfg(target_os = "macos")]
mod macos;
mod message_bar;
mod migrate;
#[cfg(windows)]
mod panic;
mod renderer;
mod scheduler;
mod string;
mod window_context;

// New component modules
mod components_init;
mod blocks_v2;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-extras"))]
mod native_input; // Native keyboard/mouse integration (experimental)
#[cfg(all(not(target_arch = "wasm32"), feature = "native-extras"))]
mod native_persistence; // Native persistence layer (experimental)
#[cfg(all(not(target_arch = "wasm32"), feature = "native-extras"))]
mod native_renderer; // Native UI rendering system (experimental)
// Re-export library native_search so crate::native_search is available in bin
pub use openagent_terminal::native_search;
#[cfg(all(not(target_arch = "wasm32"), feature = "native-extras"))]
mod shell_integration; // Native shell integration (experimental)
#[cfg(feature = "completions")]
mod completions_spec;
mod text_shaping;
mod ui_confirm;
mod utils;
mod workspace;
mod storage;

#[cfg(unix)]
use crate::cli::MessageOptions;
#[cfg(not(any(target_os = "macos", windows)))]
use crate::cli::SocketMessage;
use crate::cli::{Options, Subcommands};
use crate::config::monitor::ConfigMonitor;
use crate::config::UiConfig;
use crate::event::{Event, Processor};
#[cfg(target_os = "macos")]
use crate::macos::locale;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    panic::attach_handler();

    // When linked with the windows subsystem windows won't automatically attach
    // to the console of the parent process, so we do it explicitly. This fails
    // silently if the parent has no console.
    #[cfg(windows)]
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }

    // Load command line options.
    let options = Options::new();

    // Initialize tracing early only for CLI subcommands; the windowed app initializes logging later
    let is_cli_only = options.subcommands.is_some();
    if is_cli_only {
        let _ = crate::logging::tracing_config::initialize_tracing(
            crate::logging::tracing_config::TracingConfig::from_env(),
        );
    }

    match options.subcommands {
        #[cfg(unix)]
        Some(Subcommands::Msg(options)) => msg(options)?,
        Some(Subcommands::Migrate(options)) => migrate::migrate(options),
        None => run_openagent_terminal(options)?,
    }

    Ok(())
}


#[cfg(unix)]
fn msg(options: MessageOptions) -> Result<(), Box<dyn Error>> {
    ipc::send_message(options.socket, options.message).map_err(|err| err.into())
}

/// Temporary files stored for OpenAgent Terminal.
///
/// This stores temporary files to automate their destruction through its `Drop` implementation.
struct TemporaryFiles {
    #[cfg(unix)]
    socket_path: Option<PathBuf>,
    log_file: Option<PathBuf>,
}

impl Drop for TemporaryFiles {
    fn drop(&mut self) {
        // Clean up the IPC socket file.
        #[cfg(unix)]
        if let Some(socket_path) = &self.socket_path {
            let _ = fs::remove_file(socket_path);
        }

        // Clean up logfile.
        if let Some(log_file) = &self.log_file {
            if fs::remove_file(log_file).is_ok() {
                let _ = writeln!(io::stdout(), "Deleted log file at \"{}\"", log_file.display());
            }
        }
    }
}

/// Run main OpenAgent Terminal entrypoint.
///
/// Creates a window, the terminal state, PTY, I/O event loop, input processor,
/// config change monitor, and runs the main display loop.
fn run_openagent_terminal(mut options: Options) -> Result<(), Box<dyn Error>> {
    // Setup winit event loop.
    let window_event_loop = EventLoop::<Event>::with_user_event().build()?;

    // Initialize the tracing-based logger as soon as possible.
    let log_file = logging_v2::initialize(&options, window_event_loop.create_proxy())
        .expect("Unable to initialize tracing");

    info!("Welcome to OpenAgent Terminal");
    info!("Version {}", env!("VERSION"));

    #[cfg(all(feature = "x11", not(any(target_os = "macos", windows))))]
    info!(
        "Running on {}",
        if matches!(
            window_event_loop.display_handle().unwrap().as_raw(),
            RawDisplayHandle::Wayland(_)
        ) {
            "Wayland"
        } else {
            "X11"
        }
    );
    #[cfg(not(any(feature = "x11", target_os = "macos", windows)))]
    info!("Running on Wayland");

    // Load configuration file.
    let config = config::load(&mut options);
    log_config_path(&config);

    // Backend selection is logged at runtime by the window context initialization.
    // Log level is managed by tracing-subscriber filters

    // Set tty environment variables.
    tty::setup_env();

    // Set env vars from config.
    for (key, value) in config.env.iter() {
        env::set_var(key, value);
    }

    // Switch to home directory.
    #[cfg(target_os = "macos")]
    env::set_current_dir(home::home_dir().unwrap()).unwrap();

    // Set macOS locale.
    #[cfg(target_os = "macos")]
    locale::set_locale_environment();

    // Create the IPC socket listener.
    #[cfg(unix)]
    let socket_path = if config.ipc_socket() {
        match ipc::spawn_ipc_socket(&options, window_event_loop.create_proxy()) {
            Ok(path) => Some(path),
            Err(err) if options.daemon => return Err(err.into()),
            Err(err) => {
                log::warn!("Unable to create socket: {err:?}");
                None
            }
        }
    } else {
        None
    };

    // Setup automatic RAII cleanup for our files.
    let log_cleanup = log_file.filter(|_| !config.debug.persistent_logging);
    let _files = TemporaryFiles {
        #[cfg(unix)]
        socket_path,
        log_file: log_cleanup,
    };

    // Event processor.
    let mut processor = Processor::new(config, options, &window_event_loop);

    // Initialize components in the background
    // This will be done asynchronously during the first window creation

    // Start event loop and block until shutdown.
    let result = processor.run(window_event_loop);

    // Windows shutdown notes:
    // The historical ConPTY drop-order deadlock has been resolved in openagent-terminal-core
    // using a typestate-enforced PTY lifecycle (see
    // openagent-terminal-core/src/tty/windows/pty_lifecycle.rs). Drop order is now guaranteed
    // (ConPTY before conout), independent of outer owners.
    //
    // We still call FreeConsole() on Windows to ensure shells like cmd/powershell redraw their
    // prompt after detaching, but there is no longer a requirement to manually drop Processor
    // first.

    // Terminate the config monitor.
    if let Some(config_monitor) = processor.config_monitor.take() {
        config_monitor.shutdown();
    }

    // Without explicitly detaching the console cmd won't redraw it's prompt.
    #[cfg(windows)]
    unsafe {
        FreeConsole();
    }

    info!("Goodbye");

    result
}

fn log_config_path(config: &UiConfig) {
    if config.config_paths.is_empty() {
        return;
    }

    let mut msg = String::from("Configuration files loaded from:");
    for path in &config.config_paths {
        let _ = write!(msg, "\n  {:?}", path.display());
    }

    info!("{msg}");
}

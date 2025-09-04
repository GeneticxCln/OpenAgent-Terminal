//! Modern tracing-based logging for OpenAgent Terminal.
//!
//! This module replaces the old log-based system with structured tracing,
//! providing better observability and compatibility with the tracing ecosystem.

use std::fs::{create_dir_all, File, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::{env, process};

use log::LevelFilter;
use tracing::Subscriber;
use tracing_log::LogTracer;
use tracing_subscriber::{
    filter::EnvFilter,
fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    registry::LookupSpan,
    Layer, Registry,
};
use tracing_appender::non_blocking::WorkerGuard;
use winit::event_loop::EventLoopProxy;
use chrono::Local;

use crate::cli::Options;
use crate::logging::tracing_bridge;
use crate::event::{Event, EventType};
use crate::message_bar::{Message, MessageType};

/// Name for the environment variable containing the log file's path.
const OPENAGENT_TERMINAL_LOG_ENV: &str = "OPENAGENT_TERMINAL_LOG";

/// Initialize tracing with OpenAgent Terminal configuration
pub fn initialize(
    options: &Options,
    event_proxy: EventLoopProxy<Event>,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    initialize_with_tracing_bridge(options, event_proxy, false)
}

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// Initialize tracing with optional tracing bridge to legacy log sink
pub fn initialize_with_tracing_bridge(
    options: &Options,
    event_proxy: EventLoopProxy<Event>,
    use_tracing_bridge: bool,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    if use_tracing_bridge {
        // Initialize the tracing bridge that forwards to the legacy log sink
        return initialize_legacy_bridge(options, event_proxy);
    }
    // Initialize log bridge for any remaining log crate usage
    LogTracer::init()?;

    // Create/resolve log directory and current log file path
    let mut log_dir = env::temp_dir();
    log_dir.push("OpenAgentTerminal-logs");
    create_dir_all(&log_dir)?;

    // Current hour file path (matches tracing_appender hourly naming scheme)
    let hour_key = Local::now().format("%Y-%m-%d-%H");
    let log_path = log_dir.join(format!("openagent-terminal.log.{}", hour_key));

    // Expose log file path to the environment
    env::set_var(OPENAGENT_TERMINAL_LOG_ENV, &log_path);

    let filter = create_env_filter(options.log_level());

    // Rolling (hourly) appender + non-blocking wrapper
    let rolling = tracing_appender::rolling::hourly(&log_dir, "openagent-terminal.log");
    let (file_nb, file_guard) = tracing_appender::non_blocking(rolling);
    // Keep the guard alive for the lifetime of the process
    let _ = LOG_GUARD.set(file_guard);

    let message_bar_layer = MessageBarLayer::new(event_proxy, log_path.clone());

    // Create layered subscriber: file (non-blocking) + stdout + message bar
    let subscriber = Registry::default()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(file_nb)
                .json()
                .with_target(true)
                .with_ansi(false)
                .with_file(true)
                .with_line_number(true)
                .with_span_events(FmtSpan::CLOSE)
        )
        .with(
            fmt::layer()
                .with_writer(io::stdout)
                .with_target(true)
                .with_ansi(true)
                .compact()
        )
        .with(message_bar_layer);

    // Set global subscriber
    tracing::subscriber::set_global_default(subscriber)?;

    println!("Created log file at \"{}\"", log_path.display());
    Ok(Some(log_path))
}

/// Create environment filter based on log level
fn create_env_filter(level: LevelFilter) -> EnvFilter {
    let level_str = match level {
        LevelFilter::Off => "off",
        LevelFilter::Error => "error",
        LevelFilter::Warn => "warn", 
        LevelFilter::Info => "info",
        LevelFilter::Debug => "debug",
        LevelFilter::Trace => "trace",
    };

    // Default filter: specified level for openagent crates, WARN for others
    let default_filter = format!("openagent={},warn", level_str);
    
    EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&default_filter))
        .unwrap_or_else(|_| EnvFilter::new("warn"))
}

/// Create log file
fn create_log_file() -> io::Result<(File, PathBuf)> {
    let mut path = env::temp_dir();
    path.push(format!("OpenAgentTerminal-{}.log", process::id()));

    let file = OpenOptions::new()
        .create_new(true)
        .append(true)
        .open(&path)?;
    
    Ok((file, path))
}

/// Create AI debug log file if enabled
fn create_ai_log_file() -> io::Result<Option<File>> {
    let ai_enabled = env::var("OPENAGENT_AI_DEBUG_LOG")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);

    if !ai_enabled {
        return Ok(None);
    }

    let path = env::var("OPENAGENT_AI_DEBUG_LOG_PATH")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let mut p = env::temp_dir();
            p.push(format!("openagent_ai_debug_{}.log", process::id()));
            p
        });

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    Ok(Some(file))
}

/// Custom layer for message bar integration
struct MessageBarLayer {
    event_proxy: Arc<Mutex<EventLoopProxy<Event>>>,
    log_path: PathBuf,
}

impl MessageBarLayer {
    fn new(event_proxy: EventLoopProxy<Event>, log_path: PathBuf) -> Self {
        Self {
            event_proxy: Arc::new(Mutex::new(event_proxy)),
            log_path,
        }
    }
}

impl<S> Layer<S> for MessageBarLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        
        // Only show ERROR and WARN in message bar
        let message_type = match *metadata.level() {
            tracing::Level::ERROR => MessageType::Error,
            tracing::Level::WARN => MessageType::Warning,
            _ => return,
        };

        let event_proxy = match self.event_proxy.lock() {
            Ok(proxy) => proxy,
            Err(_) => return,
        };

        // Format the message
        let mut visitor = MessageVisitor::new();
        event.record(&mut visitor);

        #[cfg(not(windows))]
        let env_var = format!("${}", OPENAGENT_TERMINAL_LOG_ENV);
        #[cfg(windows)]
        let env_var = format!("%{}%", OPENAGENT_TERMINAL_LOG_ENV);

        let message = format!(
            "[{}] {}\nSee log at {} ({})",
            metadata.level(),
            visitor.message,
            self.log_path.display(),
            env_var,
        );

        let mut msg = Message::new(message, message_type);
        msg.set_target(metadata.target().to_owned());

        let _ = event_proxy.send_event(Event::new(EventType::Message(msg), None));
    }
}

/// Visitor to extract message from tracing event
struct MessageVisitor {
    message: String,
}

impl MessageVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
        }
    }
}

impl tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        }
    }
}

/// Logging targets that are allowed
const ALLOWED_TARGETS: &[&str] = &[
    "ipc_config",
    "config", 
    "winit_event",
    "openagent_terminal_config_derive",
    "openagent_terminal_config",
    "openagent_terminal",
    "openagent_terminal_core",
    "openagent_terminal_ai",
    "crossfont",
];

/// Check if a target should be logged (for filtering)
pub fn is_allowed_target(target: &str) -> bool {
    ALLOWED_TARGETS.iter().any(|allowed| target.starts_with(allowed))
}

/// Convenience macros for structured logging
#[macro_export]
macro_rules! log_ai_request {
    ($provider:expr, $model:expr, $prompt_len:expr) => {
        tracing::info!(
            provider = %$provider,
            model = %$model, 
            prompt_length = $prompt_len,
            event = "ai_request",
            "AI request initiated"
        );
    };
}

#[macro_export]
macro_rules! log_ai_response {
    ($provider:expr, $model:expr, $response_len:expr, $duration_ms:expr) => {
        tracing::info!(
            provider = %$provider,
            model = %$model,
            response_length = $response_len,
            duration_ms = $duration_ms,
            event = "ai_response", 
            "AI response received"
        );
    };
}

#[macro_export]
macro_rules! log_terminal_event {
    ($event_type:expr, $details:expr) => {
        tracing::debug!(
            event_type = %$event_type,
            details = %$details,
            event = "terminal_event",
            "Terminal event"
        );
    };
}

/// Initialize the legacy bridge that forwards tracing events to the custom log sink
fn initialize_legacy_bridge(
    options: &Options,
    event_proxy: EventLoopProxy<Event>,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    // First, initialize the legacy log system
    let log_file_path = crate::logging::initialize(options, event_proxy)?;
    
    // Then initialize the tracing bridge that forwards to the log system
    tracing_bridge::initialize_tracing_bridge()?;
    
    Ok(log_file_path)
}

//! Logging for OpenAgent Terminal.
//!
//! The main executable is supposed to call `initialize()` exactly once during
//! startup. All logging messages are written to stdout, given that their
//! log-level is sufficient for the level configured in `cli::Options`.

pub mod tracing_bridge;

use std::fs::{File, OpenOptions};
use std::io::{self, LineWriter, Stdout, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use std::{env, process};

use log::{Level, LevelFilter};
use regex::Regex;
use winit::event_loop::EventLoopProxy;

use crate::cli::Options;
use crate::event::{Event, EventType};
use crate::message_bar::{Message, MessageType};

/// Logging target for IPC config error messages.
pub const LOG_TARGET_IPC_CONFIG: &str = "ipc_config";

/// Name for the environment variable containing the log file's path.
const OPENAGENT_TERMINAL_LOG_ENV: &str = "OPENAGENT_TERMINAL_LOG";

/// Logging target for config error messages.
pub const LOG_TARGET_CONFIG: &str = "config";

/// Logging target for winit events.
pub const LOG_TARGET_WINIT: &str = "winit_event";

/// Name for the environment variable containing extra logging targets.
///
/// The targets are semicolon separated.
const OPENAGENT_TERMINAL_EXTRA_LOG_TARGETS_ENV: &str = "OPENAGENT_TERMINAL_EXTRA_LOG_TARGETS";

/// User configurable extra log targets to include.
fn extra_log_targets() -> &'static [String] {
    static EXTRA_LOG_TARGETS: OnceLock<Vec<String>> = OnceLock::new();

    EXTRA_LOG_TARGETS.get_or_init(|| {
        env::var(OPENAGENT_TERMINAL_EXTRA_LOG_TARGETS_ENV).map_or(Vec::new(), |targets| {
            targets.split(';').map(ToString::to_string).collect()
        })
    })
}

/// List of targets which will be logged by OpenAgent Terminal.
const ALLOWED_TARGETS: &[&str] = &[
    LOG_TARGET_IPC_CONFIG,
    LOG_TARGET_CONFIG,
    LOG_TARGET_WINIT,
    // Rebranded crate names
    "openagent_terminal_config_derive",
    "openagent_terminal_config",
    "openagent_terminal",
    "openagent_terminal_core",
    "openagent_terminal_ai",
    "crossfont",
];

/// Initialize the logger to its defaults.
pub fn initialize(
    options: &Options,
    event_proxy: EventLoopProxy<Event>,
) -> Result<Option<PathBuf>, log::SetLoggerError> {
    log::set_max_level(options.log_level());

    let logger = Logger::new(event_proxy);
    let path = logger.file_path();
    log::set_boxed_logger(Box::new(logger))?;

    Ok(path)
}

pub struct Logger {
    logfile: Mutex<OnDemandLogFile>,
    stdout: Mutex<LineWriter<Stdout>>,
    event_proxy: Mutex<EventLoopProxy<Event>>,
    start: Instant,
    // Optional AI debug logfile (env-controlled)
    ai_log_enabled: bool,
    ai_logfile: Option<Mutex<OnDemandLogFile>>,
}

impl Logger {
    fn new(event_proxy: EventLoopProxy<Event>) -> Self {
        let logfile = Mutex::new(OnDemandLogFile::new());
        let stdout = Mutex::new(LineWriter::new(io::stdout()));

        // AI debug log is opt-in via env var OPENAGENT_AI_DEBUG_LOG
        let ai_enabled = env::var("OPENAGENT_AI_DEBUG_LOG")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
        let ai_path = env::var("OPENAGENT_AI_DEBUG_LOG_PATH")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                if ai_enabled {
                    let mut p = env::temp_dir();
                    p.push(format!("openagent_ai_debug_{}.log", process::id()));
                    Some(p)
                } else {
                    None
                }
            });
        let ai_logfile = ai_path.map(|p| Mutex::new(OnDemandLogFile::with_path(p)));

        Logger {
            logfile,
            stdout,
            event_proxy: Mutex::new(event_proxy),
            start: Instant::now(),
            ai_log_enabled: ai_enabled,
            ai_logfile,
        }
    }

    fn file_path(&self) -> Option<PathBuf> {
        let logfile_lock = self.logfile.lock().ok()?;
        Some(logfile_lock.path().clone())
    }

    /// Log a record to the message bar.
    fn message_bar_log(&self, record: &log::Record<'_>, logfile_path: &str) {
        let message_type = match record.level() {
            Level::Error => MessageType::Error,
            Level::Warn => MessageType::Warning,
            _ => return,
        };

        let event_proxy = match self.event_proxy.lock() {
            Ok(event_proxy) => event_proxy,
            Err(_) => return,
        };

        #[cfg(not(windows))]
        let env_var = format!("${OPENAGENT_TERMINAL_LOG_ENV}");
        #[cfg(windows)]
        let env_var = format!("%{}%", OPENAGENT_TERMINAL_LOG_ENV);

        let message = format!(
            "[{}] {}\nSee log at {} ({})",
            record.level(),
            record.args(),
            logfile_path,
            env_var,
        );

        let mut message = Message::new(message, message_type);
        message.set_target(record.target().to_owned());

        let _ = event_proxy.send_event(Event::new(EventType::Message(message), None));
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record<'_>) {
        // Get target crate.
        let index = record
            .target()
            .find(':')
            .unwrap_or_else(|| record.target().len());
        let target = &record.target()[..index];

        // Only log our own crates, except when logging at Level::Trace.
        if !self.enabled(record.metadata()) || !is_allowed_target(record.level(), target) {
            return;
        }

        // Create log message for the given `record` and `target`.
        let message = create_log_message(record, target, self.start);

        if let Ok(mut logfile) = self.logfile.lock() {
            // Write to logfile.
            let _ = logfile.write_all(message.as_ref());

            // Log relevant entries to message bar.
            self.message_bar_log(record, &logfile.path.to_string_lossy());
        }

        // Write to stdout.
        if let Ok(mut stdout) = self.stdout.lock() {
            let _ = stdout.write_all(message.as_ref());
        }

        // AI debug log path: record only AI-related targets with sensitive-field redaction.
        if self.ai_log_enabled {
            let is_ai_target = target.starts_with("openagent_terminal_ai")
                || target.starts_with("openagent_terminal::ai_runtime")
                || record.args().to_string().to_lowercase().contains("ai");
            if is_ai_target {
                if let Some(ref ai_file_mutex) = self.ai_logfile {
                    if let Ok(mut ai_file) = ai_file_mutex.lock() {
                        let mut redacted = redact_sensitive_text(&message);
                        // Ensure it ends with newline.
                        if !redacted.ends_with('\n') {
                            redacted.push('\n');
                        }
                        let _ = ai_file.write_all(redacted.as_bytes());
                    }
                }
            }
        }
    }

    fn flush(&self) {}
}

fn create_log_message(record: &log::Record<'_>, target: &str, start: Instant) -> String {
    let runtime = start.elapsed();
    let secs = runtime.as_secs();
    let nanos = runtime.subsec_nanos();
    let mut message = format!(
        "[{}.{:0>9}s] [{:<5}] [{}] ",
        secs,
        nanos,
        record.level(),
        target
    );

    // Alignment for the lines after the first new line character in the payload. We don't deal
    // with fullwidth/unicode chars here, so just `message.len()` is sufficient.
    let alignment = message.len();

    // Push lines with added extra padding on the next line, which is trimmed later.
    let lines = record.args().to_string();
    let redacted_lines = redact_sensitive_text(&lines);
    for line in redacted_lines.split('\n') {
        let line = format!("{}\n{:width$}", line, "", width = alignment);
        message.push_str(&line);
    }

    // Drop extra trailing alignment.
    message.truncate(message.len() - alignment);
    message
}

/// Check if log messages from a crate should be logged.
fn is_allowed_target(level: Level, target: &str) -> bool {
    match (level, log::max_level()) {
        (Level::Error, LevelFilter::Trace) | (Level::Warn, LevelFilter::Trace) => true,
        _ => ALLOWED_TARGETS.contains(&target) || extra_log_targets().iter().any(|t| t == target),
    }
}

struct OnDemandLogFile {
    file: Option<LineWriter<File>>,
    created: Arc<AtomicBool>,
    path: PathBuf,
}

impl OnDemandLogFile {
    fn new() -> Self {
        let mut path = env::temp_dir();
        path.push(format!("OpenAgentTerminal-{}.log", process::id()));

        // Set log path as an environment variable.
        unsafe { env::set_var(OPENAGENT_TERMINAL_LOG_ENV, path.as_os_str()) };

        OnDemandLogFile {
            path,
            file: None,
            created: Arc::new(AtomicBool::new(false)),
        }
    }

    fn with_path(path: PathBuf) -> Self {
        OnDemandLogFile {
            path,
            file: None,
            created: Arc::new(AtomicBool::new(false)),
        }
    }

    fn file(&mut self) -> Result<&mut LineWriter<File>, io::Error> {
        // Allow to recreate the file if it has been deleted at runtime.
        if self.file.is_some() && !self.path.as_path().exists() {
            self.file = None;
        }

        // Create the file if it doesn't exist yet.
        if self.file.is_none() {
            let file = OpenOptions::new()
                .append(true)
                .create_new(true)
                .open(&self.path);

            match file {
                Ok(file) => {
                    self.file = Some(io::LineWriter::new(file));
                    self.created.store(true, Ordering::Relaxed);
                    let _ = writeln!(
                        io::stdout(),
                        "Created log file at \"{}\"",
                        self.path.display()
                    );
                }
                Err(e) => {
                    let _ = writeln!(io::stdout(), "Unable to create log file: {e}");
                    return Err(e);
                }
            }
        }

        Ok(self.file.as_mut().unwrap())
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Write for OnDemandLogFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.file()?.write(buf)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.file()?.flush()
    }
}

fn redact_sensitive_text(input: &str) -> String {
    // Redact common secret patterns; deliberately conservative to avoid false positives.
    // Covers JWTs, SSH private keys, AWS/GitHub/GCP tokens, and CLI flags with secrets.
    let patterns: &[(&str, &str)] = &[
        // Generic key-value
        (r#"(?i)(api[_-]?key)\s*[:=]\s*[^\s,\"']+"#, "$1=[REDACTED]"),
        (
            r#"(?i)(authorization)\s*:\s*bearer\s+[^\s]+"#,
            "$1: Bearer [REDACTED]",
        ),
        (r#"(?i)(token)\s*[:=]\s*[^\s,\"']+"#, "$1=[REDACTED]"),
        (r#"(?i)(secret)\s*[:=]\s*[^\s,\"']+"#, "$1=[REDACTED]"),
        (r#"(?i)(password)\s*[:=]\s*[^\s,\"']+"#, "$1=[REDACTED]"),
        (r#"(?i)\bbearer\s+[^\s]+"#, "Bearer [REDACTED]"),
        // JSON-like fields
        (
            r#"(?i)("(?:api[_-]?key|token|secret|password|authorization)")\s*:\s*"[^"]*""#,
            r#"$1: "[REDACTED]""#,
        ),
        // CLI flags with secrets
        (
            r#"(?i)--(password|token|key|secret|auth|api[_-]?key)\s*=?\s*([\"']?)([^\s\"']+)\2"#,
            "--$1=[REDACTED]",
        ),
        (r#"(?i)-(p|P)\s+([^\s]+)"#, "-$1 [REDACTED]"),
        // JWT tokens
        (
            r#"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b"#,
            "[REDACTED_JWT]",
        ),
        // AWS credentials
        (r#"\bAKIA[0-9A-Z]{16}\b"#, "[REDACTED_AWS_ACCESS_KEY]"),
        (r#"\b([A-Za-z0-9/+=]{40})\b"#, "[REDACTED_AWS_SECRET_KEY]"),
        (
            r#"aws_access_key_id\s*=\s*([^\s]+)"#,
            "aws_access_key_id = [REDACTED]",
        ),
        (
            r#"aws_secret_access_key\s*=\s*([^\s]+)"#,
            "aws_secret_access_key = [REDACTED]",
        ),
        // GitHub tokens
        (
            r#"\bgh[pousr]_[A-Za-z0-9]{36}\b"#,
            "[REDACTED_GITHUB_TOKEN]",
        ),
        // GCP service account keys
        (
            r#"\"private_key\"\s*:\s*\"-----BEGIN [^\"]+-----[\s\S]+?-----END [^\"]+-----\""#,
            r#""private_key": "[REDACTED_PRIVATE_KEY]""#,
        ),
        (
            r#"\"client_email\"\s*:\s*\"[^@]+@[^.]+\.iam\.gserviceaccount\.com\""#,
            r#""client_email": "[REDACTED_SERVICE_ACCOUNT]""#,
        ),
        // SSH private keys
        (
            r#"-----BEGIN (RSA |DSA |EC |OPENSSH )?PRIVATE KEY-----[\s\S]+?-----END (RSA |DSA |EC |OPENSSH )?PRIVATE KEY-----"#,
            "[REDACTED_SSH_PRIVATE_KEY]",
        ),
        // DB connection strings
        (
            r#"(mongodb|postgres|postgresql|mysql|redis|amqp)://[^:]+:([^@]+)@"#,
            "$1://[USER]:[REDACTED]@",
        ),
        // Slack tokens
        (
            r#"xox[baprs]-[0-9]{10,}-[0-9]{10,}-[a-zA-Z0-9]{24,}"#,
            "[REDACTED_SLACK_TOKEN]",
        ),
        // Generic UUID-like sensitive IDs
        (
            r#"(?i)(session[_-]?id|csrf[_-]?token)\s*[:=]\s*([\"']?)([a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12})\2"#,
            "$1: [REDACTED_UUID]",
        ),
    ];

    let mut out = input.to_string();
    for (pat, rep) in patterns {
        if let Ok(re) = Regex::new(pat) {
            out = re.replace_all(&out, *rep).to_string();
        }
    }
    out
}

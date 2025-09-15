//! Tracing configuration for structured logging in OpenAgent Terminal.
//!
//! This module provides structured logging with per-module filters,
//! field redaction for sensitive data, and optional AI debug logging.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{env, process};

use tracing::{Level};
use metrics_exporter_prometheus::PrometheusBuilder;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    filter::filter_fn,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer, Registry,
};

/// Environment variable to enable AI debug logging.
const OPENAGENT_AI_DEBUG_LOG_ENV: &str = "OPENAGENT_AI_DEBUG_LOG";

/// Environment variable for the AI debug log file path.
const OPENAGENT_AI_DEBUG_LOG_PATH_ENV: &str = "OPENAGENT_AI_DEBUG_LOG_PATH";

/// Configuration for the tracing system.
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Base log level for all modules.
    pub base_level: Level,
    /// Whether to enable AI debug logging.
    pub ai_debug_log: bool,
    /// Path to the AI debug log file.
    pub ai_debug_log_path: Option<PathBuf>,
    /// Per-module filter directives.
    pub module_filters: Vec<String>,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            base_level: Level::INFO,
            ai_debug_log: false,
            ai_debug_log_path: None,
            module_filters: vec![],
        }
    }
}

impl TracingConfig {
    /// Create configuration from environment variables and CLI options.
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Check if AI debug logging is enabled.
        config.ai_debug_log = env::var(OPENAGENT_AI_DEBUG_LOG_ENV)
            .ok()
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        // Set AI debug log path.
        if config.ai_debug_log {
            config.ai_debug_log_path = env::var(OPENAGENT_AI_DEBUG_LOG_PATH_ENV)
                .ok()
                .map(PathBuf::from)
                .or_else(|| {
                    let mut path = env::temp_dir();
                    path.push(format!("openagent_ai_debug_{}.log", process::id()));
                    Some(path)
                });
        }

        // Add default module filters.
        config.module_filters = vec![
            // Core modules at info level.
            "openagent_terminal=info".to_string(),
            "openagent_terminal_core=info".to_string(),
            "openagent_terminal_config=info".to_string(),

            // AI module at debug level if AI debug is enabled.
            if config.ai_debug_log {
                "openagent_terminal_ai=debug".to_string()
            } else {
                "openagent_terminal_ai=info".to_string()
            },

            // External dependencies at warn level to reduce noise.
            "winit=warn".to_string(),
            "mio=warn".to_string(),
            "notify=warn".to_string(),
        ];

        config
    }

    /// Build the environment filter string.
    pub fn build_filter(&self) -> String {
        let mut filters = self.module_filters.clone();

        // Add base level as fallback.
        filters.push(format!("{}", self.base_level));

        filters.join(",")
    }
}

/// A writer that can conditionally write to a file.
struct ConditionalFileWriter {
    path: PathBuf,
    file: Option<File>,
    enabled: Arc<AtomicBool>,
}

impl ConditionalFileWriter {
    fn new(path: PathBuf, enabled: bool) -> Self {
        Self {
            path,
            file: None,
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    fn enable(&self) {
        self.enabled.store(true, Ordering::SeqCst);
    }

    fn disable(&self) {
        self.enabled.store(false, Ordering::SeqCst);
    }
}

impl Write for ConditionalFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.enabled.load(Ordering::SeqCst) {
            return Ok(buf.len());
        }

        if self.file.is_none() {
            self.file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?
            );
            eprintln!("Created AI debug log at: {}", self.path.display());
        }

        self.file.as_mut().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(ref mut file) = self.file {
            file.flush()
        } else {
            Ok(())
        }
    }
}

/// Field redactor for sensitive information in structured logs.
pub struct SensitiveFieldRedactor;

impl SensitiveFieldRedactor {
    /// List of field names that should be redacted.
    const SENSITIVE_FIELDS: &'static [&'static str] = &[
        "api_key",
        "apikey",
        "token",
        "secret",
        "password",
        "auth",
        "authorization",
        "bearer",
        "credential",
        "private_key",
        "access_token",
        "refresh_token",
    ];

    /// Check if a field name is sensitive.
    pub fn is_sensitive(field_name: &str) -> bool {
        let lower = field_name.to_lowercase();
        Self::SENSITIVE_FIELDS.iter().any(|&s| lower.contains(s))
    }

    /// Redact a value if the field is sensitive.
    pub fn redact_if_sensitive(field_name: &str, value: &str) -> String {
        if Self::is_sensitive(field_name) {
            "[REDACTED]".to_string()
        } else {
            value.to_string()
        }
    }
}

/// Initialize the tracing system with structured logging.
pub fn initialize_tracing(config: TracingConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Create base subscriber.
    let subscriber = Registry::default();

    // Add environment filter layer.
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.build_filter()));

    // Create stdout layer with formatting.
    let stdout_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .with_span_events(FmtSpan::NONE)
        .with_ansi(true)
        .compact();

    // Build the subscriber with layers.
    let subscriber = subscriber
        .with(env_filter)
        .with(stdout_layer);

    // Initialize Prometheus metrics exporter only when explicitly enabled via env
    if let Ok(addr) = std::env::var("OPENAGENT_PROM_PORT") {
        let bind = format!("127.0.0.1:{}", addr);
        if let Ok(listener) = std::net::TcpListener::bind(&bind) {
            if let Ok(handle) = PrometheusBuilder::new().install_recorder() {
                // Spawn a tiny HTTP server to expose /metrics
                std::thread::spawn(move || {
                    for stream in listener.incoming() {
                        if let Ok(mut s) = stream {
                            let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\n\r\n");
                            let body = handle.render();
                            let _ = s.write_all(body.as_bytes());
                        }
                    }
                });
            }
        }
    }

    if config.ai_debug_log {
        if let Some(path) = config.ai_debug_log_path {
            let ai_path = path.clone();
            let ai_layer = fmt::layer()
                .with_writer(move || ConditionalFileWriter::new(ai_path.clone(), true))
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true)
                .with_span_events(FmtSpan::FULL)
                .with_ansi(false)
                .pretty()
                .with_filter(filter_fn(|metadata| {
                    // Only log AI-related modules to the debug file.
                    metadata.target().starts_with("openagent_terminal_ai")
                }));

            subscriber.with(ai_layer).try_init()?;
        } else {
            subscriber.try_init()?;
        }
    } else {
        subscriber.try_init()?;
    }

    Ok(())
}

/// Macro for logging with automatic field redaction.
#[macro_export]
macro_rules! log_with_redaction {
    ($level:expr, $($field:tt = $value:expr),* $(,)?) => {
        {
            use $crate::logging::tracing_config::SensitiveFieldRedactor;
            tracing::event!(
                $level,
                $(
                    $field = %SensitiveFieldRedactor::redact_if_sensitive(
                        stringify!($field),
                        &format!("{}", $value)
                    )
                ),*
            );
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensitive_field_detection() {
        assert!(SensitiveFieldRedactor::is_sensitive("api_key"));
        assert!(SensitiveFieldRedactor::is_sensitive("API_KEY"));
        assert!(SensitiveFieldRedactor::is_sensitive("password"));
        assert!(SensitiveFieldRedactor::is_sensitive("access_token"));
        assert!(!SensitiveFieldRedactor::is_sensitive("username"));
        assert!(!SensitiveFieldRedactor::is_sensitive("email"));
    }

    #[test]
    fn test_field_redaction() {
        assert_eq!(
            SensitiveFieldRedactor::redact_if_sensitive("api_key", "sk-12345"),
            "[REDACTED]"
        );
        assert_eq!(
            SensitiveFieldRedactor::redact_if_sensitive("username", "john_doe"),
            "john_doe"
        );
    }
}

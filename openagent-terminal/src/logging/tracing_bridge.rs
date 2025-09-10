//! Bridge between tracing and the existing log-based system.
//!
//! This module provides a tracing layer that forwards events to the log crate,
//! allowing gradual migration to tracing while keeping the custom log sink as canonical.

use std::fmt;
use tracing::field::{Field, Visit};
use tracing::{Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

/// A tracing layer that forwards events to the log crate.
///
/// This allows the existing custom log sink to remain the canonical destination
/// for all logging output, while enabling selective migration to tracing macros.
pub struct LogForwardingLayer;

impl LogForwardingLayer {
    /// Create a new log forwarding layer.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LogForwardingLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for LogForwardingLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        // Convert tracing level to log level
        let log_level = match *metadata.level() {
            Level::ERROR => log::Level::Error,
            Level::WARN => log::Level::Warn,
            Level::INFO => log::Level::Info,
            Level::DEBUG => log::Level::Debug,
            Level::TRACE => log::Level::Trace,
        };

        // Create a visitor to extract the message and fields
        let mut visitor = LogMessageVisitor::new();
        event.record(&mut visitor);

        // Forward to the log crate, which will be handled by our custom sink
        log::logger().log(
            &log::Record::builder()
                .args(format_args!("{}", visitor.message))
                .level(log_level)
                .target(metadata.target())
                .module_path(metadata.module_path())
                .file(metadata.file())
                .line(metadata.line())
                .build(),
        );
    }
}

/// Visitor that extracts the message and structured fields from a tracing event.
struct LogMessageVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl LogMessageVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
            fields: Vec::new(),
        }
    }
}

impl Visit for LogMessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        } else {
            self.fields
                .push((field.name().to_string(), format!("{:?}", value)));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields
                .push((field.name().to_string(), value.to_string()));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }
}

impl fmt::Display for LogMessageVisitor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;

        if !self.fields.is_empty() {
            write!(f, " ")?;
            for (i, (key, value)) in self.fields.iter().enumerate() {
                if i > 0 {
                    write!(f, " ")?;
                }
                write!(f, "{}={}", key, value)?;
            }
        }

        Ok(())
    }
}

/// Initialize tracing with the log forwarding layer.
///
/// This sets up tracing to forward events to the existing log infrastructure,
/// allowing gradual migration while keeping the custom sink as canonical.
pub fn initialize_tracing_bridge() -> Result<(), Box<dyn std::error::Error>> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::{EnvFilter, Registry};

    // Create environment filter with reasonable defaults for OpenAgent Terminal
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "openagent_terminal=info,openagent_terminal_core=info,openagent_terminal_ai=info,warn",
        )
    });

    // Create the log forwarding layer
    let log_forwarding_layer = LogForwardingLayer::new();

    // Set up the subscriber with the log forwarding layer
    let subscriber = Registry::default()
        .with(env_filter)
        .with(log_forwarding_layer);

    // Initialize the global subscriber
    tracing::subscriber::set_global_default(subscriber)?;

    // Initialize the log-to-tracing bridge for any remaining log calls
    tracing_log::LogTracer::init()?;

    Ok(())
}

/// Convenience macros for structured logging that bridge to the custom log sink.
/// These provide a tracing-style API but forward through the log crate to preserve
/// the existing sensitive data redaction and UI integration.
///
/// Log an AI request with structured fields.
#[macro_export]
macro_rules! log_ai_request_bridge {
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

/// Log an AI response with structured fields.
#[macro_export]
macro_rules! log_ai_response_bridge {
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

/// Log a terminal event with structured fields.
#[macro_export]
macro_rules! log_terminal_event_bridge {
    ($event_type:expr, $details:expr) => {
        tracing::debug!(
            event_type = %$event_type,
            details = %$details,
            event = "terminal_event",
            "Terminal event"
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::Registry;

    struct TestLogger {
        messages: Arc<Mutex<Vec<String>>>,
    }

    impl TestLogger {
        fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
            let messages = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    messages: messages.clone(),
                },
                messages,
            )
        }
    }

    impl log::Log for TestLogger {
        fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
            true
        }

        fn log(&self, record: &log::Record<'_>) {
            self.messages.lock().unwrap().push(format!(
                "[{}] {} - {}",
                record.level(),
                record.target(),
                record.args()
            ));
        }

        fn flush(&self) {}
    }

    #[test]
    fn test_log_forwarding_layer() {
        let (test_logger, messages) = TestLogger::new();

        // Set up the test logger
        log::set_boxed_logger(Box::new(test_logger)).unwrap();
        log::set_max_level(log::LevelFilter::Debug);

        // Set up tracing with our forwarding layer
        let subscriber = Registry::default().with(LogForwardingLayer::new());
        tracing::subscriber::set_global_default(subscriber).unwrap();

        // Test logging
        tracing::info!("test message");
        tracing::error!(error_code = 404, "something went wrong");

        // Check that messages were forwarded
        let logged_messages = messages.lock().unwrap();
        assert!(logged_messages.len() >= 2);
        assert!(logged_messages
            .iter()
            .any(|msg| msg.contains("test message")));
        assert!(logged_messages
            .iter()
            .any(|msg| msg.contains("something went wrong")));
    }
}

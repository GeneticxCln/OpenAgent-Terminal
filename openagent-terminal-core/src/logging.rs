//! Unified logging configuration for OpenAgent Terminal
//!
//! This module provides a consistent tracing setup across all crates,
//! replacing the legacy `log` crate usage with structured tracing.

use tracing::{Level, Subscriber};
use tracing_subscriber::{
    filter::EnvFilter,
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    registry::LookupSpan,
    Layer, Registry,
};

/// Initialize the global tracing subscriber with default settings
pub fn init_tracing() {
    let subscriber = create_subscriber(None, false);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global tracing subscriber");
}

/// Initialize tracing with custom configuration
pub fn init_tracing_with_config(filter: Option<String>, json_output: bool) {
    let subscriber = create_subscriber(filter, json_output);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global tracing subscriber");
}

/// Create a configured tracing subscriber
fn create_subscriber(
    filter: Option<String>,
    json_output: bool,
) -> impl Subscriber + Send + Sync {
    // Set up environment filter
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| {
            if let Some(filter) = filter {
                EnvFilter::try_new(filter)
            } else {
                // Default filter: INFO for openagent crates, WARN for others
                EnvFilter::try_new("openagent=info,warn")
            }
        })
        .expect("Failed to create env filter");

    // Create the base registry
    let registry = Registry::default();

    // Configure the formatting layer
    let fmt_layer = if json_output {
        fmt::layer()
            .json()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_target(true)
            .with_span_events(FmtSpan::CLOSE)
            .boxed()
    } else {
        fmt::layer()
            .pretty()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(false)
            .with_target(true)
            .with_span_events(FmtSpan::CLOSE)
            .boxed()
    };

    registry.with(env_filter).with(fmt_layer)
}

/// Structured logging macros with context
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
macro_rules! log_ai_stream_chunk {
    ($provider:expr, $chunk_size:expr, $total_chunks:expr) => {
        tracing::debug!(
            provider = %$provider,
            chunk_size = $chunk_size,
            total_chunks = $total_chunks,
            event = "ai_stream_chunk",
            "Streaming chunk received"
        );
    };
}

#[macro_export]
macro_rules! log_plugin_event {
    ($plugin:expr, $event_type:expr, $details:expr) => {
        tracing::info!(
            plugin = %$plugin,
            event_type = %$event_type,
            details = %$details,
            event = "plugin_event",
            "Plugin event"
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

/// Helper to bridge legacy log crate to tracing
pub fn setup_log_bridge() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Bridge log crate to tracing
    tracing_log::LogTracer::init()
        .expect("Failed to initialize log bridge");
}

/// Performance measurement span
#[macro_export]
macro_rules! measure_span {
    ($name:expr) => {
        tracing::info_span!($name, event = "performance")
    };
    ($name:expr, $($field:tt)*) => {
        tracing::info_span!($name, event = "performance", $($field)*)
    };
}

/// Error reporting with context
#[macro_export]
macro_rules! log_error_with_context {
    ($error:expr, $context:expr) => {
        tracing::error!(
            error = %$error,
            context = %$context,
            event = "error",
            "Error occurred"
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn test_default_subscriber_creation() {
        let subscriber = create_subscriber(None, false);
        // Just check it compiles and can be created
        assert!(std::mem::size_of_val(&subscriber) > 0);
    }

    #[test]
    fn test_json_subscriber_creation() {
        let subscriber = create_subscriber(Some("debug".to_string()), true);
        assert!(std::mem::size_of_val(&subscriber) > 0);
    }

    #[test]
    fn test_custom_filter() {
        let subscriber = create_subscriber(
            Some("openagent=trace,hyper=warn".to_string()),
            false
        );
        assert!(std::mem::size_of_val(&subscriber) > 0);
    }
}

/// Example migration from log to tracing
///
/// Before (using log crate):
/// ```ignore
/// use log::{info, debug, error};
///
/// info!("Processing request");
/// debug!("Request details: {:?}", request);
/// error!("Failed to process: {}", err);
/// ```
///
/// After (using tracing):
/// ```ignore
/// use tracing::{info, debug, error};
///
/// info!(event = "request_processing", "Processing request");
/// debug!(request = ?request, "Request details");
/// error!(error = %err, "Failed to process");
/// ```
///
/// Or with structured fields:
/// ```ignore
/// info!(
///     provider = "openai",
///     model = "gpt-4",
///     tokens = 1500,
///     event = "ai_request",
///     "Processing AI request"
/// );
/// ```

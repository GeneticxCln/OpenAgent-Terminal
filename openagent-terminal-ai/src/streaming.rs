//! Robust SSE (Server-Sent Events) streaming support with retry and backoff.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tracing::{debug, warn};

/// SSE parser state machine for handling fragmented frames
#[derive(Debug, Default)]
pub struct SseParser {
    /// Current partial event being built
    current_event: SseEvent,
    /// Buffer for incomplete lines
    line_buffer: String,
    /// Whether we're currently in a data field
    #[allow(dead_code)]
    in_data_field: bool,
}

#[derive(Debug, Default, Clone)]
pub struct SseEvent {
    pub id: Option<String>,
    pub event: Option<String>,
    pub data: Vec<String>,
    pub retry: Option<u64>,
}

impl SseEvent {
    fn is_empty(&self) -> bool {
        self.id.is_none() && self.event.is_none() && self.data.is_empty() && self.retry.is_none()
    }

    fn clear(&mut self) {
        self.id = None;
        self.event = None;
        self.data.clear();
        self.retry = None;
    }
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a chunk of SSE data, handling fragmentation
    pub fn parse_chunk(&mut self, chunk: &str) -> Vec<SseEvent> {
        let mut events = Vec::new();

        // Prepend any buffered data
        let data = if !self.line_buffer.is_empty() {
            let combined = format!("{}{}", self.line_buffer, chunk);
            self.line_buffer.clear();
            combined
        } else {
            chunk.to_string()
        };

        let mut lines = data.lines().peekable();

        while let Some(line) = lines.next() {
            // Check if this is the last line and it doesn't end with newline
            if lines.peek().is_none() && !data.ends_with('\n') {
                // Buffer incomplete line for next chunk
                self.line_buffer = line.to_string();
                break;
            }

            if let Some(event) = self.process_line(line) {
                events.push(event);
            }
        }

        events
    }

    /// Process a single SSE line
    fn process_line(&mut self, line: &str) -> Option<SseEvent> {
        // Empty line signals end of event
        if line.is_empty() {
            if !self.current_event.is_empty() {
                let event = self.current_event.clone();
                self.current_event.clear();
                return Some(event);
            }
            return None;
        }

        // Ignore comments
        if line.starts_with(':') {
            // This could be a heartbeat keepalive
            debug!("SSE keepalive/comment: {}", line);
            return None;
        }

        // Parse field
        let field_end = line.find(':').unwrap_or(line.len());
        let field = &line[..field_end];
        let value = if field_end < line.len() {
            let start = if line.chars().nth(field_end + 1) == Some(' ') {
                field_end + 2
            } else {
                field_end + 1
            };
            &line[start..]
        } else {
            ""
        };

        match field {
            "id" => self.current_event.id = Some(value.to_string()),
            "event" => self.current_event.event = Some(value.to_string()),
            "data" => {
                // Handle multi-line data fields
                self.current_event.data.push(value.to_string());
            }
            "retry" => {
                if let Ok(ms) = value.parse::<u64>() {
                    self.current_event.retry = Some(ms);
                }
            }
            _ => {
                debug!("Unknown SSE field: {}", field);
            }
        }

        None
    }

    /// Flush any pending event (call when stream ends)
    pub fn flush(&mut self) -> Option<SseEvent> {
        if !self.current_event.is_empty() {
            let event = self.current_event.clone();
            self.current_event.clear();
            Some(event)
        } else {
            None
        }
    }
}

/// Retry configuration with exponential backoff and jitter
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: usize,
    /// Initial retry delay
    pub initial_delay: Duration,
    /// Maximum retry delay
    pub max_delay: Duration,
    /// Backoff multiplier
    pub multiplier: f64,
    /// Jitter factor (0.0 to 1.0)
    pub jitter: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: 0.3,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for the given attempt number (0-indexed)
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        use rand::Rng;

        let base_delay =
            self.initial_delay.as_millis() as f64 * self.multiplier.powi(attempt as i32);
        let capped_delay = base_delay.min(self.max_delay.as_millis() as f64);

        // Add jitter, then clamp to [0, max_delay]
        let jitter_range = capped_delay * self.jitter;
        let jitter = rand::thread_rng().gen_range(-jitter_range..=jitter_range);
        let jittered = (capped_delay + jitter).max(0.0);
        let clamped = jittered.min(self.max_delay.as_millis() as f64) as u64;

        Duration::from_millis(clamped)
    }

    /// Check if we should retry based on attempt count
    pub fn should_retry(&self, attempt: usize) -> bool {
        attempt < self.max_retries
    }
}

/// Stream processor with backpressure handling
pub struct StreamProcessor {
    parser: SseParser,
    #[allow(dead_code)]
    retry_config: RetryConfig,
    buffer_limit: usize,
    buffered_events: Vec<SseEvent>,
}

impl StreamProcessor {
    pub fn new(retry_config: RetryConfig) -> Self {
        Self {
            parser: SseParser::new(),
            retry_config,
            buffer_limit: 1000, // Maximum events to buffer
            buffered_events: Vec::new(),
        }
    }

    /// Process incoming data with backpressure handling
    pub fn process_data(
        &mut self,
        data: &str,
        on_event: &mut dyn FnMut(&SseEvent) -> bool,
    ) -> Result<(), String> {
        let events = self.parser.parse_chunk(data);

        for event in events {
            // Check buffer limit for backpressure
            if self.buffered_events.len() >= self.buffer_limit {
                warn!("Stream buffer limit reached, applying backpressure");
                return Err("Buffer overflow - consumer too slow".to_string());
            }

            // Try to deliver event
            if !on_event(&event) {
                // Consumer rejected event, buffer it
                self.buffered_events.push(event);
            }
        }

        // Try to drain buffered events
        self.drain_buffer(on_event);

        Ok(())
    }

    /// Attempt to drain buffered events
    fn drain_buffer(&mut self, on_event: &mut dyn FnMut(&SseEvent) -> bool) {
        let mut i = 0;
        while i < self.buffered_events.len() {
            if on_event(&self.buffered_events[i]) {
                self.buffered_events.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Finalize stream processing
    pub fn finalize(&mut self, on_event: &mut dyn FnMut(&SseEvent) -> bool) {
        if let Some(event) = self.parser.flush() {
            let _ = on_event(&event);
        }
        self.drain_buffer(on_event);
    }
}

/// Provider-specific retry strategies
#[derive(Debug, Clone)]
pub enum RetryStrategy {
    /// Standard exponential backoff
    Standard(RetryConfig),
    /// OpenAI-specific (respects rate limit headers)
    OpenAI {
        config: RetryConfig,
        respect_retry_after: bool,
    },
    /// Anthropic-specific (handles overload responses)
    Anthropic {
        config: RetryConfig,
        overload_backoff: Duration,
    },
    /// Ollama-specific (handles local resource constraints)
    Ollama {
        config: RetryConfig,
        resource_wait: Duration,
    },
}

impl RetryStrategy {
    pub fn should_retry(&self, attempt: usize, error: &str, cancelled: &AtomicBool) -> bool {
        // Never retry if user cancelled
        if cancelled.load(Ordering::Relaxed) {
            return false;
        }

        // Check if error is retryable
        if !Self::is_retryable_error(error) {
            return false;
        }

        match self {
            Self::Standard(config)
            | Self::OpenAI { config, .. }
            | Self::Anthropic { config, .. }
            | Self::Ollama { config, .. } => config.should_retry(attempt),
        }
    }

    pub fn delay_for_attempt(&self, attempt: usize, error: &str) -> Duration {
        match self {
            Self::Standard(config) => config.delay_for_attempt(attempt),
            Self::OpenAI {
                config,
                respect_retry_after,
            } => {
                if *respect_retry_after {
                    if let Some(d) = Self::parse_retry_after(error) {
                        return d;
                    }
                }
                config.delay_for_attempt(attempt)
            }
            Self::Anthropic {
                config,
                overload_backoff,
            } => {
                if error.contains("overloaded") {
                    *overload_backoff
                } else {
                    config.delay_for_attempt(attempt)
                }
            }
            Self::Ollama {
                config,
                resource_wait,
            } => {
                if error.contains("resource") || error.contains("memory") {
                    *resource_wait
                } else {
                    config.delay_for_attempt(attempt)
                }
            }
        }
    }

    fn is_retryable_error(error: &str) -> bool {
        let error_lower = error.to_lowercase();

        // Retryable errors
        if error_lower.contains("timeout")
            || error_lower.contains("connection")
            || error_lower.contains("rate limit")
            || error_lower.contains("overload")
            || error_lower.contains("temporary")
            || error_lower.contains("503")
            || error_lower.contains("502")
            || error_lower.contains("429")
        {
            return true;
        }

        // Non-retryable errors
        if error_lower.contains("invalid")
            || error_lower.contains("unauthorized")
            || error_lower.contains("forbidden")
            || error_lower.contains("not found")
            || error_lower.contains("400")
            || error_lower.contains("401")
            || error_lower.contains("403")
            || error_lower.contains("404")
        {
            return false;
        }

        // Default to retryable for unknown errors
        true
    }

    /// Parse Retry-After value from an error string that may contain a fragment like
    /// "; retry-after: 60" or "; retry-after: Fri, 05 Sep 2025 23:20:00 GMT".
    /// Returns a Duration if parsing succeeds.
    fn parse_retry_after(error: &str) -> Option<Duration> {
        // Case-insensitive search for "retry-after:"
        let lower = error.to_lowercase();
        let key = "retry-after:";
        let idx = lower.find(key)?;
        let start = idx + key.len();
        let tail = &error[start..].trim();
        // Try numeric seconds first (common case)
        if let Some(first_token) = tail.split_whitespace().next() {
            if let Ok(secs) = first_token.parse::<u64>() {
                return Some(Duration::from_secs(secs));
            }
        }
        // Try HTTP-date
        if let Ok(when) = httpdate::parse_http_date(tail) {
            if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                if let Ok(target) = when.duration_since(std::time::UNIX_EPOCH) {
                    if target > now {
                        return Some(target - now);
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_sse_parser_basic() {
        let mut parser = SseParser::new();

        let data = "event: message\ndata: hello world\n\n";
        let events = parser.parse_chunk(data);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, Some("message".to_string()));
        assert_eq!(events[0].data, vec!["hello world".to_string()]);
    }

    #[test]
    fn test_sse_parser_multiline_data() {
        let mut parser = SseParser::new();

        let data = "data: line1\ndata: line2\ndata: line3\n\n";
        let events = parser.parse_chunk(data);

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].data,
            vec![
                "line1".to_string(),
                "line2".to_string(),
                "line3".to_string(),
            ]
        );
    }

    #[test]
    fn test_sse_parser_fragmented() {
        let mut parser = SseParser::new();

        // First chunk - incomplete
        let events1 = parser.parse_chunk("event: mes");
        assert_eq!(events1.len(), 0);

        // Second chunk - completes the event
        let events2 = parser.parse_chunk("sage\ndata: test\n\n");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].event, Some("message".to_string()));
        assert_eq!(events2[0].data, vec!["test".to_string()]);
    }

    #[test]
    fn test_sse_parser_keepalive() {
        let mut parser = SseParser::new();

        let data = ":keepalive\n\nevent: actual\ndata: content\n\n";
        let events = parser.parse_chunk(data);

        // Keepalive should be ignored
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, Some("actual".to_string()));
    }

    #[test]
    fn test_retry_config_backoff() {
        let config = RetryConfig::default();

        let delay0 = config.delay_for_attempt(0);
        let delay1 = config.delay_for_attempt(1);
        let delay2 = config.delay_for_attempt(2);

        // Delays should increase
        assert!(delay1 > delay0);
        assert!(delay2 > delay1);

        // But should be capped at max_delay
        let delay_max = config.delay_for_attempt(100);
        assert!(delay_max <= config.max_delay + Duration::from_secs(1)); // Allow for jitter
    }

    #[test]
    fn test_openai_retry_after_header_affects_delay() {
        let cfg = RetryConfig {
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            ..Default::default()
        };
        let strat = RetryStrategy::OpenAI {
            config: cfg.clone(),
            respect_retry_after: true,
        };

        // With retry-after present, strategy should return the header value (seconds)
        let d_with = strat.delay_for_attempt(0, "API error 429; retry-after: 60");
        assert_eq!(d_with, Duration::from_secs(60));

        // Without retry-after header, should use backoff algorithm (less than max)
        let d_without = strat.delay_for_attempt(0, "API error 429");
        assert!(d_without < cfg.max_delay);
    }

    #[test]
    fn test_anthropic_overload_backoff() {
        let cfg = RetryConfig {
            initial_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(5),
            ..Default::default()
        };
        let backoff = Duration::from_secs(3);
        let strat = RetryStrategy::Anthropic {
            config: cfg.clone(),
            overload_backoff: backoff,
        };

        // Overload error should use overload_backoff delay
        let d_over = strat.delay_for_attempt(0, "server overloaded, please retry");
        assert_eq!(d_over, backoff);

        // Non-overload should use backoff algorithm (less or equal to max)
        let d_norm = strat.delay_for_attempt(0, "temporary network issue 503");
        assert!(d_norm <= cfg.max_delay);
    }

    #[test]
    fn test_ollama_resource_wait_backoff() {
        let cfg = RetryConfig {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(4),
            ..Default::default()
        };
        let wait = Duration::from_secs(2);
        let strat = RetryStrategy::Ollama {
            config: cfg.clone(),
            resource_wait: wait,
        };

        // Resource error should use resource_wait delay
        let d_res = strat.delay_for_attempt(0, "resource busy: GPU memory");
        assert_eq!(d_res, wait);

        // Non-resource should use backoff algorithm
        let d_norm = strat.delay_for_attempt(0, "timeout contacting local model");
        assert!(d_norm <= cfg.max_delay);
    }

    #[test]
    fn test_openai_retry_after_http_date_affects_delay() {
        use httpdate::fmt_http_date;
        use std::time::{Duration, SystemTime};
        let cfg = RetryConfig::default();
        let strat = RetryStrategy::OpenAI {
            config: cfg.clone(),
            respect_retry_after: true,
        };
        // Build a future HTTP-date 5 seconds from now
        let future = SystemTime::now() + Duration::from_secs(5);
        let hdr = fmt_http_date(future);
        let msg = format!("API error 429; retry-after: {}", hdr);
        let d = strat.delay_for_attempt(0, &msg);
        assert!(d.as_secs() <= 6 && d.as_secs() >= 4, "unexpected delay: {:?}", d);
    }
}

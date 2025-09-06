#![cfg(feature = "ai-anthropic")]
#[cfg(test)]
mod anthropic_provider_tests {
    use openagent_terminal_ai::providers::AnthropicProvider;
    use openagent_terminal_ai::{AiProvider, AiRequest};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    fn create_test_request() -> AiRequest {
        AiRequest {
            scratch_text: "find large files".to_string(),
            working_directory: Some("/var/log".to_string()),
            shell_kind: Some("zsh".to_string()),
            context: vec![
                ("platform".to_string(), "macos".to_string()),
                ("terminal".to_string(), "openagent".to_string()),
            ],
        }
    }

    #[test]
    fn test_anthropic_provider_creation() {
        let provider = AnthropicProvider::new(
            "test_key".to_string(),
            "https://api.anthropic.com".to_string(),
            "claude-3-opus".to_string(),
        );
        assert!(provider.is_ok());
    }

    #[test]
    fn test_anthropic_streaming_complete_response() {
        let server = MockServer::start();
        let stream_data = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"role\":\"assistant\",\"content\":[]}}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"find \"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\". -type f \"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"-size +100M\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/messages")
                .header("x-api-key", "test_key")
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json");
            then.status(200).header("content-type", "text/event-stream").body(stream_data);
        });

        let provider = AnthropicProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "claude-3-opus".to_string(),
        )
        .unwrap();

        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
        };

        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);

        assert!(result.is_ok());
        assert_eq!(collected, "find . -type f -size +100M");
        mock.assert();
    }

    #[test]
    fn test_anthropic_streaming_with_multiple_blocks() {
        let server = MockServer::start();
        // Anthropic can send multiple content blocks
        let stream_data = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\"}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"First command: \"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"find /var/log\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );

        server.mock(|when, then| {
            when.method(POST).path("/messages");
            then.status(200).header("content-type", "text/event-stream").body(stream_data);
        });

        let provider = AnthropicProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "claude-3-opus".to_string(),
        )
        .unwrap();

        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
        };

        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);

        assert!(result.is_ok());
        assert!(collected.contains("First command"));
        assert!(collected.contains("find /var/log"));
    }

    #[test]
    fn test_anthropic_error_response() {
        let server = MockServer::start();

        let error_response = r#"{
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": "Invalid authentication"
            }
        }"#;

        server.mock(|when, then| {
            when.method(POST).path("/messages");
            then.status(401).header("content-type", "application/json").body(error_response);
        });

        let provider = AnthropicProvider::new(
            "invalid_key".to_string(),
            server.base_url(),
            "claude-3".to_string(),
        )
        .unwrap();

        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
        };

        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("401") || error.contains("authentication"));
    }

    #[test]
    fn test_anthropic_rate_limit() {
        let server = MockServer::start();

        server.mock(|when, then| {
            when.method(POST)
                .path("/messages");
            then.status(429)
                .header("retry-after", "30")
                .header("x-ratelimit-limit", "1000")
                .header("x-ratelimit-remaining", "0")
                .body(r#"{"type":"error","error":{"type":"rate_limit_error","message":"Rate limit exceeded"}}"#);
        });

        let provider = AnthropicProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "claude-3".to_string(),
        )
        .unwrap();

        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
        };

        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("429"));
    }

    #[test]
    fn test_anthropic_streaming_cancellation() {
        let server = MockServer::start();
        let stream_data = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\"}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"find\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" /var\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"/log\"}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );

        server.mock(|when, then| {
            when.method(POST).path("/messages");
            then.status(200)
                .header("content-type", "text/event-stream")
                .delay(Duration::from_millis(50))
                .body(stream_data);
        });

        let provider = AnthropicProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "claude-3".to_string(),
        )
        .unwrap();

        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut chunk_count = 0;
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
            chunk_count += 1;
            // Cancel after first chunk
            if chunk_count >= 1 {
                cancel.store(true, Ordering::SeqCst);
            }
        };

        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);

        assert!(result.is_ok());
        assert!(!collected.is_empty());
        // Should not have all the content due to cancellation
        assert!(!collected.contains("/log"));
    }

    #[test]
    fn test_anthropic_server_error() {
        let server = MockServer::start();

        server.mock(|when, then| {
            when.method(POST)
                .path("/messages");
            then.status(500)
                .body(r#"{"type":"error","error":{"type":"api_error","message":"Internal server error"}}"#);
        });

        let provider = AnthropicProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "claude-3".to_string(),
        )
        .unwrap();

        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
        };

        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("500"));
    }

    #[test]
    fn test_anthropic_unexpected_event_type() {
        let server = MockServer::start();
        // Include some unexpected event types that should be ignored
        let stream_data = concat!(
            "event: ping\n",
            "data: {\"type\":\"ping\"}\n\n",
            "event: message_start\n",
            "data: {\"type\":\"message_start\"}\n\n",
            "event: unknown_event\n",
            "data: {\"type\":\"unknown\"}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"du -sh\"}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );

        server.mock(|when, then| {
            when.method(POST).path("/messages");
            then.status(200).header("content-type", "text/event-stream").body(stream_data);
        });

        let provider = AnthropicProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "claude-3".to_string(),
        )
        .unwrap();

        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
        };

        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);

        assert!(result.is_ok());
        // Should only collect the actual content
        assert_eq!(collected, "du -sh");
    }
}

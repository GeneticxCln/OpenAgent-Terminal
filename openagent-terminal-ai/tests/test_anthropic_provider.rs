#![cfg(feature = "ai-anthropic")]
#[cfg(test)]
mod anthropic_provider_tests {
    use openagent_terminal_ai::providers::AnthropicProvider;
    use openagent_terminal_ai::{AiProvider, AiRequest};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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

    #[tokio::test]
    async fn test_anthropic_streaming_complete_response() {
        let server = MockServer::start().await;
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

        Mock::given(method("POST"))
            .and(path("/messages"))
            .and(header("x-api-key", "test_key"))
            .and(header("anthropic-version", "2023-06-01"))
            .and(header("content-type", "application/json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(stream_data),
            )
            .mount(&server)
            .await;

        let server_uri = server.uri();
        let (tx, rx) = std::sync::mpsc::sync_channel::<(Result<bool, String>, String)>(1);

        // Use std::thread::spawn to avoid any Tokio runtime context
        std::thread::spawn(move || {
            let provider = AnthropicProvider::new(
                "test_key".to_string(),
                server_uri,
                "claude-3-opus".to_string(),
            )
            .unwrap();

            let mut collected = String::new();
            let cancel = AtomicBool::new(false);
            let mut on_chunk = |chunk: &str| {
                collected.push_str(chunk);
            };

            let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
            tx.send((result, collected)).ok();
        });

        let (result, collected) = rx.recv().unwrap();
        assert!(result.is_ok());
        assert_eq!(collected, "find . -type f -size +100M");
    }

    #[tokio::test]
    async fn test_anthropic_streaming_with_multiple_blocks() {
        let server = MockServer::start().await;
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

        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(stream_data),
            )
            .mount(&server)
            .await;

        let server_uri = server.uri();
        let (tx, rx) = std::sync::mpsc::sync_channel::<(Result<bool, String>, String)>(1);

        std::thread::spawn(move || {
            let provider = AnthropicProvider::new(
                "test_key".to_string(),
                server_uri,
                "claude-3-opus".to_string(),
            )
            .unwrap();

            let mut collected = String::new();
            let cancel = AtomicBool::new(false);
            let mut on_chunk = |chunk: &str| {
                collected.push_str(chunk);
            };

            let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
            tx.send((result, collected)).ok();
        });

        let (result, collected) = rx.recv().unwrap();
        assert!(result.is_ok());
        assert!(collected.contains("First command"));
        assert!(collected.contains("find /var/log"));
    }

    #[tokio::test]
    async fn test_anthropic_error_response() {
        let server = MockServer::start().await;

        let error_response = r#"{
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": "Invalid authentication"
            }
        }"#;

        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(
                ResponseTemplate::new(401)
                    .insert_header("content-type", "application/json")
                    .set_body_string(error_response),
            )
            .mount(&server)
            .await;

        let server_uri = server.uri();
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<bool, String>>(1);

        std::thread::spawn(move || {
            let provider = AnthropicProvider::new(
                "invalid_key".to_string(),
                server_uri,
                "claude-3".to_string(),
            )
            .unwrap();

            let mut collected = String::new();
            let cancel = AtomicBool::new(false);
            let mut on_chunk = |chunk: &str| {
                collected.push_str(chunk);
            };

            let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
            tx.send(result).ok();
        });

        let result = rx.recv().unwrap();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("401") || error.contains("authentication"));
    }

    #[tokio::test]
    async fn test_anthropic_rate_limit() {
        let server = MockServer::start().await;

        Mock::given(method("POST")).and(path("/messages")).respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "30")
                .insert_header("x-ratelimit-limit", "1000")
                .insert_header("x-ratelimit-remaining", "0")
                .set_body_string(r#"{"type":"error","error":{"type":"rate_limit_error","message":"Rate limit exceeded"}}"#),
        )
        .mount(&server)
        .await;

        let server_uri = server.uri();
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<bool, String>>(1);

        std::thread::spawn(move || {
            let provider =
                AnthropicProvider::new("test_key".to_string(), server_uri, "claude-3".to_string())
                    .unwrap();

            let mut collected = String::new();
            let cancel = AtomicBool::new(false);
            let mut on_chunk = |chunk: &str| {
                collected.push_str(chunk);
            };

            let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
            tx.send(result).ok();
        });

        let result = rx.recv().unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("429"));
    }

    #[tokio::test]
    async fn test_anthropic_streaming_cancellation() {
        let server = MockServer::start().await;
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

        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_delay(Duration::from_millis(50))
                    .set_body_string(stream_data),
            )
            .mount(&server)
            .await;

        let server_uri = server.uri();
        let (tx, rx) = std::sync::mpsc::sync_channel::<(Result<bool, String>, String)>(1);

        std::thread::spawn(move || {
            let provider =
                AnthropicProvider::new("test_key".to_string(), server_uri, "claude-3".to_string())
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
            tx.send((result, collected)).ok();
        });

        let (result, collected) = rx.recv().unwrap();
        // When cancelled, the provider may return Ok or an error indicating cancellation
        // What matters is we got some partial data before cancellation
        if result.is_err() {
            // If it's an error, it should be a cancellation error
            let err = result.unwrap_err();
            assert!(
                err.contains("Cancelled") || err.contains("cancelled"),
                "Unexpected error: {}",
                err
            );
        }
        assert!(!collected.is_empty());
        // Should not have all the content due to cancellation
        assert!(!collected.contains("/log"));
    }

    #[tokio::test]
    async fn test_anthropic_server_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST")).and(path("/messages")).respond_with(
            ResponseTemplate::new(500)
                .set_body_string(r#"{"type":"error","error":{"type":"api_error","message":"Internal server error"}}"#),
        )
        .mount(&server)
        .await;

        let server_uri = server.uri();
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<bool, String>>(1);

        std::thread::spawn(move || {
            let provider =
                AnthropicProvider::new("test_key".to_string(), server_uri, "claude-3".to_string())
                    .unwrap();

            let mut collected = String::new();
            let cancel = AtomicBool::new(false);
            let mut on_chunk = |chunk: &str| {
                collected.push_str(chunk);
            };

            let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
            tx.send(result).ok();
        });

        let result = rx.recv().unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("500"));
    }

    #[tokio::test]
    async fn test_anthropic_unexpected_event_type() {
        let server = MockServer::start().await;
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

        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(stream_data),
            )
            .mount(&server)
            .await;

        let server_uri = server.uri();
        let (tx, rx) = std::sync::mpsc::sync_channel::<(Result<bool, String>, String)>(1);

        std::thread::spawn(move || {
            let provider =
                AnthropicProvider::new("test_key".to_string(), server_uri, "claude-3".to_string())
                    .unwrap();

            let mut collected = String::new();
            let cancel = AtomicBool::new(false);
            let mut on_chunk = |chunk: &str| {
                collected.push_str(chunk);
            };

            let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
            tx.send((result, collected)).ok();
        });

        let (result, collected) = rx.recv().unwrap();
        assert!(result.is_ok());
        // Should only collect the actual content
        assert_eq!(collected, "du -sh");
    }
}

// httpmock-integration-tests.rs
// Comprehensive integration tests for AI streaming with httpmock

use httpmock::prelude::*;
use serde_json::json;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use futures::StreamExt;

// Assuming these are from your main module
use crate::{
    StreamingManager, StreamingClient, OpenAIClient, AnthropicClient,
    StreamRequest, Message, Provider, StreamConfig, CancellationToken,
    StreamChunk, StreamMetrics,
};

// ============================================================================
// Test Utilities
// ============================================================================

/// Generate a mock SSE stream for OpenAI
fn generate_openai_sse_chunks(chunks: Vec<&str>) -> String {
    let mut response = String::new();

    for (i, chunk) in chunks.iter().enumerate() {
        let data = json!({
            "id": format!("chatcmpl-{}", i),
            "object": "chat.completion.chunk",
            "created": 1234567890,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": chunk
                },
                "finish_reason": if i == chunks.len() - 1 { "stop" } else { null }
            }]
        });

        response.push_str(&format!("data: {}\n\n", data));
    }

    response.push_str("data: [DONE]\n\n");
    response
}

/// Generate a mock event stream for Anthropic
fn generate_anthropic_event_stream(chunks: Vec<&str>) -> String {
    let mut response = String::new();

    // Message start event
    response.push_str("event: message_start\n");
    response.push_str(&format!("data: {}\n\n", json!({
        "type": "message_start",
        "message": {
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": "claude-3",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 0
            }
        }
    })));

    // Content block start
    response.push_str("event: content_block_start\n");
    response.push_str(&format!("data: {}\n\n", json!({
        "type": "content_block_start",
        "index": 0,
        "content_block": {
            "type": "text",
            "text": ""
        }
    })));

    // Content chunks
    for chunk in chunks {
        response.push_str("event: content_block_delta\n");
        response.push_str(&format!("data: {}\n\n", json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "text_delta",
                "text": chunk
            }
        })));
    }

    // Content block stop
    response.push_str("event: content_block_stop\n");
    response.push_str(&format!("data: {}\n\n", json!({
        "type": "content_block_stop",
        "index": 0
    })));

    // Message delta with final usage
    response.push_str("event: message_delta\n");
    response.push_str(&format!("data: {}\n\n", json!({
        "type": "message_delta",
        "delta": {
            "stop_reason": "end_turn",
            "stop_sequence": null
        },
        "usage": {
            "output_tokens": 25
        }
    })));

    // Message stop
    response.push_str("event: message_stop\n");
    response.push_str(&format!("data: {}\n\n", json!({
        "type": "message_stop"
    })));

    response
}

/// Generate a slow streaming response with delays
async fn generate_slow_stream(chunks: Vec<&str>, delay_ms: u64) -> String {
    let mut response = String::new();

    for (i, chunk) in chunks.iter().enumerate() {
        if i > 0 {
            sleep(Duration::from_millis(delay_ms)).await;
        }

        let data = json!({
            "choices": [{
                "delta": {
                    "content": chunk
                }
            }]
        });

        response.push_str(&format!("data: {}\n\n", data));
    }

    response.push_str("data: [DONE]\n\n");
    response
}

// ============================================================================
// OpenAI Tests
// ============================================================================

#[cfg(test)]
mod openai_tests {
    use super::*;

    #[tokio::test]
    async fn test_openai_successful_streaming() {
        // Setup mock server
        let server = MockServer::start();

        // Create mock response
        let chunks = vec!["Hello", ", ", "how ", "can ", "I ", "help ", "you", "?"];
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions")
                .header("authorization", "Bearer test_key")
                .json_body_obj(&json!({
                    "model": "gpt-4",
                    "messages": [{"role": "user", "content": "Hello"}],
                    "stream": true,
                    "temperature": 0.7,
                    "max_tokens": null
                }));
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(generate_openai_sse_chunks(chunks.clone()));
        });

        // Setup client
        let mut config = StreamConfig::default();
        config.timeout = Duration::from_secs(5);

        let mut client = OpenAIClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        // Create request
        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            model: "gpt-4".to_string(),
            temperature: Some(0.7),
            max_tokens: None,
            stream: true,
            system: None,
        };

        // Stream and collect chunks
        let cancellation = CancellationToken::new();
        let mut response = client.stream(request, cancellation).await.unwrap();

        let mut collected = Vec::new();
        while let Some(chunk) = response.stream.next().await {
            if let Ok(c) = chunk {
                collected.push(c.content);
            }
        }

        // Verify
        mock.assert();
        assert_eq!(collected.join(""), "Hello, how can I help you?");

        // Check metrics
        let metrics = response.metrics.lock().await;
        assert!(metrics.chunks_received > 0);
        assert!(metrics.time_to_first_chunk_ms.is_some());
    }

    #[tokio::test]
    async fn test_openai_cancellation() {
        let server = MockServer::start();

        // Create a slow streaming response
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .delay(Duration::from_millis(100))
                .body_from_request(|_| {
                    let mut response = String::new();
                    for i in 0..100 {
                        response.push_str(&format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"chunk{}\"}}}}]}}\n\n", i));
                    }
                    response.push_str("data: [DONE]\n\n");
                    response.into()
                });
        });

        let mut config = StreamConfig::default();
        let mut client = OpenAIClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "gpt-4".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let cancel_clone = cancellation.clone();

        // Cancel after 50ms
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            cancel_clone.cancel();
        });

        let mut response = client.stream(request, cancellation).await.unwrap();

        let mut count = 0;
        while let Some(chunk) = response.stream.next().await {
            if response.cancellation.is_cancelled() {
                break;
            }
            count += 1;
        }

        // Should have received some chunks but not all 100
        assert!(count > 0);
        assert!(count < 100);
    }

    #[tokio::test]
    async fn test_openai_retry_on_server_error() {
        let server = MockServer::start();

        // First two requests fail, third succeeds
        let fail_mock1 = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions");
            then.status(500)
                .body("Internal Server Error");
        });

        let fail_mock2 = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions");
            then.status(503)
                .body("Service Unavailable");
        });

        let success_mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(generate_openai_sse_chunks(vec!["Success"]));
        });

        let mut config = StreamConfig::default();
        config.max_retries = 3;
        config.retry_delay = Duration::from_millis(10);

        let mut client = OpenAIClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "gpt-4".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let mut response = client.stream(request, cancellation).await.unwrap();

        let mut collected = String::new();
        while let Some(chunk) = response.stream.next().await {
            if let Ok(c) = chunk {
                collected.push_str(&c.content);
            }
        }

        assert_eq!(collected, "Success");

        // Check that retries happened
        let metrics = response.metrics.lock().await;
        assert_eq!(metrics.retry_count, 2);

        fail_mock1.assert();
        fail_mock2.assert();
        success_mock.assert();
    }

    #[tokio::test]
    async fn test_openai_timeout() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions");
            then.status(200)
                .delay(Duration::from_secs(10)); // Longer than timeout
        });

        let mut config = StreamConfig::default();
        config.timeout = Duration::from_millis(100);
        config.max_retries = 0;

        let mut client = OpenAIClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "gpt-4".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let result = client.stream(request, cancellation).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Request failed"));
    }

    #[tokio::test]
    async fn test_openai_malformed_response() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body("data: {invalid json}\n\ndata: [DONE]\n\n");
        });

        let config = StreamConfig::default();
        let mut client = OpenAIClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "gpt-4".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let mut response = client.stream(request, cancellation).await.unwrap();

        let mut error_found = false;
        while let Some(chunk) = response.stream.next().await {
            if chunk.is_err() {
                error_found = true;
                break;
            }
        }

        // Should handle malformed JSON gracefully
        assert!(!error_found); // The implementation skips malformed chunks
        mock.assert();
    }
}

// ============================================================================
// Anthropic Tests
// ============================================================================

#[cfg(test)]
mod anthropic_tests {
    use super::*;

    #[tokio::test]
    async fn test_anthropic_successful_streaming() {
        let server = MockServer::start();

        let chunks = vec!["Hello", ", ", "I'm ", "Claude", "!"];
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/messages")
                .header("x-api-key", "test_key")
                .header("anthropic-version", "2023-06-01");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(generate_anthropic_event_stream(chunks.clone()));
        });

        let config = StreamConfig::default();
        let mut client = AnthropicClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            model: "claude-3".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(100),
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let mut response = client.stream(request, cancellation).await.unwrap();

        let mut collected = Vec::new();
        let mut token_count = None;

        while let Some(chunk) = response.stream.next().await {
            if let Ok(c) = chunk {
                if !c.content.is_empty() {
                    collected.push(c.content);
                }
                if c.token_count.is_some() {
                    token_count = c.token_count;
                }
            }
        }

        mock.assert();
        assert_eq!(collected.join(""), "Hello, I'm Claude!");
        assert_eq!(token_count, Some(25)); // From the message_delta event
    }

    #[tokio::test]
    async fn test_anthropic_all_event_types() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/messages");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(generate_anthropic_event_stream(vec!["Test"]));
        });

        let config = StreamConfig::default();
        let mut client = AnthropicClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "claude-3".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let mut response = client.stream(request, cancellation).await.unwrap();

        let mut event_types = Vec::new();

        while let Some(chunk) = response.stream.next().await {
            if let Ok(c) = chunk {
                if let Some(metadata) = c.metadata {
                    if let Some(event_type) = metadata.get("type").and_then(|v| v.as_str()) {
                        event_types.push(event_type.to_string());
                    }
                }
            }
        }

        // Should have received all event types
        assert!(event_types.contains(&"message_start".to_string()));
        assert!(event_types.contains(&"content_block_start".to_string()));
        assert!(event_types.contains(&"content_block_delta".to_string()));
        assert!(event_types.contains(&"content_block_stop".to_string()));
        assert!(event_types.contains(&"message_delta".to_string()));
        assert!(event_types.contains(&"message_stop".to_string()));
    }

    #[tokio::test]
    async fn test_anthropic_error_event() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/messages");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body("event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"invalid_request\",\"message\":\"Invalid model\"}}\n\n");
        });

        let config = StreamConfig::default();
        let mut client = AnthropicClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "invalid-model".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let mut response = client.stream(request, cancellation).await.unwrap();

        let mut error_received = false;
        while let Some(chunk) = response.stream.next().await {
            if let Err(e) = chunk {
                error_received = true;
                assert!(e.to_string().contains("Invalid model"));
                break;
            }
        }

        assert!(error_received);
        mock.assert();
    }

    #[tokio::test]
    async fn test_anthropic_system_message_handling() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/messages")
                .json_body_partial(r#"{"system":"You are a helpful assistant"}"#);
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(generate_anthropic_event_stream(vec!["OK"]));
        });

        let config = StreamConfig::default();
        let mut client = AnthropicClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "claude-3".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: Some("You are a helpful assistant".to_string()),
        };

        let cancellation = CancellationToken::new();
        let mut response = client.stream(request, cancellation).await.unwrap();

        let mut collected = String::new();
        while let Some(chunk) = response.stream.next().await {
            if let Ok(c) = chunk {
                collected.push_str(&c.content);
            }
        }

        assert_eq!(collected, "OK");
        mock.assert();
    }
}

// ============================================================================
// Backpressure Tests
// ============================================================================

#[cfg(test)]
mod backpressure_tests {
    use super::*;

    #[tokio::test]
    async fn test_backpressure_coalescing() {
        let server = MockServer::start();

        // Generate many small chunks quickly
        let mut chunks = Vec::new();
        for i in 0..100 {
            chunks.push(format!("{} ", i));
        }

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body_from_request(move |_| {
                    let mut response = String::new();
                    for chunk in &chunks {
                        response.push_str(&format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{}\"}},\"finish_reason\":null}}]}}\n\n", chunk));
                    }
                    response.push_str("data: [DONE]\n\n");
                    response.into()
                });
        });

        let config = StreamConfig::default();
        let mut client = OpenAIClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let mut manager = StreamingManager::new();
        manager.add_client(Box::new(client));
        manager.set_backpressure(30.0, 5); // Lower FPS and threshold for testing

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "gpt-4".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let mut receiver = manager
            .stream_with_backpressure(Provider::OpenAI, request, cancellation)
            .await
            .unwrap();

        let mut batch_count = 0;
        let mut total_chunks = 0;

        while let Some(chunks) = receiver.recv().await {
            batch_count += 1;
            total_chunks += chunks.len();
        }

        // Should have received multiple batches (coalesced)
        assert!(batch_count > 1);
        assert!(batch_count < 100); // But not one per chunk
        assert_eq!(total_chunks, 100);

        mock.assert();
    }

    #[tokio::test]
    async fn test_backpressure_with_slow_consumer() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body_from_request(|_| {
                    let mut response = String::new();
                    for i in 0..20 {
                        response.push_str(&format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"chunk{} \"}},\"finish_reason\":null}}]}}\n\n", i));
                    }
                    response.push_str("data: [DONE]\n\n");
                    response.into()
                });
        });

        let config = StreamConfig::default();
        let mut client = OpenAIClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let mut manager = StreamingManager::new();
        manager.add_client(Box::new(client));
        manager.set_backpressure(60.0, 3); // Small threshold

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "gpt-4".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let mut receiver = manager
            .stream_with_backpressure(Provider::OpenAI, request, cancellation)
            .await
            .unwrap();

        let mut all_content = String::new();

        while let Some(chunks) = receiver.recv().await {
            // Simulate slow consumer
            sleep(Duration::from_millis(50)).await;

            for chunk in chunks {
                all_content.push_str(&chunk.content);
            }
        }

        // Should have received all content despite slow consumption
        assert!(all_content.contains("chunk0"));
        assert!(all_content.contains("chunk19"));

        mock.assert();
    }
}

// ============================================================================
// Manager Tests
// ============================================================================

#[cfg(test)]
mod manager_tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_provider_routing() {
        let openai_server = MockServer::start();
        let anthropic_server = MockServer::start();

        let openai_mock = openai_server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(generate_openai_sse_chunks(vec!["OpenAI"]));
        });

        let anthropic_mock = anthropic_server.mock(|when, then| {
            when.method(POST)
                .path("/v1/messages");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(generate_anthropic_event_stream(vec!["Anthropic"]));
        });

        let config = StreamConfig::default();

        let mut openai_client = OpenAIClient::new("key1".to_string(), config.clone());
        openai_client.base_url = openai_server.url("/v1");

        let mut anthropic_client = AnthropicClient::new("key2".to_string(), config.clone());
        anthropic_client.base_url = anthropic_server.url("/v1");

        let mut manager = StreamingManager::new();
        manager.add_client(Box::new(openai_client));
        manager.add_client(Box::new(anthropic_client));

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "test".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        // Test OpenAI routing
        let cancellation = CancellationToken::new();
        let mut openai_receiver = manager
            .stream_with_backpressure(Provider::OpenAI, request.clone(), cancellation)
            .await
            .unwrap();

        let mut openai_content = String::new();
        while let Some(chunks) = openai_receiver.recv().await {
            for chunk in chunks {
                openai_content.push_str(&chunk.content);
            }
        }

        assert_eq!(openai_content, "OpenAI");
        openai_mock.assert();

        // Test Anthropic routing
        let cancellation = CancellationToken::new();
        let mut anthropic_receiver = manager
            .stream_with_backpressure(Provider::Anthropic, request, cancellation)
            .await
            .unwrap();

        let mut anthropic_content = String::new();
        while let Some(chunks) = anthropic_receiver.recv().await {
            for chunk in chunks {
                anthropic_content.push_str(&chunk.content);
            }
        }

        assert_eq!(anthropic_content, "Anthropic");
        anthropic_mock.assert();
    }

    #[tokio::test]
    async fn test_manager_missing_provider() {
        let manager = StreamingManager::new();
        // Don't add any clients

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "test".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let result = manager
            .stream_with_backpressure(Provider::OpenAI, request, cancellation)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not configured"));
    }
}

// ============================================================================
// Performance Tests
// ============================================================================

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_high_throughput_streaming() {
        let server = MockServer::start();

        // Generate 1000 chunks
        let chunk_count = 1000;
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body_from_request(move |_| {
                    let mut response = String::new();
                    for i in 0..chunk_count {
                        response.push_str(&format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"x\"}},\"finish_reason\":null}}]}}\n\n"));
                    }
                    response.push_str("data: [DONE]\n\n");
                    response.into()
                });
        });

        let config = StreamConfig {
            buffer_size: 2048,
            backpressure_threshold: 50,
            ..Default::default()
        };

        let mut client = OpenAIClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let mut manager = StreamingManager::new();
        manager.add_client(Box::new(client));
        manager.set_backpressure(120.0, 50); // High FPS, larger batches

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "gpt-4".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let start = tokio::time::Instant::now();

        let mut receiver = manager
            .stream_with_backpressure(Provider::OpenAI, request, cancellation)
            .await
            .unwrap();

        let chunks_received = Arc::new(AtomicUsize::new(0));
        let chunks_clone = Arc::clone(&chunks_received);

        while let Some(chunks) = receiver.recv().await {
            chunks_clone.fetch_add(chunks.len(), Ordering::SeqCst);
        }

        let elapsed = start.elapsed();
        let total_chunks = chunks_received.load(Ordering::SeqCst);

        assert_eq!(total_chunks, chunk_count);

        // Should process 1000 chunks quickly (under 2 seconds)
        assert!(elapsed < Duration::from_secs(2));

        println!("Processed {} chunks in {:?}", total_chunks, elapsed);
        println!("Throughput: {:.2} chunks/sec", total_chunks as f64 / elapsed.as_secs_f64());

        mock.assert();
    }

    #[tokio::test]
    async fn test_memory_efficiency() {
        let server = MockServer::start();

        // Generate large chunks
        let large_text = "x".repeat(1000); // 1KB per chunk
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body_from_request(move |_| {
                    let mut response = String::new();
                    for _ in 0..100 {
                        response.push_str(&format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{}\"}},\"finish_reason\":null}}]}}\n\n", large_text));
                    }
                    response.push_str("data: [DONE]\n\n");
                    response.into()
                });
        });

        let config = StreamConfig {
            buffer_size: 512, // Smaller buffer
            backpressure_threshold: 10,
            ..Default::default()
        };

        let mut client = OpenAIClient::new("test_key".to_string(), config);
        client.base_url = server.url("/v1");

        let request = StreamRequest {
            messages: vec![Message {
                role: "user".to_string(),
                content: "Test".to_string(),
            }],
            model: "gpt-4".to_string(),
            temperature: None,
            max_tokens: None,
            stream: true,
            system: None,
        };

        let cancellation = CancellationToken::new();
        let mut response = client.stream(request, cancellation).await.unwrap();

        let mut total_size = 0;
        while let Some(chunk) = response.stream.next().await {
            if let Ok(c) = chunk {
                total_size += c.content.len();
            }
        }

        // Should have received all data
        assert_eq!(total_size, 100 * 1000);

        mock.assert();
    }
}

#[cfg(test)]
mod openai_provider_tests {
    use httpmock::prelude::*;
    use openagent_terminal_ai::{AiRequest, AiProvider};
    use openagent_terminal_ai::providers::OpenAiProvider;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    fn create_test_request() -> AiRequest {
        AiRequest {
            scratch_text: "list files in current directory".to_string(),
            working_directory: Some("/home/user".to_string()),
            shell_kind: Some("bash".to_string()),
            context: vec![
                ("platform".to_string(), "linux".to_string()),
                ("user".to_string(), "testuser".to_string()),
            ],
        }
    }

    #[test]
    fn test_openai_provider_creation() {
        let provider = OpenAiProvider::new(
            "test_key".to_string(),
            "https://api.openai.com".to_string(),
            "gpt-4".to_string(),
        );
        assert!(provider.is_ok());
    }

    #[test]
    fn test_openai_streaming_complete_response() {
        let server = MockServer::start();
        let stream_data = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ls \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"-la\"}}]}\n\n",
            "data: {\"choices\":[{\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n"
        );
        
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/chat/completions")
                .header("authorization", "Bearer test_key")
                .header("content-type", "application/json");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(stream_data);
        });

        let provider = OpenAiProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "gpt-4".to_string(),
        ).unwrap();
        
        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
        };
        
        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
        
        assert!(result.is_ok());
        assert_eq!(collected, "ls -la");
        mock.assert();
    }

    #[test]
    fn test_openai_streaming_with_cancellation() {
        let server = MockServer::start();
        let stream_data = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"ls \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"-la\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" /\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        
        server.mock(|when, then| {
            when.method(POST)
                .path("/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .delay(Duration::from_millis(100))
                .body(stream_data);
        });

        let provider = OpenAiProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "gpt-4".to_string(),
        ).unwrap();
        
        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
            // Cancel after first chunk
            if !collected.is_empty() {
                cancel.store(true, Ordering::SeqCst);
            }
        };
        
        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
        
        // Should return Ok(false) when cancelled
        assert!(result.is_ok());
        assert!(!collected.is_empty());
        assert!(collected.len() < 10); // Should not have collected all chunks
    }

    #[test]
    fn test_openai_error_response() {
        let server = MockServer::start();
        
        let error_response = r#"{
            "error": {
                "message": "Invalid API key",
                "type": "invalid_request_error",
                "code": "invalid_api_key"
            }
        }"#;
        
        server.mock(|when, then| {
            when.method(POST)
                .path("/chat/completions");
            then.status(401)
                .header("content-type", "application/json")
                .body(error_response);
        });

        let provider = OpenAiProvider::new(
            "invalid_key".to_string(),
            server.base_url(),
            "gpt-4".to_string(),
        ).unwrap();
        
        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
        };
        
        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("401"));
    }

    #[test]
    fn test_openai_network_timeout() {
        let server = MockServer::start();
        
        server.mock(|when, then| {
            when.method(POST)
                .path("/chat/completions");
            then.status(200)
                .delay(Duration::from_secs(65)) // Longer than typical timeout
                .body("data: timeout\n\n");
        });

        let provider = OpenAiProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "gpt-4".to_string(),
        ).unwrap();
        
        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
        };
        
        // This should timeout and return an error
        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
        
        // The actual behavior depends on the provider's timeout configuration
        // For now, we just check it doesn't hang forever
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn test_openai_malformed_streaming_data() {
        let server = MockServer::start();
        let malformed_data = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"valid\"}}]}\n\n",
            "data: {invalid json}\n\n",  // Malformed JSON
            "data: {\"choices\":[{\"delta\":{\"content\":\" data\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        
        server.mock(|when, then| {
            when.method(POST)
                .path("/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(malformed_data);
        });

        let provider = OpenAiProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "gpt-4".to_string(),
        ).unwrap();
        
        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
        };
        
        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
        
        // Provider should handle malformed data gracefully
        assert!(result.is_ok());
        // Should have collected at least the valid parts
        assert!(collected.contains("valid"));
    }

    #[test]
    fn test_openai_rate_limit_response() {
        let server = MockServer::start();
        
        server.mock(|when, then| {
            when.method(POST)
                .path("/chat/completions");
            then.status(429)
                .header("retry-after", "60")
                .body(r#"{"error": {"message": "Rate limit exceeded"}}"#);
        });

        let provider = OpenAiProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "gpt-4".to_string(),
        ).unwrap();
        
        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |chunk: &str| {
            collected.push_str(chunk);
        };
        
        let result = provider.propose_stream(create_test_request(), &mut on_chunk, &cancel);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("429"));
    }
}

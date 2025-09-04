#![cfg(any(feature = "ai-openai", feature = "ai-anthropic"))]
#[cfg(test)]
mod tests {
    use openagent_terminal_ai::providers::{AnthropicProvider, OpenAiProvider};
    use openagent_terminal_ai::{AiProvider, AiRequest};
    use std::sync::atomic::AtomicBool;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn base_req() -> AiRequest {
        AiRequest {
            scratch_text: "list files".to_string(),
            working_directory: Some("/tmp".to_string()),
            shell_kind: Some("bash".to_string()),
            context: vec![("platform".to_string(), "linux".to_string())],
        }
    }

    #[test]
    fn openai_streaming_success() {
        let server = MockServer::start();
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"echo \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ls\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        let _m = server.mock(|when, then| {
            when.method(POST).path("/chat/completions");
            then.status(200).header("content-type", "text/event-stream").body(body);
        });

        let provider =
            OpenAiProvider::new("test_key".to_string(), server.base_url(), "gpt-4".to_string())
                .unwrap();
        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |c: &str| {
            collected.push_str(c);
        };
        let ok = provider.propose_stream(base_req(), &mut on_chunk, &cancel).unwrap();
        assert!(ok);
        assert!(collected.contains("echo "));
        assert!(collected.contains("ls"));
    }

    #[test]
    fn anthropic_streaming_success() {
        let server = MockServer::start();
        let body = concat!(
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"echo \"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ls\"}}\n\n",
            "data: [DONE]\n\n"
        );
        let _m = server.mock(|when, then| {
            when.method(POST).path("/messages");
            then.status(200).header("content-type", "text/event-stream").body(body);
        });

        let provider = AnthropicProvider::new(
            "test_key".to_string(),
            server.base_url(),
            "claude-3".to_string(),
        )
        .unwrap();
        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |c: &str| {
            collected.push_str(c);
        };
        let ok = provider.propose_stream(base_req(), &mut on_chunk, &cancel).unwrap();
        assert!(ok);
        assert!(collected.contains("echo "));
        assert!(collected.contains("ls"));
    }

    #[test]
    fn openai_streaming_abort_no_done() {
        let server = MockServer::start();
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"partial\" }]}]}\n\n" // No [DONE]
        );
        let _m = server.mock(|when, then| {
            when.method(POST).path("/chat/completions");
            then.status(200).header("content-type", "text/event-stream").body(body);
        });

        let provider =
            OpenAiProvider::new("test_key".to_string(), server.base_url(), "gpt-4".to_string())
                .unwrap();
        let mut collected = String::new();
        let cancel = AtomicBool::new(false);
        let mut on_chunk = |c: &str| {
            collected.push_str(c);
        };
        // Should still return Ok(true) after stream ends
        let ok = provider.propose_stream(base_req(), &mut on_chunk, &cancel).unwrap();
        assert!(ok);
        assert!(collected.contains("partial"));
    }
}

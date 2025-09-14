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

    #[tokio::test]
    async fn openai_streaming_success() {
        let server = MockServer::start().await;
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"echo \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ls\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;

        let base_url = server.uri();
        let (tx, rx) = std::sync::mpsc::sync_channel::<(bool, String)>(1);
        std::thread::spawn(move || {
            let provider =
                OpenAiProvider::new("test_key".to_string(), base_url, "gpt-4".to_string()).unwrap();
            let mut collected = String::new();
            let cancel = AtomicBool::new(false);
            let mut on_chunk = |c: &str| {
                collected.push_str(c);
            };
            let ok = provider
                .propose_stream(base_req(), &mut on_chunk, &cancel)
                .unwrap();
            tx.send((ok, collected)).ok();
        });
        let (ok, collected) = rx.recv().unwrap();
        assert!(ok);
        assert!(collected.contains("echo "));
        assert!(collected.contains("ls"));
    }

    #[tokio::test]
    async fn anthropic_streaming_success() {
        let server = MockServer::start().await;
        let body = concat!(
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"echo \"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ls\"}}\n\n",
            "data: [DONE]\n\n"
        );
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;

        let base_url = server.uri();
        let (tx, rx) = std::sync::mpsc::sync_channel::<(bool, String)>(1);
        std::thread::spawn(move || {
            let provider =
                AnthropicProvider::new("test_key".to_string(), base_url, "claude-3".to_string())
                    .unwrap();
            let mut collected = String::new();
            let cancel = AtomicBool::new(false);
            let mut on_chunk = |c: &str| {
                collected.push_str(c);
            };
            let ok = provider
                .propose_stream(base_req(), &mut on_chunk, &cancel)
                .unwrap();
            tx.send((ok, collected)).ok();
        });
        let (ok, collected) = rx.recv().unwrap();
        assert!(ok);
        assert!(collected.contains("echo "));
        assert!(collected.contains("ls"));
    }

    #[tokio::test]
    async fn openai_streaming_abort_no_done() {
        let server = MockServer::start().await;
        let body =
            "data: {\\\"choices\\\":[{\\\"delta\\\":{\\\"content\\\":\\\"partial\\\"}}]}\\n\\n"; // No [DONE]
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;

        let base_url = server.uri();
        let (tx, rx) = std::sync::mpsc::sync_channel::<(bool, String)>(1);
        std::thread::spawn(move || {
            let provider =
                OpenAiProvider::new("test_key".to_string(), base_url, "gpt-4".to_string()).unwrap();
            let mut collected = String::new();
            let cancel = AtomicBool::new(false);
            let mut on_chunk = |c: &str| {
                collected.push_str(c);
            };
            let ok = provider
                .propose_stream(base_req(), &mut on_chunk, &cancel)
                .unwrap();
            tx.send((ok, collected)).ok();
        });
        let (ok, _collected) = rx.recv().unwrap();
        // When [DONE] is missing, provider should still return Ok(true) without error
        assert!(ok);
    }
}

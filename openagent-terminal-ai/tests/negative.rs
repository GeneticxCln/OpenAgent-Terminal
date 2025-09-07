use openagent_terminal_ai::privacy::{sanitize_request, AiPrivacyOptions};
use openagent_terminal_ai::AiRequest;

#[test]
fn privacy_sanitizes_paths_and_secrets() {
    let req = AiRequest {
        scratch_text: "use OPENAI_API_KEY=sk-123 and go to /home/user/project".to_string(),
        working_directory: Some("/home/user/project".to_string()),
        shell_kind: Some("bash".to_string()),
        context: vec![
            ("HOME".to_string(), "/home/user".to_string()),
            ("MY_SECRET_TOKEN".to_string(), "abc123".to_string()),
        ],
    };
    let opts = AiPrivacyOptions { strip_sensitive: true, strip_cwd: true };
    let out = sanitize_request(&req, opts);

    assert!(out.scratch_text.contains("[REDACTED]"));
    assert!(out.working_directory.unwrap().contains("[REDACTED]"));
    assert!(out.context.iter().any(|(k, v)| k == "MY_SECRET_TOKEN" && v == "[REDACTED]"));
}

#[cfg(feature = "ai-openai")]
mod http_tests {
    use super::*;
    use openagent_terminal_ai::providers::OpenAiProvider;
    use openagent_terminal_ai::AiProvider;
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
    fn openai_network_failure_returns_err() {
        let provider = OpenAiProvider::new(
            "test_key".to_string(),
            "http://127.0.0.1:9".to_string(),
            "gpt-3.5-turbo".to_string(),
        )
        .unwrap();
        let res = provider.propose(base_req());
        assert!(res.is_err());
    }

#[tokio::test]
async fn openai_5xx_is_error() {
let server = MockServer::start().await;
Mock::given(method("POST")).and(path("/chat/completions")).respond_with(
                ResponseTemplate::new(500).set_body_string("internal error"),
            )
            .mount(&server)
            .await;

let provider = OpenAiProvider::new(
            "test_key".to_string(),
            server.uri(),
            "gpt-3.5-turbo".to_string(),
        )
        .unwrap();
        let res = provider.propose(base_req());
        assert!(res.is_err());
    }

#[tokio::test]
async fn openai_malformed_json_is_error() {
let server = MockServer::start().await;
Mock::given(method("POST")).and(path("/chat/completions")).respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string("not json"),
            )
            .mount(&server)
            .await;

let provider = OpenAiProvider::new(
            "test_key".to_string(),
            server.uri(),
            "gpt-3.5-turbo".to_string(),
        )
        .unwrap();
        let res = provider.propose(base_req());
        assert!(res.is_err());
    }
}

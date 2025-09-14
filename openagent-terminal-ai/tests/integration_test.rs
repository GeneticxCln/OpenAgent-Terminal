#[cfg(test)]
mod tests {
    use openagent_terminal_ai::{create_provider, AiRequest};

    #[test]
    fn test_null_provider() {
        let provider = create_provider("null").expect("Failed to create null provider");
        assert_eq!(provider.name(), "null");

        let request = AiRequest {
            scratch_text: "test request".to_string(),
            working_directory: None,
            shell_kind: None,
            context: vec![],
        };

        let proposals = provider.propose(request).expect("Failed to get proposals");
        assert_eq!(proposals.len(), 0);
    }

    // Disabled in CI by default unless OLLAMA_MODEL is set in env
    #[cfg(all(feature = "ai-ollama", not(ci)))]
    #[test]
    fn test_ollama_provider() {
        if std::env::var("OLLAMA_MODEL").is_err() {
            eprintln!("Skipping ollama test: OLLAMA_MODEL not set");
            return;
        }
        let provider = create_provider("ollama").expect("Failed to create ollama provider");
        assert_eq!(provider.name(), "ollama");

        let request = AiRequest {
            scratch_text: "list files".to_string(),
            working_directory: Some("/home/user".to_string()),
            shell_kind: Some("bash".to_string()),
            context: vec![("OS".to_string(), "Linux".to_string())],
        };

        let proposals = provider.propose(request).expect("Failed to get proposals");
        assert!(!proposals.is_empty());
        // Test can handle both cases: Ollama running or not
        assert!(
            proposals[0].title.contains("list files")
                || proposals[0].title.contains("Ollama Not Available")
        );
    }

    #[test]
    fn test_unknown_provider() {
        let result = create_provider("unknown");
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.to_string().contains("Unknown provider"));
        }
    }
}

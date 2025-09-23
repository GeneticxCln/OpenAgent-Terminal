#![cfg(feature = "ai")]

use openagent_terminal::ai_runtime::AiRuntime;
use openagent_terminal::config::ai::ProviderConfig;
use openagent_terminal::config::UiConfig;
use openagent_terminal_ai::AiRequest;

#[test]
fn switch_provider_from_null_to_ollama() {
    // Build a config with an Ollama default model/endpoint so no env is required
    let mut cfg = UiConfig::default();
    let ollama_cfg = ProviderConfig {
        api_key_env: None,
        endpoint_env: None,
        model_env: None,
        default_endpoint: Some("http://localhost:11434".to_string()),
        default_model: Some("codellama".to_string()),
        extra: std::collections::HashMap::new(),
    };
    cfg.ai.providers.insert("ollama".to_string(), ollama_cfg.clone());

    // Start runtime with the null provider
    let mut rt = AiRuntime::from_secure_config("null", &ProviderConfig::default());

    // Switch to ollama using the convenience method
    let res = rt.set_provider_by_name("ollama", &cfg);
    assert!(res.is_ok(), "set_provider_by_name failed: {:?}", res);
    assert_eq!(rt.ui.current_provider, "ollama");
    assert_eq!(rt.ui.current_model, ollama_cfg.default_model.unwrap());

    // Ensure propose paths still work in principle (no panic). Use minimal request
    let _ = rt.provider.propose(AiRequest {
        scratch_text: "echo hi".to_string(),
        working_directory: None,
        shell_kind: None,
        context: vec![("platform".to_string(), std::env::consts::OS.to_string())],
    });
}

#[cfg(feature = "ai")]
mod ai_provider_security_tests {
    use openagent_terminal::config::ai::{AiConfig, ProviderConfig};
    use openagent_terminal::config::ai_providers::{ProviderCredentials, validate_provider_isolation};
    use std::collections::HashMap;
    
    /// Test that providers with different credentials don't share API keys
    #[test]
    fn test_provider_credential_isolation() {
        // Set up test environment variables for different providers
        std::env::set_var("TEST_OPENAI_KEY", "openai-secret-key");
        std::env::set_var("TEST_ANTHROPIC_KEY", "anthropic-secret-key");
        std::env::set_var("TEST_OLLAMA_MODEL", "test-model");
        
        let openai_config = ProviderConfig {
            api_key_env: Some("TEST_OPENAI_KEY".to_string()),
            model_env: Some("TEST_OPENAI_MODEL".to_string()),
            default_model: Some("gpt-3.5-turbo".to_string()),
            default_endpoint: Some("https://api.openai.com/v1".to_string()),
            ..Default::default()
        };
        
        let anthropic_config = ProviderConfig {
            api_key_env: Some("TEST_ANTHROPIC_KEY".to_string()),
            model_env: Some("TEST_ANTHROPIC_MODEL".to_string()),
            default_model: Some("claude-3-haiku".to_string()),
            default_endpoint: Some("https://api.anthropic.com/v1".to_string()),
            ..Default::default()
        };
        
        let ollama_config = ProviderConfig {
            api_key_env: None, // No API key for local Ollama
            model_env: Some("TEST_OLLAMA_MODEL".to_string()),
            default_endpoint: Some("http://localhost:11434".to_string()),
            ..Default::default()
        };
        
        let openai_creds = ProviderCredentials::from_config("openai", &openai_config).unwrap();
        let anthropic_creds = ProviderCredentials::from_config("anthropic", &anthropic_config).unwrap();
        let ollama_creds = ProviderCredentials::from_config("ollama", &ollama_config).unwrap();
        
        // Verify that credentials are properly isolated
        assert_eq!(openai_creds.api_key.as_ref().unwrap(), "openai-secret-key");
        assert_eq!(anthropic_creds.api_key.as_ref().unwrap(), "anthropic-secret-key");
        assert!(ollama_creds.api_key.is_none()); // Ollama doesn't use API keys
        
        // Verify models are isolated
        assert_eq!(openai_creds.model.as_ref().unwrap(), "gpt-3.5-turbo");
        assert_eq!(anthropic_creds.model.as_ref().unwrap(), "claude-3-haiku");
        assert_eq!(ollama_creds.model.as_ref().unwrap(), "test-model");
        
        // Test provider isolation validation
        let mut providers = HashMap::new();
        providers.insert("openai".to_string(), openai_creds);
        providers.insert("anthropic".to_string(), anthropic_creds);
        providers.insert("ollama".to_string(), ollama_creds);
        
        assert!(validate_provider_isolation(&providers).is_ok());
        
        // Clean up test environment
        std::env::remove_var("TEST_OPENAI_KEY");
        std::env::remove_var("TEST_ANTHROPIC_KEY");
        std::env::remove_var("TEST_OLLAMA_MODEL");
    }
    
    /// Test detection of credential leakage between providers
    #[test]
    fn test_credential_leakage_detection() {
        let mut providers = HashMap::new();
        
        // Create providers that accidentally share the same API key
        let leaked_creds_1 = ProviderCredentials {
            api_key: Some("shared-leaked-key".to_string()),
            endpoint: Some("endpoint1".to_string()),
            model: Some("model1".to_string()),
            extra: HashMap::new(),
        };
        
        let leaked_creds_2 = ProviderCredentials {
            api_key: Some("shared-leaked-key".to_string()), // Same key - security violation!
            endpoint: Some("endpoint2".to_string()),
            model: Some("model2".to_string()),
            extra: HashMap::new(),
        };
        
        providers.insert("provider1".to_string(), leaked_creds_1);
        providers.insert("provider2".to_string(), leaked_creds_2);
        
        // Should detect the credential leakage
        let result = validate_provider_isolation(&providers);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Credential leakage detected"));
    }
    
    /// Test that missing required environment variables are properly handled
    #[test]
    fn test_missing_required_credentials() {
        let config = ProviderConfig {
            api_key_env: Some("MISSING_API_KEY".to_string()),
            model_env: Some("MISSING_MODEL".to_string()),
            ..Default::default()
        };
        
        // Should fail gracefully for missing required variables
        let result = ProviderCredentials::from_config("test_provider", &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not set"));
    }
    
    /// Test that default values work when environment variables are missing
    #[test]
    fn test_default_values_fallback() {
        let config = ProviderConfig {
            api_key_env: Some("MISSING_API_KEY".to_string()),
            endpoint_env: Some("MISSING_ENDPOINT".to_string()),
            model_env: Some("MISSING_MODEL".to_string()),
            default_endpoint: Some("https://default.endpoint.com".to_string()),
            default_model: Some("default-model".to_string()),
            ..Default::default()
        };
        
        // Should fail for missing API key but use defaults for endpoint/model
        let result = ProviderCredentials::from_config("test_provider", &config);
        assert!(result.is_err()); // Should fail due to missing API key
        
        // Test with API key provided
        std::env::set_var("MISSING_API_KEY", "test-key");
        let result = ProviderCredentials::from_config("test_provider", &config).unwrap();
        
        assert_eq!(result.api_key.as_ref().unwrap(), "test-key");
        assert_eq!(result.endpoint.as_ref().unwrap(), "https://default.endpoint.com");
        assert_eq!(result.model.as_ref().unwrap(), "default-model");
        
        std::env::remove_var("MISSING_API_KEY");
    }
    
    /// Test that global environment pollution doesn't occur
    #[test]
    fn test_no_global_environment_pollution() {
        // Set test credentials
        std::env::set_var("TEST_SECURE_OPENAI_KEY", "secure-openai-key");
        std::env::set_var("TEST_SECURE_ANTHROPIC_KEY", "secure-anthropic-key");
        
        let openai_config = ProviderConfig {
            api_key_env: Some("TEST_SECURE_OPENAI_KEY".to_string()),
            model_env: Some("TEST_MODEL".to_string()),
            default_model: Some("gpt-3.5-turbo".to_string()),
            ..Default::default()
        };
        
        let anthropic_config = ProviderConfig {
            api_key_env: Some("TEST_SECURE_ANTHROPIC_KEY".to_string()),
            model_env: Some("TEST_MODEL".to_string()),
            default_model: Some("claude-3-haiku".to_string()),
            ..Default::default()
        };
        
        // Before: Check that legacy env vars are NOT set
        assert!(std::env::var("OPENAI_API_KEY").is_err());
        assert!(std::env::var("ANTHROPIC_API_KEY").is_err());
        
        // Create credentials (this should NOT pollute global environment)
        let _openai_creds = ProviderCredentials::from_config("openai", &openai_config).unwrap();
        let _anthropic_creds = ProviderCredentials::from_config("anthropic", &anthropic_config).unwrap();
        
        // After: Verify global environment is still clean
        assert!(std::env::var("OPENAI_API_KEY").is_err());
        assert!(std::env::var("ANTHROPIC_API_KEY").is_err());
        
        // But our test variables should still exist
        assert_eq!(std::env::var("TEST_SECURE_OPENAI_KEY").unwrap(), "secure-openai-key");
        assert_eq!(std::env::var("TEST_SECURE_ANTHROPIC_KEY").unwrap(), "secure-anthropic-key");
        
        // Clean up
        std::env::remove_var("TEST_SECURE_OPENAI_KEY");
        std::env::remove_var("TEST_SECURE_ANTHROPIC_KEY");
    }
    
    /// Integration test: verify secure runtime creation
    #[test]
    fn test_secure_runtime_creation() {
        // Set up secure test environment
        std::env::set_var("TEST_RUNTIME_KEY", "test-runtime-key");
        std::env::set_var("TEST_RUNTIME_MODEL", "test-model");
        
        let config = ProviderConfig {
            api_key_env: Some("TEST_RUNTIME_KEY".to_string()),
            model_env: Some("TEST_RUNTIME_MODEL".to_string()),
            default_endpoint: Some("https://test.api.com".to_string()),
            ..Default::default()
        };
        
        // This should work without polluting global environment
        // Note: We can't test actual provider creation without implementing mock providers
        let credentials = ProviderCredentials::from_config("test", &config).unwrap();
        
        assert_eq!(credentials.api_key.as_ref().unwrap(), "test-runtime-key");
        assert_eq!(credentials.model.as_ref().unwrap(), "test-model");
        assert_eq!(credentials.endpoint.as_ref().unwrap(), "https://test.api.com");
        
        // Verify global env is not polluted
        assert!(std::env::var("OPENAI_API_KEY").is_err());
        assert!(std::env::var("ANTHROPIC_API_KEY").is_err());
        assert!(std::env::var("OLLAMA_ENDPOINT").is_err());
        
        std::env::remove_var("TEST_RUNTIME_KEY");
        std::env::remove_var("TEST_RUNTIME_MODEL");
    }
}

#[cfg(feature = "ai")]
mod legacy_detection_tests {
    use openagent_terminal::config::ai_providers::check_legacy_env_vars;
    
    /// Test that legacy environment variable detection works
    #[test]
    fn test_legacy_detection() {
        // Set some legacy variables
        std::env::set_var("OPENAI_API_KEY", "legacy-key");
        std::env::set_var("OLLAMA_ENDPOINT", "legacy-endpoint");
        
        // This should trigger warnings (we can't easily test the warning output here)
        // but the function should not panic
        check_legacy_env_vars();
        
        // Clean up
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("OLLAMA_ENDPOINT");
    }
}

#![allow(dead_code)]
#[cfg(feature = "ai")]
use super::ai::ProviderConfig;
#[cfg(not(feature = "ai"))]
use serde::{Deserialize, Serialize};
#[cfg(not(feature = "ai"))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key_env: Option<String>,
    pub endpoint_env: Option<String>,
    pub model_env: Option<String>,
    pub default_endpoint: Option<String>,
    pub default_model: Option<String>,
    pub extra: HashMap<String, String>,
}
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, warn};

/// Secure provider credentials container that never mutates global environment
#[derive(Debug, Clone)]
pub struct ProviderCredentials {
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub model: Option<String>,
    #[allow(dead_code)]
    pub extra: HashMap<String, String>,
}

impl ProviderCredentials {
    /// Create credentials from provider config without polluting global environment
    pub fn from_config(provider_name: &str, config: &ProviderConfig) -> Result<Self, String> {
        let mut extra = HashMap::new();

        // Resolve API key
        let api_key = if let Some(env_name) = &config.api_key_env {
            match std::env::var(env_name) {
                Ok(key) => {
                    debug!(
                        "Found API key for provider '{}' from env '{}'",
                        provider_name, env_name
                    );
                    Some(key)
                }
                Err(_) => {
                    // Fallback: check secure secrets store
                    match read_secret_from_store(env_name) {
                        Some(key) => {
                            debug!(
                                "Found API key for provider '{}' in secrets store ({}).",
                                provider_name, env_name
                            );
                            Some(key)
                        }
                        None => {
                            return Err(format!(
                                "API key environment variable '{}' not set for provider '{}'",
                                env_name, provider_name
                            ));
                        }
                    }
                }
            }
        } else {
            None
        };

        // Resolve endpoint
        let endpoint = if let Some(env_name) = &config.endpoint_env {
            match std::env::var(env_name) {
                Ok(endpoint) => {
                    debug!(
                        "Found endpoint for provider '{}' from env '{}'",
                        provider_name, env_name
                    );
                    Some(endpoint)
                }
                Err(_) => {
                    if let Some(val) = read_secret_from_store(env_name) {
                        debug!(
                            "Using endpoint for provider '{}' from secrets store ({}).",
                            provider_name, env_name
                        );
                        Some(val)
                    } else if let Some(default) = &config.default_endpoint {
                        debug!(
                            "Using default endpoint for provider '{}': {}",
                            provider_name, default
                        );
                        Some(default.clone())
                    } else {
                        warn!(
                            "No endpoint found for provider '{}' (env: {})",
                            provider_name, env_name
                        );
                        None
                    }
                }
            }
        } else {
            config.default_endpoint.clone()
        };

        // Resolve model
        let model = if let Some(env_name) = &config.model_env {
            match std::env::var(env_name) {
                Ok(model) => {
                    debug!(
                        "Found model for provider '{}' from env '{}'",
                        provider_name, env_name
                    );
                    Some(model)
                }
                Err(_) => {
                    if let Some(val) = read_secret_from_store(env_name) {
                        debug!(
                            "Using model for provider '{}' from secrets store ({}).",
                            provider_name, env_name
                        );
                        Some(val)
                    } else if let Some(default) = &config.default_model {
                        debug!(
                            "Using default model for provider '{}': {}",
                            provider_name, default
                        );
                        Some(default.clone())
                    } else {
                        return Err(format!(
                            "Model environment variable '{}' not set for provider '{}' and no \
                             default provided",
                            env_name, provider_name
                        ));
                    }
                }
            }
        } else {
            config.default_model.clone()
        };

        // Copy extra configuration
        for (key, value) in &config.extra {
            extra.insert(key.clone(), value.clone());
        }

        Ok(Self {
            api_key,
            endpoint,
            model,
            extra,
        })
    }

    /// Get API key with validation
    pub fn require_api_key(&self, provider_name: &str) -> Result<&str, String> {
        self.api_key
            .as_deref()
            .ok_or_else(|| format!("API key is required for provider '{}'", provider_name))
    }

    /// Get endpoint with validation
    pub fn require_endpoint(&self, provider_name: &str) -> Result<&str, String> {
        self.endpoint
            .as_deref()
            .ok_or_else(|| format!("Endpoint is required for provider '{}'", provider_name))
    }

    /// Get model with validation
    pub fn require_model(&self, provider_name: &str) -> Result<&str, String> {
        self.model
            .as_deref()
            .ok_or_else(|| format!("Model is required for provider '{}'", provider_name))
    }
}

/// Default provider configurations with secure environment variable names
pub fn get_default_provider_configs() -> HashMap<String, ProviderConfig> {
    let mut configs = HashMap::new();

    // OpenAI configuration
    configs.insert(
        "openai".to_string(),
        ProviderConfig {
            api_key_env: Some("OPENAGENT_OPENAI_API_KEY".to_string()),
            endpoint_env: Some("OPENAGENT_OPENAI_ENDPOINT".to_string()),
            model_env: Some("OPENAGENT_OPENAI_MODEL".to_string()),
            default_endpoint: Some("https://api.openai.com/v1".to_string()),
            default_model: Some("gpt-3.5-turbo".to_string()),
            extra: HashMap::new(),
        },
    );

    // Anthropic configuration
    configs.insert(
        "anthropic".to_string(),
        ProviderConfig {
            api_key_env: Some("OPENAGENT_ANTHROPIC_API_KEY".to_string()),
            endpoint_env: Some("OPENAGENT_ANTHROPIC_ENDPOINT".to_string()),
            model_env: Some("OPENAGENT_ANTHROPIC_MODEL".to_string()),
            default_endpoint: Some("https://api.anthropic.com/v1".to_string()),
            default_model: Some("claude-3-haiku-20240307".to_string()),
            extra: HashMap::new(),
        },
    );

    // Ollama configuration
    configs.insert(
        "ollama".to_string(),
        ProviderConfig {
            api_key_env: None, // Ollama typically doesn't require API keys
            endpoint_env: Some("OPENAGENT_OLLAMA_ENDPOINT".to_string()),
            model_env: Some("OPENAGENT_OLLAMA_MODEL".to_string()),
            default_endpoint: Some("http://localhost:11434".to_string()),
            default_model: Some("codellama".to_string()),
            extra: HashMap::new(),
        },
    );

    // OpenRouter configuration
    configs.insert(
        "openrouter".to_string(),
        ProviderConfig {
            api_key_env: Some("OPENAGENT_OPENROUTER_API_KEY".to_string()),
            endpoint_env: Some("OPENAGENT_OPENROUTER_ENDPOINT".to_string()),
            model_env: Some("OPENAGENT_OPENROUTER_MODEL".to_string()),
            default_endpoint: Some("https://openrouter.ai/api/v1".to_string()),
            default_model: None, // Force explicit model configuration by default
            extra: HashMap::new(),
        },
    );

    configs
}

/// Legacy environment variable compatibility (with deprecation warnings)
#[allow(dead_code)]
pub fn check_legacy_env_vars() {
    let legacy_vars = [
        "OPENAI_API_KEY",
        "OPENAI_API_BASE",
        "OPENAI_MODEL",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_API_BASE",
        "ANTHROPIC_MODEL",
        "OLLAMA_ENDPOINT",
        "OLLAMA_MODEL",
        "OPENROUTER_API_KEY",
        "OPENROUTER_API_BASE",
        "OPENROUTER_MODEL",
    ];

    for var in &legacy_vars {
        if std::env::var(var).is_ok() {
            warn!(
                "Legacy environment variable '{}' detected. Please migrate to namespaced \
                 variables (OPENAGENT_*). See docs/AI_ENVIRONMENT_SECURITY.md for migration guide.",
                var
            );
        }
    }
}

/// Validate that credentials don't leak between providers
#[cfg(test)]
pub fn validate_provider_isolation(
    providers: &HashMap<String, ProviderCredentials>,
) -> Result<(), String> {
    // Check that no two providers share the same credentials inadvertently
    let mut seen_keys = std::collections::HashSet::new();

    for (provider_name, creds) in providers {
        if let Some(ref api_key) = creds.api_key {
            if seen_keys.contains(api_key) {
                return Err(format!(
                    "Credential leakage detected: Provider '{}' shares API key with another \
                     provider",
                    provider_name
                ));
            }
            seen_keys.insert(api_key.clone());
        }
    }

    Ok(())
}

fn secrets_store_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("openagent-terminal").join("secrets.toml")
}

fn read_secret_from_store(env_name: &str) -> Option<String> {
    let path = secrets_store_path();
    let mut s = String::new();
    if let Ok(mut f) = fs::File::open(&path) {
        use std::io::Read;
        if f.read_to_string(&mut s).is_ok() {
            if let Ok(val) = toml::from_str::<toml::Value>(&s) {
                if let Some(tbl) = val.get("secrets").and_then(|v| v.as_table()) {
                    if let Some(v) = tbl.get(env_name).and_then(|v| v.as_str()) {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_credentials_isolation() {
        // Test that different providers get isolated credentials
        let mut providers = HashMap::new();

        let openai_config = ProviderConfig {
            api_key_env: Some("TEST_OPENAI_KEY".to_string()),
            model_env: Some("TEST_OPENAI_MODEL".to_string()),
            default_model: Some("gpt-4".to_string()),
            ..Default::default()
        };

        let anthropic_config = ProviderConfig {
            api_key_env: Some("TEST_ANTHROPIC_KEY".to_string()),
            model_env: Some("TEST_ANTHROPIC_MODEL".to_string()),
            default_model: Some("claude-3-sonnet".to_string()),
            ..Default::default()
        };

        // Set different test environment variables
        std::env::set_var("TEST_OPENAI_KEY", "openai-test-key");
        std::env::set_var("TEST_OPENAI_MODEL", "gpt-4");
        std::env::set_var("TEST_ANTHROPIC_KEY", "anthropic-test-key");
        std::env::set_var("TEST_ANTHROPIC_MODEL", "claude-3-sonnet");

        let openai_creds = ProviderCredentials::from_config("openai", &openai_config).unwrap();
        let anthropic_creds =
            ProviderCredentials::from_config("anthropic", &anthropic_config).unwrap();

        providers.insert("openai".to_string(), openai_creds);
        providers.insert("anthropic".to_string(), anthropic_creds);

        // Verify isolation
        assert!(validate_provider_isolation(&providers).is_ok());
        assert_ne!(providers["openai"].api_key, providers["anthropic"].api_key);

        // Clean up
        std::env::remove_var("TEST_OPENAI_KEY");
        std::env::remove_var("TEST_OPENAI_MODEL");
        std::env::remove_var("TEST_ANTHROPIC_KEY");
        std::env::remove_var("TEST_ANTHROPIC_MODEL");
    }

    #[test]
    fn test_credential_leakage_detection() {
        let mut providers = HashMap::new();

        // Create two providers with the same API key (simulating leakage)
        let creds1 = ProviderCredentials {
            api_key: Some("shared-key".to_string()),
            endpoint: Some("endpoint1".to_string()),
            model: Some("model1".to_string()),
            extra: HashMap::new(),
        };

        let creds2 = ProviderCredentials {
            api_key: Some("shared-key".to_string()), // Same key - should be detected
            endpoint: Some("endpoint2".to_string()),
            model: Some("model2".to_string()),
            extra: HashMap::new(),
        };

        providers.insert("provider1".to_string(), creds1);
        providers.insert("provider2".to_string(), creds2);

        // Should detect credential leakage
        assert!(validate_provider_isolation(&providers).is_err());
    }
}

// AI Providers Module - Async streaming support for multiple AI providers

use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{Result, anyhow};

/// AI Provider trait for common interface
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Get provider name
    fn name(&self) -> &str;

    /// Complete a prompt with streaming response
    async fn complete_stream(
        &self,
        prompt: &str,
        options: CompletionOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>>;

    /// Complete a prompt with full response
    async fn complete(
        &self,
        prompt: &str,
        options: CompletionOptions,
    ) -> Result<String>;

    /// Check if provider is available
    async fn health_check(&self) -> Result<bool>;

    /// Get usage statistics
    async fn get_usage(&self) -> Result<UsageStats>;
}

/// Completion options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionOptions {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: usize,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop_sequences: Vec<String>,
    pub system_prompt: Option<String>,
}

impl Default for CompletionOptions {
    fn default() -> Self {
        Self {
            model: "gpt-3.5-turbo".to_string(),
            temperature: 0.7,
            max_tokens: 2000,
            top_p: Some(1.0),
            frequency_penalty: None,
            presence_penalty: None,
            stop_sequences: vec![],
            system_prompt: None,
        }
    }
}

/// Usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    pub total_tokens: usize,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub requests_count: usize,
    pub estimated_cost: f64,
}

/// OpenAI Provider Implementation
pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    base_url: String,
    usage: Arc<RwLock<UsageStats>>,
}

impl OpenAIProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            usage: Arc::new(RwLock::new(UsageStats {
                total_tokens: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                requests_count: 0,
                estimated_cost: 0.0,
            })),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
}

#[async_trait]
impl AiProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "OpenAI"
    }

    async fn complete_stream(
        &self,
        prompt: &str,
        options: CompletionOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let messages = vec![
            serde_json::json!({
                "role": "system",
                "content": options.system_prompt.unwrap_or_else(|| "You are a helpful assistant.".to_string())
            }),
            serde_json::json!({
                "role": "user",
                "content": prompt
            })
        ];

        let request_body = serde_json::json!({
            "model": options.model,
            "messages": messages,
            "temperature": options.temperature,
            "max_tokens": options.max_tokens,
            "top_p": options.top_p,
            "frequency_penalty": options.frequency_penalty,
            "presence_penalty": options.presence_penalty,
            "stop": options.stop_sequences,
            "stream": true
        });

        let response = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("OpenAI API error: {}", error_text));
        }

        let stream = response.bytes_stream();
        let stream = stream.map(move |chunk| {
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    // Parse SSE format
                    if text.starts_with("data: ") {
                        let json_str = &text[6..];
                        if json_str.trim() == "[DONE]" {
                            return Ok(String::new());
                        }

                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                            if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                                return Ok(content.to_string());
                            }
                        }
                    }
                    Ok(String::new())
                }
                Err(e) => Err(anyhow!("Stream error: {}", e))
            }
        });

        // Update usage stats
        let mut usage = self.usage.write().await;
        usage.requests_count += 1;

        Ok(Box::pin(stream))
    }

    async fn complete(
        &self,
        prompt: &str,
        options: CompletionOptions,
    ) -> Result<String> {
        let mut stream = self.complete_stream(prompt, options).await?;
        let mut result = String::new();

        while let Some(chunk) = stream.next().await {
            result.push_str(&chunk?);
        }

        Ok(result)
    }

    async fn health_check(&self) -> Result<bool> {
        let response = self.client
            .get(format!("{}/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    async fn get_usage(&self) -> Result<UsageStats> {
        Ok(self.usage.read().await.clone())
    }
}

/// Anthropic Claude Provider
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    base_url: String,
    usage: Arc<RwLock<UsageStats>>,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://api.anthropic.com/v1".to_string(),
            usage: Arc::new(RwLock::new(UsageStats {
                total_tokens: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                requests_count: 0,
                estimated_cost: 0.0,
            })),
        }
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "Anthropic"
    }

    async fn complete_stream(
        &self,
        prompt: &str,
        options: CompletionOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let request_body = serde_json::json!({
            "model": options.model,
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "max_tokens": options.max_tokens,
            "temperature": options.temperature,
            "stream": true
        });

        let response = self.client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Anthropic API error: {}", error_text));
        }

        let stream = response.bytes_stream();
        let stream = stream.map(move |chunk| {
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    // Parse SSE format for Anthropic
                    if text.starts_with("data: ") {
                        let json_str = &text[6..];
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                            if let Some(content) = json["delta"]["text"].as_str() {
                                return Ok(content.to_string());
                            }
                        }
                    }
                    Ok(String::new())
                }
                Err(e) => Err(anyhow!("Stream error: {}", e))
            }
        });

        let mut usage = self.usage.write().await;
        usage.requests_count += 1;

        Ok(Box::pin(stream))
    }

    async fn complete(
        &self,
        prompt: &str,
        options: CompletionOptions,
    ) -> Result<String> {
        let mut stream = self.complete_stream(prompt, options).await?;
        let mut result = String::new();

        while let Some(chunk) = stream.next().await {
            result.push_str(&chunk?);
        }

        Ok(result)
    }

    async fn health_check(&self) -> Result<bool> {
        // Anthropic doesn't have a specific health endpoint
        Ok(true)
    }

    async fn get_usage(&self) -> Result<UsageStats> {
        Ok(self.usage.read().await.clone())
    }
}

/// Local Ollama Provider
pub struct OllamaProvider {
    client: Client,
    base_url: String,
    usage: Arc<RwLock<UsageStats>>,
}

impl OllamaProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "http://localhost:11434".to_string(),
            usage: Arc::new(RwLock::new(UsageStats {
                total_tokens: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                requests_count: 0,
                estimated_cost: 0.0,
            })),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn name(&self) -> &str {
        "Ollama"
    }

    async fn complete_stream(
        &self,
        prompt: &str,
        options: CompletionOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let request_body = serde_json::json!({
            "model": options.model,
            "prompt": prompt,
            "stream": true,
            "options": {
                "temperature": options.temperature,
                "num_predict": options.max_tokens,
            }
        });

        let response = self.client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Ollama API error"));
        }

        let stream = response.bytes_stream();
        let stream = stream.map(move |chunk| {
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(response) = json["response"].as_str() {
                            return Ok(response.to_string());
                        }
                    }
                    Ok(String::new())
                }
                Err(e) => Err(anyhow!("Stream error: {}", e))
            }
        });

        let mut usage = self.usage.write().await;
        usage.requests_count += 1;

        Ok(Box::pin(stream))
    }

    async fn complete(
        &self,
        prompt: &str,
        options: CompletionOptions,
    ) -> Result<String> {
        let mut stream = self.complete_stream(prompt, options).await?;
        let mut result = String::new();

        while let Some(chunk) = stream.next().await {
            result.push_str(&chunk?);
        }

        Ok(result)
    }

    async fn health_check(&self) -> Result<bool> {
        let response = self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    async fn get_usage(&self) -> Result<UsageStats> {
        Ok(self.usage.read().await.clone())
    }
}

/// AI Provider Manager
pub struct AiProviderManager {
    providers: HashMap<String, Box<dyn AiProvider>>,
    active_provider: String,
    config: AiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub default_provider: String,
    pub providers: HashMap<String, ProviderConfig>,
    pub default_options: CompletionOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub models: Vec<String>,
    pub rate_limit: Option<usize>,
}

impl AiProviderManager {
    pub fn new(config: AiConfig) -> Result<Self> {
        let mut providers: HashMap<String, Box<dyn AiProvider>> = HashMap::new();

        // Initialize enabled providers
        for (name, provider_config) in &config.providers {
            if !provider_config.enabled {
                continue;
            }

            let provider: Box<dyn AiProvider> = match name.as_str() {
                "openai" => {
                    let api_key = provider_config.api_key.as_ref()
                        .ok_or_else(|| anyhow!("OpenAI API key not configured"))?;
                    let mut provider = OpenAIProvider::new(api_key.clone());
                    if let Some(base_url) = &provider_config.base_url {
                        provider = provider.with_base_url(base_url.clone());
                    }
                    Box::new(provider)
                }
                "anthropic" => {
                    let api_key = provider_config.api_key.as_ref()
                        .ok_or_else(|| anyhow!("Anthropic API key not configured"))?;
                    Box::new(AnthropicProvider::new(api_key.clone()))
                }
                "ollama" => {
                    let mut provider = OllamaProvider::new();
                    if let Some(base_url) = &provider_config.base_url {
                        provider = provider.with_base_url(base_url.clone());
                    }
                    Box::new(provider)
                }
                _ => continue,
            };

            providers.insert(name.clone(), provider);
        }

        if providers.is_empty() {
            return Err(anyhow!("No AI providers configured"));
        }

        let active_provider = if providers.contains_key(&config.default_provider) {
            config.default_provider.clone()
        } else {
            providers.keys().next().unwrap().clone()
        };

        Ok(Self {
            providers,
            active_provider,
            config,
        })
    }

    /// Get active provider
    pub fn active_provider(&self) -> &dyn AiProvider {
        self.providers[&self.active_provider].as_ref()
    }

    /// Switch active provider
    pub fn switch_provider(&mut self, name: &str) -> Result<()> {
        if !self.providers.contains_key(name) {
            return Err(anyhow!("Provider not found: {}", name));
        }
        self.active_provider = name.to_string();
        Ok(())
    }

    /// Complete with active provider
    pub async fn complete(&self, prompt: &str, options: Option<CompletionOptions>) -> Result<String> {
        let options = options.unwrap_or_else(|| self.config.default_options.clone());
        self.active_provider().complete(prompt, options).await
    }

    /// Stream completion with active provider
    pub async fn complete_stream(
        &self,
        prompt: &str,
        options: Option<CompletionOptions>
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let options = options.unwrap_or_else(|| self.config.default_options.clone());
        self.active_provider().complete_stream(prompt, options).await
    }

    /// Get all provider names
    pub fn list_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Health check all providers
    pub async fn health_check_all(&self) -> HashMap<String, bool> {
        let mut results = HashMap::new();

        for (name, provider) in &self.providers {
            let is_healthy = provider.health_check().await.unwrap_or(false);
            results.insert(name.clone(), is_healthy);
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_manager() {
        let config = AiConfig {
            default_provider: "ollama".to_string(),
            providers: {
                let mut providers = HashMap::new();
                providers.insert("ollama".to_string(), ProviderConfig {
                    enabled: true,
                    api_key: None,
                    base_url: Some("http://localhost:11434".to_string()),
                    models: vec!["llama2".to_string()],
                    rate_limit: None,
                });
                providers
            },
            default_options: CompletionOptions::default(),
        };

        let manager = AiProviderManager::new(config).unwrap();
        assert_eq!(manager.active_provider().name(), "Ollama");
    }
}

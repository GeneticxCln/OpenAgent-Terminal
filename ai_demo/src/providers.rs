use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Ollama,
    OpenRouter,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub ollama_base_url: Option<String>,
    pub openrouter_api_key: Option<String>,
    pub openrouter_base_url: Option<String>,
    pub openrouter_referer: Option<String>,
    pub openrouter_app_title: Option<String>,
    pub request_timeout: Duration,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            anthropic_api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
            ollama_base_url: std::env::var("OLLAMA_BASE_URL").ok().or_else(|| Some("http://localhost:11434".to_string())),
            openrouter_api_key: std::env::var("OPENROUTER_API_KEY").ok(),
            openrouter_base_url: std::env::var("OPENROUTER_BASE_URL").ok().or_else(|| Some("https://openrouter.ai".to_string())),
            openrouter_referer: std::env::var("OPENROUTER_REFERER").ok(),
            openrouter_app_title: std::env::var("OPENROUTER_APP").ok(),
            request_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AiProviders {
    http: Client,
    cfg: ProviderConfig,
}

impl AiProviders {
    pub fn new(cfg: ProviderConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(cfg.request_timeout)
            .build()
            .context("failed building reqwest client")?;
        Ok(Self { http, cfg })
    }

    pub async fn chat(
        &self,
        provider: ProviderKind,
        model: &str,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String> {
        match provider {
            ProviderKind::OpenAI => self.openai_chat(model, system_prompt, user_prompt).await,
            ProviderKind::Anthropic => self.anthropic_chat(model, system_prompt, user_prompt).await,
            ProviderKind::Ollama => self.ollama_chat(model, system_prompt, user_prompt).await,
            ProviderKind::OpenRouter => self.openrouter_chat(model, system_prompt, user_prompt).await,
        }
    }

    async fn openai_chat(&self, model: &str, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let key = self
            .cfg
            .openai_api_key
            .as_ref()
            .ok_or_else(|| anyhow!("OPENAI_API_KEY not set"))?;

        #[derive(Serialize)]
        struct Message<'a> { role: &'a str, content: &'a str }
        #[derive(Serialize)]
        struct Req<'a> { model: &'a str, messages: Vec<Message<'a>>, temperature: f32 }
        #[derive(Deserialize)]
        struct Resp { choices: Vec<Choice> }
        #[derive(Deserialize)]
        struct Choice { message: OpenAiMsg }
        #[derive(Deserialize)]
        struct OpenAiMsg { content: String }

        let req = Req {
            model,
            messages: vec![
                Message { role: "system", content: system_prompt },
                Message { role: "user", content: user_prompt },
            ],
            temperature: 0.2,
        };

        let fut = self
            .http
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(key)
            .json(&req)
            .send();
        let resp = timeout(self.cfg.request_timeout, fut).await??;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI error: {} - {}", status, body));
        }
        let data: Resp = resp.json().await.context("OpenAI JSON parse error")?;
        let text = data
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| anyhow!("OpenAI returned no choices"))?;
        Ok(text)
    }

    async fn anthropic_chat(&self, model: &str, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let key = self
            .cfg
            .anthropic_api_key
            .as_ref()
            .ok_or_else(|| anyhow!("ANTHROPIC_API_KEY not set"))?;

        #[derive(Serialize)]
        struct Msg<'a> { role: &'a str, content: &'a str }
        #[derive(Serialize)]
        struct Req<'a> { model: &'a str, max_tokens: u32, system: &'a str, messages: Vec<Msg<'a>> }
        #[derive(Deserialize)]
        struct Resp { content: Vec<AnthContent> }
        #[derive(Deserialize)]
        struct AnthContent { text: String }

        let req = Req {
            model,
            max_tokens: 1024,
            system: system_prompt,
            messages: vec![Msg { role: "user", content: user_prompt }],
        };

        let fut = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .json(&req)
            .send();
        let resp = timeout(self.cfg.request_timeout, fut).await??;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic error: {} - {}", status, body));
        }
        let data: Resp = resp.json().await.context("Anthropic JSON parse error")?;
        let text = data
            .content
            .into_iter()
            .next()
            .map(|c| c.text)
            .ok_or_else(|| anyhow!("Anthropic returned empty content"))?;
        Ok(text)
    }

    async fn ollama_chat(&self, model: &str, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let base = self.cfg.ollama_base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string());
        let url = Url::parse(&base)
            .and_then(|u| u.join("/api/chat"))
            .context("Invalid OLLAMA_BASE_URL")?;

        #[derive(Serialize)]
        struct Message<'a> { role: &'a str, content: &'a str }
        #[derive(Serialize)]
        struct Req<'a> { model: &'a str, messages: Vec<Message<'a>>, options: OllamaOptions }
        #[derive(Serialize, Default)]
        struct OllamaOptions { temperature: f32, num_ctx: u32 }
        #[derive(Deserialize)]
        struct Resp { message: Option<MsgResp> }
        #[derive(Deserialize)]
        struct MsgResp { content: String }

        let req = Req {
            model,
            messages: vec![
                Message { role: "system", content: system_prompt },
                Message { role: "user", content: user_prompt },
            ],
            options: OllamaOptions { temperature: 0.2, num_ctx: 4096 },
        };

        let fut = self.http.post(url).json(&req).send();
        let resp = timeout(self.cfg.request_timeout, fut).await??;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Ollama error: {} - {}", status, body));
        }
        let data: Resp = resp.json().await.context("Ollama JSON parse error")?;
        let text = data
            .message
            .map(|m| m.content)
            .ok_or_else(|| anyhow!("Ollama returned no message"))?;
        Ok(text)
    }

    async fn openrouter_chat(&self, model: &str, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let key = self
            .cfg
            .openrouter_api_key
            .as_ref()
            .ok_or_else(|| anyhow!("OPENROUTER_API_KEY not set"))?;
        let base = self.cfg.openrouter_base_url.clone().unwrap_or_else(|| "https://openrouter.ai".to_string());
        let url = Url::parse(&base)
            .and_then(|u| u.join("/api/v1/chat/completions"))
            .context("Invalid OPENROUTER_BASE_URL")?;

        #[derive(Serialize)]
        struct Message<'a> { role: &'a str, content: &'a str }
        #[derive(Serialize)]
        struct Req<'a> { model: &'a str, messages: Vec<Message<'a>>, temperature: f32 }
        #[derive(Deserialize)]
        struct Resp { choices: Vec<Choice> }
        #[derive(Deserialize)]
        struct Choice { message: ORMsg }
        #[derive(Deserialize)]
        struct ORMsg { content: String }

        let req = Req {
            model,
            messages: vec![
                Message { role: "system", content: system_prompt },
                Message { role: "user", content: user_prompt },
            ],
            temperature: 0.2,
        };

        let mut req_builder = self.http.post(url).bearer_auth(key).json(&req);
        if let Some(ref referer) = self.cfg.openrouter_referer { req_builder = req_builder.header("HTTP-Referer", referer); }
        if let Some(ref app) = self.cfg.openrouter_app_title { req_builder = req_builder.header("X-Title", app); }

        let resp = timeout(self.cfg.request_timeout, req_builder.send()).await??;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("OpenRouter error: {} - {}", status, body));
        }
        let data: Resp = resp.json().await.context("OpenRouter JSON parse error")?;
        let text = data
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| anyhow!("OpenRouter returned no choices"))?;
        Ok(text)
    }
}

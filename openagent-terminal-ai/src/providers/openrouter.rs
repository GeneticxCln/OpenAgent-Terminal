use crate::privacy::{sanitize_request, AiPrivacyOptions};
use crate::streaming::{RetryConfig, RetryStrategy};
use crate::{AiProposal, AiProvider, AiRequest};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tracing::{debug, error, info};

pub struct OpenRouterProvider {
    api_key: String,
    endpoint: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl OpenRouterProvider {
    pub fn new(api_key: String, endpoint: String, model: String) -> Result<Self, String> {
        if api_key.is_empty() {
            return Err("OpenRouter API key is required".to_string());
        }

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(20))
            .user_agent("openagent-terminal-ai/0.1")
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            api_key,
            endpoint,
            model,
            client,
        })
    }

    pub fn from_env() -> Result<Self, String> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| "OPENROUTER_API_KEY environment variable not set".to_string())?;
        let endpoint = std::env::var("OPENROUTER_API_BASE")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());
        let model = std::env::var("OPENROUTER_MODEL").map_err(|_| {
            "OPENROUTER_MODEL environment variable not set. Configure a model explicitly via \
             config.ai.providers.openrouter.model_env (e.g., OPENAGENT_OPENROUTER_MODEL) or \
             export OPENROUTER_MODEL before launching."
                .to_string()
        })?;

        info!("Initializing OpenRouter provider with model: {}", model);
        Self::new(api_key, endpoint, model)
    }
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: i32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
}

// Streaming chunk structures for Chat Completions SSE
#[derive(Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
}

fn ai_log_verbose() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| {
        matches!(
            std::env::var("OPENAGENT_AI_LOG_VERBOSITY").ok().as_deref(),
            Some("verbose")
        )
    })
}
fn ai_log_summary() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| {
        matches!(
            std::env::var("OPENAGENT_AI_LOG_VERBOSITY").ok().as_deref(),
            Some("summary") | Some("verbose")
        )
    })
}

impl AiProvider for OpenRouterProvider {
    fn name(&self) -> &'static str {
        "openrouter"
    }

    fn propose(&self, req: AiRequest) -> Result<Vec<AiProposal>, String> {
        if ai_log_summary() {
            info!(
                "openrouter_propose_start model={} endpoint={}",
                self.model, self.endpoint
            );
        }
        // Build the prompt (sanitized)
        let req = sanitize_request(&req, AiPrivacyOptions::from_env());
        let mut system_prompt = String::from(
            "You are a helpful terminal command assistant. Provide only the necessary shell \
             commands with brief comment explanations. Format your response as a list of \
             commands, one per line. Start each explanation line with #.",
        );

        if let Some(shell) = &req.shell_kind {
            system_prompt.push_str(&format!(" The user is using {} shell.", shell));
        }

        if let Some(dir) = &req.working_directory {
            system_prompt.push_str(&format!(" Current directory: {}", dir));
        }

        for (key, value) in &req.context {
            system_prompt.push_str(&format!(" {}: {}.", key, value));
        }

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            ChatMessage {
                role: "user".to_string(),
                content: req.scratch_text.clone(),
            },
        ];

        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            temperature: 0.7,
            max_tokens: 500,
            stream: false,
        };

        debug!("Sending request to OpenRouter API");

        let url = format!("{}/chat/completions", self.endpoint);
        let retry = RetryStrategy::OpenAI {
            config: RetryConfig::default(),
            respect_retry_after: true,
        };
        let mut attempt = 0usize;
        let completion: ChatCompletionResponse = loop {
            let send = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send();
            let response = match send {
                Ok(resp) => resp,
                Err(e) => {
                    let msg = format!("Failed to send request: {}", e);
                    if retry.should_retry(attempt, &msg, &std::sync::atomic::AtomicBool::new(false))
                    {
                        let delay = retry.delay_for_attempt(attempt, &msg);
                        if ai_log_summary() {
                            info!(
                                "openrouter_propose_retry attempt={} delay_ms={}",
                                attempt + 1,
                                delay.as_millis()
                            );
                        }
                        std::thread::sleep(delay);
                        attempt += 1;
                        continue;
                    } else {
                        return Err(msg);
                    }
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let headers = response.headers().clone();
                let error_text = response
                    .text()
                    .unwrap_or_else(|_| "Unknown error".to_string());
                let mut msg = format!("API error {}: {}", status, error_text);
                if let Some(s) = headers.get("retry-after").and_then(|v| v.to_str().ok()) {
                    msg.push_str(&format!("; retry-after: {}", s));
                }
                if let Some(s) = headers
                    .get("x-ratelimit-reset-after")
                    .or_else(|| headers.get("x-rate-limit-reset-after"))
                    .and_then(|v| v.to_str().ok())
                {
                    msg.push_str(&format!("; x-ratelimit-reset-after: {}", s));
                }
                if let Some(s) = headers
                    .get("x-ratelimit-reset")
                    .or_else(|| headers.get("x-rate-limit-reset"))
                    .and_then(|v| v.to_str().ok())
                {
                    msg.push_str(&format!("; x-ratelimit-reset: {}", s));
                }
                error!("OpenRouter API error {}: {}", status, error_text);
                if retry.should_retry(attempt, &msg, &std::sync::atomic::AtomicBool::new(false)) {
                    let delay = retry.delay_for_attempt(attempt, &msg);
                    if ai_log_summary() {
                        info!(
                            "openrouter_propose_retry_http attempt={} delay_ms={}",
                            attempt + 1,
                            delay.as_millis()
                        );
                    }
                    std::thread::sleep(delay);
                    attempt += 1;
                    continue;
                } else {
                    return Err(msg);
                }
            }

            if ai_log_summary() {
                debug!(
                    "openrouter_propose_response_status status={}",
                    response.status()
                );
            }
            match response.json() {
                Ok(json) => break json,
                Err(e) => {
                    let msg = format!("Failed to parse response: {}", e);
                    if retry.should_retry(attempt, &msg, &std::sync::atomic::AtomicBool::new(false))
                    {
                        let delay = retry.delay_for_attempt(attempt, &msg);
                        std::thread::sleep(delay);
                        attempt += 1;
                        continue;
                    } else {
                        return Err(msg);
                    }
                }
            }
        };

        if let Some(choice) = completion.choices.first() {
            let content = &choice.message.content;
            let commands: Vec<String> = content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.to_string())
                .collect();

            if commands.is_empty() {
                if ai_log_summary() {
                    info!("openrouter_propose_complete commands=0");
                }
                Ok(vec![AiProposal {
                    title: format!("Response for: {}", req.scratch_text),
                    description: Some("No specific commands suggested".to_string()),
                    proposed_commands: vec!["# No commands generated".to_string()],
                }])
            } else {
                if ai_log_summary() {
                    info!("openrouter_propose_complete commands={}", commands.len());
                }
                Ok(vec![AiProposal {
                    title: format!("OpenRouter suggestion for: {}", req.scratch_text),
                    description: Some(format!("Generated by {}", self.model)),
                    proposed_commands: commands,
                }])
            }
        } else {
            Err("No response from OpenRouter".to_string())
        }
    }

    fn propose_stream(
        &self,
        req: AiRequest,
        on_chunk: &mut dyn FnMut(&str),
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        if ai_log_summary() {
            info!(
                "openrouter_stream_start model={} endpoint={}",
                self.model, self.endpoint
            );
        }
        use eventsource_stream::Eventsource;
        use futures_util::StreamExt;

        // Build the prompt (sanitized)
        let req = sanitize_request(&req, AiPrivacyOptions::from_env());
        let mut system_prompt = String::from(
            "You are a helpful terminal command assistant. Provide only the necessary shell \
             commands with brief comment explanations. Format your response as a list of \
             commands, one per line. Start each explanation line with #.",
        );

        if let Some(shell) = &req.shell_kind {
            system_prompt.push_str(&format!(" The user is using {} shell.", shell));
        }
        if let Some(dir) = &req.working_directory {
            system_prompt.push_str(&format!(" Current directory: {}", dir));
        }
        for (key, value) in &req.context {
            system_prompt.push_str(&format!(" {}: {}.", key, value));
        }

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            ChatMessage {
                role: "user".to_string(),
                content: req.scratch_text.clone(),
            },
        ];

        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            temperature: 0.7,
            max_tokens: 500,
            stream: true,
        };

        let url = format!("{}/chat/completions", self.endpoint);
        let retry = RetryStrategy::OpenAI {
            config: RetryConfig::default(),
            respect_retry_after: true,
        };
        let mut attempt = 0usize;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;

        rt.block_on(async {
            loop {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err("Cancelled".to_string());
                }

                let client = match reqwest::Client::builder()
                    .connect_timeout(std::time::Duration::from_secs(5))
                    .user_agent("openagent-terminal-ai/0.1")
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => return Err(format!("Failed to create HTTP client: {}", e)),
                };

                let send_result = client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("Content-Type", "application/json")
                    .header("Accept", "text/event-stream")
                    .json(&request_body)
                    .timeout(std::time::Duration::from_secs(600))
                    .send()
                    .await;

                let response = match send_result {
                    Ok(resp) => resp,
                    Err(e) => {
                        let msg = format!("Failed to send request: {}", e);
                        if retry.should_retry(attempt, &msg, cancel) {
                            let delay = retry.delay_for_attempt(attempt, &msg);
                            if ai_log_summary() {
                                info!("openrouter_stream_retry attempt={} delay_ms={}", attempt + 1, delay.as_millis());
                            }
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                            continue;
                        } else {
                            return Err(msg);
                        }
                    },
                };

                if !response.status().is_success() {
                    let status = response.status();
                    let headers = response.headers().clone();
                    let error_text = match response.text().await {
                        Ok(t) => t,
                        Err(_) => "Unknown error".to_string(),
                    };
                    let mut msg = format!("API error {}: {}", status, error_text);
                    if let Some(s) = headers.get("retry-after").and_then(|v| v.to_str().ok()) {
                        msg.push_str(&format!("; retry-after: {}", s));
                    }
                    if let Some(s) = headers
                        .get("x-ratelimit-reset-after")
                        .or_else(|| headers.get("x-rate-limit-reset-after"))
                        .and_then(|v| v.to_str().ok())
                    {
                        msg.push_str(&format!("; x-ratelimit-reset-after: {}", s));
                    }
                    if let Some(s) = headers
                        .get("x-ratelimit-reset")
                        .or_else(|| headers.get("x-rate-limit-reset"))
                        .and_then(|v| v.to_str().ok())
                    {
                        msg.push_str(&format!("; x-ratelimit-reset: {}", s));
                    }
                    error!("OpenRouter API error {}: {}", status, error_text);
                    if retry.should_retry(attempt, &msg, cancel) {
                        let delay = retry.delay_for_attempt(attempt, &msg);
                        if ai_log_summary() {
                            info!("openrouter_stream_retry_http attempt={} delay_ms={}", attempt + 1, delay.as_millis());
                        }
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    } else {
                        return Err(msg);
                    }
                }

                let mut stream = response.bytes_stream().eventsource();
                loop {
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        if ai_log_summary() { info!("openrouter_stream_cancelled"); }
                        return Err("Cancelled".to_string());
                    }
                    match tokio::time::timeout(std::time::Duration::from_millis(200), stream.next()).await {
                        Ok(Some(Ok(event))) => {
                            let data = event.data;
                            if data.trim() == "[DONE]" {
                                break;
                            }
                            match serde_json::from_str::<ChatCompletionChunk>(&data) {
                                Ok(chunk) => {
                                    for choice in chunk.choices.into_iter() {
                                        if let Some(c) = choice.delta.content {
                                            if ai_log_verbose() { debug!("openrouter_stream_chunk len={}", c.len()); }
                                            on_chunk(&c);
                                        }
                                    }
                                },
                                Err(e) => {
                                    debug!("Skipping non-JSON or unexpected SSE data from OpenRouter: {}", e);
                                    lazy_static::lazy_static! {
                                        static ref CONTENT_RE: regex::Regex = regex::Regex::new(r#"\"content\"\s*:\s*\"([^\"]*)\""#).unwrap();
                                    }
                                    if let Some(cap) = CONTENT_RE.captures(&data).and_then(|caps| caps.get(1)) {
                                        let c = cap.as_str().to_string();
                                        if !c.is_empty() {
                                            if ai_log_verbose() { debug!("openrouter_stream_chunk_fallback len={}", c.len()); }
                                            on_chunk(&c);
                                        }
                                    }
                                },
                            }
                        },
                        Ok(Some(Err(e))) => {
                            return Err(format!("Stream error: {}", e));
                        },
                        Ok(None) => {
                            break;
                        },
                        Err(_) => {
                            continue;
                        },
                    }
                }

                if ai_log_summary() {
                    info!("openrouter_stream_finished");
                }
                return Ok(true);
            }
        })
    }
}

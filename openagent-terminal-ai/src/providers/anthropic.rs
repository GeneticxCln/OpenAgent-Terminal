use crate::{AiProvider, AiProposal, AiRequest};
use crate::privacy::{sanitize_request, AiPrivacyOptions};
use serde::{Deserialize, Serialize};
use log::{debug, error, info};
use std::io::{BufRead, BufReader};
use std::sync::OnceLock;

pub struct AnthropicProvider {
    api_key: String,
    endpoint: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String, endpoint: String, model: String) -> Result<Self, String> {
        if api_key.is_empty() {
            return Err("Anthropic API key is required".to_string());
        }
        
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(20))
            .user_agent("openagent-terminal-ai/0.1")
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        
        Ok(Self { api_key, endpoint, model, client })
    }
    
    pub fn from_env() -> Result<Self, String> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;
        let endpoint = std::env::var("ANTHROPIC_API_BASE")
            .unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string());
        let model = std::env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-3-haiku-20240307".to_string());
        
        info!("Initializing Anthropic provider with model: {}", model);
        Self::new(api_key, endpoint, model)
    }
}

#[derive(Serialize)]
struct MessageRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: i32,
    temperature: f32,
    system: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct MessageResponse {
    content: Vec<Content>,
}

#[derive(Deserialize)]
struct Content {
    text: String,
}

// Minimal streaming event payload; we only care about delta.text
#[derive(Deserialize)]
struct AnthropicStreamData {
    #[allow(dead_code)]
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    delta: Option<AnthropicDelta>,
}

#[derive(Deserialize)]
struct AnthropicDelta {
    #[serde(default)]
    text: Option<String>,
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

impl AiProvider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }
    
    fn propose(&self, req: AiRequest) -> Result<Vec<AiProposal>, String> {
if ai_log_summary() { info!("anthropic_propose_start model={} endpoint={}", self.model, self.endpoint); }
        // Build the system prompt (sanitized)
        let req = sanitize_request(&req, AiPrivacyOptions::from_env());
        let mut system_prompt = String::from(
            "You are a helpful terminal command assistant. \
             Provide only the necessary shell commands with brief comment explanations. \
             Format your response as a list of commands, one per line. \
             Start each explanation line with #. \
             Be concise and practical."
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
            Message {
                role: "user".to_string(),
                content: req.scratch_text.clone(),
            },
        ];
        
        let request_body = MessageRequest {
            model: self.model.clone(),
            messages,
            max_tokens: 500,
            temperature: 0.7,
            system: system_prompt,
            stream: false,
        };
        
        debug!("Sending request to Anthropic API");
        
        let url = format!("{}/messages", self.endpoint);
        let response = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .map_err(|e| format!("Failed to send request: {}", e))?;
if ai_log_summary() { debug!("anthropic_propose_response_status status={}", response.status()); }
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            error!("Anthropic API error {}: {}", status, error_text);
            return Err(format!("API error {}: {}", status, error_text));
        }
        
        let message_response: MessageResponse = response.json()
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        if let Some(content) = message_response.content.first() {
            let text = &content.text;
            let commands: Vec<String> = text
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.to_string())
                .collect();
            
            if commands.is_empty() {
if ai_log_summary() { info!("anthropic_propose_complete commands=0"); }
                Ok(vec![AiProposal {
                    title: format!("Response for: {}", req.scratch_text),
                    description: Some("No specific commands suggested".to_string()),
                    proposed_commands: vec!["# No commands generated".to_string()],
                }])
            } else {
if ai_log_summary() { info!("anthropic_propose_complete commands={}", commands.len()); }
                Ok(vec![AiProposal {
                    title: format!("Claude suggestion for: {}", req.scratch_text),
                    description: Some(format!("Generated by {}", self.model)),
                    proposed_commands: commands,
                }])
            }
        } else {
            Err("No response from Anthropic".to_string())
        }
    }
    fn propose_stream(
        &self,
        req: AiRequest,
        on_chunk: &mut dyn FnMut(&str),
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
if ai_log_summary() { info!("anthropic_stream_start model={} endpoint={}", self.model, self.endpoint); }
        let req = sanitize_request(&req, AiPrivacyOptions::from_env());
        let mut system_prompt = String::from(
            "You are a helpful terminal command assistant. \
             Provide only the necessary shell commands with brief comment explanations. \
             Format your response as a list of commands, one per line. \
             Start each explanation line with #. \
             Be concise and practical."
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
            Message { role: "user".to_string(), content: req.scratch_text.clone() },
        ];

        let request_body = MessageRequest {
            model: self.model.clone(),
            messages,
            max_tokens: 500,
            temperature: 0.7,
            system: system_prompt,
            stream: true,
        };

        let url = format!("{}/messages", self.endpoint);
        let response = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&request_body)
            .send()
            .map_err(|e| format!("Failed to send request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            error!("Anthropic API error {}: {}", status, error_text);
            return Err(format!("API error {}: {}", status, error_text));
        }

        // Stream SSE lines; Anthropic sends JSON objects in data: lines
        let reader = BufReader::new(response);
        for line in reader.lines() {
if cancel.load(std::sync::atomic::Ordering::Relaxed) { if ai_log_summary() { info!("anthropic_stream_cancelled"); } break; }
            let line = line.map_err(|e| format!("Stream read error: {}", e))?;
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }
            if let Some(rest) = trimmed.strip_prefix("data: ") {
                if rest.trim() == "[DONE]" { break; }
                match serde_json::from_str::<AnthropicStreamData>(rest) {
                    Ok(ev) => {
                        if let Some(delta) = ev.delta {
if let Some(txt) = delta.text { if ai_log_verbose() { debug!("anthropic_stream_chunk len={}", txt.len()); } on_chunk(&txt); }
                        }
                    }
                    Err(e) => {
                        debug!("Skipping unexpected Anthropic SSE data: {}", e);
                    }
                }
            }
        }

if ai_log_summary() { info!("anthropic_stream_finished"); }
        Ok(true)
    }
}

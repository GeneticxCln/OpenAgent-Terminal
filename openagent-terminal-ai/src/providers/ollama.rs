use crate::privacy::{sanitize_request, AiPrivacyOptions};
use crate::streaming::{RetryConfig, RetryStrategy};
use crate::{AiProposal, AiProvider, AiRequest};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::sync::OnceLock;
use tracing::{debug, error, info};

pub struct OllamaProvider {
    endpoint: String,
    model: String,
    client: reqwest::blocking::Client,
}

fn ai_log_verbose() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| {
        matches!(std::env::var("OPENAGENT_AI_LOG_VERBOSITY").ok().as_deref(), Some("verbose"))
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

impl OllamaProvider {
    /// Stream tokens from Ollama and invoke on_chunk for each text fragment.
    #[allow(dead_code)]
    fn stream_generate(
        &self,
        prompt: String,
        on_chunk: &mut dyn FnMut(&str),
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<(), String> {
        if ai_log_summary() {
            info!(
                "ollama_stream_start model={} endpoint={} prompt_len={}",
                self.model,
                self.endpoint,
                prompt.len()
            );
        }
        let url = format!("{}/api/generate", self.endpoint);
        let req_body = OllamaRequest { model: self.model.clone(), prompt, stream: true };

        let response = self
            .client
            .post(&url)
            .json(&req_body)
            // Allow long-running local generation streams
            .timeout(std::time::Duration::from_secs(600))
            .send()
            .map_err(|e| format!("Failed to send request: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        // Parse line-delimited JSON events
        let reader = BufReader::new(response);
        for line in reader.lines() {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                if ai_log_summary() {
                    info!("ollama_stream_cancelled");
                }
                break;
            }
            let line = line.map_err(|e| format!("Stream read error: {}", e))?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<OllamaGenerateResponse>(&line) {
                Ok(ev) => {
                    if !ev.response.is_empty() {
                        if ai_log_verbose() {
                            debug!("ollama_stream_chunk len={}", ev.response.len());
                        }
                        on_chunk(&ev.response);
                    }
                    if ev.done {
                        break;
                    }
                }
                Err(e) => {
                    debug!("Skipping non-JSON stream line: {} (err: {})", line, e);
                }
            }
        }
        if ai_log_summary() {
            info!("ollama_stream_finished");
        }
        Ok(())
    }
}

impl OllamaProvider {
    pub fn new(endpoint: String, model: String) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(20))
            .user_agent("openagent-terminal-ai/0.1")
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self { endpoint, model, client })
    }

    pub fn from_env() -> Result<Self, String> {
        let endpoint = std::env::var("OPENAGENT_OLLAMA_ENDPOINT")
            .or_else(|_| std::env::var("OLLAMA_HOST"))
            .or_else(|_| std::env::var("OLLAMA_ENDPOINT"))
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = std::env::var("OPENAGENT_OLLAMA_MODEL")
            .or_else(|_| std::env::var("OLLAMA_MODEL"))
            .map_err(|_| {
                "Model environment variable not set. Prefer OLLAMA_MODEL (or map an alternate \
                 OPENAGENT_OLLAMA_MODEL via config). Configure via \
                 config.ai.providers.ollama.model_env or export the variable before launching."
                    .to_string()
            })?;

        info!("Initializing Ollama provider with endpoint: {} and model: {}", endpoint, model);
        Self::new(endpoint, model)
    }

    fn check_availability(&self) -> bool {
        let url = format!("{}/api/tags", self.endpoint);
        match self.client.get(&url).send() {
            Ok(response) => response.status().is_success(),
            Err(e) => {
                debug!("Ollama not available: {}", e);
                false
            }
        }
    }

    fn stream_propose(
        &self,
        req: AiRequest,
        on_chunk: &mut dyn FnMut(&str),
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        if !self.check_availability() {
            return Ok(false);
        }
        let opts = AiPrivacyOptions::from_env();
        let req = sanitize_request(&req, opts);
        let mut prompt = String::new();
        prompt.push_str("You are a helpful terminal command assistant. ");
        prompt.push_str(
            "Provide only the necessary shell commands with brief comment explanations. ",
        );
        prompt.push_str("Do not include any other text or formatting. ");
        if let Some(shell) = &req.shell_kind {
            prompt.push_str(&format!("Shell: {}. ", shell));
        }
        if let Some(dir) = &req.working_directory {
            prompt.push_str(&format!("Working directory: {}. ", dir));
        }
        for (key, value) in &req.context {
            prompt.push_str(&format!("{}: {}. ", key, value));
        }
        prompt.push_str(&format!("\nUser request: {}\n", req.scratch_text));
        prompt.push_str("\nProvide the commands:");

        // Async streaming with responsive cancellation
        use futures_util::StreamExt;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        let endpoint = self.endpoint.clone();
        let model = self.model.clone();
        rt.block_on(async {
            let client = match reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(5))
                .user_agent("openagent-terminal-ai/0.1")
                .build()
            {
                Ok(c) => c,
                Err(e) => return Err(format!("Failed to create HTTP client: {}", e)),
            };
            let url = format!("{}/api/generate", endpoint);
            let req_body = OllamaRequest { model, prompt, stream: true };
            let resp = client
                .post(&url)
                .json(&req_body)
                .timeout(std::time::Duration::from_secs(600))
                .send()
                .await
                .map_err(|e| format!("Failed to send request: {}", e))?;
            if !resp.status().is_success() {
                return Err(format!("API error: {}", resp.status()));
            }
            let mut stream = resp.bytes_stream();
            let mut buffer = Vec::<u8>::new();
            loop {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    if ai_log_summary() {
                        info!("ollama_stream_cancelled");
                    }
                    return Err("Cancelled".to_string());
                }
                match tokio::time::timeout(std::time::Duration::from_millis(200), stream.next())
                    .await
                {
                    Ok(Some(Ok(chunk))) => {
                        buffer.extend_from_slice(&chunk);
                        // Process complete lines
                        while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                            let line = buffer.drain(..=pos).collect::<Vec<u8>>();
                            let line = String::from_utf8_lossy(&line).trim().to_string();
                            if line.is_empty() {
                                continue;
                            }
                            match serde_json::from_str::<OllamaGenerateResponse>(&line) {
                                Ok(ev) => {
                                    if !ev.response.is_empty() {
                                        if ai_log_verbose() {
                                            debug!("ollama_stream_chunk len={}", ev.response.len());
                                        }
                                        on_chunk(&ev.response);
                                    }
                                    if ev.done {
                                        return Ok(true);
                                    }
                                }
                                Err(e) => {
                                    debug!("Skipping non-JSON stream line: {} (err: {})", line, e);
                                }
                            }
                        }
                    }
                    Ok(Some(Err(e))) => return Err(format!("Stream read error: {}", e)),
                    Ok(None) => break,
                    Err(_) => continue, // timeout; re-check cancel
                }
            }
            Ok(true)
        })
    }
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    #[allow(dead_code)]
    model: String,
    response: String,
    #[allow(dead_code)]
    done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(dead_code)]
    context: Option<Vec<i64>>,
}

impl AiProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn propose(&self, req: AiRequest) -> Result<Vec<AiProposal>, String> {
        if ai_log_summary() {
            info!("ollama_propose_start model={} endpoint={}", self.model, self.endpoint);
        }
        // Check if Ollama is available
        if !self.check_availability() {
            return Ok(vec![AiProposal {
                title: "Ollama Not Available".to_string(),
                description: Some("Please ensure Ollama is running locally".to_string()),
                proposed_commands: vec![
                    "# Install Ollama: curl -fsSL https://ollama.ai/install.sh | sh".to_string(),
                    "# Start Ollama: ollama serve".to_string(),
                    "# Pull a model: ollama pull codellama".to_string(),
                ],
            }]);
        }

        // Build a context-aware prompt
        let opts = AiPrivacyOptions::from_env();
        let req = sanitize_request(&req, opts);
        let mut prompt = String::new();
        prompt.push_str("You are a helpful terminal command assistant. ");
        prompt.push_str(
            "Provide only the necessary shell commands with brief comment explanations. ",
        );
        prompt.push_str("Do not include any other text or formatting. ");

        if let Some(shell) = &req.shell_kind {
            prompt.push_str(&format!("Shell: {}. ", shell));
        }

        if let Some(dir) = &req.working_directory {
            prompt.push_str(&format!("Working directory: {}. ", dir));
        }

        // Add context if provided
        for (key, value) in &req.context {
            prompt.push_str(&format!("{}: {}. ", key, value));
        }

        prompt.push_str(&format!("\nUser request: {}\n", req.scratch_text));
        prompt.push_str("\nProvide the commands:");

        if ai_log_verbose() {
            debug!("Sending prompt to Ollama: {}", prompt);
        }

        // Make the actual API call
        let url = format!("{}/api/generate", self.endpoint);
        let ollama_request = OllamaRequest { model: self.model.clone(), prompt, stream: false };

        let retry = RetryStrategy::Ollama {
            config: RetryConfig::default(),
            resource_wait: std::time::Duration::from_secs(1),
        };
        let mut attempt = 0usize;
        loop {
            match self.client.post(&url).json(&ollama_request).send() {
                Ok(response) => {
                    if ai_log_summary() {
                        debug!("ollama_propose_response_status status={}", response.status());
                    }
                    if response.status().is_success() {
                        match response.json::<OllamaGenerateResponse>() {
                            Ok(ollama_response) => {
                                // Parse the response into commands
                                let commands: Vec<String> = ollama_response
                                    .response
                                    .lines()
                                    .filter(|line| !line.trim().is_empty())
                                    .map(|line| line.to_string())
                                    .collect();

                                if commands.is_empty() {
                                    if ai_log_summary() {
                                        info!("ollama_propose_complete commands=0");
                                    }
                                    return Ok(vec![AiProposal {
                                        title: format!("Response for: {}", req.scratch_text),
                                        description: Some(
                                            "No specific commands suggested".to_string(),
                                        ),
                                        proposed_commands: vec![
                                            "# No commands generated".to_string()
                                        ],
                                    }]);
                                } else {
                                    if ai_log_summary() {
                                        info!(
                                            "ollama_propose_complete commands={}",
                                            commands.len()
                                        );
                                    }
                                    return Ok(vec![AiProposal {
                                        title: format!("Suggestion for: {}", req.scratch_text),
                                        description: Some("AI-generated commands".to_string()),
                                        proposed_commands: commands,
                                    }]);
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse Ollama response: {}", e);
                                let msg = format!("Failed to parse response: {}", e);
                                if retry.should_retry(
                                    attempt,
                                    &msg,
                                    &std::sync::atomic::AtomicBool::new(false),
                                ) {
                                    let delay = retry.delay_for_attempt(attempt, &msg);
                                    std::thread::sleep(delay);
                                    attempt += 1;
                                    continue;
                                } else {
                                    return Err(msg);
                                }
                            }
                        }
                    } else {
                        let msg = format!("API error: {}", response.status());
                        error!("{}", msg);
                        if retry.should_retry(
                            attempt,
                            &msg,
                            &std::sync::atomic::AtomicBool::new(false),
                        ) {
                            let delay = retry.delay_for_attempt(attempt, &msg);
                            std::thread::sleep(delay);
                            attempt += 1;
                            continue;
                        } else {
                            return Err(msg);
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("Connection error: {}", e);
                    error!("{}", msg);
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
        }
    }

    fn propose_stream(
        &self,
        req: AiRequest,
        on_chunk: &mut dyn FnMut(&str),
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<bool, String> {
        self.stream_propose(req, on_chunk, cancel)
    }
}

use crate::{AiProvider, AiProposal, AiRequest};
use serde::{Deserialize, Serialize};
use log::{debug, error, info};

pub struct OllamaProvider {
    endpoint: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl OllamaProvider {
    pub fn new(endpoint: String, model: String) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        
        Ok(Self { endpoint, model, client })
    }
    
    pub fn from_env() -> Result<Self, String> {
        let endpoint = std::env::var("OLLAMA_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| "codellama".to_string());
        
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
        let mut prompt = String::new();
        prompt.push_str("You are a helpful terminal command assistant. ");
        prompt.push_str("Provide only the necessary shell commands with brief comment explanations. ");
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
        
        debug!("Sending prompt to Ollama: {}", prompt);
        
        // Make the actual API call
        let url = format!("{}/api/generate", self.endpoint);
        let ollama_request = OllamaRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
        };
        
        match self.client.post(&url)
            .json(&ollama_request)
            .send() {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<OllamaGenerateResponse>() {
                        Ok(ollama_response) => {
                            // Parse the response into commands
                            let commands: Vec<String> = ollama_response.response
                                .lines()
                                .filter(|line| !line.trim().is_empty())
                                .map(|line| line.to_string())
                                .collect();
                            
                            if commands.is_empty() {
                                Ok(vec![AiProposal {
                                    title: format!("Response for: {}", req.scratch_text),
                                    description: Some("No specific commands suggested".to_string()),
                                    proposed_commands: vec!["# No commands generated".to_string()],
                                }])
                            } else {
                                Ok(vec![AiProposal {
                                    title: format!("Suggestion for: {}", req.scratch_text),
                                    description: Some("AI-generated commands".to_string()),
                                    proposed_commands: commands,
                                }])
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse Ollama response: {}", e);
                            Err(format!("Failed to parse response: {}", e))
                        }
                    }
                } else {
                    error!("Ollama API error: {}", response.status());
                    Err(format!("API error: {}", response.status()))
                }
            }
            Err(e) => {
                error!("Failed to connect to Ollama: {}", e);
                Err(format!("Connection error: {}", e))
            }
        }
    }
}

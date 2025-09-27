//! AI Runtime System
//! 
//! This module provides the core AI runtime functionality for OpenAgent Terminal,
//! including AI provider management, streaming responses, and UI state management.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Result, anyhow, Context};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use crate::event::Event;

// HTTP client for providers
use reqwest::Client;
use futures_util::StreamExt;

/// Stub agent types for AI runtime
#[derive(Debug, Clone)]
pub struct Agent {
    id: String,
    name: String,
}

#[derive(Debug, Clone)]
pub struct AgentManager {
    agents: Vec<Agent>,
}

impl AgentManager {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
        }
    }
}

impl Default for AgentManager {
    fn default() -> Self { Self::new() }
}

#[derive(Debug, Clone)]
pub struct AgentRequest {
    pub prompt: String,
    pub context: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub content: String,
    pub metadata: HashMap<String, String>,
}

/// AI provider types supported by the runtime
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum AiProvider {
    #[default]
    Ollama,
    OpenAI,
    Anthropic,
    OpenRouter,
    Custom(String),
}

/// Configuration for AI providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub timeout_seconds: u64,
}

impl Default for AiProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_key: None,
            base_url: None,
            model: "gpt-3.5-turbo".to_string(),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            timeout_seconds: 30,
        }
    }
}

/// AI streaming response chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStreamChunk {
    pub content: String,
    pub is_complete: bool,
    pub metadata: HashMap<String, String>,
}

/// AI copy format for different output types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiCopyFormat {
    Text,
    Code,
    Markdown,
}

/// AI proposal structure for command suggestions
#[derive(Debug, Clone)]
pub struct AiProposal {
    pub title: String,
    pub description: Option<String>,
    pub proposed_commands: Vec<String>,
}

/// Current UI state for AI panel
#[derive(Debug, Clone)]
pub struct AiUiState {
    pub active: bool,
    pub streaming_active: bool,
    pub is_loading: bool,
    pub current_response: String,
    pub error_message: Option<String>,
    pub inline_suggestion: Option<String>,
    pub provider: AiProvider,
    pub available_providers: Vec<AiProvider>,
    pub conversation_id: Option<String>,
    pub selected_proposal: Option<usize>,
    pub proposals: Vec<AiProposal>,
    pub scratch: String,
    pub cursor_position: usize,
    // Additional fields for compatibility
    pub streaming_text: String,
    pub project_context_line: Option<String>,
    pub current_provider: String,
    pub current_model: String,
    pub streaming_last_redraw: Option<std::time::Instant>,
}

impl Default for AiUiState {
    fn default() -> Self {
        Self {
            active: false,
            streaming_active: false,
            is_loading: false,
            current_response: String::new(),
            error_message: None,
            inline_suggestion: None,
            provider: AiProvider::default(),
            available_providers: vec![
                AiProvider::Ollama,
                AiProvider::OpenAI,
                AiProvider::Anthropic,
                AiProvider::OpenRouter,
            ],
            conversation_id: None,
            selected_proposal: None,
            proposals: Vec::new(),
            scratch: String::new(),
            cursor_position: 0,
            // Additional fields for compatibility
            streaming_text: String::new(),
            project_context_line: None,
            current_provider: "ollama".to_string(),
            current_model: "llama2".to_string(),
            streaming_last_redraw: None,
        }
    }
}

/// Core AI runtime managing providers and conversations
pub struct AiRuntime {
    /// Current AI UI state
    pub ui: AiUiState,
    
    /// Agent manager for AI operations
    agent_manager: Arc<RwLock<AgentManager>>,
    
    /// Provider configurations
    providers: HashMap<AiProvider, AiProviderConfig>,
    
    /// Registry of custom providers (name -> provider)
    custom_providers: HashMap<String, Arc<dyn CustomAiProvider>>, 
    
    /// Current active provider
    active_provider: AiProvider,
    
    /// Event sender for UI updates
    event_sender: Option<mpsc::UnboundedSender<Event>>,
    
    /// Conversation history: conversation_id -> Vec<(prompt, response)>
    conversations: HashMap<String, Vec<(String, String)>>,
    
    /// Global prompt history (MRU order, 0 = most recent)
    prompt_history: Vec<String>,
    /// Current navigation position in prompt history (None = not navigating)
    prompt_history_index: Option<usize>,
    
    /// Response streaming state
    streaming_response: Option<String>,
    
    /// HTTP client
    http: Client,

    /// Last activity timestamp
    last_activity: Instant,

    /// Configuration knobs (keep in sync with config::ai)
    context_config: crate::config::ai::AiContextConfig,
    routing_mode: crate::config::ai::AiRoutingMode,
    apply_joiner: crate::config::ai::AiApplyJoinStrategy,
    history_retention: crate::config::ai::AiHistoryRetention,
}

/// Trait for registering custom AI providers at runtime
#[async_trait]
pub trait CustomAiProvider: Send + Sync {
    async fn chat(&self, http: Client, cfg: AiProviderConfig, prompt: String) -> Result<String>;
    /// Optional streaming; default implementation falls back to non-streaming and emits a single chunk
    async fn chat_stream(
        &self,
        http: Client,
        cfg: AiProviderConfig,
        prompt: String,
        proxy: winit::event_loop::EventLoopProxy<crate::event::Event>,
        window_id: winit::window::WindowId,
    ) -> Result<()> {
        let text = self.chat(http, cfg, prompt).await?;
        let _ = proxy.send_event(crate::event::Event::new(
            crate::event::EventType::AiStreamChunk(AiStreamChunk { content: text, is_complete: true, metadata: HashMap::new() }),
            window_id,
        ));
        let _ = proxy.send_event(crate::event::Event::new(crate::event::EventType::AiStreamFinished, window_id));
        Ok(())
    }
}

impl AiRuntime {
    /// Create a new AI runtime with default configuration
    pub fn new() -> Self {
        let mut providers = HashMap::new();
        providers.insert(AiProvider::Ollama, AiProviderConfig {
            model: "llama2".to_string(),
            base_url: Some("http://localhost:11434".to_string()),
            ..Default::default()
        });
        providers.insert(AiProvider::OpenAI, AiProviderConfig::default());
        providers.insert(AiProvider::Anthropic, AiProviderConfig {
            model: "claude-3-sonnet-20240229".to_string(),
            ..Default::default()
        });
        providers.insert(AiProvider::OpenRouter, AiProviderConfig {
            model: "meta-llama/llama-3.1-8b-instruct:free".to_string(),
            base_url: Some("https://openrouter.ai/api/v1".to_string()),
            ..Default::default()
        });

        Self {
            ui: AiUiState::default(),
            agent_manager: Arc::new(RwLock::new(AgentManager::new())),
            providers,
            custom_providers: HashMap::new(),
            active_provider: AiProvider::default(),
            event_sender: None,
            conversations: HashMap::new(),
            prompt_history: Vec::new(),
            prompt_history_index: None,
            streaming_response: None,
            http: Client::builder().timeout(Duration::from_secs(45)).build().expect("http client"),
            last_activity: Instant::now(),
            context_config: crate::config::ai::AiContextConfig::default(),
            routing_mode: crate::config::ai::AiRoutingMode::default(),
            apply_joiner: crate::config::ai::AiApplyJoinStrategy::default(),
            history_retention: crate::config::ai::AiHistoryRetention::default(),
        }
    }

    /// Create AI runtime from resolved runtime configuration
    pub fn from_secure_config(provider: &str, config: &AiProviderConfig) -> Result<Self> {
        let mut runtime = Self::new();
        
        let ai_provider = match provider.to_lowercase().as_str() {
            "ollama" => AiProvider::Ollama,
            "openai" => AiProvider::OpenAI,
            "anthropic" => AiProvider::Anthropic,
            "openrouter" => AiProvider::OpenRouter,
            custom => AiProvider::Custom(custom.to_string()),
        };
        
        runtime.providers.insert(ai_provider.clone(), config.clone());
        runtime.active_provider = ai_provider;
        runtime.ui.provider = runtime.active_provider.clone();
        
        Ok(runtime)
    }

    /// Create AI runtime from provider config (env-based), resolving credentials/endpoints safely
    pub fn from_secure_config_env(provider: &str, cfg: &crate::config::ai::ProviderConfig) -> Result<Self> {
let creds = crate::config::ai_providers::ProviderCredentials::from_config(provider, cfg)
            .map_err(|e| anyhow::anyhow!(e))?;
        let ai_cfg = AiProviderConfig {
            enabled: true,
            api_key: creds.api_key,
            base_url: creds.endpoint,
            model: creds.model.unwrap_or_else(|| "gpt-3.5-turbo".to_string()),
            max_tokens: None,
            temperature: None,
            timeout_seconds: 30,
        };
        Self::from_secure_config(provider, &ai_cfg)
    }

    /// Set event sender for UI updates
    pub fn set_event_sender(&mut self, sender: mpsc::UnboundedSender<Event>) {
        self.event_sender = Some(sender);
    }

    /// Register a custom AI provider implementation
    pub fn register_custom_provider<N: Into<String>>(&mut self, name: N, provider: Arc<dyn CustomAiProvider>) {
        self.custom_providers.insert(name.into(), provider);
    }

    /// Unregister a custom AI provider implementation
    pub fn unregister_custom_provider(&mut self, name: &str) {
        self.custom_providers.remove(name);
    }

    /// Switch to a different AI provider
    pub fn switch_provider(&mut self, provider: AiProvider) -> Result<()> {
        if !self.providers.contains_key(&provider) {
            return Err(anyhow!("Provider {:?} not configured", provider));
        }
        
        self.active_provider = provider.clone();
        self.ui.provider = provider;
        self.last_activity = Instant::now();
        
        Ok(())
    }

    /// Start AI conversation with given prompt (non-streaming)
    pub async fn start_conversation(&mut self, prompt: String) -> Result<String> {
        self.ui.is_loading = true;
        self.ui.streaming_active = true;
        self.ui.error_message = None;
        self.last_activity = Instant::now();
        
        // Generate conversation ID
        let conversation_id = uuid::Uuid::new_v4().to_string();
        self.ui.conversation_id = Some(conversation_id.clone());
        
        // Initialize conversation history and prompt history
        self.conversations.insert(conversation_id.clone(), vec![(prompt.clone(), String::new())]);
        self.push_prompt_history(&prompt);
        
        // Perform provider request (non-streaming fallback)
        let reply = self.provider_chat(&prompt).await?;
        
        // Push as a complete stream chunk
        let chunk = AiStreamChunk { content: reply, is_complete: true, metadata: HashMap::new() };
        self.process_stream_chunk(chunk);
        
        Ok(conversation_id)
    }

    /// Process streaming chunk from AI provider
    pub fn process_stream_chunk(&mut self, chunk: AiStreamChunk) {
        self.ui.current_response.push_str(&chunk.content);
        
        if chunk.is_complete {
            self.ui.streaming_active = false;
            self.ui.is_loading = false;
            
            // Update conversation history
            if let Some(conversation_id) = &self.ui.conversation_id {
                if let Some(history) = self.conversations.get_mut(conversation_id) {
                    if let Some((_, response)) = history.last_mut() {
                        *response = self.ui.current_response.clone();
                    }
                }
            }
        }
        
        self.last_activity = Instant::now();
    }

    /// Stop current AI operation
    pub fn stop(&mut self) {
        self.ui.streaming_active = false;
        self.ui.is_loading = false;
        self.streaming_response = None;
        self.last_activity = Instant::now();
    }

    /// Clear current conversation
    pub fn clear(&mut self) {
        self.ui.current_response.clear();
        self.ui.error_message = None;
        self.ui.conversation_id = None;
        self.ui.selected_proposal = None;
        self.ui.proposals.clear();
        self.streaming_response = None;
        self.last_activity = Instant::now();
    }

    /// Copy AI output in specified format
    pub fn copy_output(&self, format: AiCopyFormat) -> Option<String> {
        if self.ui.current_response.is_empty() {
            return None;
        }

        match format {
            AiCopyFormat::Text => Some(self.ui.current_response.clone()),
            AiCopyFormat::Code => {
                // Extract code blocks from markdown and strip language identifiers if present
                let code_blocks: Vec<&str> = self.ui.current_response
                    .split("```")
                    .skip(1)
                    .step_by(2)
                    .collect();

                if code_blocks.is_empty() {
                    Some(self.ui.current_response.clone())
                } else {
                    let cleaned: Vec<String> = code_blocks
                        .into_iter()
                        .map(|block| {
                            // If the first line looks like a language tag (e.g., "rust"), drop it
                            let mut s = block.to_string();
                            if let Some(pos) = s.find('\n') {
                                let first = s[..pos].trim();
                                let is_lang = !first.is_empty()
                                    && first.len() <= 32
                                    && first.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
                                if is_lang {
                                    s = s[pos + 1..].to_string();
                                }
                            }
                            s.trim().to_string()
                        })
                        .collect();
                    Some(cleaned.join("\n\n"))
                }
            }
            AiCopyFormat::Markdown => Some(self.ui.current_response.clone()),
        }
    }

    /// Set error message
    pub fn set_error(&mut self, error: String) {
        self.ui.error_message = Some(error);
        self.ui.streaming_active = false;
        self.ui.is_loading = false;
        self.last_activity = Instant::now();
    }

    /// Set inline suggestion
    pub fn set_inline_suggestion(&mut self, suggestion: Option<String>) {
        self.ui.inline_suggestion = suggestion;
        self.last_activity = Instant::now();
    }

    /// Add proposal to the list
    pub fn add_proposal(&mut self, proposal: AiProposal) {
        self.ui.proposals.push(proposal);
        self.last_activity = Instant::now();
    }
    
    /// Add simple string proposal (convenience method)
    pub fn add_simple_proposal(&mut self, command: String) {
        let proposal = AiProposal {
            title: command.clone(),
            description: None,
            proposed_commands: vec![command],
        };
        self.add_proposal(proposal);
    }

    /// Select next proposal
    pub fn select_next_proposal(&mut self) {
        if !self.ui.proposals.is_empty() {
            let current = self.ui.selected_proposal.unwrap_or(0);
            self.ui.selected_proposal = Some((current + 1) % self.ui.proposals.len());
        }
    }

    /// Select previous proposal
    pub fn select_prev_proposal(&mut self) {
        if !self.ui.proposals.is_empty() {
            let current = self.ui.selected_proposal.unwrap_or(0);
            let len = self.ui.proposals.len();
            self.ui.selected_proposal = Some((current + len - 1) % len);
        }
    }

    /// Get currently selected proposal
    pub fn get_selected_proposal(&self) -> Option<&AiProposal> {
        if let Some(index) = self.ui.selected_proposal {
            self.ui.proposals.get(index)
        } else {
            None
        }
    }

    /// Check if runtime is active
    pub fn is_active(&self) -> bool {
        self.ui.active || self.ui.streaming_active || self.ui.is_loading
    }

    /// Get time since last activity
    pub fn time_since_activity(&self) -> Duration {
        self.last_activity.elapsed()
    }
    
    /// Cancel current AI operation
    pub fn cancel(&mut self) {
        self.ui.streaming_active = false;
        self.ui.is_loading = false;
        self.streaming_response = None;
        self.last_activity = Instant::now();
    }
    
    /// Insert text to prompt/scratch area
    pub fn insert_text(&mut self, text: &str) {
        self.ui.scratch.insert_str(self.ui.cursor_position, text);
        self.ui.cursor_position += text.len();
        self.last_activity = Instant::now();
    }
    
    /// Backspace in prompt/scratch area
    pub fn backspace(&mut self) {
        if self.ui.cursor_position > 0 {
            self.ui.cursor_position -= 1;
            self.ui.scratch.remove(self.ui.cursor_position);
            self.last_activity = Instant::now();
        }
    }
    
    /// Delete forward in prompt/scratch area
    pub fn delete_forward(&mut self) {
        if self.ui.cursor_position < self.ui.scratch.len() {
            self.ui.scratch.remove(self.ui.cursor_position);
            self.last_activity = Instant::now();
        }
    }
    
    /// Move cursor left
    pub fn cursor_left(&mut self) {
        if self.ui.cursor_position > 0 {
            self.ui.cursor_position -= 1;
        }
    }
    
    /// Move cursor right
    pub fn cursor_right(&mut self) {
        if self.ui.cursor_position < self.ui.scratch.len() {
            self.ui.cursor_position += 1;
        }
    }
    
    /// Push a prompt into the global history (deduplicated, MRU at index 0)
    fn push_prompt_history(&mut self, prompt: &str) {
        if prompt.trim().is_empty() { return; }
        // Remove duplicates of exact same prompt
        self.prompt_history.retain(|p| p != prompt);
        self.prompt_history.insert(0, prompt.to_string());
        // Cap size
        if self.prompt_history.len() > 200 {
            self.prompt_history.truncate(200);
        }
        // Reset navigation when a new prompt is inserted
        self.prompt_history_index = None;
    }

    /// History navigation methods
    pub fn history_previous(&mut self) {
        if self.prompt_history.is_empty() { return; }
        let new_index = match self.prompt_history_index {
            None => Some(0),
            Some(i) => Some((i + 1).min(self.prompt_history.len().saturating_sub(1))),
        };
        if let Some(i) = new_index {
            if let Some(prompt) = self.prompt_history.get(i) {
                self.ui.scratch = prompt.clone();
                self.ui.cursor_position = self.ui.scratch.len();
                self.prompt_history_index = Some(i);
            }
        }
        self.last_activity = Instant::now();
    }
    
    pub fn history_next(&mut self) {
        if self.prompt_history.is_empty() { return; }
        match self.prompt_history_index {
            None => {
                // Already at newest; clear
                self.ui.scratch.clear();
                self.ui.cursor_position = 0;
            }
            Some(i) => {
                if i == 0 {
                    // Move to empty (beyond newest)
                    self.prompt_history_index = None;
                    self.ui.scratch.clear();
                    self.ui.cursor_position = 0;
                } else {
                    let new_i = i - 1;
                    if let Some(prompt) = self.prompt_history.get(new_i) {
                        self.ui.scratch = prompt.clone();
                        self.ui.cursor_position = self.ui.scratch.len();
                        self.prompt_history_index = Some(new_i);
                    }
                }
            }
        }
        self.last_activity = Instant::now();
    }
    
    /// Toggle panel alias
    pub fn toggle_panel(&mut self) {
        self.toggle();
    }
    
    /// Next/previous proposal aliases
    pub fn next_proposal(&mut self) {
        self.select_next_proposal();
    }
    
    pub fn previous_proposal(&mut self) {
        self.select_prev_proposal();
    }
    
    /// Apply command from current proposal
    pub fn apply_command(&mut self, _execute: bool) -> Option<(String, String)> {
        if let Some(proposal) = self.get_selected_proposal() {
            let command = proposal.proposed_commands.first().unwrap_or(&proposal.title).clone();
            Some((command, "Applied".to_string()))
        } else {
            None
        }
    }
    
    /// Insert prompt text to terminal
    pub fn insert_to_prompt(&mut self) -> Option<String> {
        if !self.ui.scratch.is_empty() {
            let text = self.ui.scratch.clone();
            self.ui.scratch.clear();
            self.ui.cursor_position = 0;
            Some(text)
        } else {
            None
        }
    }
    
    /// Start propose stream with context (uses provider)
    pub fn start_propose_stream_with_context(
        &mut self,
        context: crate::ai_context_provider::PtyAiContext,
        proxy: winit::event_loop::EventLoopProxy<crate::event::Event>,
        window_id: winit::window::WindowId,
    ) {
        let prompt = format!(
            "Given the working directory '{}' and recent commands count {}, suggest the next 3 useful commands with brief reasons.",
            context.terminal_context.working_directory.display(),
            context.terminal_context.recent_commands.len()
        );
        // Snapshot provider/client/config to avoid capturing &mut self across threads
        let prov = self.active_provider.clone();
        let cfg = self
            .providers
            .get(&prov)
            .cloned()
            .unwrap_or_else(AiProviderConfig::default);
        let http = self.http.clone();
        // Handle custom providers by using the registry (non-static dispatch)
        match prov.clone() {
            AiProvider::Custom(name) => {
                let custom = self.custom_providers.get(&name).cloned();
                if let Some(custom) = custom {
                    tokio::spawn(async move {
                        if let Err(e) = custom.chat_stream(http, cfg, prompt, proxy.clone(), window_id).await {
                            let _ = proxy.send_event(crate::event::Event::new(
                                crate::event::EventType::AiStreamError(e.to_string()),
                                window_id,
                            ));
                        }
                    });
                } else {
                    // Fallback error event if custom provider missing
                    let _ = proxy.send_event(crate::event::Event::new(
                        crate::event::EventType::AiStreamError(format!("Custom provider '{}' not registered", name)),
                        window_id,
                    ));
                }
            }
            _ => {
                tokio::spawn(async move {
                    if let Err(e) = AiRuntime::provider_chat_stream_owned(http, prov, cfg, prompt, proxy.clone(), window_id).await {
                        let _ = proxy.send_event(crate::event::Event::new(
                            crate::event::EventType::AiStreamError(e.to_string()),
                            window_id,
                        ));
                    }
                });
            }
        }
        self.ui.is_loading = true;
        self.ui.streaming_active = true;
        self.ui.streaming_text.clear();
    }
    
    /// Start inline suggest (provider-backed)
    pub fn start_inline_suggest(
        &mut self,
        prefix: String,
        proxy: winit::event_loop::EventLoopProxy<crate::event::Event>,
        window_id: winit::window::WindowId,
    ) {
        let prompt = format!("Given this partial command: '{}', suggest a concise completion (single line).", prefix);
        let prov = self.active_provider.clone();
        let cfg = self
            .providers
            .get(&prov)
            .cloned()
            .unwrap_or_else(AiProviderConfig::default);
        let http = self.http.clone();
        match prov.clone() {
            AiProvider::Custom(name) => {
                if let Some(custom) = self.custom_providers.get(&name).cloned() {
                    let http = self.http.clone();
                    tokio::spawn(async move {
                        match custom.chat(http, cfg, prompt).await {
                            Ok(s) => {
                                let suffix = s.lines().next().unwrap_or("").to_string();
                                let _ = proxy.send_event(crate::event::Event::new(
                                    crate::event::EventType::AiInlineSuggestionReady(suffix),
                                    window_id,
                                ));
                            }
                            Err(e) => {
                                let _ = proxy.send_event(crate::event::Event::new(
                                    crate::event::EventType::AiStreamError(format!(
                                        "Inline suggestion failed: {}",
                                        e
                                    )),
                                    window_id,
                                ));
                            }
                        }
                    });
                } else {
                    let _ = proxy.send_event(crate::event::Event::new(
                        crate::event::EventType::AiStreamError(format!("Custom provider '{}' not registered", name)),
                        window_id,
                    ));
                }
            }
            _ => {
                tokio::spawn(async move {
                    match AiRuntime::provider_chat_owned(http, prov, cfg, prompt).await {
                        Ok(s) => {
                            let suffix = s.lines().next().unwrap_or("").to_string();
                            let _ = proxy.send_event(crate::event::Event::new(
                                crate::event::EventType::AiInlineSuggestionReady(suffix),
                                window_id,
                            ));
                        }
                        Err(e) => {
                            let _ = proxy.send_event(crate::event::Event::new(
                                crate::event::EventType::AiStreamError(format!(
                                    "Inline suggestion failed: {}",
                                    e
                                )),
                                window_id,
                            ));
                        }
                    }
                });
            }
        }
        self.ui.is_loading = true;
    }
    
    /// Start AI conversation with streaming (preferred in UI contexts)
    pub fn start_conversation_stream(
        &mut self,
        prompt: String,
        proxy: winit::event_loop::EventLoopProxy<crate::event::Event>,
        window_id: winit::window::WindowId,
    ) -> Result<String> {
        self.ui.is_loading = true;
        self.ui.streaming_active = true;
        self.ui.error_message = None;
        self.ui.streaming_text.clear();
        self.last_activity = Instant::now();

        // Generate conversation ID and record history
        let conversation_id = uuid::Uuid::new_v4().to_string();
        self.ui.conversation_id = Some(conversation_id.clone());
        self.conversations.insert(conversation_id.clone(), vec![(prompt.clone(), String::new())]);
        self.push_prompt_history(&prompt);

        // Snapshot provider and config
        let prov = self.active_provider.clone();
        let cfg = self
            .providers
            .get(&prov)
            .cloned()
            .unwrap_or_else(AiProviderConfig::default);
        let http = self.http.clone();
        match prov.clone() {
            AiProvider::Custom(name) => {
                if let Some(custom) = self.custom_providers.get(&name).cloned() {
                    tokio::spawn(async move {
                        if let Err(e) = custom.chat_stream(http, cfg, prompt, proxy.clone(), window_id).await {
                            let _ = proxy.send_event(crate::event::Event::new(
                                crate::event::EventType::AiStreamError(format!("Explain failed: {}", e)),
                                window_id,
                            ));
                        }
                    });
                } else {
                    let _ = proxy.send_event(crate::event::Event::new(
                        crate::event::EventType::AiStreamError(format!("Custom provider '{}' not registered", name)),
                        window_id,
                    ));
                }
            }
            _ => {
                tokio::spawn(async move {
                    if let Err(e) = AiRuntime::provider_chat_stream_owned(http, prov, cfg, prompt, proxy.clone(), window_id).await {
                        let _ = proxy.send_event(crate::event::Event::new(
                            crate::event::EventType::AiStreamError(format!("Explain failed: {}", e)),
                            window_id,
                        ));
                    }
                });
            }
        }

        Ok(conversation_id)
    }

    /// Propose explain
    pub fn propose_explain(
        &mut self,
        text: String,
        working_dir: std::path::PathBuf,
        _shell: crate::blocks_v2::ShellType,
        proxy: winit::event_loop::EventLoopProxy<crate::event::Event>,
        window_id: winit::window::WindowId,
    ) {
        let prompt = format!(
            "Explain this output encountered in {}: \n{}\nProvide cause and next steps.",
            working_dir.display(),
            text
        );
        let prov = self.active_provider.clone();
        let cfg = self
            .providers
            .get(&prov)
            .cloned()
            .unwrap_or_else(AiProviderConfig::default);
        let http = self.http.clone();
        tokio::spawn(async move {
            if let Err(e) = AiRuntime::provider_chat_stream_owned(http, prov, cfg, prompt, proxy.clone(), window_id).await {
                let _ = proxy.send_event(crate::event::Event::new(
                    crate::event::EventType::AiStreamError(format!("Explain failed: {}", e)),
                    window_id,
                ));
            }
        });
        self.ui.is_loading = true;
        self.ui.streaming_active = true;
        self.ui.streaming_text.clear();
    }
    
    /// Propose fix
    pub fn propose_fix(
        &mut self,
        error_text: String,
        context: String,
        working_dir: std::path::PathBuf,
        _shell: crate::blocks_v2::ShellType,
        proxy: winit::event_loop::EventLoopProxy<crate::event::Event>,
        window_id: winit::window::WindowId,
    ) {
        let prompt = format!(
            "Given this error in {}: \n{}\nContext: {}\nProvide 1-3 concrete shell commands to fix, with brief justification.",
            working_dir.display(),
            error_text,
            context
        );
        let prov = self.active_provider.clone();
        let cfg = self
            .providers
            .get(&prov)
            .cloned()
            .unwrap_or_else(AiProviderConfig::default);
        let http = self.http.clone();
        match prov.clone() {
            AiProvider::Custom(name) => {
                if let Some(custom) = self.custom_providers.get(&name).cloned() {
                    tokio::spawn(async move {
                        if let Err(e) = custom.chat_stream(http, cfg, prompt, proxy.clone(), window_id).await {
                            let _ = proxy.send_event(crate::event::Event::new(
                                crate::event::EventType::AiStreamError(format!("Fix proposal failed: {}", e)),
                                window_id,
                            ));
                        }
                    });
                } else {
                    let _ = proxy.send_event(crate::event::Event::new(
                        crate::event::EventType::AiStreamError(format!("Custom provider '{}' not registered", name)),
                        window_id,
                    ));
                }
            }
            _ => {
                tokio::spawn(async move {
                    if let Err(e) = AiRuntime::provider_chat_stream_owned(http, prov, cfg, prompt, proxy.clone(), window_id).await {
                        let _ = proxy.send_event(crate::event::Event::new(
                            crate::event::EventType::AiStreamError(format!("Fix proposal failed: {}", e)),
                            window_id,
                        ));
                    }
                });
            }
        }
        self.ui.is_loading = true;
        self.ui.streaming_active = true;
        self.ui.streaming_text.clear();
    }
    
    /// Reconfigure provider using env-based provider config, resolving credentials/endpoints
    pub fn reconfigure_to(&mut self, provider_name: &str, config: &crate::config::ai::ProviderConfig) {
        let p = match provider_name.to_lowercase().as_str() {
            "ollama" => AiProvider::Ollama,
            "openai" => AiProvider::OpenAI,
            "anthropic" => AiProvider::Anthropic,
            "openrouter" => AiProvider::OpenRouter,
            other => AiProvider::Custom(other.to_string()),
        };
        match crate::config::ai_providers::ProviderCredentials::from_config(provider_name, config) {
            Ok(creds) => {
                let cfg = AiProviderConfig {
                    enabled: true,
                    api_key: creds.api_key,
                    base_url: creds.endpoint,
                    model: creds.model.unwrap_or_else(|| "gpt-3.5-turbo".to_string()),
                    max_tokens: None,
                    temperature: None,
                    timeout_seconds: 30,
                };
                self.providers.insert(p.clone(), cfg);
                let _ = self.switch_provider(p);
                self.ui.current_provider = provider_name.to_string();
            }
            Err(e) => {
                self.set_error(format!("Provider reconfigure error ({}): {}", provider_name, e));
            }
        }
    }
    
    

    /// Apply full context configuration
    pub fn set_context_config(&mut self, cfg: crate::config::ai::AiContextConfig) {
        self.context_config = cfg;
    }

    /// Set routing mode
    pub fn set_routing_mode(&mut self, mode: crate::config::ai::AiRoutingMode) {
        self.routing_mode = mode;
    }

    /// Set apply joiner strategy
    pub fn set_apply_joiner(&mut self, strategy: crate::config::ai::AiApplyJoinStrategy) {
        self.apply_joiner = strategy;
    }

    /// Set history retention settings
    pub fn set_history_retention(&mut self, hr: crate::config::ai::AiHistoryRetention) {
        self.history_retention = hr;
    }

    /// Toggle AI panel visibility
    pub fn toggle(&mut self) {
        self.ui.active = !self.ui.active;
        if !self.ui.active {
            self.clear();
        }
        self.last_activity = Instant::now();
    }

    /// Regenerate last response (non-streaming fallback)
    pub async fn regenerate(&mut self) -> Result<()> {
        if let Some(conversation_id) = &self.ui.conversation_id {
            if let Some(history) = self.conversations.get(conversation_id) {
                if let Some((prompt, _)) = history.last() {
                    let prompt = prompt.clone();
                    self.ui.current_response.clear();
                    self.start_conversation(prompt).await?;
                }
            }
        }
        Ok(())
    }

    /// Regenerate last response using streaming (preferred for UI)
    pub fn regenerate_streaming(
        &mut self,
        proxy: winit::event_loop::EventLoopProxy<crate::event::Event>,
        window_id: winit::window::WindowId,
    ) {
        if let Some(conversation_id) = &self.ui.conversation_id {
            if let Some(history) = self.conversations.get(conversation_id) {
                if let Some((prompt, _)) = history.last() {
                    let prompt = prompt.clone();
                    // Reset UI state for streaming
                    self.ui.current_response.clear();
                    self.ui.streaming_text.clear();
                    self.ui.is_loading = true;
                    self.ui.streaming_active = true;
                    self.ui.error_message = None;
                    self.last_activity = Instant::now();

                    // Snapshot provider and config
                    let prov = self.active_provider.clone();
                    let cfg = self
                        .providers
                        .get(&prov)
                        .cloned()
                        .unwrap_or_else(AiProviderConfig::default);
                    let http = self.http.clone();
                    tokio::spawn(async move {
                        if let Err(e) = AiRuntime::provider_chat_stream_owned(http, prov, cfg, prompt, proxy.clone(), window_id).await {
                            let _ = proxy.send_event(crate::event::Event::new(
                                crate::event::EventType::AiStreamError(e.to_string()),
                                window_id,
                            ));
                        }
                    });
                }
            }
        }
    }
    
    /// Configure a specific AI provider with custom config type
    pub fn configure_provider<T>(&mut self, provider: AiProvider, config: T) -> Result<()>
    where
        T: Into<AiProviderConfig>,
    {
        let config = config.into();
        self.providers.insert(provider.clone(), config);
        
        // If this is the first enabled provider, make it active
        if self.providers.len() == 1 {
            self.active_provider = provider.clone();
            self.ui.provider = provider;
        }
        
        Ok(())
    }
}

impl AiRuntime {
    async fn provider_chat(&self, user_prompt: &str) -> Result<String> {
        let prov = self.active_provider.clone();
        let cfg = self
            .providers
            .get(&prov)
            .cloned()
            .ok_or_else(|| anyhow!("Active provider not configured"))?;
        match prov.clone() {
            AiProvider::Custom(name) => {
                let prov_impl = self
                    .custom_providers
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| anyhow!(format!("Custom provider '{}' not registered", name)))?;
                prov_impl.chat(self.http.clone(), cfg, user_prompt.to_string()).await
            }
            _ => Self::provider_chat_owned(
                self.http.clone(),
                prov,
                cfg,
                user_prompt.to_string(),
            )
            .await,
        }
    }

    /// Submit a prompt (optionally with serialized context) and get a structured response
    pub async fn submit_prompt(&self, prompt: String, _context: Option<String>) -> Result<AgentResponse> {
        let content = self.provider_chat(&prompt).await?;
        Ok(AgentResponse { content, metadata: HashMap::new() })
    }

    async fn provider_chat_owned(
        http: Client,
        prov: AiProvider,
        cfg: AiProviderConfig,
        user_prompt: String,
    ) -> Result<String> {
        let _timeout = Duration::from_secs(cfg.timeout_seconds);
        match prov {
            AiProvider::OpenAI => {
                #[derive(Serialize)] struct Msg<'a>{role:&'a str, content:&'a str}
                #[derive(Serialize)] struct Req<'a>{model:&'a str, messages:Vec<Msg<'a>>, temperature: f32}
                #[derive(Deserialize)] struct Resp{choices:Vec<Choice>}
                #[derive(Deserialize)] struct Choice{message: OM}
                #[derive(Deserialize)] struct OM{content:String}
                let key = cfg.api_key.clone().ok_or_else(|| anyhow!("OPENAI api_key missing"))?;
let req = Req{model:&cfg.model, messages: vec![Msg{role:"system", content:"You are an expert terminal assistant."}, Msg{role:"user", content:&user_prompt}], temperature: cfg.temperature.unwrap_or(0.2)};
                let resp = http
                    .post("https://api.openai.com/v1/chat/completions")
                    .bearer_auth(key)
                    .json(&req)
                    .send()
                    .await?;
                if !resp.status().is_success(){ return Err(anyhow!("OpenAI {}", resp.text().await.unwrap_or_default())); }
                let data: Resp = resp.json().await.context("OpenAI JSON error")?;
                Ok(data.choices.into_iter().next().map(|c| c.message.content).unwrap_or_default())
            }
            AiProvider::Anthropic => {
                #[derive(Serialize)] struct Msg<'a>{role:&'a str, content:&'a str}
                #[derive(Serialize)] struct Req<'a>{model:&'a str, system:&'a str, messages:Vec<Msg<'a>>, max_tokens:u32}
                #[derive(Deserialize)] struct Resp{content:Vec<C>}
                #[derive(Deserialize)] struct C{text:String}
                let key = cfg.api_key.clone().ok_or_else(|| anyhow!("ANTHROPIC api_key missing"))?;
let req = Req{model:&cfg.model, system:"You are an expert terminal assistant.", messages: vec![Msg{role:"user", content:&user_prompt}], max_tokens: cfg.max_tokens.unwrap_or(1024)};
                let base = cfg
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.anthropic.com".to_string());
                let url = format!("{}/v1/messages", base);
                let resp = http
                    .post(url)
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&req)
                    .send()
                    .await?;
                if !resp.status().is_success(){ return Err(anyhow!("Anthropic {}", resp.text().await.unwrap_or_default())); }
                let data: Resp = resp.json().await.context("Anthropic JSON error")?;
                Ok(data.content.into_iter().next().map(|c| c.text).unwrap_or_default())
            }
            AiProvider::OpenRouter => {
                #[derive(Serialize)] struct Msg<'a>{role:&'a str, content:&'a str}
                #[derive(Serialize)] struct Req<'a>{model:&'a str, messages:Vec<Msg<'a>>}
                #[derive(Deserialize)] struct Resp{choices:Vec<Choice>}
                #[derive(Deserialize)] struct Choice{message: OM}
                #[derive(Deserialize)] struct OM{content:String}
                let key = cfg.api_key.clone().ok_or_else(|| anyhow!("OPENROUTER api_key missing"))?;
                let base = cfg
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
                let url = format!("{}/chat/completions", base);
                let req = Req {
                    model: &cfg.model,
                    messages: vec![
                        Msg { role: "system", content: "You are an expert terminal assistant." },
                        Msg { role: "user", content: &user_prompt },
                    ],
                };
let resp = http.post(url).bearer_auth(key).json(&req).send().await?;
                if !resp.status().is_success(){ return Err(anyhow!("OpenRouter {}", resp.text().await.unwrap_or_default())); }
                let data: Resp = resp.json().await.context("OpenRouter JSON error")?;
                Ok(data.choices.into_iter().next().map(|c| c.message.content).unwrap_or_default())
            }
            AiProvider::Ollama => {
                #[derive(Serialize)] struct Msg<'a>{role:&'a str, content:&'a str}
                #[derive(Serialize)] struct Req<'a>{model:&'a str, messages:Vec<Msg<'a>>}
                #[derive(Deserialize)] struct Resp{message:Option<OM>}
                #[derive(Deserialize)] struct OM{content:String}
                let base = cfg
                    .base_url
                    .clone()
                    .unwrap_or_else(|| "http://localhost:11434".to_string());
                let url = format!("{}/api/chat", base);
                let req = Req {
                    model: &cfg.model,
                    messages: vec![
                        Msg { role: "system", content: "You are an expert terminal assistant." },
                        Msg { role: "user", content: &user_prompt },
                    ],
                };
                let resp = http.post(url).json(&req).send().await?;
                if !resp.status().is_success(){ return Err(anyhow!("Ollama {}", resp.text().await.unwrap_or_default())); }
                let data: Resp = resp.json().await.context("Ollama JSON error")?;
                Ok(data.message.map(|m| m.content).unwrap_or_default())
            }
            AiProvider::Custom(_) => {
                // Generic OpenAI-compatible fallback for custom providers
                #[derive(Serialize)] struct Msg<'a>{role:&'a str, content:&'a str}
                #[derive(Serialize)] struct Req<'a>{model:&'a str, messages:Vec<Msg<'a>>, #[allow(dead_code)] temperature: Option<f32>}
                #[derive(Deserialize)] struct Resp{choices:Vec<Choice>}
                #[derive(Deserialize)] struct Choice{message: OM}
                #[derive(Deserialize)] struct OM{content:String}
                let base = cfg
                    .base_url
                    .clone()
                    .ok_or_else(|| anyhow!("Custom provider requires base_url in config"))?;
                let url = format!("{}/chat/completions", base.trim_end_matches('/'));
                let req = Req{
                    model:&cfg.model,
                    messages: vec![
                        Msg{role:"system", content:"You are an expert terminal assistant."},
                        Msg{role:"user", content:&user_prompt}
                    ],
                    temperature: cfg.temperature,
                };
                let mut request = http.post(url).json(&req);
                if let Some(key) = cfg.api_key.clone() {
                    request = request.bearer_auth(key);
                }
                let resp = request.send().await?;
                if !resp.status().is_success(){ return Err(anyhow!("Custom provider {}", resp.text().await.unwrap_or_default())); }
                let data: Resp = resp.json().await.context("Custom provider JSON error")?;
                Ok(data.choices.into_iter().next().map(|c| c.message.content).unwrap_or_default())
            },
        }
    }

    /// Streaming variant that emits AiStreamChunk events as content arrives.
    async fn provider_chat_stream_owned(
        http: Client,
        prov: AiProvider,
        cfg: AiProviderConfig,
        user_prompt: String,
        proxy: winit::event_loop::EventLoopProxy<crate::event::Event>,
        window_id: winit::window::WindowId,
    ) -> Result<()> {
        match prov {
            AiProvider::OpenAI => {
                #[derive(serde::Serialize)] struct Msg<'a>{role:&'a str, content:&'a str}
                #[derive(serde::Serialize)] struct Req<'a>{model:&'a str, messages:Vec<Msg<'a>>, temperature: f32, stream: bool}
                let key = cfg.api_key.clone().ok_or_else(|| anyhow!("OPENAI api_key missing"))?;
                let req = Req{
                    model:&cfg.model,
                    messages: vec![
                        Msg{role:"system", content:"You are an expert terminal assistant."},
                        Msg{role:"user", content:&user_prompt}
                    ],
                    temperature: cfg.temperature.unwrap_or(0.2),
                    stream: true,
                };
                let resp = http
                    .post("https://api.openai.com/v1/chat/completions")
                    .bearer_auth(key)
                    .json(&req)
                    .send()
                    .await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("OpenAI {}", resp.text().await.unwrap_or_default()));
                }
                let mut stream = resp.bytes_stream();
                // Buffer for partial lines
                let mut buf = Vec::<u8>::new();
                while let Some(next) = stream.next().await {
                    let chunk = next?;
                    buf.extend_from_slice(&chunk);
                    // Process complete lines
                    let mut start = 0usize;
                    for i in 0..buf.len() {
                        if buf[i] == b'\n' {
                            let line = &buf[start..i];
                            start = i + 1;
                            // Skip keepalive or empty lines
                            if line.is_empty() || line[0] == b':' { continue; }
                            // Expect lines like: b"data: {...}" or b"data: [DONE]"
                            const DATA: &str = "data: ";
                            if line.len() >= DATA.len() && &line[..DATA.len()] == DATA.as_bytes() {
                                let payload = &line[DATA.len()..];
                                if payload == b"[DONE]" { continue; }
                                // Parse JSON with delta content
                                #[derive(serde::Deserialize)] struct Choice { delta: Delta }
                                #[derive(serde::Deserialize)] struct Delta { content: Option<String> }
                                #[derive(serde::Deserialize)] struct SResp { choices: Vec<Choice> }
                                if let Ok(sr) = serde_json::from_slice::<SResp>(payload) {
                                    if let Some(text) = sr.choices.into_iter().filter_map(|c| c.delta.content).next() {
                                        let _ = proxy.send_event(crate::event::Event::new(
                                            crate::event::EventType::AiStreamChunk(AiStreamChunk { content: text, is_complete: false, metadata: HashMap::new() }),
                                            window_id,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    // Retain trailing partial
                    if start > 0 { buf.drain(0..start); }
                }
                let _ = proxy.send_event(crate::event::Event::new(crate::event::EventType::AiStreamFinished, window_id));
                Ok(())
            }
            AiProvider::Anthropic => {
                // Anthropic Messages API streaming over SSE
                // We stream events and extract text from content_block_delta events
                #[derive(serde::Serialize)] struct Msg<'a>{role:&'a str, content:&'a str}
                #[derive(serde::Serialize)] struct Req<'a>{model:&'a str, system:&'a str, messages:Vec<Msg<'a>>, max_tokens:u32, stream: bool}
                // Event payloads have a type and may include delta or content_block
                #[derive(serde::Deserialize)] struct ADelta{ #[serde(default)] text: Option<String> }
                #[derive(serde::Deserialize)] struct AContentBlock{ #[serde(default)] text: Option<String> }
                #[derive(serde::Deserialize)] struct AEvent{ #[serde(rename="type")] etype: String, delta: Option<ADelta>, content_block: Option<AContentBlock> }
                let key = cfg.api_key.clone().ok_or_else(|| anyhow!("ANTHROPIC api_key missing"))?;
                let base = cfg.base_url.clone().unwrap_or_else(|| "https://api.anthropic.com".to_string());
                let url = format!("{}/v1/messages", base);
                let req = Req{ model: &cfg.model, system: "You are an expert terminal assistant.", messages: vec![Msg{role:"user", content:&user_prompt}], max_tokens: cfg.max_tokens.unwrap_or(1024), stream: true };
                let resp = http.post(url)
                    .header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&req)
                    .send()
                    .await?;
                if !resp.status().is_success(){ return Err(anyhow!("Anthropic {}", resp.text().await.unwrap_or_default())); }
                let mut stream = resp.bytes_stream();
                let mut buf = Vec::<u8>::new();
                while let Some(next) = stream.next().await {
                    let chunk = next?;
                    buf.extend_from_slice(&chunk);
                    let mut start = 0usize;
                    for i in 0..buf.len() {
                        if buf[i] == b'\n' {
                            let line = &buf[start..i];
                            start = i + 1;
                            if line.is_empty() { continue; }
                            // We expect SSE lines with either "event:" or "data:". Focus on data lines.
                            const DATA: &str = "data: ";
                            if line.len() >= DATA.len() && &line[..DATA.len()] == DATA.as_bytes() {
                                let payload = &line[DATA.len()..];
                                if payload == b"[DONE]" { continue; }
                                if let Ok(ev) = serde_json::from_slice::<AEvent>(payload) {
                                    // content_block_delta -> delta.text
                                    if ev.etype == "content_block_delta" {
                                        if let Some(ADelta{ text: Some(t) }) = ev.delta { if !t.is_empty() {
                                            let _ = proxy.send_event(crate::event::Event::new(
                                                crate::event::EventType::AiStreamChunk(AiStreamChunk { content: t, is_complete: false, metadata: HashMap::new() }),
                                                window_id,
                                            ));
                                        }}
                                    // content_block_start may include initial text (rare but allowed)
                                    } else if ev.etype == "content_block_start" {
                                        if let Some(AContentBlock{ text: Some(t) }) = ev.content_block { if !t.is_empty() {
                                            let _ = proxy.send_event(crate::event::Event::new(
                                                crate::event::EventType::AiStreamChunk(AiStreamChunk { content: t, is_complete: false, metadata: HashMap::new() }),
                                                window_id,
                                            ));
                                        }}
                                    }
                                }
                            }
                        }
                    }
                    if start > 0 { buf.drain(0..start); }
                }
                let _ = proxy.send_event(crate::event::Event::new(crate::event::EventType::AiStreamFinished, window_id));
                Ok(())
            }
            AiProvider::OpenRouter => {
                // OpenRouter is OpenAI-compatible for chat completions streaming
                #[derive(serde::Serialize)] struct Msg<'a>{role:&'a str, content:&'a str}
                #[derive(serde::Serialize)] struct Req<'a>{model:&'a str, messages:Vec<Msg<'a>>, stream: bool}
                #[derive(serde::Deserialize)] struct Choice{ delta: OM }
                #[derive(serde::Deserialize)] struct OM{ content: Option<String> }
                #[derive(serde::Deserialize)] struct SResp{ choices: Vec<Choice> }
                let key = cfg.api_key.clone().ok_or_else(|| anyhow!("OPENROUTER api_key missing"))?;
                let base = cfg.base_url.clone().unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
                let url = format!("{}/chat/completions", base);
                let req = Req{ model: &cfg.model, messages: vec![ Msg{role:"system", content:"You are an expert terminal assistant."}, Msg{role:"user", content:&user_prompt} ], stream: true };
                let resp = http.post(url)
                    .bearer_auth(key)
                    .json(&req)
                    .send()
                    .await?;
                if !resp.status().is_success(){ return Err(anyhow!("OpenRouter {}", resp.text().await.unwrap_or_default())); }
                let mut stream = resp.bytes_stream();
                let mut buf = Vec::<u8>::new();
                while let Some(next) = stream.next().await {
                    let chunk = next?;
                    buf.extend_from_slice(&chunk);
                    let mut start = 0usize;
                    for i in 0..buf.len() {
                        if buf[i] == b'\n' {
                            let line = &buf[start..i];
                            start = i + 1;
                            if line.is_empty() || line[0] == b':' { continue; }
                            const DATA: &str = "data: ";
                            if line.len() >= DATA.len() && &line[..DATA.len()] == DATA.as_bytes() {
                                let payload = &line[DATA.len()..];
                                if payload == b"[DONE]" { continue; }
                                if let Ok(sr) = serde_json::from_slice::<SResp>(payload) {
                                    if let Some(text) = sr.choices.into_iter().filter_map(|c| c.delta.content).next() {
                                        let _ = proxy.send_event(crate::event::Event::new(
                                            crate::event::EventType::AiStreamChunk(AiStreamChunk { content: text, is_complete: false, metadata: HashMap::new() }),
                                            window_id,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    if start > 0 { buf.drain(0..start); }
                }
                let _ = proxy.send_event(crate::event::Event::new(crate::event::EventType::AiStreamFinished, window_id));
                Ok(())
            }
            AiProvider::Ollama => {
                // Stream JSON lines from Ollama chat API
                #[derive(serde::Serialize)] struct Msg<'a>{role:&'a str, content:&'a str}
                #[derive(serde::Serialize)] struct Req<'a>{model:&'a str, messages:Vec<Msg<'a>>, stream: bool}
                #[derive(serde::Deserialize)] struct M{content: String}
                #[derive(serde::Deserialize)] struct Line{message: Option<M>, response: Option<String>, done: Option<bool>}
                let base = cfg.base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string());
                let url = format!("{}/api/chat", base);
                let req = Req{model:&cfg.model, messages: vec![Msg{role:"system", content:"You are an expert terminal assistant."}, Msg{role:"user", content:&user_prompt}], stream: true};
                let resp = http.post(url).json(&req).send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("Ollama {}", resp.text().await.unwrap_or_default()));
                }
                let mut stream = resp.bytes_stream();
                let mut buf = Vec::<u8>::new();
                while let Some(next) = stream.next().await {
                    let chunk = next?;
                    buf.extend_from_slice(&chunk);
                    let mut start = 0usize;
                    for i in 0..buf.len() {
                        if buf[i] == b'\n' {
                            let line = &buf[start..i];
                            start = i + 1;
                            if line.is_empty() { continue; }
                            if let Ok(l) = serde_json::from_slice::<Line>(line) {
                                if let Some(done) = l.done { if done { continue; } }
                                if let Some(m) = l.message { if !m.content.is_empty() {
                                    let _ = proxy.send_event(crate::event::Event::new(
                                        crate::event::EventType::AiStreamChunk(AiStreamChunk { content: m.content, is_complete: false, metadata: HashMap::new() }),
                                        window_id,
                                    ));
                                }} else if let Some(resp) = l.response { if !resp.is_empty() {
                                    let _ = proxy.send_event(crate::event::Event::new(
                                        crate::event::EventType::AiStreamChunk(AiStreamChunk { content: resp, is_complete: false, metadata: HashMap::new() }),
                                        window_id,
                                    ));
                                }}
                            }
                        }
                    }
                    if start > 0 { buf.drain(0..start); }
                }
                let _ = proxy.send_event(crate::event::Event::new(crate::event::EventType::AiStreamFinished, window_id));
                Ok(())
            }
            // For other providers, fall back to non-streaming for now
            _ => {
                match AiRuntime::provider_chat_owned(http, prov, cfg, user_prompt).await {
                    Ok(text) => {
                        let _ = proxy.send_event(crate::event::Event::new(
                            crate::event::EventType::AiStreamChunk(AiStreamChunk { content: text, is_complete: true, metadata: HashMap::new() }),
                            window_id,
                        ));
                        let _ = proxy.send_event(crate::event::Event::new(crate::event::EventType::AiStreamFinished, window_id));
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

}

impl Default for AiRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Constants for AI inline suggestions
pub const AI_INLINE_SUGGEST_DEBOUNCE: Duration = Duration::from_millis(500);

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_ai_runtime_creation() {
        let runtime = AiRuntime::new();
        assert!(!runtime.ui.active);
        assert_eq!(runtime.active_provider, AiProvider::default());
        assert!(!runtime.ui.streaming_active);
    }

    #[test]
    fn test_provider_switching() {
        let mut runtime = AiRuntime::new();
        assert!(runtime.switch_provider(AiProvider::OpenAI).is_ok());
        assert_eq!(runtime.active_provider, AiProvider::OpenAI);
        assert_eq!(runtime.ui.provider, AiProvider::OpenAI);
    }

    #[test]
    fn test_copy_output_formats() {
        let mut runtime = AiRuntime::new();
        runtime.ui.current_response = "Here is some code:\n```rust\nfn main() {}\n```\nThat's it!".to_string();
        
        let text_copy = runtime.copy_output(AiCopyFormat::Text);
        assert!(text_copy.is_some());
        assert!(text_copy.unwrap().contains("Here is some code"));
        
        let code_copy = runtime.copy_output(AiCopyFormat::Code);
        assert!(code_copy.is_some());
        assert_eq!(code_copy.unwrap().trim(), "fn main() {}");
    }

    #[test]
    fn test_proposal_navigation() {
        let mut runtime = AiRuntime::new();
        runtime.add_simple_proposal("Proposal 1".to_string());
        runtime.add_simple_proposal("Proposal 2".to_string());
        runtime.add_simple_proposal("Proposal 3".to_string());
        
        // Test next navigation
        runtime.select_next_proposal();
        assert_eq!(runtime.ui.selected_proposal, Some(1));
        
        runtime.select_next_proposal();
        assert_eq!(runtime.ui.selected_proposal, Some(2));
        
        runtime.select_next_proposal(); // Should wrap around
        assert_eq!(runtime.ui.selected_proposal, Some(0));
        
        // Test previous navigation
        runtime.select_prev_proposal();
        assert_eq!(runtime.ui.selected_proposal, Some(2));
    }
}
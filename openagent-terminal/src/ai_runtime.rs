//! AI runtime: UI state and provider wiring (optional feature)
#![cfg(feature = "ai")]

use std::collections::VecDeque;
use log::{debug, error, info};

use openagent_terminal_ai::{AiProvider, AiProposal, AiRequest, create_provider};

/// Maximum history entries to keep
const MAX_HISTORY: usize = 100;

#[derive(Debug, Clone)]
pub struct AiUiState {
    pub active: bool,
    pub scratch: String,
    pub cursor_position: usize,
    pub proposals: Vec<AiProposal>,
    pub selected_proposal: usize,
    pub is_loading: bool,
    pub error_message: Option<String>,
    pub history: VecDeque<String>,
    pub history_index: Option<usize>,
    // Streaming state
    pub streaming_active: bool,
    pub streaming_text: String,
}

impl Default for AiUiState {
    fn default() -> Self {
        Self {
            active: false,
            scratch: String::new(),
            cursor_position: 0,
            proposals: Vec::new(),
            selected_proposal: 0,
            is_loading: false,
            error_message: None,
            history: VecDeque::with_capacity(MAX_HISTORY),
            history_index: None,
            streaming_active: false,
            streaming_text: String::new(),
        }
    }
}

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;
use crate::event::{Event, EventType};

pub struct AiRuntime {
    pub ui: AiUiState,
    pub provider: Arc<dyn AiProvider>,
    cancel_flag: Arc<AtomicBool>,
}

impl AiRuntime {
    pub fn new(provider: Box<dyn AiProvider>) -> Self {
        info!("AI runtime initialized with provider: {}", provider.name());
        Self { ui: AiUiState::default(), provider: Arc::from(provider), cancel_flag: Arc::new(AtomicBool::new(false)) }
    }

    pub fn from_config(
        provider_id: Option<&str>,
        endpoint_env: Option<&str>,
        api_key_env: Option<&str>,
        model_env: Option<&str>,
    ) -> Self {
        // Set environment variables if provided
        if let Some(env_name) = endpoint_env {
            if let Ok(value) = std::env::var(env_name) {
                std::env::set_var("OLLAMA_ENDPOINT", value.clone());
                std::env::set_var("OPENAI_API_BASE", value);
            }
        }
        if let Some(env_name) = api_key_env {
            if let Ok(value) = std::env::var(env_name) {
                std::env::set_var("OPENAI_API_KEY", value.clone());
                std::env::set_var("ANTHROPIC_API_KEY", value);
            }
        }
        if let Some(env_name) = model_env {
            if let Ok(value) = std::env::var(env_name) {
                std::env::set_var("OLLAMA_MODEL", value.clone());
                std::env::set_var("OPENAI_MODEL", value);
            }
        }
        
        let provider_name = provider_id.unwrap_or("null");
        let provider = match create_provider(provider_name) {
            Ok(p) => {
                info!("Successfully created AI provider: {}", provider_name);
                p
            },
            Err(e) => {
                error!("Failed to create provider '{}': {}", provider_name, e);
                Box::new(openagent_terminal_ai::NullProvider::default())
            }
        };
        
        Self::new(provider)
    }

    /// Begin a streaming proposal in a background thread. Falls back to blocking propose if the
    /// provider doesn't support streaming.
    pub fn start_propose_stream(
        &mut self,
        working_directory: Option<String>,
        shell_kind: Option<String>,
        event_proxy: EventLoopProxy<Event>,
        window_id: WindowId,
    ) {
        info!("ai_runtime_stream_start provider={} scratch_len={}", self.provider.name(), self.ui.scratch.len());
        if self.ui.scratch.trim().is_empty() {
            self.ui.error_message = Some("Query cannot be empty".to_string());
            return;
        }

        // Reset state
        self.ui.proposals.clear();
        self.ui.selected_proposal = 0;
        self.ui.error_message = None;
        self.ui.is_loading = true;
        self.ui.streaming_active = true;
        self.ui.streaming_text.clear();
        self.cancel_flag.store(false, Ordering::Relaxed);

        let cancel = self.cancel_flag.clone();
        let provider = self.provider.clone();
        let req = AiRequest {
            scratch_text: self.ui.scratch.clone(),
            working_directory,
            shell_kind,
            context: vec![("platform".to_string(), std::env::consts::OS.to_string())],
        };

        // Spawn background worker
        let _ = thread::Builder::new().name("ai-stream".into()).spawn(move || {
            // First try streaming
            let mut on_chunk = |chunk: &str| {
                // Send chunk event
                let _ = event_proxy.send_event(Event::new(EventType::AiStreamChunk(chunk.to_string()), window_id));
            };
            match provider.propose_stream(req.clone(), &mut on_chunk, &cancel) {
                Ok(true) => {
                    info!("ai_runtime_stream_finished provider={}", provider.name());
                    let _ = event_proxy.send_event(Event::new(EventType::AiStreamFinished, window_id));
                },
                Ok(false) => {
                    info!("ai_runtime_fallback_blocking provider={}", provider.name());
                    let result = provider.propose(req);
                    match result {
                        Ok(proposals) => {
                            info!("ai_runtime_blocking_complete proposals={}", proposals.len());
                            let _ = event_proxy.send_event(Event::new(EventType::AiProposals(proposals), window_id));
                        },
                        Err(e) => {
                            error!("ai_runtime_blocking_error error={}", e);
                            let _ = event_proxy.send_event(Event::new(EventType::AiStreamError(e), window_id));
                        },
                    }
                },
                Err(e) => {
                    error!("ai_runtime_stream_error error={}", e);
                    let _ = event_proxy.send_event(Event::new(EventType::AiStreamError(e), window_id));
                },
            }
        });
    }

    /// Cancel any in-flight streaming.
    pub fn cancel(&mut self) {
        info!("ai_runtime_cancel_requested provider={}", self.provider.name());
        self.cancel_flag.store(true, Ordering::SeqCst);
        self.ui.streaming_active = false;
        self.ui.is_loading = false;
    }

    pub fn propose(&mut self, working_directory: Option<String>, shell_kind: Option<String>) {
        if self.ui.scratch.trim().is_empty() {
            self.ui.error_message = Some("Query cannot be empty".to_string());
            return;
        }
        
        // Add to history
        self.ui.history.push_front(self.ui.scratch.clone());
        if self.ui.history.len() > MAX_HISTORY {
            self.ui.history.pop_back();
        }
        self.ui.history_index = None;
        
        // Clear previous state
        self.ui.proposals.clear();
        self.ui.selected_proposal = 0;
        self.ui.error_message = None;
        self.ui.is_loading = true;
        
        debug!("Submitting AI query: {}", self.ui.scratch);
        
        let req = AiRequest {
            scratch_text: self.ui.scratch.clone(),
            working_directory,
            shell_kind,
            context: vec![
                ("platform".to_string(), std::env::consts::OS.to_string()),
            ],
        };
        
        match self.provider.propose(req) {
            Ok(proposals) => {
                info!("Received {} proposals", proposals.len());
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            },
            Err(e) => {
                error!("AI query failed: {}", e);
                self.ui.error_message = Some(format!("Query failed: {}", e));
                self.ui.is_loading = false;
            }
        }
    }
    
    /// Toggle AI panel visibility
    pub fn toggle_panel(&mut self) {
        self.ui.active = !self.ui.active;
        if self.ui.active {
            debug!("AI panel opened");
            self.ui.cursor_position = self.ui.scratch.len();
        } else {
            debug!("AI panel closed");
        }
    }
    
    /// Insert text at cursor position
    pub fn insert_text(&mut self, text: &str) {
        self.ui.scratch.insert_str(self.ui.cursor_position, text);
        self.ui.cursor_position += text.len();
    }
    
    /// Handle backspace
    pub fn backspace(&mut self) {
        if self.ui.cursor_position > 0 {
            self.ui.cursor_position -= 1;
            self.ui.scratch.remove(self.ui.cursor_position);
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
    
    /// Navigate history
    pub fn history_previous(&mut self) {
        if self.ui.history.is_empty() {
            return;
        }
        
        let new_index = match self.ui.history_index {
            None => 0,
            Some(i) if i < self.ui.history.len() - 1 => i + 1,
            Some(i) => i,
        };
        
        if let Some(entry) = self.ui.history.get(new_index) {
            self.ui.scratch = entry.clone();
            self.ui.cursor_position = self.ui.scratch.len();
            self.ui.history_index = Some(new_index);
        }
    }
    
    pub fn history_next(&mut self) {
        match self.ui.history_index {
            Some(0) => {
                self.ui.history_index = None;
                self.ui.scratch.clear();
                self.ui.cursor_position = 0;
            },
            Some(i) => {
                let new_index = i - 1;
                if let Some(entry) = self.ui.history.get(new_index) {
                    self.ui.scratch = entry.clone();
                    self.ui.cursor_position = self.ui.scratch.len();
                    self.ui.history_index = Some(new_index);
                }
            },
            None => {},
        }
    }
    
    /// Select next proposal
    pub fn next_proposal(&mut self) {
        if !self.ui.proposals.is_empty() {
            self.ui.selected_proposal = (self.ui.selected_proposal + 1) % self.ui.proposals.len();
        }
    }
    
    /// Select previous proposal
    pub fn previous_proposal(&mut self) {
        if !self.ui.proposals.is_empty() {
            if self.ui.selected_proposal == 0 {
                self.ui.selected_proposal = self.ui.proposals.len() - 1;
            } else {
                self.ui.selected_proposal -= 1;
            }
        }
    }
    
    /// Get selected proposal commands
    pub fn get_selected_commands(&self) -> Option<String> {
        self.ui.proposals.get(self.ui.selected_proposal)
            .map(|p| p.proposed_commands.join("\n"))
    }
}


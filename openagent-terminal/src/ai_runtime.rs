//! AI runtime: UI state and provider wiring (optional feature)
#![allow(dead_code)]

use log::{debug, error, info};
use std::collections::VecDeque;

use crate::security::{SecurityLens, SecurityPolicy};
use openagent_terminal_ai::build_request_with_context;
use openagent_terminal_ai::context::{
    BasicEnvProvider, ContextManager, FileTreeProvider, FileTreeRootStrategy, GitProvider,
};
use openagent_terminal_ai::privacy::{sanitize_request, AiPrivacyOptions};
use openagent_terminal_ai::providers::{
    AnthropicProvider, OllamaProvider, OpenAiProvider, OpenRouterProvider,
};
use openagent_terminal_ai::{create_provider, AiProposal, AiProvider, AiRequest};

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
    #[allow(dead_code)]
    pub history: VecDeque<String>,
    #[allow(dead_code)]
    pub history_index: Option<usize>,
    // Streaming state
    pub streaming_active: bool,
    pub streaming_text: String,
    /// Last time we requested a redraw due to a streaming chunk (for throttling)
    pub streaming_last_redraw: Option<std::time::Instant>,
    /// Inline suggestion text to render as ghost text at the terminal prompt (suffix suggestion)
    pub inline_suggestion: Option<String>,
    /// Current provider id (e.g., "openrouter", "openai", "anthropic", "ollama") for UI display
    pub current_provider: String,
    /// Current model identifier used by the provider (for compact model badge in UI)
    pub current_model: String,
}

impl AiRuntime {
    /// Load previously persisted AI history (best-effort).
    fn load_history(&mut self) {
        let path = Self::history_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(entries) = serde_json::from_str::<Vec<String>>(&data) {
                for s in entries.into_iter().rev() {
                    // maintain most-recent-first order
                    self.ui.history.push_front(s);
                    if self.ui.history.len() > MAX_HISTORY {
                        self.ui.history.pop_back();
                        break;
                    }
                }
            }
        }
    }

    /// Persist AI history to disk (best-effort, synchronous, small file).
    fn save_history(&self) {
        let path = Self::history_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let list: Vec<String> = self.ui.history.iter().cloned().collect();
        if let Ok(json) = serde_json::to_string_pretty(&list) {
            let _ = std::fs::write(&path, json);
        }
    }

    fn history_path() -> std::path::PathBuf {
        let base = dirs::data_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        base.join("openagent-terminal")
            .join("ai")
            .join("history.json")
    }
    /// Reconfigure this runtime to a new provider using secure config, preserving UI scratch/cursor/history.
    pub fn reconfigure_to(
        &mut self,
        provider_name: &str,
        config: &crate::config::ai::ProviderConfig,
    ) {
        use crate::config::ai_providers::ProviderCredentials;
        self.ui.error_message = None;
        // Load credentials
        let credentials = match ProviderCredentials::from_config(provider_name, config) {
            Ok(creds) => creds,
            Err(e) => {
                self.ui.error_message = Some(format!(
                    "Secure credential loading failed for '{}': {}",
                    provider_name, e
                ));
                return;
            }
        };
        // Create provider for new config
        let new_provider: Result<Box<dyn AiProvider>, String> = match provider_name {
            "openai" => {
                let api_key = match credentials.require_api_key(provider_name) {
                    Ok(s) => s.to_string(),
                    Err(e) => {
                        self.ui.error_message =
                            Some(format!("{} provider config error: {}", provider_name, e));
                        return;
                    }
                };
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("https://api.openai.com/v1")
                    .to_string();
                let model = match credentials.require_model(provider_name) {
                    Ok(s) => s.to_string(),
                    Err(e) => {
                        self.ui.error_message =
                            Some(format!("{} provider config error: {}", provider_name, e));
                        return;
                    }
                };
                self.ui.current_model = model.clone();
                OpenAiProvider::new(api_key, endpoint, model)
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
            }
            "openrouter" => {
                let api_key = match credentials.require_api_key(provider_name) {
                    Ok(s) => s.to_string(),
                    Err(e) => {
                        self.ui.error_message =
                            Some(format!("{} provider config error: {}", provider_name, e));
                        return;
                    }
                };
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("https://openrouter.ai/api/v1")
                    .to_string();
                let model = match credentials.require_model(provider_name) {
                    Ok(s) => s.to_string(),
                    Err(e) => {
                        self.ui.error_message =
                            Some(format!("{} provider config error: {}", provider_name, e));
                        return;
                    }
                };
                self.ui.current_model = model.clone();
                OpenRouterProvider::new(api_key, endpoint, model)
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
            }
            "anthropic" => {
                let api_key = match credentials.require_api_key(provider_name) {
                    Ok(s) => s.to_string(),
                    Err(e) => {
                        self.ui.error_message =
                            Some(format!("{} provider config error: {}", provider_name, e));
                        return;
                    }
                };
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("https://api.anthropic.com")
                    .to_string();
                let model = match credentials.require_model(provider_name) {
                    Ok(s) => s.to_string(),
                    Err(e) => {
                        self.ui.error_message =
                            Some(format!("{} provider config error: {}", provider_name, e));
                        return;
                    }
                };
                self.ui.current_model = model.clone();
                AnthropicProvider::new(api_key, endpoint, model)
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
            }
            "ollama" => {
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("http://localhost:11434")
                    .to_string();
                let model = credentials
                    .require_model(provider_name)
                    .map(|s| s.to_string())
                    .or_else(|_| {
                        config
                            .default_model
                            .clone()
                            .ok_or_else(|| "Model required".to_string())
                    })
                    .unwrap_or_else(|_| "".to_string());
                self.ui.current_model = model.clone();
                OllamaProvider::new(endpoint, model).map(|p| Box::new(p) as Box<dyn AiProvider>)
            }
            _ => Err(format!("Unknown provider: {}", provider_name)),
        };
        match new_provider {
            Ok(p) => {
                self.provider = Arc::from(p);
                self.ui.current_provider = provider_name.to_string();
                // Reset transient result state
                self.ui.proposals.clear();
                self.ui.selected_proposal = 0;
                self.ui.is_loading = false;
                self.ui.streaming_active = false;
                self.ui.streaming_text.clear();
                self.ui.error_message = None;
            }
            Err(e) => {
                self.ui.error_message = Some(format!(
                    "Failed to reconfigure provider to '{}': {}",
                    provider_name, e
                ));
            }
        }
    }

    /// Start background computation of an inline suggestion based on the current prompt prefix.
    /// The provider is invoked in a separate thread to avoid blocking the UI.
    pub fn start_inline_suggest(
        &mut self,
        prefix: String,
        event_proxy: EventLoopProxy<Event>,
        window_id: WindowId,
    ) {
        // Clear any previous suggestion immediately
        self.ui.inline_suggestion = None;

        // Build a lightweight prompt for inline completion
        // We bias providers towards command completion, not multi-line explanations.
        let prompt = format!(
            "Complete the shell command. Only return the completed command.\nPartial: \
             {}\nCompletion:",
            prefix
        );

        let provider = self.provider.clone();
        let req = AiRequest {
            scratch_text: prompt,
            working_directory: None,
            shell_kind: None,
            context: vec![
                ("mode".to_string(), "inline".to_string()),
                ("platform".to_string(), std::env::consts::OS.to_string()),
            ],
        };

        let _ = std::thread::Builder::new()
            .name("ai-inline".into())
            .spawn(move || {
                // Non-streaming, single-shot proposal
                let result = provider.propose(req);
                let suggestion = match result {
                    Ok(mut props) => {
                        // Take the first command from the first proposal, if any
                        if let Some(prop) = props.first_mut() {
                            prop.proposed_commands
                                .first()
                                .map(|cmd| compute_suffix(cmd, &prefix))
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                };

                let payload = crate::event::EventType::AiInlineSuggestionReady(
                    suggestion.unwrap_or_default(),
                );
                let _ = event_proxy.send_event(Event::new(payload, window_id));
            });

        // Helper to compute the suffix not yet typed
        fn compute_suffix(candidate: &str, typed: &str) -> String {
            if let Some(stripped) = candidate.strip_prefix(typed) {
                return stripped.to_string();
            }
            // Fallback: compute longest common prefix ignoring consecutive spaces
            let mut i = 0usize;
            let ca: Vec<char> = candidate.chars().collect();
            let ta: Vec<char> = typed.chars().collect();
            while i < ca.len() && i < ta.len() && ca[i] == ta[i] {
                i += 1;
            }
            ca[i..].iter().collect()
        }
    }
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
            streaming_last_redraw: None,
            inline_suggestion: None,
            current_provider: "null".to_string(),
            current_model: String::new(),
        }
    }
}

use crate::event::{Event, EventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

pub struct AiRuntime {
    pub ui: AiUiState,
    pub provider: Arc<dyn AiProvider>,
    cancel_flag: Arc<AtomicBool>,
    security_lens: SecurityLens,
    // Config-driven context collection policy
    context_cfg: crate::config::ai::AiContextConfig,
}

impl AiRuntime {
    pub fn new(provider: Box<dyn AiProvider>) -> Self {
        info!("AI runtime initialized with provider: {}", provider.name());
        let mut ui = AiUiState::default();
        ui.current_provider = provider.name().to_string();
        // Model is provider-specific; when constructed via from_secure_config we will set it.
        ui.current_model.clear();
        let mut rt = Self {
            ui,
            provider: Arc::from(provider),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            security_lens: SecurityLens::new(SecurityPolicy::default()),
            context_cfg: crate::config::ai::AiContextConfig::default(),
        };
        // Load persisted history best-effort
        rt.load_history();
        rt
    }

    pub fn from_config(
        provider_id: Option<&str>,
        _endpoint_env: Option<&str>,
        _api_key_env: Option<&str>,
        _model_env: Option<&str>,
    ) -> Self {
        use tracing::warn;

        // Check for legacy environment variable usage
        crate::config::ai_providers::check_legacy_env_vars();

        // DEPRECATED: This method is deprecated in favor of from_secure_config
        // Maintain backward compatibility but warn users
        warn!(
            "AI runtime from_config is deprecated. Please use from_secure_config with \
             provider-specific configuration."
        );

        // For backward compatibility, attempt to create provider using legacy approach
        let provider_name = provider_id.unwrap_or("null");
        let provider_result = create_provider(provider_name);
        match provider_result {
            Ok(p) => {
                info!("Successfully created AI provider: {}", provider_name);
                Self::new(p)
            }
            Err(e) => {
                error!("Failed to create provider '{}': {}", provider_name, e);
                let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                rt.ui.error_message = Some(format!(
                    "AI provider initialization failed: {}. Please check your AI settings \
                     (provider, endpoint, api key, model). Consider migrating to secure provider \
                     configuration - see docs/AI_ENVIRONMENT_SECURITY.md",
                    e
                ));
                rt
            }
        }
    }

    /// Create AI runtime from secure provider configuration (recommended approach)
    pub fn from_secure_config(
        provider_name: &str,
        config: &crate::config::ai::ProviderConfig,
    ) -> Self {
        use crate::config::ai_providers::ProviderCredentials;

        info!(
            "Initializing AI runtime with secure provider configuration: {}",
            provider_name
        );

        // Extract credentials securely without polluting global environment
        let credentials = match ProviderCredentials::from_config(provider_name, config) {
            Ok(creds) => creds,
            Err(e) => {
                error!(
                    "Failed to load secure credentials for provider '{}': {}",
                    provider_name, e
                );
                let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                rt.ui.error_message = Some(format!(
                    "Secure credential loading failed for '{}': {}. Check your environment \
                     variables and configuration.",
                    provider_name, e
                ));
                return rt;
            }
        };

        // Create provider with isolated credentials
        let mut selected_model: Option<String> = None;
        let provider_result = match provider_name {
            "openai" => {
                let api_key = match credentials.require_api_key(provider_name) {
                    Ok(key) => key.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("https://api.openai.com/v1")
                    .to_string();
                let model = match credentials.require_model(provider_name) {
                    Ok(model) => model.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                selected_model = Some(model.clone());
                OpenAiProvider::new(api_key, endpoint, model)
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
            }
            "openrouter" => {
                let api_key = match credentials.require_api_key(provider_name) {
                    Ok(key) => key.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("https://openrouter.ai/api/v1")
                    .to_string();
                let model = match credentials.require_model(provider_name) {
                    Ok(model) => model.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                selected_model = Some(model.clone());
                OpenRouterProvider::new(api_key, endpoint, model)
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
            }
            "anthropic" => {
                let api_key = match credentials.require_api_key(provider_name) {
                    Ok(key) => key.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("https://api.anthropic.com")
                    .to_string();
                let model = match credentials.require_model(provider_name) {
                    Ok(model) => model.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                selected_model = Some(model.clone());
                AnthropicProvider::new(api_key, endpoint, model)
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
            }
            "ollama" => {
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("http://localhost:11434")
                    .to_string();
                let model = match credentials.require_model(provider_name) {
                    Ok(model) => model.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                selected_model = Some(model.clone());
                OllamaProvider::new(endpoint, model).map(|p| Box::new(p) as Box<dyn AiProvider>)
            }
            "null" => Ok(Box::new(openagent_terminal_ai::NullProvider) as Box<dyn AiProvider>),
            _ => Err(format!("Unknown provider: {}", provider_name)),
        };

        match provider_result {
            Ok(provider) => {
                info!("Successfully created secure AI provider: {}", provider_name);
                let mut rt = Self::new(provider);
                rt.ui.current_provider = provider_name.to_string();
                if let Some(m) = selected_model.take() {
                    rt.ui.current_model = m;
                } else {
                    // Fallback to config default if available
                    rt.ui.current_model = config
                        .default_model
                        .clone()
                        .unwrap_or_else(|| String::new());
                }
                rt
            }
            Err(e) => {
                error!(
                    "Failed to create secure provider '{}': {}",
                    provider_name, e
                );
                let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                rt.ui.error_message = Some(format!(
                    "Secure AI provider initialization failed: {}. Please verify your \
                     configuration and credentials.",
                    e
                ));
                rt
            }
        }
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
        let _span = tracing::info_span!(
            "ai.start_propose_stream",
            provider = %self.provider.name(),
            scratch_len = self.ui.scratch.len()
        )
        .entered();
        info!(
            "ai_runtime_stream_start provider={} scratch_len={}",
            self.provider.name(),
            self.ui.scratch.len()
        );
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
        self.ui.streaming_last_redraw = None;
        self.cancel_flag.store(false, Ordering::Relaxed);

        let cancel = self.cancel_flag.clone();
        let provider = self.provider.clone();
        let req_raw = AiRequest {
            scratch_text: self.ui.scratch.clone(),
            working_directory,
            shell_kind,
            context: vec![("platform".to_string(), std::env::consts::OS.to_string())],
        };
        // Build rich context with config-driven providers and sanitize
        let (cm, budget_kb) = self.build_context_manager();
        let req = build_request_with_context(req_raw, &cm, budget_kb);

        // Spawn background worker
        let _ = thread::Builder::new()
            .name("ai-stream".into())
            .spawn(move || {
                // First try streaming
                let mut batch_buf = String::new();
                let mut last_flush = std::time::Instant::now();
                let batch_ms = std::env::var("OPENAGENT_AI_STREAM_REDRAW_MS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(16);

                let mut flush = || {
                    if !batch_buf.is_empty() {
                        let payload = std::mem::take(&mut batch_buf);
                        let _ = event_proxy
                            .send_event(Event::new(EventType::AiStreamChunk(payload), window_id));
                        last_flush = std::time::Instant::now();
                    }
                };

                let mut on_chunk = |chunk: &str| {
                    // Micro-batch: accumulate small chunks and flush at most every batch_ms
                    batch_buf.push_str(chunk);
                    let now = std::time::Instant::now();
                    if now.saturating_duration_since(last_flush).as_millis() as u64 >= batch_ms {
                        flush();
                    }
                };
                match provider.propose_stream(req.clone(), &mut on_chunk, &cancel) {
                    Ok(true) => {
                        // Flush any pending chunk before finishing
                        flush();
                        info!("ai_runtime_stream_finished provider={}", provider.name());
                        let _ = event_proxy
                            .send_event(Event::new(EventType::AiStreamFinished, window_id));
                    }
                    Ok(false) => {
                        info!("ai_runtime_fallback_blocking provider={}", provider.name());
                        let result = provider.propose(req);
                        match result {
                            Ok(proposals) => {
                                info!("ai_runtime_blocking_complete proposals={}", proposals.len());
                                let _ = event_proxy.send_event(Event::new(
                                    EventType::AiProposals(proposals),
                                    window_id,
                                ));
                            }
                            Err(e) => {
                                error!("ai_runtime_blocking_error error={}", e);
                                let _ = event_proxy
                                    .send_event(Event::new(EventType::AiStreamError(e), window_id));
                            }
                        }
                    }
                    Err(e) => {
                        if e.eq_ignore_ascii_case("cancelled") || e.eq_ignore_ascii_case("canceled")
                        {
                            info!("ai_runtime_stream_cancelled provider={}", provider.name());
                            // Treat cancellation as a graceful finish, do not surface an error
                            let _ = event_proxy
                                .send_event(Event::new(EventType::AiStreamFinished, window_id));
                        } else {
                            error!("ai_runtime_stream_error error={}", e);
                            let _ = event_proxy
                                .send_event(Event::new(EventType::AiStreamError(e), window_id));
                        }
                    }
                }
            });
    }

    /// Cancel any in-flight streaming.
    pub fn cancel(&mut self) {
        info!(
            "ai_runtime_cancel_requested provider={}",
            self.provider.name()
        );
        self.cancel_flag.store(true, Ordering::SeqCst);
        self.ui.streaming_active = false;
        self.ui.is_loading = false;
    }

    pub fn propose(&mut self, working_directory: Option<String>, shell_kind: Option<String>) {
        let _span = tracing::info_span!(
            "ai.propose_blocking",
            provider = %self.provider.name(),
            scratch_len = self.ui.scratch.len()
        )
        .entered();
        if self.ui.scratch.trim().is_empty() {
            self.ui.error_message = Some("Query cannot be empty".to_string());
            return;
        }

        // Add to history
        self.ui.history.push_front(self.ui.scratch.clone());
        // Persist updated history
        self.save_history();
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

        let req_raw = AiRequest {
            scratch_text: self.ui.scratch.clone(),
            working_directory,
            shell_kind,
            context: vec![("platform".to_string(), std::env::consts::OS.to_string())],
        };
        let (cm, budget_kb) = self.build_context_manager();
        let req = build_request_with_context(req_raw, &cm, budget_kb);

        let t0 = std::time::Instant::now();
        match self.provider.propose(req) {
            Ok(proposals) => {
                let dt = t0.elapsed();
                info!("Received {} proposals", proposals.len());
                tracing::info!(elapsed_ms = dt.as_millis() as u64, "ai.propose_complete");
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            }
            Err(e) => {
                let dt = t0.elapsed();
                error!("AI query failed: {}", e);
                tracing::info!(elapsed_ms = dt.as_millis() as u64, "ai.propose_error");
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

    /// Forward delete at cursor (DEL key)
    pub fn delete_forward(&mut self) {
        if self.ui.cursor_position < self.ui.scratch.len() {
            self.ui.scratch.remove(self.ui.cursor_position);
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
            }
            Some(i) => {
                let new_index = i - 1;
                if let Some(entry) = self.ui.history.get(new_index) {
                    self.ui.scratch = entry.clone();
                    self.ui.cursor_position = self.ui.scratch.len();
                    self.ui.history_index = Some(new_index);
                }
            }
            None => {}
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
        self.ui
            .proposals
            .get(self.ui.selected_proposal)
            .map(|p| p.proposed_commands.join("\n"))
    }

    /// Regenerate the last proposal
    pub fn regenerate(&mut self, event_proxy: EventLoopProxy<Event>, window_id: WindowId) {
        // Clear current proposals and streaming state
        self.ui.proposals.clear();
        self.ui.selected_proposal = 0;
        self.ui.error_message = None;
        self.ui.streaming_text.clear();

        // Restart the proposal stream with the same scratch text
        // Note: Context should be provided by the caller in real usage
        // This is a standalone method that doesn't have access to context provider
        let working_directory = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().to_string());
        let shell_kind = std::env::var("SHELL").ok().map(|s| {
            openagent_terminal_core::tty::pty_manager::ShellKind::from_shell_name(&s)
                .to_str()
                .to_string()
        });
        self.start_propose_stream(working_directory, shell_kind, event_proxy, window_id);
    }

    /// Insert selected proposal text to the prompt
    pub fn insert_to_prompt(&mut self) -> Option<String> {
        if self.ui.streaming_active && !self.ui.streaming_text.is_empty() {
            // Use streaming text if available
            Some(self.ui.streaming_text.clone())
        } else {
            // Use selected proposal
            self.ui.proposals.get(self.ui.selected_proposal).map(|p| {
                let mut result = String::new();
                if let Some(desc) = &p.description {
                    result.push_str(desc);
                    result.push_str("\n\n");
                }
                result.push_str(&p.proposed_commands.join("\n"));
                result
            })
        }
    }

    /// Apply command with safe-run (dry-run by default)
    pub fn apply_command(&mut self, dry_run: bool) -> Option<(String, bool)> {
        self.ui
            .proposals
            .get(self.ui.selected_proposal)
            .and_then(|p| p.proposed_commands.first())
            .map(|cmd| {
                if dry_run {
                    // Analyze risk and annotate a dry-run output
                    let risk = self.security_lens.analyze_command(cmd);
                    let mut annotated = String::new();
                    annotated.push_str(&format!(
                        "# Security Lens: {:?} - {}\n",
                        risk.level, risk.explanation
                    ));
                    // Note: factors field is only available in full security-lens feature
                    #[cfg(feature = "security-lens")]
                    if !risk.factors.is_empty() {
                        annotated.push_str("# Risk factors:\n");
                        for f in &risk.factors {
                            annotated
                                .push_str(&format!("#  - {} ({})\n", f.description, f.category));
                        }
                    }
                    if !risk.mitigations.is_empty() {
                        annotated.push_str("# Suggested mitigations:\n");
                        for m in &risk.mitigations {
                            annotated.push_str(&format!("#  - {}\n", m));
                        }
                    }
                    annotated.push_str(&format!("echo 'DRY RUN: {}'\n# To execute: {}", cmd, cmd));
                    (annotated, true)
                } else {
                    (cmd.clone(), false)
                }
            })
    }

    /// Copy output in the specified format
    pub fn copy_output(&self, format: crate::event::AiCopyFormat) -> Option<String> {
        use crate::event::AiCopyFormat;

        let content = if self.ui.streaming_active && !self.ui.streaming_text.is_empty() {
            self.ui.streaming_text.clone()
        } else if let Some(proposal) = self.ui.proposals.get(self.ui.selected_proposal) {
            let mut result = String::new();
            if let Some(desc) = &proposal.description {
                result.push_str(desc);
                result.push_str("\n\n");
            }
            result.push_str(&proposal.proposed_commands.join("\n"));
            result
        } else {
            return None;
        };

        Some(match format {
            AiCopyFormat::Text => content,
            AiCopyFormat::Code => {
                // Format as code block
                format!("```bash\n{}\n```", content)
            }
            AiCopyFormat::Markdown => {
                // Format as markdown with title and description
                let mut md = String::new();
                if let Some(proposal) = self.ui.proposals.get(self.ui.selected_proposal) {
                    md.push_str(&format!("## {}\n\n", proposal.title));
                    if let Some(desc) = &proposal.description {
                        md.push_str(desc);
                        md.push_str("\n\n");
                    }
                    if !proposal.proposed_commands.is_empty() {
                        md.push_str("### Commands\n\n");
                        md.push_str("```bash\n");
                        md.push_str(&proposal.proposed_commands.join("\n"));
                        md.push_str("\n```\n");
                    }
                } else {
                    // Fallback for streaming text
                    md.push_str("```\n");
                    md.push_str(&content);
                    md.push_str("\n```\n");
                }
                md
            }
        })
    }

    /// Context-aware propose method
    pub fn propose_with_context(
        &mut self,
        context: Option<openagent_terminal_core::tty::pty_manager::PtyAiContext>,
    ) {
        let _span = tracing::info_span!(
            "ai.propose_with_context",
            provider = %self.provider.name(),
            scratch_len = self.ui.scratch.len()
        )
        .entered();
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

        debug!("Submitting AI query with context: {}", self.ui.scratch);

        let (working_directory, shell_kind) = if let Some(ctx) = context {
            let (wd, sk) = ctx.to_strings();
            (Some(wd), Some(sk))
        } else {
            (None, None)
        };

        let req_raw = AiRequest {
            scratch_text: self.ui.scratch.clone(),
            working_directory,
            shell_kind,
            context: vec![("platform".to_string(), std::env::consts::OS.to_string())],
        };
        let req = sanitize_request(&req_raw, AiPrivacyOptions::from_env());

        let t0 = std::time::Instant::now();
        match self.provider.propose(req) {
            Ok(proposals) => {
                let dt = t0.elapsed();
                info!("Received {} proposals with context", proposals.len());
                tracing::info!(
                    elapsed_ms = dt.as_millis() as u64,
                    "ai.propose_with_context_complete"
                );
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            }
            Err(e) => {
                let dt = t0.elapsed();
                error!("AI query with context failed: {}", e);
                tracing::info!(
                    elapsed_ms = dt.as_millis() as u64,
                    "ai.propose_with_context_error"
                );
                self.ui.error_message = Some(format!("Query failed: {}", e));
                self.ui.is_loading = false;
            }
        }
    }

    /// Context-aware streaming propose method
    pub fn start_propose_stream_with_context(
        &mut self,
        context: Option<openagent_terminal_core::tty::pty_manager::PtyAiContext>,
        event_proxy: EventLoopProxy<Event>,
        window_id: WindowId,
    ) {
        let (working_directory, shell_kind) = if let Some(ctx) = context {
            let (wd, sk) = ctx.to_strings();
            (Some(wd), Some(sk))
        } else {
            (None, None)
        };

        self.start_propose_stream(working_directory, shell_kind, event_proxy, window_id);
    }

    /// Check if we can perform actions (have content to act on)
    pub fn has_content(&self) -> bool {
        (!self.ui.streaming_text.is_empty() && self.ui.streaming_active)
            || !self.ui.proposals.is_empty()
    }

    /// Generate an explanation for a given command or output snippet.
    /// The explanation is produced by the current AI provider with context flags.
    pub fn propose_explain(
        &mut self,
        target_text: String,
        working_directory: Option<String>,
        shell_kind: Option<String>,
    ) {
        let _span = tracing::info_span!(
            "ai.propose_explain",
            provider = %self.provider.name(),
            target_len = target_text.len()
        )
        .entered();
        if target_text.trim().is_empty() {
            self.ui.error_message = Some("Nothing to explain".to_string());
            return;
        }

        self.ui.proposals.clear();
        self.ui.selected_proposal = 0;
        self.ui.error_message = None;
        self.ui.is_loading = true;

        let mut context = vec![
            ("mode".to_string(), "explain".to_string()),
            ("explain_target".to_string(), target_text.clone()),
            ("platform".to_string(), std::env::consts::OS.to_string()),
        ];
        if let Some(ref sh) = shell_kind {
            context.push(("shell".into(), sh.clone()));
        }
        if let Some(ref dir) = working_directory {
            context.push(("cwd".into(), dir.clone()));
        }

        let req_raw = AiRequest {
            // Keep the scratch as the current query if present, else use the target_text
            scratch_text: if self.ui.scratch.trim().is_empty() {
                format!("Explain: {}", target_text)
            } else {
                self.ui.scratch.clone()
            },
            working_directory,
            shell_kind,
            context,
        };
        let (cm, budget_kb) = self.build_context_manager();
        let req = build_request_with_context(req_raw, &cm, budget_kb);

        let t0 = std::time::Instant::now();
        match self.provider.propose(req) {
            Ok(proposals) => {
                let dt = t0.elapsed();
                tracing::info!(
                    elapsed_ms = dt.as_millis() as u64,
                    "ai.propose_explain_complete"
                );
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            }
            Err(e) => {
                let dt = t0.elapsed();
                tracing::info!(
                    elapsed_ms = dt.as_millis() as u64,
                    "ai.propose_explain_error"
                );
                self.ui.error_message = Some(format!("Explain failed: {}", e));
                self.ui.is_loading = false;
            }
        }
    }

    /// Suggest a fix for an error snippet, optionally with the failed command.
    pub fn propose_fix(
        &mut self,
        error_text: String,
        failed_command: Option<String>,
        working_directory: Option<String>,
        shell_kind: Option<String>,
    ) {
        let _span = tracing::info_span!(
            "ai.propose_fix",
            provider = %self.provider.name(),
            error_len = error_text.len()
        )
        .entered();
        if error_text.trim().is_empty() {
            self.ui.error_message = Some("No error text provided".to_string());
            return;
        }

        self.ui.proposals.clear();
        self.ui.selected_proposal = 0;
        self.ui.error_message = None;
        self.ui.is_loading = true;

        let mut context = vec![
            ("mode".to_string(), "fix".to_string()),
            ("error".to_string(), error_text.clone()),
            ("platform".to_string(), std::env::consts::OS.to_string()),
        ];
        if let Some(ref fc) = failed_command {
            context.push(("failed_command".into(), fc.clone()));
        }
        if let Some(ref sh) = shell_kind {
            context.push(("shell".into(), sh.clone()));
        }
        if let Some(ref dir) = working_directory {
            context.push(("cwd".into(), dir.clone()));
        }

        let prompt = if let Some(fc) = &failed_command {
            format!(
                "Error encountered while running '{}':\n{}\nSuggest a fix.",
                fc, error_text
            )
        } else {
            format!("Error: {}\nSuggest a fix.", error_text)
        };

        let req_raw = AiRequest {
            scratch_text: prompt,
            working_directory,
            shell_kind,
            context,
        };
        let req = sanitize_request(&req_raw, AiPrivacyOptions::from_env());

        let t0 = std::time::Instant::now();
        match self.provider.propose(req) {
            Ok(proposals) => {
                let dt = t0.elapsed();
                tracing::info!(
                    elapsed_ms = dt.as_millis() as u64,
                    "ai.propose_fix_complete"
                );
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            }
            Err(e) => {
                let dt = t0.elapsed();
                tracing::info!(elapsed_ms = dt.as_millis() as u64, "ai.propose_fix_error");
                self.ui.error_message = Some(format!("Fix suggestion failed: {}", e));
                self.ui.is_loading = false;
            }
        }
    }

    /// Apply runtime AI context configuration
    pub fn set_context_config(&mut self, cfg: crate::config::ai::AiContextConfig) {
        self.context_cfg = cfg;
    }

    fn build_context_manager(&self) -> (ContextManager, usize) {
        let mut cm = ContextManager::new();
        if self.context_cfg.enabled {
            // Timeouts (soft)
            cm.set_timeouts(
                Some(self.context_cfg.timeouts.per_provider_ms),
                Some(self.context_cfg.timeouts.overall_ms),
            );
            // Providers in order
            for name in &self.context_cfg.providers {
                match name.as_str() {
                    "env" => cm.add_provider_with_timeout(
                        Box::new(BasicEnvProvider),
                        self.context_cfg
                            .timeouts
                            .env_ms
                            .or(Some(self.context_cfg.timeouts.per_provider_ms)),
                    ),
                    "git" => cm.add_provider_with_timeout(
                        Box::new(GitProvider::new(
                            self.context_cfg.git.include_branch,
                            self.context_cfg.git.include_status,
                        )),
                        self.context_cfg
                            .timeouts
                            .git_ms
                            .or(Some(self.context_cfg.timeouts.per_provider_ms)),
                    ),
                    "file_tree" => {
                        let strat = match self.context_cfg.file_tree.root_strategy {
                            crate::config::ai::AiRootStrategy::Git => {
                                FileTreeRootStrategy::RepoRoot
                            }
                            crate::config::ai::AiRootStrategy::Cwd => FileTreeRootStrategy::Cwd,
                        };
                        cm.add_provider_with_timeout(
                            Box::new(FileTreeProvider::new(
                                self.context_cfg.file_tree.max_entries,
                                strat,
                            )),
                            self.context_cfg
                                .timeouts
                                .file_tree_ms
                                .or(Some(self.context_cfg.timeouts.per_provider_ms)),
                        );
                    }
                    _ => {}
                }
            }
        }
        // Convert bytes -> KB rounding up
        let mut kb = (self.context_cfg.max_bytes + 1023) / 1024;
        if !self.context_cfg.enabled {
            kb = 0;
        }
        (cm, kb)
    }
}

//! AI runtime: UI state and provider wiring (optional feature)
#![cfg(feature = "ai")]

use log::{debug, error, info};
use std::collections::VecDeque;

use crate::security_lens::{SecurityLens, SecurityPolicy};
use openagent_terminal_ai::{create_provider, AiProposal, AiProvider, AiRequest};
use openagent_terminal_ai::providers::{AnthropicProvider, OllamaProvider, OpenAiProvider};

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
    /// Inline suggestion text to render as ghost text at the terminal prompt (suffix suggestion)
    pub inline_suggestion: Option<String>,
}

impl AiRuntime {
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
            "Complete the shell command. Only return the completed command.\nPartial: {}\nCompletion:",
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

        let _ = std::thread::Builder::new().name("ai-inline".into()).spawn(move || {
            // Non-streaming, single-shot proposal
            let result = provider.propose(req);
            let suggestion = match result {
                Ok(mut props) => {
                    // Take the first command from the first proposal, if any
                    if let Some(prop) = props.first_mut() {
                        if let Some(cmd) = prop.proposed_commands.first() {
                            // Compute suffix to suggest (only the part not already typed)
                            Some(compute_suffix(cmd, &prefix))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                },
                Err(_) => None,
            };

            let payload =
                crate::event::EventType::AiInlineSuggestionReady(suggestion.unwrap_or_default());
            let _ = event_proxy.send_event(Event::new(payload, window_id));
        });

        // Helper to compute the suffix not yet typed
        fn compute_suffix(candidate: &str, typed: &str) -> String {
            if candidate.starts_with(typed) {
                return candidate[typed.len()..].to_string();
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
            inline_suggestion: None,
        }
    }
}

use crate::event::{Event, EventType};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

pub struct AiRuntime {
    pub ui: AiUiState,
    pub provider: Arc<dyn AiProvider>,
    cancel_flag: Arc<AtomicBool>,
    security_lens: SecurityLens,
}

impl AiRuntime {
    pub fn new(provider: Box<dyn AiProvider>) -> Self {
        info!("AI runtime initialized with provider: {}", provider.name());
        Self {
            ui: AiUiState::default(),
            provider: Arc::from(provider),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            security_lens: SecurityLens::new(SecurityPolicy::default()),
        }
    }

    pub fn from_config(
        provider_id: Option<&str>,
        endpoint_env: Option<&str>,
        api_key_env: Option<&str>,
        model_env: Option<&str>,
    ) -> Self {
        use tracing::warn;
        
        // Check for legacy environment variable usage
        crate::config::ai_providers::check_legacy_env_vars();
        
        // DEPRECATED: This method is deprecated in favor of from_secure_config
        // Maintain backward compatibility but warn users
        warn!("AI runtime from_config is deprecated. Please use from_secure_config with provider-specific configuration.");
        
        // For backward compatibility, attempt to create provider using legacy approach
        let provider_name = provider_id.unwrap_or("null");
        let provider_result = create_provider(provider_name);
        match provider_result {
            Ok(p) => {
                info!("Successfully created AI provider: {}", provider_name);
                Self::new(p)
            },
            Err(e) => {
                error!("Failed to create provider '{}': {}", provider_name, e);
                let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider::default()));
                rt.ui.error_message = Some(format!(
                    "AI provider initialization failed: {}. Please check your AI settings (provider, endpoint, api key, model). \
                     Consider migrating to secure provider configuration - see docs/AI_ENVIRONMENT_SECURITY.md",
                    e
                ));
                rt
            },
        }
    }

    /// Create AI runtime from secure provider configuration (recommended approach)
    pub fn from_secure_config(
        provider_name: &str,
        config: &crate::config::ai::ProviderConfig,
    ) -> Self {
        use crate::config::ai_providers::ProviderCredentials;
        
        info!("Initializing AI runtime with secure provider configuration: {}", provider_name);
        
        // Extract credentials securely without polluting global environment
        let credentials = match ProviderCredentials::from_config(provider_name, config) {
            Ok(creds) => creds,
            Err(e) => {
                error!("Failed to load secure credentials for provider '{}': {}", provider_name, e);
                let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider::default()));
                rt.ui.error_message = Some(format!(
                    "Secure credential loading failed for '{}': {}. Check your environment variables and configuration.",
                    provider_name, e
                ));
                return rt;
            }
        };
        
        // Create provider with isolated credentials
        let provider_result = match provider_name {
            "openai" => {
                let api_key = match credentials.require_api_key(provider_name) {
                    Ok(key) => key.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider::default()));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                let endpoint = credentials.require_endpoint(provider_name).unwrap_or("https://api.openai.com/v1").to_string();
                let model = match credentials.require_model(provider_name) {
                    Ok(model) => model.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider::default()));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                OpenAiProvider::new(api_key, endpoint, model)
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
            },
            "anthropic" => {
                let api_key = match credentials.require_api_key(provider_name) {
                    Ok(key) => key.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider::default()));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                let endpoint = credentials.require_endpoint(provider_name).unwrap_or("https://api.anthropic.com").to_string();
                let model = match credentials.require_model(provider_name) {
                    Ok(model) => model.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider::default()));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                AnthropicProvider::new(api_key, endpoint, model)
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
            },
            "ollama" => {
                let endpoint = credentials.require_endpoint(provider_name).unwrap_or("http://localhost:11434").to_string();
                let model = match credentials.require_model(provider_name) {
                    Ok(model) => model.to_string(),
                    Err(e) => {
                        let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider::default()));
                        rt.ui.error_message = Some(e);
                        return rt;
                    }
                };
                OllamaProvider::new(endpoint, model)
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
            },
            "null" => Ok(Box::new(openagent_terminal_ai::NullProvider::default()) as Box<dyn AiProvider>),
            _ => Err(format!("Unknown provider: {}", provider_name))
        };
        
        match provider_result {
            Ok(provider) => {
                info!("Successfully created secure AI provider: {}", provider_name);
                Self::new(provider)
            },
            Err(e) => {
                error!("Failed to create secure provider '{}': {}", provider_name, e);
                let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider::default()));
                rt.ui.error_message = Some(format!(
                    "Secure AI provider initialization failed: {}. Please verify your configuration and credentials.",
                    e
                ));
                rt
            },
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
                let _ = event_proxy
                    .send_event(Event::new(EventType::AiStreamChunk(chunk.to_string()), window_id));
            };
            match provider.propose_stream(req.clone(), &mut on_chunk, &cancel) {
                Ok(true) => {
                    info!("ai_runtime_stream_finished provider={}", provider.name());
                    let _ =
                        event_proxy.send_event(Event::new(EventType::AiStreamFinished, window_id));
                },
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
                        },
                        Err(e) => {
                            error!("ai_runtime_blocking_error error={}", e);
                            let _ = event_proxy
                                .send_event(Event::new(EventType::AiStreamError(e), window_id));
                        },
                    }
                },
                Err(e) => {
                    error!("ai_runtime_stream_error error={}", e);
                    let _ =
                        event_proxy.send_event(Event::new(EventType::AiStreamError(e), window_id));
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
            context: vec![("platform".to_string(), std::env::consts::OS.to_string())],
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
            },
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
        self.ui.proposals.get(self.ui.selected_proposal).map(|p| p.proposed_commands.join("\n"))
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
        let working_directory = std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string());
        let shell_kind = std::env::var("SHELL").ok().and_then(|s| {
            Some(openagent_terminal_core::tty::pty_manager::ShellKind::from_shell_name(&s).to_str().to_string())
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
            },
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
            },
        })
    }

    /// Context-aware propose method
    pub fn propose_with_context(&mut self, context: Option<openagent_terminal_core::tty::pty_manager::PtyAiContext>) {
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
            ctx.to_strings()
        } else {
            (None, None)
        };

        let req = AiRequest {
            scratch_text: self.ui.scratch.clone(),
            working_directory,
            shell_kind,
            context: vec![("platform".to_string(), std::env::consts::OS.to_string())],
        };

        match self.provider.propose(req) {
            Ok(proposals) => {
                info!("Received {} proposals with context", proposals.len());
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            },
            Err(e) => {
                error!("AI query with context failed: {}", e);
                self.ui.error_message = Some(format!("Query failed: {}", e));
                self.ui.is_loading = false;
            },
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
            ctx.to_strings()
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

        let req = AiRequest {
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

        match self.provider.propose(req) {
            Ok(proposals) => {
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            },
            Err(e) => {
                self.ui.error_message = Some(format!("Explain failed: {}", e));
                self.ui.is_loading = false;
            },
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
            format!("Error encountered while running '{}':\n{}\nSuggest a fix.", fc, error_text)
        } else {
            format!("Error: {}\nSuggest a fix.", error_text)
        };

        let req = AiRequest { scratch_text: prompt, working_directory, shell_kind, context };

        match self.provider.propose(req) {
            Ok(proposals) => {
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            },
            Err(e) => {
                self.ui.error_message = Some(format!("Fix suggestion failed: {}", e));
                self.ui.is_loading = false;
            },
        }
    }
}

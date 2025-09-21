use super::{
    AgentCapabilities, AgentError, AgentRequest, AgentResponse, AiAgent, CodeAction, CodeContext,
    PrivacyLevel,
};
use crate::agents::types::ConcurrencyState;
use crate::{AiProvider, AiRequest};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{timeout, Duration};
use tracing::debug;
use uuid::Uuid;

/// Enhanced code generation agent that goes beyond simple command suggestions
pub struct CodeGenerationAgent {
    provider: std::sync::Arc<dyn AiProvider>,
    config: CodeGenConfig,
    /// Concurrency state to prevent race conditions
    concurrency_state: ConcurrencyState,
    /// Active operations tracking
    active_operations: Arc<RwLock<HashMap<String, HashSet<Uuid>>>>,
    /// Operation timeout
    timeout_duration: Duration,
}

#[derive(Debug, Clone)]
pub struct CodeGenConfig {
    pub max_tokens: usize,
    pub temperature: f32,
    pub languages: Vec<String>,
    pub enable_context_analysis: bool,
}

impl Default for CodeGenConfig {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.3,
            languages: vec![
                "rust".to_string(),
                "python".to_string(),
                "javascript".to_string(),
                "typescript".to_string(),
                "java".to_string(),
                "go".to_string(),
                "c".to_string(),
                "cpp".to_string(),
            ],
            enable_context_analysis: true,
        }
    }
}

impl CodeGenerationAgent {
    pub fn new(provider: std::sync::Arc<dyn AiProvider>) -> Self {
        Self::with_config(provider, CodeGenConfig::default())
    }

    pub fn with_config(provider: std::sync::Arc<dyn AiProvider>, config: CodeGenConfig) -> Self {
        Self {
            provider,
            config,
            concurrency_state: ConcurrencyState::default(),
            active_operations: Arc::new(RwLock::new(HashMap::new())),
            timeout_duration: Duration::from_secs(60),
        }
    }

    /// Register a new operation to prevent race conditions
    async fn register_operation(&self, operation_type: &str) -> Result<OperationGuard, AgentError> {
        let operation_id = Uuid::new_v4();
        let key = operation_type.to_string();

        // Check for semaphore limits
        let semaphore = {
            let mut locks = self.concurrency_state.operation_locks.lock().await;
            locks.entry(key.clone()).or_insert_with(|| Arc::new(Semaphore::new(3))).clone()
        };

        // Try to acquire semaphore with timeout
        let permit = match timeout(Duration::from_secs(5), semaphore.acquire_owned()).await {
            Ok(Ok(permit)) => permit,
            Ok(Err(e)) => {
                return Err(AgentError::ProcessingError(format!(
                    "Failed to acquire semaphore: {}",
                    e
                )))
            }
            Err(_) => {
                return Err(AgentError::ProcessingError(
                    "Timeout waiting for operation lock".to_string(),
                ))
            }
        };

        // Register operation
        {
            let mut active_ops = self.active_operations.write().await;
            active_ops.entry(key.clone()).or_insert_with(HashSet::new).insert(operation_id);
        }

        // Update resource usage
        {
            let mut usage = self.concurrency_state.resource_usage.write().await;
            usage.active_threads += 1;
        }

        debug!("Registered operation: {} ({})", operation_type, operation_id);

        Ok(OperationGuard {
            id: operation_id,
            key,
            active_operations: self.active_operations.clone(),
            _permit: permit,
            resource_usage: self.concurrency_state.resource_usage.clone(),
        })
    }

    /// Generate code based on natural language description
    async fn generate_code(
        &self,
        language: Option<String>,
        context: &CodeContext,
        prompt: &str,
    ) -> Result<(String, String), AgentError> {
        let enhanced_prompt =
            self.build_code_generation_prompt(language.as_deref(), context, prompt);

        let ai_request = AiRequest {
            scratch_text: enhanced_prompt,
            working_directory: context
                .current_file
                .as_ref()
                .and_then(|f| std::path::Path::new(f).parent())
                .map(|p| p.to_string_lossy().to_string()),
            shell_kind: None,
            context: vec![
                ("request_type".to_string(), "code_generation".to_string()),
                ("language".to_string(), language.unwrap_or_else(|| "auto".to_string())),
            ],
        };

        let proposals = self.provider.propose(ai_request).map_err(AgentError::ProcessingError)?;

        if proposals.is_empty() {
            return Err(AgentError::ProcessingError("No code generated".to_string()));
        }

        // Extract code and explanation from the first proposal
        let proposal = &proposals[0];
        let code = proposal.proposed_commands.join("\n");
        let explanation =
            proposal.description.clone().unwrap_or_else(|| "Generated code".to_string());

        Ok((code, explanation))
    }

    /// Complete partial code
    async fn complete_code(
        &self,
        language: Option<String>,
        context: &CodeContext,
    ) -> Result<Vec<String>, AgentError> {
        if let Some(selection) = &context.selection {
            let prompt = format!(
                "Complete this {} code:\n\n{}",
                language.as_deref().unwrap_or(""),
                selection
            );

            let (completion, _) = self.generate_code(language, context, &prompt).await?;
            Ok(vec![completion])
        } else {
            Err(AgentError::InvalidRequest("No code selection provided for completion".to_string()))
        }
    }

    /// Refactor existing code
    async fn refactor_code(
        &self,
        language: Option<String>,
        context: &CodeContext,
        prompt: &str,
    ) -> Result<(String, String), AgentError> {
        if let Some(selection) = &context.selection {
            let refactor_prompt = format!(
                "Refactor this {} code according to the instruction: {}\n\nCode to refactor:\n{}",
                language.as_deref().unwrap_or(""),
                prompt,
                selection
            );

            self.generate_code(language, context, &refactor_prompt).await
        } else {
            Err(AgentError::InvalidRequest(
                "No code selection provided for refactoring".to_string(),
            ))
        }
    }

    /// Build enhanced prompt for code generation
    fn build_code_generation_prompt(
        &self,
        language: Option<&str>,
        context: &CodeContext,
        user_prompt: &str,
    ) -> String {
        let mut prompt = String::new();

        // System prompt: production-quality, no placeholders/templates
        prompt.push_str(
            "You are an expert software engineer generating production-ready code. ",
        );
        prompt.push_str(
            "Do NOT use placeholders or templates. Provide concrete, working code with real names, ",
        );
        prompt.push_str(
            "no 'TODO', no '...'. Prefer small, cohesive units and include minimal documentation where it helps. ",
        );

        // Language-specific instructions
        if let Some(lang) = language {
            let lang_lc = lang.to_lowercase();
            prompt.push_str(&format!("Generate {} code. ", lang_lc));

            match lang_lc.as_str() {
                // Rust guidance
                "rust" => {
                    prompt.push_str(
                        "Follow Rust idioms: use ownership/borrowing correctly; prefer iterators over loops; ",
                    );
                    prompt.push_str(
                        "handle errors with Result and the ? operator; avoid unwrap/expect in library code; ",
                    );
                    prompt.push_str(
                        "design APIs to be clippy-friendly (consider lint fixes); document safety and error cases; ",
                    );
                    prompt.push_str(
                        "avoid unnecessary clones; prefer &str/&[T] over owned values when appropriate. ",
                    );
                }
                // Python guidance
                "python" => {
                    prompt.push_str(
                        "Follow PEP 8; include type hints (PEP 484); write clear docstrings (Google or NumPy style); ",
                    );
                    prompt.push_str(
                        "avoid mutable default args; use pathlib for paths; context managers for I/O; ",
                    );
                    prompt.push_str(
                        "raise specific exceptions; prefer logging over prints in libraries. ",
                    );
                }
                // JavaScript / TypeScript guidance
                "javascript" | "typescript" => {
                    prompt.push_str(
                        "Use modern ES2019+ syntax; prefer const/let; arrow functions where appropriate; ",
                    );
                    prompt.push_str("use strict equality; avoid implicit globals; ");
                    if lang_lc == "typescript" {
                        prompt.push_str(
                            "use strict typing (no any), explicit return types, discriminated unions where helpful. ",
                        );
                    } else {
                        prompt.push_str(
                            "include JSDoc where helpful and keep modules cohesive. ",
                        );
                    }
                }
                // Go guidance
                "go" => {
                    prompt.push_str(
                        "Keep functions small; return (T, error) and handle errors explicitly; ",
                    );
                    prompt.push_str("respect context.Context for cancelation; write idiomatic names. ");
                }
                // Java guidance
                "java" => {
                    prompt.push_str(
                        "Use clear interfaces; prefer immutability; use try-with-resources; ",
                    );
                    prompt.push_str("validate inputs and document thrown exceptions. ");
                }
                // C/C++ guidance (lightweight)
                "c" | "cpp" => {
                    prompt.push_str(
                        "Prefer RAII (C++) and smart pointers; avoid raw new/delete; ",
                    );
                    prompt.push_str("check return values; avoid UB; document lifetime and ownership. ");
                }
                // Shell scripts (when applicable)
                "shell" | "bash" | "zsh" => {
                    prompt.push_str(
                        "Emit portable POSIX-compliant commands when possible; quote variables safely; ",
                    );
                    prompt.push_str("avoid dangerous flags; do not include commentary in the command output. ");
                }
                _ => {}
            }
        }

        // Add project context if available
        if self.config.enable_context_analysis {
            if !context.project_files.is_empty() {
                prompt.push_str("\nProject context (sample files):\n");
                for file in context.project_files.iter().take(10) {
                    prompt.push_str("- ");
                    prompt.push_str(file);
                    prompt.push('\n');
                }
            }

            if !context.dependencies.is_empty() {
                prompt.push_str("Dependencies: ");
                prompt.push_str(&context.dependencies.join(", "));
                prompt.push('\n');
            }
        }

        // Add user's request last
        prompt.push_str("\nUser request: ");
        prompt.push_str(user_prompt);
        prompt.push('\n');

        // Output format guidance
        prompt.push_str(
            "\nReturn production-ready code and a concise explanation. Keep explanations brief and practical. ",
        );

        prompt
    }
}

#[async_trait]
impl AiAgent for CodeGenerationAgent {
    fn name(&self) -> &'static str {
        "code_generation"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    async fn process(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        match request {
            AgentRequest::CodeGeneration { language, context, prompt, action } => {
                // Register a scoped operation guard to enforce concurrency limits
                let op_label = match &action {
                    CodeAction::Generate => "generate",
                    CodeAction::Complete => "complete",
                    CodeAction::Refactor => "refactor",
                    CodeAction::Explain => "explain",
                    CodeAction::Optimize => "optimize",
                    CodeAction::Convert { .. } => "convert",
                };
                let _guard =
                    self.register_operation(&format!("code_generation:{}", op_label)).await?;

                let result = match action {
                    CodeAction::Generate => {
                        let (code, explanation) =
                            self.generate_code(language.clone(), &context, &prompt).await?;
                        (code, explanation, vec!["Generated successfully".to_string()])
                    }
                    CodeAction::Complete => {
                        let completions = self.complete_code(language.clone(), &context).await?;
                        (completions.join("\n"), "Code completion".to_string(), completions)
                    }
                    CodeAction::Refactor => {
                        let (code, explanation) =
                            self.refactor_code(language.clone(), &context, &prompt).await?;
                        (code, explanation, vec!["Refactored successfully".to_string()])
                    }
                    CodeAction::Explain => {
                        if let Some(selection) = &context.selection {
                            let explain_prompt = format!("Explain this code:\n{}", selection);
                            let (_, explanation) = self
                                .generate_code(language.clone(), &context, &explain_prompt)
                                .await?;
                            ("".to_string(), explanation, vec![])
                        } else {
                            return Err(AgentError::InvalidRequest(
                                "No code provided for explanation".to_string(),
                            ));
                        }
                    }
                    CodeAction::Optimize => {
                        if let Some(selection) = &context.selection {
                            let optimize_prompt =
                                format!("Optimize this code for performance:\n{}", selection);
                            let (code, explanation) = self
                                .generate_code(language.clone(), &context, &optimize_prompt)
                                .await?;
                            (code, explanation, vec!["Optimized for performance".to_string()])
                        } else {
                            return Err(AgentError::InvalidRequest(
                                "No code provided for optimization".to_string(),
                            ));
                        }
                    }
                    CodeAction::Convert { target_language } => {
                        if let Some(selection) = &context.selection {
                            let convert_prompt = format!(
                                "Convert this {} code to {}:\n{}",
                                language.as_deref().unwrap_or(""),
                                target_language,
                                selection
                            );
                            let (code, explanation) = self
                                .generate_code(
                                    Some(target_language.clone()),
                                    &context,
                                    &convert_prompt,
                                )
                                .await?;
                            (code, explanation, vec![format!("Converted to {}", target_language)])
                        } else {
                            return Err(AgentError::InvalidRequest(
                                "No code provided for conversion".to_string(),
                            ));
                        }
                    }
                };

                Ok(AgentResponse::Code {
                    generated_code: result.0,
                    language: language.unwrap_or_else(|| "text".to_string()),
                    explanation: result.1,
                    suggestions: result.2,
                })
            }
            _ => Err(AgentError::NotSupported(
                "Only code generation requests are supported".to_string(),
            )),
        }
    }

    fn can_handle(&self, request: &AgentRequest) -> bool {
        matches!(request, AgentRequest::CodeGeneration { .. })
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            supported_languages: self.config.languages.clone(),
            supported_frameworks: vec![
                "React".to_string(),
                "Vue".to_string(),
                "Django".to_string(),
                "Flask".to_string(),
                "Express".to_string(),
                "Actix".to_string(),
                "Tokio".to_string(),
            ],
            features: vec![
                "code_generation".to_string(),
                "code_completion".to_string(),
                "code_refactoring".to_string(),
                "code_explanation".to_string(),
                "code_optimization".to_string(),
                "language_conversion".to_string(),
            ],
            requires_internet: false, // Depends on underlying provider
            privacy_level: PrivacyLevel::Local, // Depends on underlying provider
        }
    }
}

/// RAII guard for tracking active operations
struct OperationGuard {
    id: Uuid,
    key: String,
    active_operations: Arc<RwLock<HashMap<String, HashSet<Uuid>>>>,
    _permit: tokio::sync::OwnedSemaphorePermit,
    resource_usage: Arc<RwLock<crate::agents::types::ResourceUsage>>,
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        let active_ops = self.active_operations.clone();
        let key = self.key.clone();
        let id = self.id;
        let resource_usage = self.resource_usage.clone();

        // Spawn a task to release the operation when dropped
        // This is needed because we can't use .await in drop()
        tokio::spawn(async move {
            // Unregister operation
            {
                let mut ops = active_ops.write().await;
                if let Some(set) = ops.get_mut(&key) {
                    set.remove(&id);
                    if set.is_empty() {
                        ops.remove(&key);
                    }
                }
            }

            // Update resource usage
            {
                let mut usage = resource_usage.write().await;
                usage.active_threads = usage.active_threads.saturating_sub(1);
            }

            debug!("Released operation lock: {} ({})", key, id);
        });
    }
}

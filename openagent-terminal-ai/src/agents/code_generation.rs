use super::{
    AiAgent, AgentRequest, AgentResponse, AgentError, AgentCapabilities,
    CodeAction, CodeContext, PrivacyLevel
};
use async_trait::async_trait;
use crate::{AiProvider, AiRequest};

/// Enhanced code generation agent that goes beyond simple command suggestions
pub struct CodeGenerationAgent {
    provider: Box<dyn AiProvider>,
    config: CodeGenConfig,
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
    pub fn new(provider: Box<dyn AiProvider>) -> Self {
        Self::with_config(provider, CodeGenConfig::default())
    }
    
    pub fn with_config(provider: Box<dyn AiProvider>, config: CodeGenConfig) -> Self {
        Self { provider, config }
    }
    
    /// Generate code based on natural language description
    async fn generate_code(
        &self,
        language: Option<String>,
        context: &CodeContext,
        prompt: &str,
    ) -> Result<(String, String), AgentError> {
        let enhanced_prompt = self.build_code_generation_prompt(language.as_deref(), context, prompt);
        
        let ai_request = AiRequest {
            scratch_text: enhanced_prompt,
            working_directory: context.current_file.as_ref()
                .and_then(|f| std::path::Path::new(f).parent())
                .map(|p| p.to_string_lossy().to_string()),
            shell_kind: None,
            context: vec![
                ("request_type".to_string(), "code_generation".to_string()),
                ("language".to_string(), language.unwrap_or("auto".to_string())),
            ],
        };
        
        let proposals = self.provider.propose(ai_request)
            .map_err(|e| AgentError::ProcessingError(e))?;
        
        if proposals.is_empty() {
            return Err(AgentError::ProcessingError("No code generated".to_string()));
        }
        
        // Extract code and explanation from the first proposal
        let proposal = &proposals[0];
        let code = proposal.proposed_commands.join("\n");
        let explanation = proposal.description.clone()
            .unwrap_or_else(|| "Generated code".to_string());
        
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
            Err(AgentError::InvalidRequest("No code selection provided for refactoring".to_string()))
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
        
        // System prompt for code generation
        prompt.push_str("You are an expert software developer AI assistant specialized in code generation. ");
        prompt.push_str("Generate clean, efficient, and well-documented code following best practices. ");
        
        // Language-specific instructions
        if let Some(lang) = language {
            prompt.push_str(&format!("Generate {} code. ", lang));
            
            // Add language-specific best practices
            match lang.to_lowercase().as_str() {
                "rust" => {
                    prompt.push_str("Follow Rust idioms: use ownership properly, handle errors with Result, ");
                    prompt.push_str("prefer iterators, and include appropriate lifetimes. ");
                }
                "python" => {
                    prompt.push_str("Follow PEP 8 style guidelines, use type hints, ");
                    prompt.push_str("and include proper docstrings. ");
                }
                "javascript" | "typescript" => {
                    prompt.push_str("Use modern ES6+ syntax, prefer const/let over var, ");
                    prompt.push_str("and include proper TypeScript types if applicable. ");
                }
                _ => {}
            }
        }
        
        // Add project context if available
        if self.config.enable_context_analysis {
            if !context.project_files.is_empty() {
                prompt.push_str("\nProject context:\n");
                for file in context.project_files.iter().take(10) {
                    prompt.push_str(&format!("- {}\n", file));
                }
            }
            
            if !context.dependencies.is_empty() {
                prompt.push_str("Dependencies: ");
                prompt.push_str(&context.dependencies.join(", "));
                prompt.push_str("\n");
            }
        }
        
        // Add user's request
        prompt.push_str(&format!("\nUser request: {}\n", user_prompt));
        
        // Output format instructions
        prompt.push_str("\nProvide the code with brief explanations. ");
        prompt.push_str("Focus on functionality and clarity.");
        
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
                let result = match action {
                    CodeAction::Generate => {
                        let (code, explanation) = self.generate_code(language.clone(), &context, &prompt).await?;
                        (code, explanation, vec!["Generated successfully".to_string()])
                    }
                    CodeAction::Complete => {
                        let completions = self.complete_code(language.clone(), &context).await?;
                        (completions.join("\n"), "Code completion".to_string(), completions)
                    }
                    CodeAction::Refactor => {
                        let (code, explanation) = self.refactor_code(language.clone(), &context, &prompt).await?;
                        (code, explanation, vec!["Refactored successfully".to_string()])
                    }
                    CodeAction::Explain => {
                        if let Some(selection) = &context.selection {
                            let explain_prompt = format!("Explain this code:\n{}", selection);
                            let (_, explanation) = self.generate_code(language.clone(), &context, &explain_prompt).await?;
                            ("".to_string(), explanation, vec![])
                        } else {
                            return Err(AgentError::InvalidRequest("No code provided for explanation".to_string()));
                        }
                    }
                    CodeAction::Optimize => {
                        if let Some(selection) = &context.selection {
                            let optimize_prompt = format!("Optimize this code for performance:\n{}", selection);
                            let (code, explanation) = self.generate_code(language.clone(), &context, &optimize_prompt).await?;
                            (code, explanation, vec!["Optimized for performance".to_string()])
                        } else {
                            return Err(AgentError::InvalidRequest("No code provided for optimization".to_string()));
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
                            let (code, explanation) = self.generate_code(Some(target_language.clone()), &context, &convert_prompt).await?;
                            (code, explanation, vec![format!("Converted to {}", target_language)])
                        } else {
                            return Err(AgentError::InvalidRequest("No code provided for conversion".to_string()));
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
            _ => Err(AgentError::NotSupported("Only code generation requests are supported".to_string())),
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
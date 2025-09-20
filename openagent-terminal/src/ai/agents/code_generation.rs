// Code Generation Agent
// Specialized AI agent for generating high-quality code from natural language descriptions

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::*;
use openagent_terminal_ai::AiProvider;

/// Specialized agent for code generation tasks
pub struct CodeGenerationAgent {
    id: String,
    name: String,
    ai_provider: Option<Box<dyn AiProvider>>,
    config: CodeGenerationConfig,
    is_initialized: bool,
    last_activity: chrono::DateTime<chrono::Utc>,
}

/// Configuration for code generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGenerationConfig {
    pub preferred_languages: Vec<String>,
    pub code_style: CodeStyle,
    pub include_tests: bool,
    pub include_documentation: bool,
    pub max_tokens: usize,
    pub temperature: f32,
}

/// Code generation style preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeStyle {
    Functional,
    ObjectOriented,
    Procedural,
    Hybrid,
}

/// Code generation request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGenerationRequest {
    pub requirements: String,
    pub language: Option<String>,
    pub context_files: Vec<String>,
    pub existing_code: Option<String>,
    pub style_preferences: Option<CodeStyle>,
    pub include_tests: bool,
    pub include_docs: bool,
}

/// Code generation response payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGenerationResponse {
    pub generated_code: String,
    pub language: String,
    pub explanation: String,
    pub test_code: Option<String>,
    pub documentation: Option<String>,
    pub dependencies: Vec<String>,
    pub confidence_score: f32,
}

impl Default for CodeGenerationConfig {
    fn default() -> Self {
        Self {
            preferred_languages: vec![
                "rust".to_string(),
                "typescript".to_string(),
                "python".to_string(),
                "go".to_string(),
            ],
            code_style: CodeStyle::Hybrid,
            include_tests: true,
            include_documentation: true,
            max_tokens: 4000,
            temperature: 0.3, // Lower temperature for more deterministic code
        }
    }
}

impl CodeGenerationAgent {
    pub fn new() -> Self {
        Self {
            id: "code-generation".to_string(),
            name: "Code Generation Agent".to_string(),
            ai_provider: None,
            config: CodeGenerationConfig::default(),
            is_initialized: false,
            last_activity: chrono::Utc::now(),
        }
    }

    pub fn with_ai_provider(mut self, ai_provider: Box<dyn AiProvider>) -> Self {
        self.ai_provider = Some(ai_provider);
        self
    }

    pub fn with_config(mut self, config: CodeGenerationConfig) -> Self {
        self.config = config;
        self
    }

    /// Create a system prompt for code generation
    fn create_system_prompt(
        &self,
        request: &CodeGenerationRequest,
        context: &AgentContext,
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str("You are an expert software engineer specialized in generating high-quality, production-ready code. ");
        prompt.push_str("Your code should be:\n");
        prompt.push_str("- Well-structured and readable\n");
        prompt.push_str("- Following best practices and conventions\n");
        prompt.push_str("- Properly documented with comments\n");
        prompt.push_str("- Secure and efficient\n");
        prompt.push_str("- Type-safe where applicable\n\n");

        // Add language preference
        if let Some(lang) = &request.language {
            prompt.push_str(&format!("Generate code in {}.\n", lang));
        } else {
            prompt.push_str(&format!(
                "Prefer these languages: {}.\n",
                self.config.preferred_languages.join(", ")
            ));
        }

        // Add style preferences
        match &request.style_preferences.as_ref().unwrap_or(&self.config.code_style) {
            CodeStyle::Functional => {
                prompt.push_str("Use functional programming patterns where appropriate.\n")
            }
            CodeStyle::ObjectOriented => {
                prompt.push_str("Use object-oriented design principles.\n")
            }
            CodeStyle::Procedural => prompt.push_str("Use procedural programming approach.\n"),
            CodeStyle::Hybrid => {
                prompt.push_str("Use the most appropriate programming paradigm for the task.\n")
            }
        }

        // Add context information
        if let Some(project_root) = &context.project_root {
            prompt.push_str(&format!("Project context: {}\n", project_root));
        }

        if !context.open_files.is_empty() {
            prompt.push_str(&format!(
                "Current files in workspace: {}\n",
                context.open_files.join(", ")
            ));
        }

        // Add test and documentation requirements
        if request.include_tests {
            prompt.push_str("Include comprehensive unit tests.\n");
        }

        if request.include_docs {
            prompt.push_str("Include detailed documentation and examples.\n");
        }

        prompt.push_str("\nReturn your response in this JSON format:\n");
        prompt.push_str("{\n");
        prompt.push_str("  \"generated_code\": \"<the actual code>\",\n");
        prompt.push_str("  \"language\": \"<programming language used>\",\n");
        prompt.push_str("  \"explanation\": \"<detailed explanation of the solution>\",\n");
        if request.include_tests {
            prompt.push_str("  \"test_code\": \"<unit tests for the code>\",\n");
        }
        if request.include_docs {
            prompt.push_str("  \"documentation\": \"<documentation and usage examples>\",\n");
        }
        prompt.push_str("  \"dependencies\": [\"<list of required dependencies>\"],\n");
        prompt.push_str("  \"confidence_score\": <0.0 to 1.0 confidence in solution>\n");
        prompt.push_str("}\n");

        prompt
    }

    /// Process a code generation request
    async fn generate_code(
        &self,
        request: CodeGenerationRequest,
        context: &AgentContext,
    ) -> Result<CodeGenerationResponse> {
        let system_prompt = self.create_system_prompt(&request, context);

        let mut user_prompt = format!("Requirements: {}\n", request.requirements);

        // Add existing code context if provided
        if let Some(existing) = &request.existing_code {
            user_prompt.push_str(&format!(
                "\nExisting code to modify or extend:\n```\n{}\n```\n",
                existing
            ));
        }

        // Add context from files if provided
        if !request.context_files.is_empty() {
            user_prompt.push_str("\nRelevant context from project files:\n");
            for file in &request.context_files {
                user_prompt.push_str(&format!("File: {}\n", file));
                // In a real implementation, we'd read the file contents here
            }
        }

        let ai_request = openagent_terminal_ai::AiRequest {
            scratch_text: format!("{}\n\n{}", system_prompt, user_prompt),
            working_directory: Some(context.current_directory.clone()),
            shell_kind: Some("bash".to_string()), // TODO: Get from context
            context: vec![
                ("mode".to_string(), "code_generation".to_string()),
                ("language".to_string(), request.language.clone().unwrap_or("rust".to_string())),
            ],
        };

        let proposals = if let Some(provider) = &self.ai_provider {
            provider.propose(ai_request).map_err(|e| anyhow!("AI provider error: {}", e))?
        } else {
            // Return a mock response when no AI provider is available
            vec![openagent_terminal_ai::AiProposal {
                title: "Mock Code Generation".to_string(),
                description: Some("Generated mock code response".to_string()),
                proposed_commands: vec![serde_json::json!({
                    "generated_code": "fn example() { println!(\"Hello, World!\"); }",
                    "language": "rust",
                    "explanation": "A simple example function that prints Hello World",
                    "dependencies": [],
                    "confidence_score": 0.8
                })
                .to_string()],
            }]
        };
        let response = proposals
            .first()
            .ok_or_else(|| anyhow!("No response from AI provider"))?
            .proposed_commands
            .first()
            .unwrap_or(&"No code generated".to_string())
            .clone();

        // Parse the JSON response
        let parsed: CodeGenerationResponse = serde_json::from_str(&response)
            .map_err(|e| anyhow!("Failed to parse code generation response: {}", e))?;

        Ok(parsed)
    }

    /// Create artifacts from the code generation response
    fn create_artifacts(&self, response: &CodeGenerationResponse) -> Vec<AgentArtifact> {
        let mut artifacts = Vec::new();

        // Main code artifact
        artifacts.push(AgentArtifact {
            id: Uuid::new_v4(),
            artifact_type: ArtifactType::Code,
            content: response.generated_code.clone(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("language".to_string(), response.language.clone());
                meta.insert("confidence".to_string(), response.confidence_score.to_string());
                meta
            },
        });

        // Test code artifact if available
        if let Some(test_code) = &response.test_code {
            artifacts.push(AgentArtifact {
                id: Uuid::new_v4(),
                artifact_type: ArtifactType::Code,
                content: test_code.clone(),
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("type".to_string(), "tests".to_string());
                    meta.insert("language".to_string(), response.language.clone());
                    meta
                },
            });
        }

        // Documentation artifact if available
        if let Some(docs) = &response.documentation {
            artifacts.push(AgentArtifact {
                id: Uuid::new_v4(),
                artifact_type: ArtifactType::Documentation,
                content: docs.clone(),
                metadata: HashMap::new(),
            });
        }

        artifacts
    }

    /// Create suggested actions from the response
    fn create_suggested_actions(&self, response: &CodeGenerationResponse) -> Vec<SuggestedAction> {
        let mut actions = Vec::new();

        // Suggest creating the main code file
        actions.push(SuggestedAction {
            action_type: ActionType::CreateFile,
            description: format!("Create {} file with generated code", response.language),
            command: None,
            priority: ActionPriority::High,
            safe_to_auto_execute: false, // Never auto-execute file creation
        });

        // Suggest installing dependencies if any
        if !response.dependencies.is_empty() {
            for dep in &response.dependencies {
                actions.push(SuggestedAction {
                    action_type: ActionType::InstallDependency,
                    description: format!("Install dependency: {}", dep),
                    command: Some(self.get_install_command(&response.language, dep)),
                    priority: ActionPriority::Medium,
                    safe_to_auto_execute: false,
                });
            }
        }

        // Suggest running tests if test code was generated
        if response.test_code.is_some() {
            actions.push(SuggestedAction {
                action_type: ActionType::RunTest,
                description: "Run generated tests".to_string(),
                command: Some(self.get_test_command(&response.language)),
                priority: ActionPriority::Medium,
                safe_to_auto_execute: false,
            });
        }

        actions
    }

    /// Get the appropriate install command for a dependency
    fn get_install_command(&self, language: &str, dependency: &str) -> String {
        match language.to_lowercase().as_str() {
            "rust" => format!("cargo add {}", dependency),
            "typescript" | "javascript" => format!("npm install {}", dependency),
            "python" => format!("pip install {}", dependency),
            "go" => format!("go get {}", dependency),
            _ => format!("# Install {} for {}", dependency, language),
        }
    }

    /// Get the appropriate test command for a language
    fn get_test_command(&self, language: &str) -> String {
        match language.to_lowercase().as_str() {
            "rust" => "cargo test".to_string(),
            "typescript" | "javascript" => "npm test".to_string(),
            "python" => "pytest".to_string(),
            "go" => "go test".to_string(),
            _ => format!("# Run tests for {}", language),
        }
    }
}

#[async_trait]
impl Agent for CodeGenerationAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Generates high-quality code from natural language requirements, including tests and documentation"
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::CodeGeneration, AgentCapability::CodeAnalysis]
    }

    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        if !self.is_initialized {
            return Err(anyhow!("Agent not initialized"));
        }

        // Note: Cannot mutate last_activity in async handle_request due to &self

        match request.request_type {
            AgentRequestType::GenerateCode => {
                let gen_request: CodeGenerationRequest = serde_json::from_value(request.payload)
                    .map_err(|e| anyhow!("Invalid code generation request: {}", e))?;

                let response = self.generate_code(gen_request, &request.context).await?;
                let artifacts = self.create_artifacts(&response);
                let actions = self.create_suggested_actions(&response);

                Ok(AgentResponse {
                    request_id: request.id,
                    agent_id: self.id.clone(),
                    success: true,
                    payload: serde_json::to_value(&response)?,
                    artifacts,
                    next_actions: actions,
                    metadata: HashMap::new(),
                })
            }
            _ => Err(anyhow!("Unsupported request type: {:?}", request.request_type)),
        }
    }

    fn can_handle(&self, request_type: &AgentRequestType) -> bool {
        matches!(request_type, AgentRequestType::GenerateCode)
    }

    async fn status(&self) -> AgentStatus {
        AgentStatus {
            is_healthy: self.is_initialized,
            is_busy: false, // TODO: Track concurrent requests
            last_activity: self.last_activity,
            current_task: None,
            error_message: None,
        }
    }

    async fn initialize(&mut self, config: AgentConfig) -> Result<()> {
        // Load any custom configuration
        if let Some(code_config) = config.custom_settings.get("code_generation") {
            if let Ok(parsed_config) =
                serde_json::from_value::<CodeGenerationConfig>(code_config.clone())
            {
                self.config = parsed_config;
            }
        }

        self.is_initialized = true;
        self.last_activity = chrono::Utc::now();

        tracing::info!("Code Generation Agent initialized");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.is_initialized = false;
        tracing::info!("Code Generation Agent shut down");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_code_generation_agent_creation() {
        let agent = CodeGenerationAgent::new();

        assert_eq!(agent.id(), "code-generation");
        assert_eq!(agent.name(), "Code Generation Agent");
        assert!(agent.capabilities().contains(&AgentCapability::CodeGeneration));
        assert!(agent.can_handle(&AgentRequestType::GenerateCode));
        assert!(!agent.can_handle(&AgentRequestType::CheckSecurity));
    }

    #[test]
    fn test_system_prompt_creation() {
        let agent = CodeGenerationAgent::new();

        let request = CodeGenerationRequest {
            requirements: "Create a function to calculate fibonacci numbers".to_string(),
            language: Some("rust".to_string()),
            context_files: vec![],
            existing_code: None,
            style_preferences: Some(CodeStyle::Functional),
            include_tests: true,
            include_docs: true,
        };

        let context = AgentContext {
            project_root: Some("/home/user/project".to_string()),
            current_directory: "/home/user/project/src".to_string(),
            current_branch: Some("feature/fibonacci".to_string()),
            open_files: vec!["main.rs".to_string()],
            recent_commands: vec![],
            environment_vars: HashMap::new(),
            user_preferences: HashMap::new(),
        };

        let prompt = agent.create_system_prompt(&request, &context);

        assert!(prompt.contains("rust"));
        assert!(prompt.contains("functional"));
        assert!(prompt.contains("unit tests"));
        assert!(prompt.contains("documentation"));
        assert!(prompt.contains("/home/user/project"));
    }
}

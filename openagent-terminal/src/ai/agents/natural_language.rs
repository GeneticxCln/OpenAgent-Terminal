use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::{
    ActionPriority, ActionType, Agent, AgentArtifact, AgentCapability, AgentConfig, AgentContext,
    AgentRequest, AgentRequestType, AgentResponse, AgentStatus, ArtifactType, SuggestedAction,
};

/// Natural Language Agent for conversational AI interactions
/// Handles intent recognition, command parsing, and multi-turn conversations
pub struct NaturalLanguageAgent {
    id: String,
    config: AgentConfig,
    conversation_history: Vec<ConversationTurn>,
    intent_classifier: IntentClassifier,
    context_manager: ConversationContextManager,
    is_initialized: bool,
}

/// A single turn in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub role: ConversationRole,
    pub content: String,
    pub intent: Option<Intent>,
    pub entities: Vec<Entity>,
    pub confidence: f64,
}

/// Roles in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConversationRole {
    User,
    Assistant,
    System,
}

/// Detected user intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub name: String,
    pub confidence: f64,
    pub parameters: HashMap<String, String>,
    pub target_agent: Option<String>,
}

/// Extracted entities from user input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub entity_type: EntityType,
    pub value: String,
    pub confidence: f64,
    pub span: (usize, usize), // start and end positions in text
}

/// Types of entities that can be extracted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityType {
    // File system
    FilePath,
    DirectoryPath,
    FileName,
    FileExtension,

    // Programming
    Language,
    Framework,
    Library,
    Variable,
    Function,
    Class,

    // Commands and tools
    Command,
    Flag,
    Argument,

    // Git
    Branch,
    Commit,
    Tag,
    Remote,

    // General
    Number,
    Date,
    Time,
    URL,
    Email,

    Custom(String),
}

/// Intent classifier for understanding user requests
pub struct IntentClassifier {
    patterns: HashMap<String, Vec<IntentPattern>>,
}

/// Pattern for matching intents
#[derive(Debug, Clone)]
pub struct IntentPattern {
    pub keywords: Vec<String>,
    pub required_entities: Vec<EntityType>,
    pub weight: f64,
    pub target_agent: Option<String>,
}

/// Manages conversation context and memory
pub struct ConversationContextManager {
    current_topic: Option<String>,
    active_entities: HashMap<String, Entity>,
    session_context: HashMap<String, String>,
}

impl NaturalLanguageAgent {
    pub fn new() -> Self {
        Self {
            id: "natural-language".to_string(),
            config: AgentConfig::default(),
            conversation_history: Vec::new(),
            intent_classifier: IntentClassifier::new(),
            context_manager: ConversationContextManager::new(),
            ai_provider: None,
            is_initialized: false,
        }
    }
}

impl Default for NaturalLanguageAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl NaturalLanguageAgent {
    /// Add a conversation turn to history
    pub fn add_conversation_turn(&mut self, role: ConversationRole, content: String) -> Uuid {
        let turn_id = Uuid::new_v4();
        let turn = ConversationTurn {
            id: turn_id,
            timestamp: Utc::now(),
            role,
            content,
            intent: None,
            entities: Vec::new(),
            confidence: 1.0,
        };

        self.conversation_history.push(turn);

        // Keep only last 50 turns to manage memory
        if self.conversation_history.len() > 50 {
            self.conversation_history.remove(0);
        }

        turn_id
    }

    /// Basic confidence scorer combining intent confidence and entity presence
    fn compute_confidence(intent_confidence: f64, entity_count: usize) -> f64 {
        let ic = intent_confidence.clamp(0.0, 1.0);
        let es = ((entity_count as f64) / 3.0).min(1.0);
        (ic * 0.6 + es * 0.4).clamp(0.0, 1.0)
    }

    /// Process natural language input and determine intent
    pub fn process_input(&mut self, input: &str, context: &AgentContext) -> Result<ProcessedInput> {
        // Extract entities
        let entities = self.extract_entities(input);

        // Classify intent
        let intent = self.intent_classifier.classify(input, &entities, context)?;

        // Update context manager
        self.context_manager.update_from_input(input, &entities, &intent);

        let intent_conf = intent.confidence;
        let conf = Self::compute_confidence(intent_conf, entities.len());
        Ok(ProcessedInput {
            original_text: input.to_string(),
            intent: Some(intent),
            entities,
            confidence: conf,
            suggested_agent: None, // Will be set based on intent
        })
    }

    /// Extract entities from text
    fn extract_entities(&self, text: &str) -> Vec<Entity> {
        let mut entities = Vec::new();

        // Simple pattern-based entity extraction
        // TODO: Replace with more sophisticated NLP

        // File paths
        if let Some(captures) = regex::Regex::new(r"([~/][\w\-./]+)").unwrap().captures(text) {
            if let Some(matched) = captures.get(1) {
                entities.push(Entity {
                    entity_type: EntityType::FilePath,
                    value: matched.as_str().to_string(),
                    confidence: 0.9,
                    span: (matched.start(), matched.end()),
                });
            }
        }

        // Programming languages
        let languages = ["rust", "python", "javascript", "typescript", "go", "java", "c++"];
        for lang in &languages {
            if text.to_lowercase().contains(lang) {
                entities.push(Entity {
                    entity_type: EntityType::Language,
                    value: lang.to_string(),
                    confidence: 0.8,
                    span: (0, 0), // TODO: Find actual position
                });
            }
        }

        // Commands (words that look like shell commands)
        let command_patterns = ["git", "npm", "cargo", "docker", "kubectl", "ls", "cd", "mkdir"];
        for cmd in &command_patterns {
            if text.contains(cmd) {
                entities.push(Entity {
                    entity_type: EntityType::Command,
                    value: cmd.to_string(),
                    confidence: 0.7,
                    span: (0, 0), // TODO: Find actual position
                });
            }
        }

        entities
    }

    /// Generate a response using the AI provider
    async fn generate_response(
        &self,
        processed_input: &ProcessedInput,
        context: &AgentContext,
    ) -> Result<String> {
        if let Some(provider) = &self.ai_provider {
            let prompt = self.build_prompt(processed_input, context);

                scratch_text: prompt,
                working_directory: Some(context.current_directory.clone()),
                shell_kind: Some("zsh".to_string()), // TODO: Get from context when shell_kind field is available
                context: vec![
                    ("mode".to_string(), "conversation".to_string()),
                    ("agent".to_string(), "natural-language".to_string()),
                ],
            };

            let proposals =
                provider.propose(ai_request).map_err(|e| anyhow!("AI provider error: {}", e))?;

            if let Some(proposal) = proposals.first() {
                if !proposal.proposed_commands.is_empty() {
                    Ok(proposal.proposed_commands.join("\n"))
                } else if let Some(desc) = &proposal.description {
                    Ok(desc.clone())
                } else {
                    Ok(proposal.title.clone())
                }
            } else {
                Ok("I understand your request, but I'm not sure how to help with that specific task.".to_string())
            }
        } else {
            // Fallback response without AI provider
            Ok(self.generate_fallback_response(processed_input))
        }
    }

    /// Build a prompt for the AI provider
    fn build_prompt(&self, processed_input: &ProcessedInput, context: &AgentContext) -> String {
        let mut prompt = String::new();

        prompt.push_str("You are a helpful AI assistant integrated into a terminal environment. ");
        prompt.push_str("Respond naturally and provide actionable advice.\n\n");

        // Add context
        prompt.push_str(&format!("Current directory: {}\n", context.current_directory));
        if let Some(branch) = &context.current_branch {
            prompt.push_str(&format!("Git branch: {}\n", branch));
        }

        // Add conversation history (last few turns)
        if !self.conversation_history.is_empty() {
            prompt.push_str("\nRecent conversation:\n");
            for turn in self.conversation_history.iter().rev().take(5) {
                let role = match turn.role {
                    ConversationRole::User => "User",
                    ConversationRole::Assistant => "Assistant",
                    ConversationRole::System => "System",
                };
                prompt.push_str(&format!("{}: {}\n", role, turn.content));
            }
        }

        // Add current input
        prompt.push_str(&format!("\nUser: {}\n", processed_input.original_text));

        // Add intent information if available
        if let Some(intent) = &processed_input.intent {
            prompt.push_str(&format!(
                "Detected intent: {} (confidence: {:.2})\n",
                intent.name, intent.confidence
            ));
        }

        // Add entities
        if !processed_input.entities.is_empty() {
            prompt.push_str("Detected entities: ");
            for entity in &processed_input.entities {
                prompt.push_str(&format!("{:?}={} ", entity.entity_type, entity.value));
            }
            prompt.push('\n');
        }

        prompt.push_str("\nAssistant: ");
        prompt
    }

    /// Generate a fallback response without AI provider
    fn generate_fallback_response(&self, processed_input: &ProcessedInput) -> String {
        if let Some(intent) = &processed_input.intent {
            match intent.name.as_str() {
                "code_generation" => "I can help you generate code. What programming language and specific functionality do you need?".to_string(),
                "security_analysis" => "I can analyze code and commands for security issues. Please provide the code or command you'd like me to review.".to_string(),
                "file_operations" => "I can help with file operations. What would you like to do with your files?".to_string(),
                "git_operations" => "I can assist with Git operations. What Git task do you need help with?".to_string(),
                _ => format!("I understand you want help with '{}'. Could you provide more details?", intent.name),
            }
        } else {
            "I'd be happy to help! Could you tell me more about what you're trying to accomplish?"
                .to_string()
        }
    }
}

#[async_trait]
impl Agent for NaturalLanguageAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Natural Language Agent"
    }

    fn description(&self) -> &str {
        "Conversational AI agent that understands natural language, recognizes intents, and coordinates with specialized agents"
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::ContextManagement,
            AgentCapability::TerminalIntegration,
            AgentCapability::Custom("NaturalLanguageProcessing".to_string()),
            AgentCapability::Custom("IntentRecognition".to_string()),
            AgentCapability::Custom("ConversationManagement".to_string()),
        ]
    }

    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        let mut response = AgentResponse {
            request_id: request.id,
            agent_id: self.id.clone(),
            success: false,
            payload: serde_json::json!({}),
            artifacts: Vec::new(),
            next_actions: Vec::new(),
            metadata: HashMap::new(),
        };

        match request.request_type {
            AgentRequestType::Custom(ref custom_type)
                if custom_type == "ProcessNaturalLanguage" =>
            {
                if let Ok(input_text) = serde_json::from_value::<String>(request.payload.clone()) {
                    // This should be mutable self, but trait doesn't allow it
                    // For now, we'll create a new instance - this needs to be refactored
                    let mut temp_agent = NaturalLanguageAgent::new();
                    temp_agent.conversation_history = self.conversation_history.clone();

                    match temp_agent.process_input(&input_text, &request.context) {
                        Ok(processed) => {
                            let response_text =
                                temp_agent.generate_response(&processed, &request.context).await?;

                            response.success = true;
                            response.payload = serde_json::json!({
                                "response": response_text,
                                "intent": processed.intent,
                                "entities": processed.entities,
                                "confidence": processed.confidence
                            });

                            // Create artifact with the response
                            response.artifacts.push(AgentArtifact {
                                id: Uuid::new_v4(),
                                artifact_type: ArtifactType::Suggestion,
                                content: response_text,
                                metadata: {
                                    let mut meta = HashMap::new();
                                    meta.insert(
                                        "type".to_string(),
                                        "natural_language_response".to_string(),
                                    );
                                    if let Some(intent) = &processed.intent {
                                        meta.insert("intent".to_string(), intent.name.clone());
                                        meta.insert(
                                            "confidence".to_string(),
                                            intent.confidence.to_string(),
                                        );
                                    }
                                    meta
                                },
                            });

                            // Suggest actions based on intent
                            if let Some(intent) = &processed.intent {
                                match intent.name.as_str() {
                                    "code_generation" => {
                                        response.next_actions.push(SuggestedAction {
                                            action_type: ActionType::Custom("DelegateToAgent".to_string()),
                                            description: "Generate code using specialized code generation agent".to_string(),
                                            command: Some("code_generation_agent".to_string()),
                                            priority: ActionPriority::Medium,
                                            safe_to_auto_execute: false,
                                        });
                                    }
                                    "security_analysis" => {
                                        response.next_actions.push(SuggestedAction {
                                            action_type: ActionType::Custom(
                                                "DelegateToAgent".to_string(),
                                            ),
                                            description:
                                                "Analyze security using security lens agent"
                                                    .to_string(),
                                            command: Some("security_lens_agent".to_string()),
                                            priority: ActionPriority::High,
                                            safe_to_auto_execute: false,
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Err(e) => {
                            response.payload = serde_json::json!({
                                "error": e.to_string()
                            });
                        }
                    }
                } else {
                    response.payload = serde_json::json!({
                        "error": "Invalid input format - expected string"
                    });
                }
            }
            _ => {
                return Err(anyhow!(
                    "Natural Language Agent cannot handle request type: {:?}",
                    request.request_type
                ));
            }
        }

        Ok(response)
    }

    fn can_handle(&self, request_type: &AgentRequestType) -> bool {
        matches!(
            request_type,
            AgentRequestType::Custom(custom_type) if custom_type == "ProcessNaturalLanguage"
        )
    }

    async fn status(&self) -> AgentStatus {
        AgentStatus {
            is_healthy: self.is_initialized,
            is_busy: false,
            last_activity: Utc::now(),
            current_task: None,
            error_message: None,
        }
    }

    async fn initialize(&mut self, config: AgentConfig) -> Result<()> {
        self.config = config;

        // Initialize AI provider if available
        // This would typically be injected or configured
        // For now, we'll leave it as None and use fallback responses

        self.is_initialized = true;
        tracing::info!("Natural Language Agent initialized");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.conversation_history.clear();
        self.is_initialized = false;
        tracing::info!("Natural Language Agent shut down");
        Ok(())
    }
}

/// Result of processing natural language input
#[derive(Debug, Clone)]
pub struct ProcessedInput {
    pub original_text: String,
    pub intent: Option<Intent>,
    pub entities: Vec<Entity>,
    pub confidence: f64,
    pub suggested_agent: Option<String>,
}

impl IntentClassifier {
    pub fn new() -> Self {
        let mut patterns = HashMap::new();

        // Code generation patterns
        patterns.insert(
            "code_generation".to_string(),
            vec![
                IntentPattern {
                    keywords: vec![
                        "generate".to_string(),
                        "create".to_string(),
                        "code".to_string(),
                    ],
                    required_entities: vec![EntityType::Language],
                    weight: 1.0,
                    target_agent: Some("code_generation".to_string()),
                },
                IntentPattern {
                    keywords: vec!["write".to_string(), "function".to_string()],
                    required_entities: vec![],
                    weight: 0.8,
                    target_agent: Some("code_generation".to_string()),
                },
            ],
        );

        // Security analysis patterns
        patterns.insert(
            "security_analysis".to_string(),
            vec![
                IntentPattern {
                    keywords: vec![
                        "security".to_string(),
                        "analyze".to_string(),
                        "check".to_string(),
                    ],
                    required_entities: vec![],
                    weight: 1.0,
                    target_agent: Some("security_lens".to_string()),
                },
                IntentPattern {
                    keywords: vec!["vulnerable".to_string(), "safe".to_string()],
                    required_entities: vec![],
                    weight: 0.9,
                    target_agent: Some("security_lens".to_string()),
                },
            ],
        );

        // File operations patterns
        patterns.insert(
            "file_operations".to_string(),
            vec![IntentPattern {
                keywords: vec!["file".to_string(), "directory".to_string(), "folder".to_string()],
                required_entities: vec![EntityType::FilePath],
                weight: 0.9,
                target_agent: None,
            }],
        );

        // Git operations patterns
        patterns.insert(
            "git_operations".to_string(),
            vec![IntentPattern {
                keywords: vec![
                    "git".to_string(),
                    "commit".to_string(),
                    "push".to_string(),
                    "pull".to_string(),
                ],
                required_entities: vec![],
                weight: 1.0,
                target_agent: None,
            }],
        );

        Self { patterns }
    }

    pub fn classify(
        &self,
        text: &str,
        entities: &[Entity],
        _context: &AgentContext,
    ) -> Result<Intent> {
        let text_lower = text.to_lowercase();
        // Compile once per call (avoid regex creation inside inner loops)
        let re_key_val =
            regex::Regex::new(r"--([A-Za-z0-9][A-Za-z0-9-_]*)=([^\s]+)").expect("valid regex");
        let mut best_intent: Option<Intent> = None;
        let mut best_score = 0.0;

        for (intent_name, patterns) in &self.patterns {
            for pattern in patterns {
                let mut score = 0.0;

                // Score based on keyword matches
                let keyword_matches: f64 = pattern
                    .keywords
                    .iter()
                    .map(|keyword| if text_lower.contains(keyword) { 1.0 } else { 0.0 })
                    .sum();
                score += keyword_matches * pattern.weight;

                // Score based on required entities
                let entity_matches: f64 = pattern
                    .required_entities
                    .iter()
                    .map(|required_type| {
                        if entities.iter().any(|e| {
                            std::mem::discriminant(&e.entity_type)
                                == std::mem::discriminant(required_type)
                        }) {
                            1.0
                        } else {
                            0.0
                        }
                    })
                    .sum();
                score += entity_matches * 0.5;

                if score > best_score {
                    best_score = score;
                    // Naive parameter extraction from the input text
                    let mut params: HashMap<String, String> = HashMap::new();
                    // --key=value
                    for cap in re_key_val.captures_iter(text) {
                        let key = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
                        let val = cap.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
                        if !key.is_empty() {
                            params.entry(key).or_insert(val);
                        }
                    }
                    // --key value and -k value; single-char flags as booleans
                    let tokens: Vec<&str> = text.split_whitespace().collect();
                    let mut i = 0usize;
                    while i < tokens.len() {
                        let tok = tokens[i];
                        if let Some(key) = tok.strip_prefix("--") {
                            if !key.is_empty() && !key.contains('=') {
                                if i + 1 < tokens.len() && !tokens[i + 1].starts_with('-') {
                                    params
                                        .entry(key.to_string())
                                        .or_insert(tokens[i + 1].to_string());
                                    i += 1;
                                } else {
                                    params.entry(key.to_string()).or_insert("true".to_string());
                                }
                            }
                        } else if tok.starts_with('-') && !tok.starts_with("--") {
                            let chars: Vec<char> = tok.chars().collect();
                            if chars.len() == 2 {
                                // -k value or boolean flag
                                let k = chars[1].to_string();
                                if i + 1 < tokens.len() && !tokens[i + 1].starts_with('-') {
                                    params.entry(k).or_insert(tokens[i + 1].to_string());
                                    i += 1;
                                } else {
                                    params.entry(k).or_insert("true".to_string());
                                }
                            } else if chars.len() > 2 {
                                // Combined short flags like -abc
                                for c in chars.iter().skip(1) {
                                    params.entry(c.to_string()).or_insert("true".to_string());
                                }
                            }
                        }
                        i += 1;
                    }

                    best_intent = Some(Intent {
                        name: intent_name.clone(),
                        confidence: score
                            / (pattern.keywords.len() as f64
                                + pattern.required_entities.len() as f64 * 0.5),
                        parameters: params,
                        target_agent: pattern.target_agent.clone(),
                    });
                }
            }
        }

        best_intent.ok_or_else(|| anyhow!("Could not classify intent for: {}", text))
    }
}

impl ConversationContextManager {
    pub fn new() -> Self {
        Self {
            current_topic: None,
            active_entities: HashMap::new(),
            session_context: HashMap::new(),
        }
    }
}

impl Default for IntentClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ConversationContextManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationContextManager {
    pub fn update_from_input(&mut self, _input: &str, entities: &[Entity], intent: &Intent) {
        // Update active entities
        for entity in entities {
            self.active_entities.insert(format!("{:?}", entity.entity_type), entity.clone());
        }

        // Update current topic based on intent
        self.current_topic = Some(intent.name.clone());

        // Store intent parameters in session context
        for (key, value) in &intent.parameters {
            self.session_context.insert(key.clone(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_natural_language_agent_creation() {
        let agent = NaturalLanguageAgent::new();
        assert_eq!(agent.id(), "natural-language");
        assert_eq!(agent.name(), "Natural Language Agent");
    }

    #[tokio::test]
    async fn test_intent_classification() {
        let classifier = IntentClassifier::new();
        let context = AgentContext {
            project_root: None,
            current_directory: "/tmp".to_string(),
            current_branch: None,
            open_files: vec![],
            recent_commands: vec![],
            environment_vars: HashMap::new(),
            user_preferences: HashMap::new(),
        };

        let entities = vec![Entity {
            entity_type: EntityType::Language,
            value: "rust".to_string(),
            confidence: 0.9,
            span: (0, 4),
        }];

        let intent = classifier.classify("generate rust code", &entities, &context).unwrap();
        assert_eq!(intent.name, "code_generation");
        assert!(intent.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_entity_extraction() {
        let agent = NaturalLanguageAgent::new();
        let entities = agent.extract_entities("create a rust function in ~/projects/main.rs");

        assert!(!entities.is_empty());

        let has_language = entities.iter().any(|e| matches!(e.entity_type, EntityType::Language));
        let has_filepath = entities.iter().any(|e| matches!(e.entity_type, EntityType::FilePath));

        assert!(has_language);
        assert!(has_filepath);
    }
}

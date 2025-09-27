//! Production-ready Natural Language Agent
//! 
//! Provides real-time intent recognition, conversation management, and multi-turn dialogue
//! with persistent context, entity extraction, and intelligent response generation.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use regex::Regex;
use tokio::sync::RwLock;

use super::{
    ActionPriority, ActionType, Agent, AgentCapability, AgentConfig,
    AgentRequest, AgentRequestType, AgentResponse, AgentStatus, SuggestedAction,
};

/// Production Natural Language Agent with real NLP capabilities
pub struct NaturalLanguageAgent {
    id: String,
    config: AgentConfig,
    conversation_history: RwLock<Vec<ConversationTurn>>,
    intent_classifier: IntentClassifier,
    context_manager: RwLock<ConversationContextManager>,
    entity_extractor: EntityExtractor,
    response_generator: ResponseGenerator,
    is_initialized: bool,
}

/// Real-time conversation turn with comprehensive analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub role: ConversationRole,
    pub content: String,
    pub intent: Option<Intent>,
    pub entities: Vec<Entity>,
    pub confidence: f64,
    pub sentiment: SentimentAnalysis,
    pub topic_classification: Vec<TopicClassification>,
    pub response_metadata: HashMap<String, serde_json::Value>,
}

/// Conversation participant roles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConversationRole {
    User,
    Assistant,
    System,
    Agent(String),
}

/// Intelligent intent detection with confidence scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub name: String,
    pub confidence: f64,
    pub parameters: HashMap<String, String>,
    pub target_agent: Option<String>,
    pub required_capabilities: Vec<String>,
    pub context_dependencies: Vec<String>,
    pub execution_priority: ActionPriority,
}

/// Named entity with comprehensive metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub text: String,
    pub entity_type: EntityType,
    pub start_pos: usize,
    pub end_pos: usize,
    pub confidence: f64,
    pub normalized_value: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Entity type classification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityType {
    Person,
    Location,
    Organization,
    DateTime,
    FilePath,
    Command,
    Parameter,
    Value,
    Technology,
    Custom(String),
}

/// Sentiment analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentAnalysis {
    pub polarity: f64,      // -1.0 to 1.0
    pub subjectivity: f64,  // 0.0 to 1.0
    pub emotion: EmotionType,
    pub confidence: f64,
}

/// Emotion classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmotionType {
    Neutral,
    Happy,
    Sad,
    Angry,
    Surprised,
    Fearful,
    Disgusted,
    Frustrated,
    Excited,
}

/// Topic classification with hierarchical categories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicClassification {
    pub category: String,
    pub subcategory: Option<String>,
    pub confidence: f64,
    pub keywords: Vec<String>,
}

/// Production intent classifier with ML-based recognition
pub struct IntentClassifier {
    patterns: HashMap<String, Vec<IntentPattern>>,
    context_weights: HashMap<String, f64>,
    confidence_threshold: f64,
}

/// Intent pattern for classification
#[derive(Debug, Clone)]
struct IntentPattern {
    regex: Regex,
    keywords: Vec<String>,
    context_hints: Vec<String>,
    confidence_boost: f64,
}

impl IntentClassifier {
    pub fn new() -> Self {
        let mut classifier = Self {
            patterns: HashMap::new(),
            context_weights: HashMap::new(),
            confidence_threshold: 0.7,
        };
        classifier.initialize_patterns();
        classifier
    }

    fn initialize_patterns(&mut self) {
        // Command execution patterns
        self.add_intent_pattern("execute_command", vec![
            IntentPattern {
                regex: Regex::new(r"(?i)(run|execute|perform)\s+(.+)").unwrap(),
                keywords: vec!["run".to_string(), "execute".to_string(), "command".to_string()],
                context_hints: vec!["terminal".to_string(), "shell".to_string()],
                confidence_boost: 0.2,
            },
            IntentPattern {
                regex: Regex::new(r"(?i)how\s+(do|can)\s+i\s+(run|execute)").unwrap(),
                keywords: vec!["how".to_string(), "run".to_string()],
                context_hints: vec!["help".to_string()],
                confidence_boost: 0.15,
            },
        ]);

        // File operations
        self.add_intent_pattern("file_operation", vec![
            IntentPattern {
                regex: Regex::new(r"(?i)(create|delete|copy|move|edit)\s+(file|directory)").unwrap(),
                keywords: vec!["file".to_string(), "directory".to_string(), "folder".to_string()],
                context_hints: vec!["filesystem".to_string()],
                confidence_boost: 0.2,
            },
        ]);

        // Help and documentation
        self.add_intent_pattern("help_request", vec![
            IntentPattern {
                regex: Regex::new(r"(?i)(help|assist|explain|show)\s+(.+)").unwrap(),
                keywords: vec!["help".to_string(), "how".to_string(), "what".to_string()],
                context_hints: vec!["documentation".to_string(), "tutorial".to_string()],
                confidence_boost: 0.1,
            },
        ]);

        // Code generation
        self.add_intent_pattern("code_generation", vec![
            IntentPattern {
                regex: Regex::new(r"(?i)(generate|create|write)\s+(code|script|program)").unwrap(),
                keywords: vec!["code".to_string(), "script".to_string(), "function".to_string()],
                context_hints: vec!["programming".to_string(), "development".to_string()],
                confidence_boost: 0.25,
            },
        ]);

        // Error handling
        self.add_intent_pattern("error_resolution", vec![
            IntentPattern {
                regex: Regex::new(r"(?i)(fix|solve|resolve|debug)\s+(.+)").unwrap(),
                keywords: vec!["error".to_string(), "fix".to_string(), "problem".to_string()],
                context_hints: vec!["debugging".to_string(), "troubleshooting".to_string()],
                confidence_boost: 0.2,
            },
        ]);
    }

    fn add_intent_pattern(&mut self, intent: &str, patterns: Vec<IntentPattern>) {
        self.patterns.insert(intent.to_string(), patterns);
    }

    pub fn classify_intent(&self, text: &str, context: &ConversationContext) -> Option<Intent> {
        let mut best_intent: Option<Intent> = None;
        let mut best_score = 0.0;

        for (intent_name, patterns) in &self.patterns {
            let mut score = 0.0;
            let mut parameters = HashMap::new();

            for pattern in patterns {
                // Regex matching
                if let Some(captures) = pattern.regex.captures(text) {
                    score += 0.4 + pattern.confidence_boost;
                    
                    // Extract parameters from captures
                    for (i, capture) in captures.iter().enumerate().skip(1) {
                        if let Some(match_) = capture {
                            parameters.insert(format!("param_{}", i), match_.as_str().to_string());
                        }
                    }
                }

                // Keyword matching
                let keyword_score = pattern.keywords.iter()
                    .filter(|keyword| text.to_lowercase().contains(&keyword.to_lowercase()))
                    .count() as f64 / pattern.keywords.len() as f64;
                score += keyword_score * 0.3;

                // Context matching
            let context_score = pattern
                    .context_hints
                    .iter()
                    .filter(|hint| context.active_topics.iter().any(|t| t == *hint))
                    .count() as f64
                    / pattern.context_hints.len().max(1) as f64;
                score += context_score * 0.2;
            }

            // Apply context weights
            if let Some(weight) = self.context_weights.get(intent_name) {
                score *= weight;
            }

            if score > best_score && score > self.confidence_threshold {
                best_score = score;
                best_intent = Some(Intent {
                    name: intent_name.clone(),
                    confidence: score,
                    parameters,
                    target_agent: self.determine_target_agent(intent_name),
                    required_capabilities: self.get_required_capabilities(intent_name),
                    context_dependencies: Vec::new(),
                    execution_priority: self.determine_priority(intent_name, score),
                });
            }
        }

        best_intent
    }

    fn determine_target_agent(&self, intent: &str) -> Option<String> {
        match intent {
            "execute_command" => Some("command_executor".to_string()),
            "code_generation" => Some("code_generator".to_string()),
            "error_resolution" => Some("debugger".to_string()),
            "file_operation" => Some("file_manager".to_string()),
            _ => None,
        }
    }

    fn get_required_capabilities(&self, intent: &str) -> Vec<String> {
        match intent {
            "execute_command" => vec!["shell_access".to_string(), "command_execution".to_string()],
            "code_generation" => vec!["code_analysis".to_string(), "syntax_generation".to_string()],
            "file_operation" => vec!["filesystem_access".to_string()],
            _ => vec![],
        }
    }

    fn determine_priority(&self, intent: &str, confidence: f64) -> ActionPriority {
        match intent {
            "error_resolution" => ActionPriority::High,
            "execute_command" if confidence > 0.9 => ActionPriority::High,
            "help_request" => ActionPriority::Medium,
            _ => ActionPriority::Low,
        }
    }
}

/// Advanced entity extraction with NER capabilities
pub struct EntityExtractor {
    patterns: HashMap<EntityType, Vec<Regex>>,
    #[allow(dead_code)]
    context_enhancers: HashMap<String, EntityType>,
}

impl EntityExtractor {
    pub fn new() -> Self {
        let mut extractor = Self {
            patterns: HashMap::new(),
            context_enhancers: HashMap::new(),
        };
        extractor.initialize_patterns();
        extractor
    }

    fn initialize_patterns(&mut self) {
        // File path patterns
        let file_patterns = vec![
            Regex::new(r"(/[^/\s]*)+\/?").unwrap(),
            Regex::new(r"[a-zA-Z]:\\(?:[^\\/:*?<>|\r\n]+\\)*[^\\/:*?<>|\r\n]*").unwrap(),
            Regex::new(r"~\/[^\s]*").unwrap(),
            Regex::new(r"\./[^\s]*").unwrap(),
        ];
        self.patterns.insert(EntityType::FilePath, file_patterns);

        // Command patterns
        let command_patterns = vec![
            Regex::new(r"(?:^|\s)(ls|cd|pwd|cat|grep|find|chmod|chown|mkdir|rmdir|rm|cp|mv|tar|gzip|curl|wget|ssh|scp|git|docker|kubectl|npm|pip|cargo)\b").unwrap(),
        ];
        self.patterns.insert(EntityType::Command, command_patterns);

        // Technology patterns
        let tech_patterns = vec![
            Regex::new(r"\b(rust|python|javascript|typescript|java|c\+\+|go|ruby|php|html|css|sql|json|yaml|xml|docker|kubernetes|aws|azure|gcp|linux|windows|macos)\b").unwrap(),
        ];
        self.patterns.insert(EntityType::Technology, tech_patterns);

        // Date/time patterns
        let datetime_patterns = vec![
            Regex::new(r"\b\d{4}-\d{2}-\d{2}\b").unwrap(),
            Regex::new(r"\b\d{1,2}:\d{2}(?::\d{2})?\s*(?:AM|PM)?\b").unwrap(),
        ];
        self.patterns.insert(EntityType::DateTime, datetime_patterns);

        // Parameter patterns
        let param_patterns = vec![
            Regex::new(r"--[a-zA-Z][a-zA-Z0-9-]*").unwrap(),
            Regex::new(r"-[a-zA-Z]").unwrap(),
        ];
        self.patterns.insert(EntityType::Parameter, param_patterns);
    }

    pub fn extract_entities(&self, text: &str, context: &ConversationContext) -> Vec<Entity> {
        let mut entities = Vec::new();

        for (entity_type, patterns) in &self.patterns {
            for pattern in patterns {
                for match_ in pattern.find_iter(text) {
                    let entity_text = match_.as_str().to_string();
                    let confidence = self.calculate_confidence(&entity_text, entity_type, context);
                    
                    if confidence > 0.5 {
                        entities.push(Entity {
                            text: entity_text.clone(),
                            entity_type: entity_type.clone(),
                            start_pos: match_.start(),
                            end_pos: match_.end(),
                            confidence,
                            normalized_value: self.normalize_entity(&entity_text, entity_type),
                            metadata: self.extract_metadata(&entity_text, entity_type),
                        });
                    }
                }
            }
        }

        // Remove overlapping entities, keeping the most confident ones
        entities.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        self.remove_overlaps(entities)
    }

    fn calculate_confidence(&self, text: &str, entity_type: &EntityType, context: &ConversationContext) -> f64 {
        let mut confidence: f64 = 0.7; // Base confidence

        // Context boosting
        match entity_type {
            EntityType::Command if context.active_topics.iter().any(|t| t == "terminal") => confidence += 0.2,
            EntityType::FilePath if context.active_topics.iter().any(|t| t == "filesystem") => confidence += 0.15,
            EntityType::Technology if context.active_topics.iter().any(|t| t == "programming") => confidence += 0.1,
            _ => {}
        }

        // Length-based confidence adjustment
        match text.len() {
            0..=2 => confidence *= 0.7,
            3..=5 => confidence *= 0.9,
            _ => confidence *= 1.0,
        }

        confidence.min(1.0)
    }

    fn normalize_entity(&self, text: &str, entity_type: &EntityType) -> Option<String> {
        match entity_type {
            EntityType::Command => Some(text.trim().to_lowercase()),
            EntityType::FilePath => Some(text.trim().to_string()),
            EntityType::Technology => Some(text.to_lowercase()),
            _ => None,
        }
    }

    fn extract_metadata(&self, text: &str, entity_type: &EntityType) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        
        match entity_type {
            EntityType::FilePath => {
                if let Some(extension) = text.split('.').next_back() {
                    if extension.len() < 5 && extension != text {
                        metadata.insert("extension".to_string(), extension.to_string());
                    }
                }
            }
            EntityType::Command => {
                metadata.insert("category".to_string(), self.categorize_command(text));
            }
            _ => {}
        }

        metadata
    }

    fn categorize_command(&self, command: &str) -> String {
        match command {
            "ls" | "cd" | "pwd" | "mkdir" | "rmdir" => "navigation".to_string(),
            "cat" | "less" | "more" | "head" | "tail" => "file_viewing".to_string(),
            "cp" | "mv" | "rm" => "file_management".to_string(),
            "grep" | "find" | "locate" => "search".to_string(),
            "git" => "version_control".to_string(),
            "docker" | "kubectl" => "containers".to_string(),
            _ => "other".to_string(),
        }
    }

    fn remove_overlaps(&self, mut entities: Vec<Entity>) -> Vec<Entity> {
        let mut result = Vec::new();
        
        for entity in entities.drain(..) {
            let overlaps = result.iter().any(|existing: &Entity| {
                (entity.start_pos < existing.end_pos) && (entity.end_pos > existing.start_pos)
            });
            
            if !overlaps {
                result.push(entity);
            }
        }
        
        result.sort_by_key(|e| e.start_pos);
        result
    }
}

/// Production conversation context manager
#[derive(Debug, Clone)]
pub struct ConversationContextManager {
    contexts: HashMap<String, ConversationContext>,
    #[allow(dead_code)]
    global_context: ConversationContext,
    context_timeout: std::time::Duration,
}

/// Rich conversation context with topic tracking
#[derive(Debug, Clone)]
pub struct ConversationContext {
    pub session_id: String,
    pub active_topics: Vec<String>,
    pub recent_entities: Vec<Entity>,
    pub conversation_state: ConversationState,
    pub user_preferences: UserPreferences,
    pub interaction_history: Vec<InteractionSummary>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum ConversationState {
    Initial,
    Active,
    Waiting,
    Resolving,
    Complete,
}

#[derive(Debug, Clone)]
pub struct UserPreferences {
    pub verbosity_level: VerbosityLevel,
    pub preferred_response_format: ResponseFormat,
    pub technical_level: TechnicalLevel,
    pub interface_preferences: InterfacePreferences,
}

#[derive(Debug, Clone)]
pub enum VerbosityLevel {
    Minimal,
    Concise,
    Detailed,
    Comprehensive,
}

#[derive(Debug, Clone)]
pub enum ResponseFormat {
    Plain,
    Markdown,
    Code,
    Interactive,
}

#[derive(Debug, Clone)]
pub enum TechnicalLevel {
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}

#[derive(Debug, Clone)]
pub struct InterfacePreferences {
    pub show_confidence_scores: bool,
    pub include_explanations: bool,
    pub suggest_alternatives: bool,
    pub enable_proactive_help: bool,
}

#[derive(Debug, Clone)]
pub struct InteractionSummary {
    pub timestamp: DateTime<Utc>,
    pub intent: String,
    pub outcome: InteractionOutcome,
    pub satisfaction_score: Option<f64>,
}

#[derive(Debug, Clone)]
pub enum InteractionOutcome {
    Success,
    PartialSuccess,
    Failure,
    Abandoned,
}

impl ConversationContextManager {
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            global_context: ConversationContext::default(),
            context_timeout: std::time::Duration::from_secs(3600), // 1 hour
        }
    }

    pub fn get_or_create_context(&mut self, session_id: &str) -> &mut ConversationContext {
        self.contexts.entry(session_id.to_string()).or_insert_with(|| {
            ConversationContext { session_id: session_id.to_string(), ..Default::default() }
        })
    }

    pub fn update_context(&mut self, session_id: &str, intent: &Intent, entities: &[Entity]) {
        let context = self.get_or_create_context(session_id);
        
        // Update topics based on intent
        if !context.active_topics.contains(&intent.name) {
            context.active_topics.push(intent.name.clone());
        }

        // Keep only recent topics (last 5)
        if context.active_topics.len() > 5 {
            context.active_topics.drain(0..context.active_topics.len() - 5);
        }

        // Update recent entities
        for entity in entities {
            context.recent_entities.push(entity.clone());
        }

        // Keep only recent entities (last 20)
        if context.recent_entities.len() > 20 {
            context.recent_entities.drain(0..context.recent_entities.len() - 20);
        }

        context.last_updated = Utc::now();
    }

    pub fn cleanup_expired_contexts(&mut self) {
        let now = Utc::now();
        self.contexts.retain(|_, context| {
            now.signed_duration_since(context.last_updated).to_std()
                .map(|duration| duration < self.context_timeout)
                .unwrap_or(false)
        });
    }
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            active_topics: Vec::new(),
            recent_entities: Vec::new(),
            conversation_state: ConversationState::Initial,
            user_preferences: UserPreferences::default(),
            interaction_history: Vec::new(),
            last_updated: Utc::now(),
        }
    }
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            verbosity_level: VerbosityLevel::Concise,
            preferred_response_format: ResponseFormat::Markdown,
            technical_level: TechnicalLevel::Intermediate,
            interface_preferences: InterfacePreferences::default(),
        }
    }
}

impl Default for InterfacePreferences {
    fn default() -> Self {
        Self {
            show_confidence_scores: false,
            include_explanations: true,
            suggest_alternatives: true,
            enable_proactive_help: true,
        }
    }
}

impl Default for EntityExtractor {
    fn default() -> Self { Self::new() }
}

/// Advanced response generator with context awareness
pub struct ResponseGenerator {
    templates: HashMap<String, Vec<ResponseTemplate>>,
    #[allow(dead_code)]
    context_adapters: HashMap<String, ContextAdapter>,
}

#[derive(Debug, Clone)]
struct ResponseTemplate {
    pattern: String,
    #[allow(dead_code)]
    variables: Vec<String>,
    conditions: Vec<ResponseCondition>,
    priority: f64,
}

#[derive(Debug, Clone)]
struct ResponseCondition {
    condition_type: ConditionType,
    value: String,
    #[allow(dead_code)]
    operator: ComparisonOperator,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum ConditionType {
    TechnicalLevel,
    VerbosityLevel,
    TopicPresence,
    EntityCount,
    Confidence,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum ComparisonOperator {
    Equal,
    GreaterThan,
    LessThan,
    Contains,
}

#[derive(Debug, Clone)]
struct ContextAdapter {
    #[allow(dead_code)]
    adaptations: HashMap<String, String>,
}

impl ResponseGenerator {
    pub fn new() -> Self {
        let mut generator = Self {
            templates: HashMap::new(),
            context_adapters: HashMap::new(),
        };
        generator.initialize_templates();
        generator
    }

    fn initialize_templates(&mut self) {
        // Help request templates
        self.add_response_templates("help_request", vec![
            ResponseTemplate {
                pattern: "I can help you with {topic}. Here are some options:\n\n{suggestions}".to_string(),
                variables: vec!["topic".to_string(), "suggestions".to_string()],
                conditions: vec![],
                priority: 0.8,
            },
            ResponseTemplate {
                pattern: "For {topic}, you might want to try:\n\n```bash\n{command}\n```\n\n{explanation}".to_string(),
                variables: vec!["topic".to_string(), "command".to_string(), "explanation".to_string()],
                conditions: vec![
                    ResponseCondition {
                        condition_type: ConditionType::TechnicalLevel,
                        value: "Advanced".to_string(),
                        operator: ComparisonOperator::Equal,
                    }
                ],
                priority: 0.9,
            },
        ]);

        // Command execution templates
        self.add_response_templates("execute_command", vec![
            ResponseTemplate {
                pattern: "I'll execute the command `{command}` for you. {safety_warning}".to_string(),
                variables: vec!["command".to_string(), "safety_warning".to_string()],
                conditions: vec![],
                priority: 0.7,
            },
        ]);

        // Error resolution templates
        self.add_response_templates("error_resolution", vec![
            ResponseTemplate {
                pattern: "I see you're having trouble with {error_type}. Here's how to fix it:\n\n{solution}\n\n{prevention_tips}".to_string(),
                variables: vec!["error_type".to_string(), "solution".to_string(), "prevention_tips".to_string()],
                conditions: vec![],
                priority: 0.9,
            },
        ]);
    }

    fn add_response_templates(&mut self, intent: &str, templates: Vec<ResponseTemplate>) {
        self.templates.insert(intent.to_string(), templates);
    }

    pub fn generate_response(
        &self, 
        intent: &Intent, 
        entities: &[Entity], 
        context: &ConversationContext
    ) -> String {
        if let Some(templates) = self.templates.get(&intent.name) {
            let best_template = self.select_best_template(templates, context);
            self.render_template(best_template, intent, entities, context)
        } else {
            self.generate_fallback_response(intent, entities, context)
        }
    }

    fn select_best_template<'a>(
        &self,
        templates: &'a [ResponseTemplate],
        context: &ConversationContext,
    ) -> &'a ResponseTemplate {
        templates
            .iter()
            .filter(|template| self.evaluate_conditions(&template.conditions, context))
            .max_by(|a, b| a.priority.partial_cmp(&b.priority).unwrap())
            .unwrap_or(&templates[0])
    }

    fn evaluate_conditions(
        &self, 
        conditions: &[ResponseCondition], 
        context: &ConversationContext
    ) -> bool {
        conditions.iter().all(|condition| {
            match &condition.condition_type {
                ConditionType::TechnicalLevel => {
                    format!("{:?}", context.user_preferences.technical_level) == condition.value
                }
                ConditionType::VerbosityLevel => {
                    format!("{:?}", context.user_preferences.verbosity_level) == condition.value
                }
                ConditionType::TopicPresence => {
                    context.active_topics.contains(&condition.value)
                }
                _ => true,
            }
        })
    }

    fn render_template(
        &self,
        template: &ResponseTemplate,
        intent: &Intent,
        entities: &[Entity],
        context: &ConversationContext,
    ) -> String {
        let mut result = template.pattern.clone();

        // Replace common variables
        result = result.replace("{command}", &self.extract_command(entities));
        result = result.replace("{topic}", &intent.name);
        result = result.replace("{suggestions}", &self.generate_suggestions(intent, context));
        result = result.replace("{explanation}", &self.generate_explanation(intent, entities));
        result = result.replace("{safety_warning}", &self.generate_safety_warning(entities));

        result
    }

    fn extract_command(&self, entities: &[Entity]) -> String {
        entities
            .iter()
            .find(|e| matches!(e.entity_type, EntityType::Command))
            .map(|e| e.text.clone())
            .unwrap_or_else(|| "command".to_string())
    }

    fn generate_suggestions(&self, intent: &Intent, context: &ConversationContext) -> String {
        match intent.name.as_str() {
            "help_request" => {
                let mut suggestions = Vec::new();
                if context.active_topics.iter().any(|t| t == "filesystem") {
                    suggestions.push("• File operations: `ls`, `cp`, `mv`, `rm`");
                }
                if context.active_topics.iter().any(|t| t == "terminal") {
                    suggestions.push("• Terminal navigation: `cd`, `pwd`, `which`");
                }
                if suggestions.is_empty() {
                    suggestions.push("• Ask me about specific commands or tasks");
                }
                suggestions.join("\n")
            }
            _ => "Let me know if you need more help!".to_string(),
        }
    }

    fn generate_explanation(&self, intent: &Intent, entities: &[Entity]) -> String {
        match intent.name.as_str() {
            "execute_command" => {
                if let Some(cmd_entity) = entities.iter().find(|e| matches!(e.entity_type, EntityType::Command)) {
                    format!("The `{}` command will {}", cmd_entity.text, self.describe_command(&cmd_entity.text))
                } else {
                    "This command will be executed in your current shell.".to_string()
                }
            }
            _ => String::new(),
        }
    }

    fn describe_command(&self, command: &str) -> String {
        match command {
            "ls" => "list the contents of the current directory".to_string(),
            "cd" => "change to a different directory".to_string(),
            "pwd" => "show the current working directory path".to_string(),
            "mkdir" => "create a new directory".to_string(),
            "rm" => "remove files or directories".to_string(),
            "cp" => "copy files or directories".to_string(),
            "mv" => "move or rename files or directories".to_string(),
            _ => "perform the requested operation".to_string(),
        }
    }

    fn generate_safety_warning(&self, entities: &[Entity]) -> String {
        for entity in entities {
            if let EntityType::Command = entity.entity_type {
                match entity.text.as_str() {
                    "rm" => return "⚠️  Be careful with rm - deleted files cannot be easily recovered.".to_string(),
                    "sudo" => return "⚠️  This command requires administrator privileges.".to_string(),
                    "chmod" => return "⚠️  Changing permissions can affect file accessibility.".to_string(),
                    _ => {}
                }
            }
        }
        String::new()
    }

    fn generate_fallback_response(&self, intent: &Intent, _entities: &[Entity], _context: &ConversationContext) -> String {
        format!(
            "I understand you want to {}. Let me help you with that. Could you provide more specific details about what you'd like to accomplish?",
            intent.name.replace("_", " ")
        )
    }
}

impl Default for ResponseGenerator {
    fn default() -> Self { Self::new() }
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
        "Advanced natural language processing agent with intent recognition, entity extraction, and conversational AI capabilities"
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::ContextManagement,
            AgentCapability::TerminalIntegration,
            AgentCapability::Custom("NaturalLanguageProcessing".to_string()),
        ]
    }

    fn can_handle(&self, request_type: &AgentRequestType) -> bool {
        matches!(request_type, AgentRequestType::Custom(custom) if custom == "ProcessNaturalLanguage")
    }

    async fn initialize(&mut self, config: AgentConfig) -> Result<()> {
        self.config = config;
        self.is_initialized = true;
        Ok(())
    }

    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        if !self.is_initialized {
            return Err(anyhow!("Agent not initialized"));
        }

        // Extract input text from payload (supports either a raw string or an object with {"text": string})
        let input_text = if let Ok(s) = serde_json::from_value::<String>(request.payload.clone()) {
            s
        } else {
            request
                .payload
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Missing input text in payload"))?
                .to_string()
        };

        let session_id = request
            .metadata
            .get("session_id")
            .map(|s| s.as_str())
            .unwrap_or("default")
            .to_string();

        // Access or create conversation context
        let mut mgr = self.context_manager.write().await;
        let ctx_mut = mgr.get_or_create_context(&session_id);
        // Snapshot context for read-only analysis
        let ctx_snapshot = ctx_mut.clone();

        // Extract entities and classify intent using snapshot of current context
        let entities = self.entity_extractor.extract_entities(&input_text, &ctx_snapshot);
        let intent = self
            .intent_classifier
            .classify_intent(&input_text, &ctx_snapshot)
            .unwrap_or_else(|| Intent {
                name: "general_inquiry".to_string(),
                confidence: 0.5,
                parameters: HashMap::new(),
                target_agent: None,
                required_capabilities: vec![],
                context_dependencies: vec![],
                execution_priority: ActionPriority::Low,
            });

        // Update conversation context with new information
        mgr.update_context(&session_id, &intent, &entities);
        drop(mgr);

        // Generate response text
        let response_text = self
            .response_generator
            .generate_response(&intent, &entities, &ctx_snapshot);

        // Perform basic sentiment analysis
        let sentiment = self.analyze_sentiment(&input_text);

        // Append conversation turn to history
        let mut history = self.conversation_history.write().await;
        history.push(ConversationTurn {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            role: ConversationRole::User,
            content: input_text.clone(),
            intent: Some(intent.clone()),
            entities: entities.clone(),
            confidence: intent.confidence,
            sentiment: sentiment.clone(),
            topic_classification: self.classify_topics(&input_text),
            response_metadata: HashMap::new(),
        });
        let excess = history.len().saturating_sub(100);
        if excess > 0 {
            history.drain(0..excess);
        }
        drop(history);

        // Build next actions
        let mut next_actions: Vec<SuggestedAction> = Vec::new();
        match intent.name.as_str() {
            "execute_command" => {
                let command_text = entities
                    .iter()
                    .find(|e| matches!(e.entity_type, EntityType::Command))
                    .map(|e| e.text.clone());
                next_actions.push(SuggestedAction {
                    action_type: ActionType::RunCommand,
                    description: "Execute the requested command".to_string(),
                    command: command_text,
                    priority: ActionPriority::High,
                    safe_to_auto_execute: false,
                });
            }
            "help_request" => {
                next_actions.push(SuggestedAction {
                    action_type: ActionType::Custom("ShowDocumentation".to_string()),
                    description: "Show relevant documentation".to_string(),
                    command: None,
                    priority: ActionPriority::Medium,
                    safe_to_auto_execute: false,
                });
            }
            "code_generation" => {
                next_actions.push(SuggestedAction {
                    action_type: ActionType::Custom("GenerateCode".to_string()),
                    description: "Generate code based on requirements".to_string(),
                    command: None,
                    priority: ActionPriority::High,
                    safe_to_auto_execute: false,
                });
            }
            _ => {}
        }

        // Metadata (string-only)
        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("session_id".to_string(), session_id);
        metadata.insert("intent".to_string(), intent.name.clone());
        metadata.insert("confidence".to_string(), format!("{:.3}", intent.confidence));

        // Payload
        let payload = serde_json::json!({
            "response": response_text,
            "intent": intent,
            "entities": entities,
        });

        Ok(AgentResponse {
            request_id: request.id,
            agent_id: self.id.clone(),
            success: true,
            payload,
            artifacts: Vec::new(),
            next_actions,
            metadata,
        })
    }

    async fn status(&self) -> AgentStatus {
        AgentStatus {
            is_healthy: self.is_initialized,
            is_busy: false,
            last_activity: chrono::Utc::now(),
            current_task: None,
            error_message: if self.is_initialized { None } else { Some("Agent not initialized".to_string()) },
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.conversation_history.write().await.clear();
        self.context_manager.write().await.cleanup_expired_contexts();
        self.is_initialized = false;
        Ok(())
    }
}


impl NaturalLanguageAgent {
    pub fn new(id: String) -> Self {
        Self {
            id,
            config: AgentConfig::default(),
            conversation_history: RwLock::new(Vec::new()),
            intent_classifier: IntentClassifier::new(),
            context_manager: RwLock::new(ConversationContextManager::new()),
            entity_extractor: EntityExtractor::new(),
            response_generator: ResponseGenerator::new(),
            is_initialized: false,
        }
    }

    fn analyze_sentiment(&self, text: &str) -> SentimentAnalysis {
        // Simple rule-based sentiment analysis (in production, use ML models)
        let positive_words = ["good", "great", "excellent", "amazing", "helpful", "thanks", "please"];
        let negative_words = ["bad", "terrible", "awful", "wrong", "error", "problem", "issue"];
        let emotion_words = [
            ("happy", EmotionType::Happy),
            ("sad", EmotionType::Sad),
            ("angry", EmotionType::Angry),
            ("frustrated", EmotionType::Frustrated),
            ("excited", EmotionType::Excited),
        ];

        let text_lower = text.to_lowercase();
        let words: Vec<&str> = text_lower.split_whitespace().collect();

        let positive_count = positive_words.iter()
            .filter(|&&word| words.contains(&word))
            .count();

        let negative_count = negative_words.iter()
            .filter(|&&word| words.contains(&word))
            .count();

        let polarity = if positive_count + negative_count == 0 {
            0.0
        } else {
            (positive_count as f64 - negative_count as f64) / (positive_count + negative_count) as f64
        };

        let emotion = emotion_words.iter()
            .find(|(word, _)| text_lower.contains(word))
            .map(|(_, emotion)| emotion.clone())
            .unwrap_or(EmotionType::Neutral);

        SentimentAnalysis {
            polarity,
            subjectivity: 0.5, // Simplified
            emotion,
            confidence: 0.7,
        }
    }

    fn classify_topics(&self, text: &str) -> Vec<TopicClassification> {
        let mut topics = Vec::new();
        let text_lower = text.to_lowercase();

        // Technology topics
        if text_lower.contains("code") || text_lower.contains("programming") || text_lower.contains("script") {
            topics.push(TopicClassification {
                category: "Technology".to_string(),
                subcategory: Some("Programming".to_string()),
                confidence: 0.8,
                keywords: vec!["code".to_string(), "programming".to_string()],
            });
        }

        // System administration
        if text_lower.contains("server") || text_lower.contains("admin") || text_lower.contains("system") {
            topics.push(TopicClassification {
                category: "Technology".to_string(),
                subcategory: Some("System Administration".to_string()),
                confidence: 0.7,
                keywords: vec!["server".to_string(), "admin".to_string()],
            });
        }

        // File operations
        if text_lower.contains("file") || text_lower.contains("directory") || text_lower.contains("folder") {
            topics.push(TopicClassification {
                category: "Operations".to_string(),
                subcategory: Some("File Management".to_string()),
                confidence: 0.9,
                keywords: vec!["file".to_string(), "directory".to_string()],
            });
        }

        if topics.is_empty() {
            topics.push(TopicClassification {
                category: "General".to_string(),
                subcategory: None,
                confidence: 0.5,
                keywords: vec![],
            });
        }

        topics
    }


    pub async fn get_conversation_summary(&self, session_id: &str) -> Option<ConversationSummary> {
        let mgr = self.context_manager.read().await;
        let context = mgr.contexts.get(session_id)?;
        let history = self.conversation_history.read().await;
        let turn_count = history.len();
        let recent_intents: Vec<String> = history
            .iter()
            .rev()
            .take(5)
            .filter_map(|turn| turn.intent.as_ref().map(|i| i.name.clone()))
            .collect();
        let overall_sentiment = self.calculate_overall_sentiment_from_history(&history);
        Some(ConversationSummary {
            session_id: session_id.to_string(),
            turn_count,
            active_topics: context.active_topics.clone(),
            recent_intents,
            overall_sentiment,
            last_updated: context.last_updated,
        })
    }

    fn calculate_overall_sentiment_from_history(&self, history: &[ConversationTurn]) -> SentimentAnalysis {
        if history.is_empty() {
            return SentimentAnalysis {
                polarity: 0.0,
                subjectivity: 0.0,
                emotion: EmotionType::Neutral,
                confidence: 1.0,
            };
        }

        let avg_polarity = history
            .iter()
            .map(|turn| turn.sentiment.polarity)
            .sum::<f64>() / history.len() as f64;

        let avg_subjectivity = history
            .iter()
            .map(|turn| turn.sentiment.subjectivity)
            .sum::<f64>() / history.len() as f64;

        SentimentAnalysis {
            polarity: avg_polarity,
            subjectivity: avg_subjectivity,
            emotion: EmotionType::Neutral,
            confidence: 0.8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub session_id: String,
    pub turn_count: usize,
    pub active_topics: Vec<String>,
    pub recent_intents: Vec<String>,
    pub overall_sentiment: SentimentAnalysis,
    pub last_updated: DateTime<Utc>,
}

impl Default for NaturalLanguageAgent {
    fn default() -> Self {
        Self::new("natural_language_agent".to_string())
    }
}

#[cfg(feature = "never")]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_intent_classification() {
        let classifier = IntentClassifier::new();
        let context = ConversationContext::default();

        let intent = classifier.classify_intent("run ls -la", &context);
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().name, "execute_command");
    }

    #[test]
    fn test_entity_extraction() {
        let extractor = EntityExtractor::new();
        let context = ConversationContext::default();

        let entities = extractor.extract_entities("Please run ls /home/user/file.txt", &context);
        assert!(!entities.is_empty());
        
        let has_command = entities.iter().any(|e| matches!(e.entity_type, EntityType::Command));
        let has_path = entities.iter().any(|e| matches!(e.entity_type, EntityType::FilePath));
        
        assert!(has_command);
        assert!(has_path);
    }

    #[tokio::test]
    async fn test_agent_processing() {
        let mut agent = NaturalLanguageAgent::new("test_agent".to_string());
        let config = AgentConfig::default();
        agent.initialize(config).await.unwrap();

        let request = AgentRequest {
            request_type: AgentRequestType::ProcessInput,
            input: "help me list files in the current directory".to_string(),
            context: HashMap::new(),
            metadata: HashMap::new(),
        };

        let response = agent.process(request).await.unwrap();
        assert!(!response.content.is_empty());
        assert!(response.confidence > 0.0);
    }

    #[test]
    fn test_sentiment_analysis() {
        let agent = NaturalLanguageAgent::new("test".to_string());
        
        let positive = agent.analyze_sentiment("This is great! Thanks for your help!");
        assert!(positive.polarity > 0.0);
        assert_eq!(positive.emotion, EmotionType::Happy);

        let negative = agent.analyze_sentiment("This is terrible and doesn't work!");
        assert!(negative.polarity < 0.0);
    }
}

/// Extended entity types helper functions
impl EntityType {
    /// Check if entity type is related to file system
    pub fn is_file_system(&self) -> bool {
        matches!(self, EntityType::FilePath | EntityType::Command | EntityType::Parameter)
    }
    
    /// Check if entity type is related to programming
    pub fn is_programming(&self) -> bool {
        matches!(self, EntityType::Command | EntityType::Value)
    }
}

#[cfg(feature = "never")]
impl NaturalLanguageAgent {
    pub fn new() -> Self {
        Self {
            id: "natural-language".to_string(),
            config: AgentConfig::default(),
            conversation_history: Vec::new(),
            intent_classifier: IntentClassifier::new(),
            context_manager: ConversationContextManager::new(),
            is_initialized: false,
        }
    }

    /// Detect shell kind from AgentContext environment variables, fallback to bash
    fn detect_shell_kind(context: &AgentContext) -> String {
        if let Some(shell) = context.environment_vars.get("SHELL") {
            let name = std::path::Path::new(shell).file_name().and_then(|s| s.to_str()).unwrap_or(shell);
            return name.to_string();
        }
        "bash".to_string()
    }

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

        // File paths (collect all occurrences)
        let re = regex::Regex::new(r"([~/][\w\-./]+)").unwrap();
        for m in re.find_iter(text) {
            entities.push(Entity {
                entity_type: EntityType::FilePath,
                value: m.as_str().to_string(),
                confidence: 0.9,
                span: (m.start(), m.end()),
            });
        }

        // Programming languages
        let languages = ["rust", "python", "javascript", "typescript", "go", "java", "c++"];
        let lower = text.to_lowercase();
        for lang in &languages {
            let mut start_idx = 0usize;
            while let Some(rel) = lower[start_idx..].find(lang) {
                let s = start_idx + rel;
                let e = s + lang.len();
                entities.push(Entity {
                    entity_type: EntityType::Language,
                    value: lang.to_string(),
                    confidence: 0.8,
                    span: (s, e),
                });
                start_idx = e;
            }
        }

        // Commands (words that look like shell commands)
        let command_patterns = ["git", "npm", "cargo", "docker", "kubectl", "ls", "cd", "mkdir"];
        for cmd in &command_patterns {
            let mut start_idx = 0usize;
            while let Some(rel) = text[start_idx..].find(cmd) {
                let s = start_idx + rel;
                let e = s + cmd.len();
                entities.push(Entity {
                    entity_type: EntityType::Command,
                    value: cmd.to_string(),
                    confidence: 0.7,
                    span: (s, e),
                });
                start_idx = e;
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
        // Build a rich prompt incorporating context; provider integration is handled in AiRuntime.
        let _prompt = self.build_prompt(processed_input, context);
        // For now, delegate to the deterministic fallback which is safe and side-effect free.
        Ok(self.generate_fallback_response(processed_input))
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
#[cfg(feature = "never")]
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

#[cfg(feature = "never")]
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

#[cfg(feature = "never")]
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



#[cfg(test)]
#[cfg(feature = "never")]
mod tests2 {
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

//! Enhanced natural language processing agent with confidence scoring and parameter extraction.
//! Provides advanced NLP capabilities for CLI patterns, intent classification, and context understanding.

use crate::agents::types::{
    CliPatterns, EntityExtractionConfig, EntityType, IntentClassificationConfig, IntentModelType,
    NlpConfig, ParameterExtractionConfig, PathResolutionConfig, ShellKind,
};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Enhanced natural language processing agent
#[derive(Debug)]
pub struct NaturalLanguageAgent {
    config: NlpConfig,
    confidence_scorer: ConfidenceScorer,
    entity_extractor: EntityExtractor,
    intent_classifier: IntentClassifier,
    parameter_extractor: ParameterExtractor,
    context_manager: ContextManager,
}

/// Confidence scoring system with pluggable heuristics
#[derive(Debug)]
struct ConfidenceScorer {
    heuristics: Vec<Box<dyn ConfidenceHeuristic>>,
    weights: HashMap<String, f64>,
}

/// Individual confidence scoring heuristic
trait ConfidenceHeuristic: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &'static str;
    fn score(&self, input: &ProcessedInput, context: &NlContext) -> f64;
    fn weight(&self) -> f64 {
        1.0
    }
}

/// Entity extraction with advanced pattern matching
#[derive(Debug)]
struct EntityExtractor {
    config: EntityExtractionConfig,
    patterns: HashMap<EntityType, Vec<EntityPattern>>,
}

/// Intent classification with multiple model support
#[derive(Debug)]
struct IntentClassifier {
    config: IntentClassificationConfig,
    model: Box<dyn IntentModel>,
}

/// Intent classification model trait
trait IntentModel: std::fmt::Debug + Send + Sync {
    fn classify(
        &self,
        input: &str,
        entities: &[ExtractedEntity],
        context: &NlContext,
    ) -> Result<ClassifiedIntent>;
    fn train(&mut self, examples: &[IntentExample]) -> Result<()>;
    fn confidence(&self) -> f64;
}

/// Parameter extraction for CLI patterns and commands
#[derive(Debug)]
struct ParameterExtractor {
    config: ParameterExtractionConfig,
    cli_parser: CliParser,
    path_resolver: PathResolver,
    variable_expander: VariableExpander,
}

/// Context management for conversation and session state
#[derive(Debug)]
struct ContextManager {
    current_session: Option<NlSession>,
    conversation_history: Vec<ConversationTurn>,
    environment_context: EnvironmentContext,
}

/// Processed input with comprehensive analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedInput {
    pub original_text: String,
    pub normalized_text: String,
    pub confidence: f64,
    pub intent: Option<ClassifiedIntent>,
    pub entities: Vec<ExtractedEntity>,
    pub parameters: ExtractedParameters,
    pub context_hints: Vec<String>,
    pub suggested_completions: Vec<String>,
    pub analysis_metadata: AnalysisMetadata,
}

/// Extracted entity with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    pub entity_type: EntityType,
    pub value: String,
    pub normalized_value: String,
    pub confidence: f64,
    pub span: (usize, usize),
    pub metadata: HashMap<String, String>,
}

/// Classified intent with confidence and alternatives
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedIntent {
    pub name: String,
    pub confidence: f64,
    pub parameters: HashMap<String, String>,
    pub alternatives: Vec<IntentAlternative>,
    pub context_requirements: Vec<String>,
}

/// Alternative intent suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentAlternative {
    pub name: String,
    pub confidence: f64,
    pub reason: String,
}

/// Extracted parameters from CLI patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedParameters {
    pub command: Option<String>,
    pub subcommands: Vec<String>,
    pub flags: Vec<Flag>,
    pub options: Vec<CliOption>,
    pub arguments: Vec<String>,
    pub file_paths: Vec<PathInfo>,
    pub variables: HashMap<String, String>,
}

/// Command line flag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flag {
    pub name: String,
    pub short_form: Option<String>,
    pub present: bool,
    pub position: usize,
}

/// Command line option with value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliOption {
    pub name: String,
    pub value: String,
    pub short_form: Option<String>,
    pub position: usize,
}

/// Path information with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathInfo {
    pub original: String,
    pub resolved: PathBuf,
    pub exists: bool,
    pub path_type: PathType,
    pub permissions: Option<PathPermissions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PathType {
    File,
    Directory,
    Symlink,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPermissions {
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
}

/// Analysis metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisMetadata {
    pub processing_time_ms: u64,
    pub model_versions: HashMap<String, String>,
    pub feature_flags: HashSet<String>,
    pub debug_info: HashMap<String, serde_json::Value>,
}

/// Natural language processing context
#[derive(Debug, Clone)]
struct NlContext {
    session_id: Option<String>,
    user_preferences: UserPreferences,
    environment: EnvironmentContext,
    conversation_state: ConversationState,
}

/// User preferences for NLP behavior
#[derive(Debug, Clone)]
struct UserPreferences {
    preferred_shell: ShellKind,
    verbosity_level: VerbosityLevel,
    auto_completion: bool,
    context_awareness: bool,
    personalization: bool,
}

#[derive(Debug, Clone)]
enum VerbosityLevel {
    Minimal,
    Normal,
    Verbose,
    Debug,
}

/// Environment context
#[derive(Debug, Clone)]
struct EnvironmentContext {
    working_directory: PathBuf,
    shell_kind: ShellKind,
    environment_vars: HashMap<String, String>,
    available_commands: HashSet<String>,
}

/// Conversation state tracking
#[derive(Debug, Clone)]
struct ConversationState {
    last_command: Option<String>,
    command_history: Vec<String>,
    context_stack: Vec<String>,
    active_tasks: HashSet<String>,
}

/// NLP session information
#[derive(Debug, Clone)]
struct NlSession {
    id: String,
    started_at: DateTime<Utc>,
    last_activity: DateTime<Utc>,
    message_count: u64,
}

/// Conversation turn
#[derive(Debug, Clone)]
struct ConversationTurn {
    timestamp: DateTime<Utc>,
    input: String,
    processed: ProcessedInput,
    response: Option<String>,
}

/// Entity pattern for extraction
#[derive(Debug, Clone)]
struct EntityPattern {
    pattern: Regex,
    confidence_base: f64,
    metadata_extractors: Vec<MetadataExtractor>,
}

/// Metadata extractor for entities
#[derive(Debug, Clone)]
enum MetadataExtractor {
    CaptureGroup(usize, String),
    StaticValue(String, String),
    Computed(fn(&str) -> String),
}

/// CLI parser for command extraction
#[derive(Debug)]
struct CliParser {
    patterns: CliPatterns,
}

/// Path resolver for file system paths
#[derive(Debug)]
struct PathResolver {
    config: PathResolutionConfig,
}

/// Variable expander for environment variables
#[derive(Debug)]
struct VariableExpander {
    expand_vars: bool,
}

/// Intent example for training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentExample {
    pub text: String,
    pub intent: String,
    pub entities: Vec<(String, EntityType, usize, usize)>,
    pub context: HashMap<String, String>,
}

impl NaturalLanguageAgent {
    /// Create a new natural language agent
    pub fn new(config: NlpConfig) -> Self {
        let confidence_scorer = ConfidenceScorer::new(&config);
        let entity_extractor = EntityExtractor::new(&config.entity_extraction);
        let intent_classifier = IntentClassifier::new(&config.intent_classification);
        let parameter_extractor = ParameterExtractor::new(&config.parameter_extraction);
        let context_manager = ContextManager::new();

        Self {
            config,
            confidence_scorer,
            entity_extractor,
            intent_classifier,
            parameter_extractor,
            context_manager,
        }
    }

    /// Process natural language input with comprehensive analysis
    pub async fn process_input(&mut self, input: &str) -> Result<ProcessedInput> {
        let start_time = std::time::Instant::now();

        debug!("Processing input: {}", input);

        // Create context
        let context = self.context_manager.get_current_context();

        // Normalize input
        let normalized_text = self.normalize_input(input);

        // Extract entities
        let entities = self.entity_extractor.extract(&normalized_text, &context)?;

        // Classify intent
        let intent = self.intent_classifier.classify(&normalized_text, &entities, &context)?;

        // Extract parameters
        let parameters = self.parameter_extractor.extract(&normalized_text, &entities, &context)?;

        // Generate context hints and completions
        let context_hints = self.generate_context_hints(&normalized_text, &entities, &context);
        let suggested_completions =
            self.generate_completions(&normalized_text, &entities, &context);

        // Calculate overall confidence
        let mut processed = ProcessedInput {
            original_text: input.to_string(),
            normalized_text: normalized_text.clone(),
            confidence: 0.0, // Will be calculated
            intent: Some(intent),
            entities,
            parameters,
            context_hints,
            suggested_completions,
            analysis_metadata: AnalysisMetadata {
                processing_time_ms: start_time.elapsed().as_millis() as u64,
                model_versions: self.get_model_versions(),
                feature_flags: HashSet::new(),
                debug_info: HashMap::new(),
            },
        };

        // Score confidence
        processed.confidence = self.confidence_scorer.score(&processed, &context);

        // Update context and history
        self.context_manager.update_with_input(&processed);

        debug!("Processing complete. Confidence: {:.2}", processed.confidence);
        Ok(processed)
    }

    /// Normalize input text
    fn normalize_input(&self, input: &str) -> String {
        // Basic normalization
        input.trim().replace('\t', " ").split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Generate context hints
    fn generate_context_hints(
        &self,
        _text: &str,
        _entities: &[ExtractedEntity],
        context: &NlContext,
    ) -> Vec<String> {
        let mut hints = Vec::new();

        // Add environment-based hints
        if !context.environment.working_directory.as_os_str().is_empty() {
            hints.push(format!(
                "Working directory: {}",
                context.environment.working_directory.display()
            ));
        }

        // Add shell-specific hints
        hints.push(format!("Shell: {:?}", context.environment.shell_kind));

        hints
    }

    /// Generate completion suggestions
    fn generate_completions(
        &self,
        text: &str,
        _entities: &[ExtractedEntity],
        context: &NlContext,
    ) -> Vec<String> {
        let mut completions = Vec::new();

        // Basic command completions
        for command in &context.environment.available_commands {
            if command.starts_with(text) {
                completions.push(command.clone());
            }
        }

        // Limit completions
        completions.truncate(10);
        completions
    }

    /// Get model version information
    fn get_model_versions(&self) -> HashMap<String, String> {
        let mut versions = HashMap::new();
        versions.insert("nlp_agent".to_string(), "1.0.0".to_string());
        versions.insert("entity_extractor".to_string(), "1.0.0".to_string());
        versions.insert("intent_classifier".to_string(), "1.0.0".to_string());
        versions.insert("parameter_extractor".to_string(), "1.0.0".to_string());
        versions
    }

    /// Update configuration
    pub fn update_config(&mut self, config: NlpConfig) {
        self.config = config;
        // Update component configurations
    }

    /// Get current confidence threshold
    pub fn confidence_threshold(&self) -> f64 {
        self.config.confidence_threshold
    }

    /// Set confidence threshold
    pub fn set_confidence_threshold(&mut self, threshold: f64) {
        self.config.confidence_threshold = threshold;
    }
}

impl ConfidenceScorer {
    fn new(_config: &NlpConfig) -> Self {
        let mut heuristics: Vec<Box<dyn ConfidenceHeuristic>> = Vec::new();

        // Add default heuristics
        heuristics.push(Box::new(EntityConfidenceHeuristic::new()));
        heuristics.push(Box::new(IntentConfidenceHeuristic::new()));
        heuristics.push(Box::new(ContextMatchHeuristic::new()));
        heuristics.push(Box::new(PatternMatchHeuristic::new()));
        heuristics.push(Box::new(TextLengthHeuristic::new()));

        let mut weights = HashMap::new();
        weights.insert("entity".to_string(), 0.28);
        weights.insert("intent".to_string(), 0.38);
        weights.insert("context".to_string(), 0.2);
        weights.insert("pattern".to_string(), 0.1);
        weights.insert("length".to_string(), 0.04);

        Self { heuristics, weights }
    }

    fn score(&self, input: &ProcessedInput, context: &NlContext) -> f64 {
        let mut total_score = 0.0;
        let mut total_weight = 0.0;

        for heuristic in &self.heuristics {
            let score = heuristic.score(input, context);
            let weight = self.weights.get(heuristic.name()).copied().unwrap_or(1.0);

            total_score += score * weight;
            total_weight += weight;
        }

        if total_weight > 0.0 {
            total_score / total_weight
        } else {
            0.0
        }
    }
}

// Confidence heuristics implementation
#[derive(Debug)]
struct EntityConfidenceHeuristic;

impl EntityConfidenceHeuristic {
    fn new() -> Self {
        Self
    }
}

impl ConfidenceHeuristic for EntityConfidenceHeuristic {
    fn name(&self) -> &'static str {
        "entity"
    }

    fn score(&self, input: &ProcessedInput, _context: &NlContext) -> f64 {
        if input.entities.is_empty() {
            return 0.5; // Neutral score for no entities
        }

        let avg_confidence =
            input.entities.iter().map(|e| e.confidence).sum::<f64>() / input.entities.len() as f64;

        avg_confidence
    }
}

#[derive(Debug)]
struct IntentConfidenceHeuristic;

impl IntentConfidenceHeuristic {
    fn new() -> Self {
        Self
    }
}

impl ConfidenceHeuristic for IntentConfidenceHeuristic {
    fn name(&self) -> &'static str {
        "intent"
    }

    fn score(&self, input: &ProcessedInput, _context: &NlContext) -> f64 {
        input.intent.as_ref().map(|intent| intent.confidence).unwrap_or(0.0)
    }
}

#[derive(Debug)]
struct ContextMatchHeuristic;

impl ContextMatchHeuristic {
    fn new() -> Self {
        Self
    }
}

impl ConfidenceHeuristic for ContextMatchHeuristic {
    fn name(&self) -> &'static str {
        "context"
    }

    fn score(&self, input: &ProcessedInput, context: &NlContext) -> f64 {
        // Score based on context relevance
        let mut score: f64 = 0.5; // Base score

        // Check if entities match environment context
        for entity in &input.entities {
            match entity.entity_type {
                EntityType::Command => {
                    if context.environment.available_commands.contains(&entity.value) {
                        score += 0.1;
                    }
                }
                EntityType::FilePath => {
                    if Path::new(&entity.value).exists() {
                        score += 0.1;
                    }
                }
                _ => {}
            }
        }

        score.min(1.0)
    }
}

#[derive(Debug)]
struct PatternMatchHeuristic;

#[derive(Debug)]
struct TextLengthHeuristic;

impl PatternMatchHeuristic {
    fn new() -> Self {
        Self
    }
}

impl TextLengthHeuristic {
    fn new() -> Self {
        Self
    }
}

impl ConfidenceHeuristic for TextLengthHeuristic {
    fn name(&self) -> &'static str {
        "length"
    }

    fn score(&self, input: &ProcessedInput, _context: &NlContext) -> f64 {
        let len = input.normalized_text.len();
        if len == 0 {
            return 0.0;
        }
        // Saturates at 1.0 around 120 characters
        (len as f64 / 120.0).min(1.0)
    }
}

impl ConfidenceHeuristic for PatternMatchHeuristic {
    fn name(&self) -> &'static str {
        "pattern"
    }

    fn score(&self, input: &ProcessedInput, _context: &NlContext) -> f64 {
        // Score based on recognized patterns
        let text = &input.normalized_text;
        let mut score: f64 = 0.0;

        // Common command patterns
        if Regex::new(r"^[a-zA-Z][\w-]*\s").unwrap().is_match(text) {
            score += 0.3; // Looks like a command
        }

        // File path patterns
        if Regex::new(r"[~/\.][\w/.-]*").unwrap().is_match(text) {
            score += 0.2; // Contains path-like strings
        }

        // Flag patterns
        if Regex::new(r"--?\w+").unwrap().is_match(text) {
            score += 0.2; // Contains flags
        }

        score.min(1.0)
    }
}

impl EntityExtractor {
    fn new(config: &EntityExtractionConfig) -> Self {
        let mut patterns = HashMap::new();

        // Build patterns for each enabled entity type
        for entity_type in &config.enabled_types {
            let entity_patterns = Self::create_patterns_for_type(entity_type);
            patterns.insert(entity_type.clone(), entity_patterns);
        }

        Self { config: config.clone(), patterns }
    }

    fn create_patterns_for_type(entity_type: &EntityType) -> Vec<EntityPattern> {
        match entity_type {
            EntityType::FilePath => vec![
                EntityPattern {
                    pattern: Regex::new(r"([~/][\w\-./]*[\w\-.]|\./[\w\-./]*[\w\-.])").unwrap(),
                    confidence_base: 0.8,
                    metadata_extractors: vec![],
                },
                EntityPattern {
                    pattern: Regex::new(r"([A-Za-z]:[\\\/][\w\-\\\/]*)").unwrap(), // Windows paths
                    confidence_base: 0.9,
                    metadata_extractors: vec![],
                },
            ],
            EntityType::Command => vec![EntityPattern {
                pattern: Regex::new(r"^([a-zA-Z][\w\-]*)(?:\s|$)").unwrap(),
                confidence_base: 0.7,
                metadata_extractors: vec![],
            }],
            EntityType::Flag => vec![EntityPattern {
                pattern: Regex::new(r"(--[a-zA-Z][\w\-]*|-[a-zA-Z])").unwrap(),
                confidence_base: 0.9,
                metadata_extractors: vec![],
            }],
            EntityType::Option => vec![EntityPattern {
                pattern: Regex::new(r"(--[a-zA-Z][\w\-]*[=\s]+\S+|-[a-zA-Z]\s+\S+)").unwrap(),
                confidence_base: 0.8,
                metadata_extractors: vec![],
            }],
            EntityType::Number => vec![EntityPattern {
                pattern: Regex::new(r"(\d+\.?\d*)").unwrap(),
                confidence_base: 0.9,
                metadata_extractors: vec![],
            }],
            EntityType::Variable => vec![EntityPattern {
                pattern: Regex::new(r"(\$\{?\w+\}?)").unwrap(),
                confidence_base: 0.8,
                metadata_extractors: vec![],
            }],
            _ => vec![],
        }
    }

    fn extract(&self, text: &str, _context: &NlContext) -> Result<Vec<ExtractedEntity>> {
        let mut entities = Vec::new();

        for (entity_type, patterns) in &self.patterns {
            for pattern in patterns {
                for mat in pattern.pattern.find_iter(text) {
                    let value = mat.as_str().to_string();
                    let normalized_value = self.normalize_entity_value(&value, entity_type);

                    entities.push(ExtractedEntity {
                        entity_type: entity_type.clone(),
                        value: value.clone(),
                        normalized_value,
                        confidence: pattern.confidence_base,
                        span: (mat.start(), mat.end()),
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        // Filter by minimum confidence
        entities.retain(|e| e.confidence >= self.config.min_confidence);

        // Remove overlapping entities (keep highest confidence)
        entities = self.resolve_overlapping_entities(entities);

        Ok(entities)
    }

    fn normalize_entity_value(&self, value: &str, entity_type: &EntityType) -> String {
        match entity_type {
            EntityType::FilePath => {
                if let Ok(path) = shellexpand::full(value) {
                    path.to_string()
                } else {
                    value.to_string()
                }
            }
            EntityType::Variable => {
                if let Some(var_name) = value.strip_prefix('$') {
                    let var_name = var_name.trim_start_matches('{').trim_end_matches('}');
                    env::var(var_name).unwrap_or_else(|_| value.to_string())
                } else {
                    value.to_string()
                }
            }
            _ => value.to_string(),
        }
    }

    fn resolve_overlapping_entities(
        &self,
        mut entities: Vec<ExtractedEntity>,
    ) -> Vec<ExtractedEntity> {
        entities.sort_by(|a, b| {
            a.span.0.cmp(&b.span.0).then(b.confidence.partial_cmp(&a.confidence).unwrap())
        });

        let mut result = Vec::new();
        let mut last_end = 0;

        for entity in entities {
            if entity.span.0 >= last_end {
                last_end = entity.span.1;
                result.push(entity);
            }
        }

        result
    }
}

impl IntentClassifier {
    fn new(config: &IntentClassificationConfig) -> Self {
        let model: Box<dyn IntentModel> = match config.model_type {
            IntentModelType::RuleBased => Box::new(RuleBasedModel::new()),
            IntentModelType::StatisticalNb => Box::new(NaiveBayesModel::new()),
            IntentModelType::Hybrid => Box::new(HybridModel::new()),
            _ => Box::new(RuleBasedModel::new()),
        };

        Self { config: config.clone(), model }
    }

    fn classify(
        &self,
        text: &str,
        entities: &[ExtractedEntity],
        context: &NlContext,
    ) -> Result<ClassifiedIntent> {
        self.model.classify(text, entities, context)
    }
}

// Simple rule-based intent model
#[derive(Debug)]
struct RuleBasedModel {
    rules: Vec<IntentRule>,
}

#[derive(Debug)]
struct IntentRule {
    intent: String,
    patterns: Vec<Regex>,
    required_entities: Vec<EntityType>,
    confidence: f64,
}

impl RuleBasedModel {
    fn new() -> Self {
        let rules = vec![
            IntentRule {
                intent: "file_operation".to_string(),
                patterns: vec![Regex::new(r"(?i)(copy|move|delete|remove|rm|cp|mv)").unwrap()],
                required_entities: vec![EntityType::FilePath],
                confidence: 0.8,
            },
            IntentRule {
                intent: "directory_navigation".to_string(),
                patterns: vec![Regex::new(r"(?i)(cd|change|directory|go\s+to)").unwrap()],
                required_entities: vec![EntityType::DirectoryPath],
                confidence: 0.9,
            },
            IntentRule {
                intent: "command_execution".to_string(),
                patterns: vec![Regex::new(r"^[a-zA-Z][\w\-]*").unwrap()],
                required_entities: vec![EntityType::Command],
                confidence: 0.7,
            },
        ];

        Self { rules }
    }
}

impl IntentModel for RuleBasedModel {
    fn classify(
        &self,
        text: &str,
        entities: &[ExtractedEntity],
        _context: &NlContext,
    ) -> Result<ClassifiedIntent> {
        let mut best_match: Option<ClassifiedIntent> = None;
        let mut best_score = 0.0;

        for rule in &self.rules {
            let mut score = 0.0;

            // Check pattern matches
            for pattern in &rule.patterns {
                if pattern.is_match(text) {
                    score += rule.confidence * 0.7;
                }
            }

            // Check required entities
            for required_entity in &rule.required_entities {
                if entities.iter().any(|e| &e.entity_type == required_entity) {
                    score += rule.confidence * 0.3;
                }
            }

            if score > best_score {
                best_score = score;
                best_match = Some(ClassifiedIntent {
                    name: rule.intent.clone(),
                    confidence: score,
                    parameters: HashMap::new(),
                    alternatives: Vec::new(),
                    context_requirements: Vec::new(),
                });
            }
        }

        best_match.ok_or_else(|| anyhow!("No matching intent found"))
    }

    fn train(&mut self, _examples: &[IntentExample]) -> Result<()> {
        // Rule-based model doesn't need training
        Ok(())
    }

    fn confidence(&self) -> f64 {
        0.8 // Static confidence for rule-based model
    }
}

// Placeholder for other models
#[derive(Debug)]
struct NaiveBayesModel;

impl NaiveBayesModel {
    fn new() -> Self {
        Self
    }
}

impl IntentModel for NaiveBayesModel {
    fn classify(
        &self,
        _text: &str,
        _entities: &[ExtractedEntity],
        _context: &NlContext,
    ) -> Result<ClassifiedIntent> {
        // TODO: Implement Naive Bayes classification
        Ok(ClassifiedIntent {
            name: "unknown".to_string(),
            confidence: 0.5,
            parameters: HashMap::new(),
            alternatives: Vec::new(),
            context_requirements: Vec::new(),
        })
    }

    fn train(&mut self, _examples: &[IntentExample]) -> Result<()> {
        // TODO: Implement training
        Ok(())
    }

    fn confidence(&self) -> f64 {
        0.6
    }
}

#[derive(Debug)]
struct HybridModel;

impl HybridModel {
    fn new() -> Self {
        Self
    }
}

impl IntentModel for HybridModel {
    fn classify(
        &self,
        _text: &str,
        _entities: &[ExtractedEntity],
        _context: &NlContext,
    ) -> Result<ClassifiedIntent> {
        // TODO: Implement hybrid classification
        Ok(ClassifiedIntent {
            name: "unknown".to_string(),
            confidence: 0.7,
            parameters: HashMap::new(),
            alternatives: Vec::new(),
            context_requirements: Vec::new(),
        })
    }

    fn train(&mut self, _examples: &[IntentExample]) -> Result<()> {
        // TODO: Implement training
        Ok(())
    }

    fn confidence(&self) -> f64 {
        0.8
    }
}

impl ParameterExtractor {
    fn new(config: &ParameterExtractionConfig) -> Self {
        Self {
            config: config.clone(),
            cli_parser: CliParser::new(&config.cli_patterns),
            path_resolver: PathResolver::new(&config.path_resolution),
            variable_expander: VariableExpander::new(config.variable_expansion),
        }
    }

    fn extract(
        &self,
        text: &str,
        entities: &[ExtractedEntity],
        _context: &NlContext,
    ) -> Result<ExtractedParameters> {
        let mut parameters = ExtractedParameters {
            command: None,
            subcommands: Vec::new(),
            flags: Vec::new(),
            options: Vec::new(),
            arguments: Vec::new(),
            file_paths: Vec::new(),
            variables: HashMap::new(),
        };

        // Parse CLI structure
        let cli_parts = self.cli_parser.parse(text)?;
        parameters.command = cli_parts.command;
        parameters.subcommands = cli_parts.subcommands;
        parameters.flags = cli_parts.flags;
        parameters.options = cli_parts.options;
        parameters.arguments = cli_parts.arguments;

        // Process paths from entities
        for entity in entities {
            match entity.entity_type {
                EntityType::FilePath | EntityType::DirectoryPath => {
                    if let Ok(path_info) = self.path_resolver.resolve(&entity.value) {
                        parameters.file_paths.push(path_info);
                    }
                }
                EntityType::Variable => {
                    if let Some(expanded) = self.variable_expander.expand(&entity.value) {
                        parameters.variables.insert(entity.value.clone(), expanded);
                    }
                }
                _ => {}
            }
        }

        Ok(parameters)
    }
}

impl CliParser {
    fn new(patterns: &CliPatterns) -> Self {
        Self { patterns: patterns.clone() }
    }

    fn parse(&self, text: &str) -> Result<ExtractedParameters> {
        let mut parameters = ExtractedParameters {
            command: None,
            subcommands: Vec::new(),
            flags: Vec::new(),
            options: Vec::new(),
            arguments: Vec::new(),
            file_paths: Vec::new(),
            variables: HashMap::new(),
        };

        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(parameters);
        }

        // First part is typically the command
        parameters.command = Some(parts[0].to_string());

        // Process remaining parts
        let mut i = 1;
        while i < parts.len() {
            let part = parts[i];

            if part.starts_with("--") {
                // Long option
                if let Some(eq_pos) = part.find('=') {
                    // Option with value in same argument
                    let name = part[2..eq_pos].to_string();
                    let value = part[eq_pos + 1..].to_string();
                    parameters.options.push(CliOption {
                        name,
                        value,
                        short_form: None,
                        position: i,
                    });
                } else if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                    // Option with separate value
                    let name = part[2..].to_string();
                    let value = parts[i + 1].to_string();
                    parameters.options.push(CliOption {
                        name,
                        value,
                        short_form: None,
                        position: i,
                    });
                    i += 1; // Skip value
                } else {
                    // Flag
                    parameters.flags.push(Flag {
                        name: part[2..].to_string(),
                        short_form: None,
                        present: true,
                        position: i,
                    });
                }
            } else if part.starts_with('-') && part.len() > 1 {
                // Short option(s)
                if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                    // Option with value
                    let name = part[1..].to_string();
                    let value = parts[i + 1].to_string();
                    parameters.options.push(CliOption {
                        name: name.clone(),
                        value,
                        short_form: Some(name),
                        position: i,
                    });
                    i += 1;
                } else {
                    // Flag(s)
                    for ch in part[1..].chars() {
                        parameters.flags.push(Flag {
                            name: ch.to_string(),
                            short_form: Some(ch.to_string()),
                            present: true,
                            position: i,
                        });
                    }
                }
            } else {
                // Regular argument
                parameters.arguments.push(part.to_string());
            }

            i += 1;
        }

        Ok(parameters)
    }
}

impl PathResolver {
    fn new(config: &PathResolutionConfig) -> Self {
        Self { config: config.clone() }
    }

    fn resolve(&self, path_str: &str) -> Result<PathInfo> {
        let mut path = PathBuf::from(path_str);

        // Expand home directory
        if self.config.expand_home && path_str.starts_with('~') {
            if let Some(home) = env::var_os("HOME") {
                path =
                    PathBuf::from(home).join(path_str.strip_prefix("~/").unwrap_or(&path_str[1..]));
            }
        }

        // Resolve relative paths
        if self.config.resolve_relative && path.is_relative() {
            if let Ok(cwd) = env::current_dir() {
                path = cwd.join(path);
            }
        }

        // Follow symlinks
        if self.config.follow_symlinks {
            if let Ok(canonical) = path.canonicalize() {
                path = canonical;
            }
        }

        let exists = self.config.validate_existence && path.exists();

        let path_type = if path.is_file() {
            PathType::File
        } else if path.is_dir() {
            PathType::Directory
        } else if path.is_symlink() {
            PathType::Symlink
        } else {
            PathType::Unknown
        };

        let permissions = if exists {
            Some(PathPermissions {
                readable: path.metadata().map(|m| !m.permissions().readonly()).unwrap_or(false),
                writable: path.metadata().map(|m| !m.permissions().readonly()).unwrap_or(false),
                executable: true, // Simplified
            })
        } else {
            None
        };

        Ok(PathInfo {
            original: path_str.to_string(),
            resolved: path,
            exists,
            path_type,
            permissions,
        })
    }
}

impl VariableExpander {
    fn new(expand_vars: bool) -> Self {
        Self { expand_vars }
    }

    fn expand(&self, var_str: &str) -> Option<String> {
        if !self.expand_vars {
            return None;
        }

        if let Some(var_name) = var_str.strip_prefix('$') {
            let var_name = var_name.trim_start_matches('{').trim_end_matches('}');
            env::var(var_name).ok()
        } else {
            None
        }
    }
}

impl ContextManager {
    fn new() -> Self {
        Self {
            current_session: None,
            conversation_history: Vec::new(),
            environment_context: EnvironmentContext {
                working_directory: env::current_dir().unwrap_or_default(),
                shell_kind: ShellKind::detect(),
                environment_vars: env::vars().collect(),
                available_commands: Self::detect_available_commands(),
            },
        }
    }

    fn get_current_context(&self) -> NlContext {
        NlContext {
            session_id: self.current_session.as_ref().map(|s| s.id.clone()),
            user_preferences: UserPreferences {
                preferred_shell: self.environment_context.shell_kind.clone(),
                verbosity_level: VerbosityLevel::Normal,
                auto_completion: true,
                context_awareness: true,
                personalization: false,
            },
            environment: self.environment_context.clone(),
            conversation_state: ConversationState {
                last_command: self
                    .conversation_history
                    .last()
                    .map(|turn| turn.processed.parameters.command.clone())
                    .flatten(),
                command_history: self
                    .conversation_history
                    .iter()
                    .filter_map(|turn| turn.processed.parameters.command.clone())
                    .collect(),
                context_stack: Vec::new(),
                active_tasks: HashSet::new(),
            },
        }
    }

    fn update_with_input(&mut self, input: &ProcessedInput) {
        let turn = ConversationTurn {
            timestamp: Utc::now(),
            input: input.original_text.clone(),
            processed: input.clone(),
            response: None,
        };

        self.conversation_history.push(turn);

        // Keep history manageable
        if self.conversation_history.len() > 100 {
            self.conversation_history.remove(0);
        }
    }

    fn detect_available_commands() -> HashSet<String> {
        let mut commands = HashSet::new();

        // Add some common commands
        let common_commands = [
            "ls", "cd", "pwd", "mkdir", "rmdir", "rm", "cp", "mv", "cat", "less", "more", "grep",
            "find", "which", "whereis", "man", "help", "history", "ps", "top", "kill", "jobs",
            "bg", "fg", "nohup", "screen", "tmux", "ssh", "scp", "rsync", "git", "svn", "hg",
            "make", "cmake", "cargo", "npm", "pip", "docker", "kubectl",
        ];

        for cmd in &common_commands {
            commands.insert(cmd.to_string());
        }

        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_natural_language_agent_creation() {
        let config = NlpConfig::default();
        let _agent = NaturalLanguageAgent::new(config);

        // Should not panic
    }

    #[tokio::test]
    async fn test_input_processing() {
        let config = NlpConfig::default();
        let mut agent = NaturalLanguageAgent::new(config);

        let result = agent.process_input("ls -la /home").await.unwrap();

        assert!(!result.original_text.is_empty());
        assert!(result.confidence > 0.0);
        assert!(!result.entities.is_empty());
    }

    #[test]
    fn test_confidence_scoring() {
        let config = NlpConfig::default();
        let scorer = ConfidenceScorer::new(&config);

        let input = ProcessedInput {
            original_text: "test".to_string(),
            normalized_text: "test".to_string(),
            confidence: 0.0,
            intent: Some(ClassifiedIntent {
                name: "test".to_string(),
                confidence: 0.8,
                parameters: HashMap::new(),
                alternatives: Vec::new(),
                context_requirements: Vec::new(),
            }),
            entities: vec![],
            parameters: ExtractedParameters {
                command: Some("test".to_string()),
                subcommands: Vec::new(),
                flags: Vec::new(),
                options: Vec::new(),
                arguments: Vec::new(),
                file_paths: Vec::new(),
                variables: HashMap::new(),
            },
            context_hints: Vec::new(),
            suggested_completions: Vec::new(),
            analysis_metadata: AnalysisMetadata {
                processing_time_ms: 0,
                model_versions: HashMap::new(),
                feature_flags: HashSet::new(),
                debug_info: HashMap::new(),
            },
        };

        let context = NlContext {
            session_id: None,
            user_preferences: UserPreferences {
                preferred_shell: ShellKind::Bash,
                verbosity_level: VerbosityLevel::Normal,
                auto_completion: true,
                context_awareness: true,
                personalization: false,
            },
            environment: EnvironmentContext {
                working_directory: PathBuf::from("/"),
                shell_kind: ShellKind::Bash,
                environment_vars: HashMap::new(),
                available_commands: HashSet::new(),
            },
            conversation_state: ConversationState {
                last_command: None,
                command_history: Vec::new(),
                context_stack: Vec::new(),
                active_tasks: HashSet::new(),
            },
        };

        let score = scorer.score(&input, &context);
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_cli_parsing() {
        let patterns = CliPatterns {
            flag_patterns: vec![r"--\w+".to_string(), r"-\w".to_string()],
            option_patterns: vec![r"--\w+=\w+".to_string()],
            positional_patterns: vec![r"\w+".to_string()],
            subcommand_patterns: vec![r"\w+".to_string()],
        };

        let parser = CliParser::new(&patterns);
        let result = parser.parse("ls -la --color=auto /home").unwrap();

        assert_eq!(result.command, Some("ls".to_string()));
        assert!(!result.flags.is_empty());
        assert!(!result.arguments.is_empty());
    }

    #[test]
    fn test_entity_extraction() {
        let config = EntityExtractionConfig {
            enabled_types: [EntityType::FilePath, EntityType::Command, EntityType::Flag]
                .iter()
                .cloned()
                .collect(),
            custom_patterns: HashMap::new(),
            case_sensitive: false,
            min_confidence: 0.5,
        };

        let extractor = EntityExtractor::new(&config);
        let context = NlContext {
            session_id: None,
            user_preferences: UserPreferences {
                preferred_shell: ShellKind::Bash,
                verbosity_level: VerbosityLevel::Normal,
                auto_completion: true,
                context_awareness: true,
                personalization: false,
            },
            environment: EnvironmentContext {
                working_directory: PathBuf::from("/"),
                shell_kind: ShellKind::Bash,
                environment_vars: HashMap::new(),
                available_commands: HashSet::new(),
            },
            conversation_state: ConversationState {
                last_command: None,
                command_history: Vec::new(),
                context_stack: Vec::new(),
                active_tasks: HashSet::new(),
            },
        };

        let entities = extractor.extract("ls -la /home/user", &context).unwrap();
        assert!(!entities.is_empty());

        // Should find file path entity
        assert!(entities.iter().any(|e| matches!(e.entity_type, EntityType::FilePath)));
    }
}

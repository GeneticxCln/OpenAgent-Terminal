//! Shared types for AI agents in the openagent-terminal-ai crate.
//! Core types, enums, and structures used across all agents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

/// Workflow execution graph for sequential and parallel task coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionGraph {
    pub id: Uuid,
    pub name: String,
    pub nodes: HashMap<String, WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub execution_strategy: ExecutionStrategy,
    pub status: WorkflowStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Individual node in a workflow execution graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub agent_id: Option<String>,
    pub dependencies: Vec<String>,
    pub status: NodeStatus,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
    pub timeout_ms: Option<u64>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub parallel_group: Option<String>,
}

/// Edge connecting workflow nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
    pub condition: Option<ExecutionCondition>,
    pub weight: f64,
}

/// Types of workflow nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    /// Execute a task with an agent
    Task { agent_capability: String, payload: serde_json::Value },
    /// Decision point in workflow
    Decision { condition_expr: String, true_branch: String, false_branch: String },
    /// Parallel execution group
    ParallelGroup { nodes: Vec<String>, join_strategy: JoinStrategy },
    /// Synchronization point
    Barrier { wait_for: Vec<String> },
    /// Start node
    Start,
    /// End node
    End,
}

/// Execution strategies for workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStrategy {
    /// Execute tasks sequentially in topological order
    Sequential,
    /// Execute independent tasks in parallel
    Parallel { max_concurrency: usize },
    /// Hybrid approach with parallel groups
    Hybrid,
    /// Custom execution logic
    Custom { executor_name: String },
}

/// Conditions for edge traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionCondition {
    /// Always execute
    Always,
    /// Execute if previous node succeeded
    OnSuccess,
    /// Execute if previous node failed
    OnFailure,
    /// Execute based on output value
    OutputEquals { key: String, value: serde_json::Value },
    /// Custom condition expression
    Custom { expression: String },
}

/// Strategies for joining parallel execution results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JoinStrategy {
    /// Wait for all parallel tasks to complete
    WaitAll,
    /// Wait for any task to complete successfully
    WaitAny,
    /// Wait for first N tasks to complete
    WaitFirst { count: usize },
    /// Custom join logic
    Custom { strategy_name: String },
}

/// Status of workflow execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Paused,
}

/// Status of individual workflow nodes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Skipped,
    Retrying,
}

/// Concurrency state management for preventing race conditions
#[derive(Debug, Clone)]
pub struct ConcurrencyState {
    /// Currently running operations by agent ID
    pub active_operations: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Lock registry for preventing overlapping operations
    pub operation_locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Semaphore>>>>,
    /// Resource usage tracking
    pub resource_usage: Arc<RwLock<ResourceUsage>>,
    /// Maximum concurrent operations per agent
    pub max_concurrent_ops: usize,
}

/// Resource usage tracking
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    pub cpu_usage: f64,
    pub memory_usage_mb: u64,
    pub active_threads: usize,
    pub queue_depth: usize,
}

/// Project context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContextInfo {
    pub working_directory: String,
    pub shell_kind: ShellKind,
    pub repository_info: Option<RepositoryInfo>,
    pub project_type: Option<ProjectType>,
    pub language_info: LanguageInfo,
    pub build_system: Option<BuildSystem>,
    pub environment_vars: HashMap<String, String>,
    pub cached_at: DateTime<Utc>,
    pub cache_ttl_seconds: u64,
}

/// Shell type information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Unknown(String),
}

/// Repository information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryInfo {
    pub vcs_type: VcsType,
    pub remote_url: Option<String>,
    pub current_branch: Option<String>,
    pub current_commit: Option<String>,
    pub status: RepoStatus,
    pub root_path: String,
}

/// Version control system types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VcsType {
    Git,
    Mercurial,
    Subversion,
    Bazaar,
    Unknown,
}

/// Repository status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStatus {
    pub is_clean: bool,
    pub modified_files: Vec<String>,
    pub untracked_files: Vec<String>,
    pub staged_files: Vec<String>,
    pub ahead: i32,
    pub behind: i32,
}

/// Project type detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectType {
    RustCargo,
    NodeJs,
    Python,
    Go,
    Java,
    CSharp,
    Cpp,
    Generic,
}

/// Language information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub primary_language: String,
    pub detected_languages: HashMap<String, f64>, // language -> percentage
    pub frameworks: Vec<String>,
    pub package_managers: Vec<String>,
}

/// Build system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuildSystem {
    Cargo,
    Npm,
    Yarn,
    Pnpm,
    Maven,
    Gradle,
    Make,
    CMake,
    Bazel,
    Custom(String),
}

/// Quality validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityConfig {
    pub enabled_checks: HashSet<QualityCheck>,
    pub security_level: SecurityLevel,
    pub performance_thresholds: PerformanceThresholds,
    pub style_rules: StyleRules,
    pub custom_rules: Vec<CustomRule>,
}

/// Types of quality checks
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub enum QualityCheck {
    Security,
    Performance,
    Style,
    Complexity,
    Documentation,
    Testing,
    Dependencies,
    Custom(String),
}

/// Security analysis levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityLevel {
    Basic,
    Standard,
    Strict,
    Custom(HashSet<String>),
}

/// Performance thresholds for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    pub max_complexity: u32,
    pub max_function_length: u32,
    pub max_file_size_kb: u32,
    pub min_test_coverage: f64,
}

/// Style rules configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleRules {
    pub enforce_formatting: bool,
    pub max_line_length: u32,
    pub indent_style: IndentStyle,
    pub naming_conventions: HashMap<String, NamingConvention>,
}

/// Indentation style preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndentStyle {
    Spaces(u8),
    Tabs,
    Mixed,
}

/// Naming convention rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NamingConvention {
    CamelCase,
    PascalCase,
    SnakeCase,
    KebabCase,
    UpperCase,
    LowerCase,
    Custom(String), // regex pattern
}

/// Custom validation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRule {
    pub name: String,
    pub description: String,
    pub pattern: String, // regex pattern
    pub severity: Severity,
    pub message: String,
}

/// Severity levels for issues
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Natural language processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlpConfig {
    pub confidence_threshold: f64,
    pub entity_extraction: EntityExtractionConfig,
    pub intent_classification: IntentClassificationConfig,
    pub parameter_extraction: ParameterExtractionConfig,
}

/// Entity extraction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityExtractionConfig {
    pub enabled_types: HashSet<EntityType>,
    pub custom_patterns: HashMap<String, String>,
    pub case_sensitive: bool,
    pub min_confidence: f64,
}

/// Intent classification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentClassificationConfig {
    pub model_type: IntentModelType,
    pub training_data: Vec<IntentExample>,
    pub fallback_intent: String,
    pub max_suggestions: usize,
}

/// Types of intent classification models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntentModelType {
    RuleBased,
    StatisticalNb, // Naive Bayes
    NeuralNetwork,
    Hybrid,
}

/// Training example for intent classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentExample {
    pub text: String,
    pub intent: String,
    pub entities: Vec<(String, EntityType, usize, usize)>, // value, type, start, end
    pub context: HashMap<String, String>,
}

/// Parameter extraction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterExtractionConfig {
    pub cli_patterns: CliPatterns,
    pub path_resolution: PathResolutionConfig,
    pub variable_expansion: bool,
    pub quote_handling: QuoteHandling,
}

/// CLI parameter patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliPatterns {
    pub flag_patterns: Vec<String>,       // regex patterns for flags
    pub option_patterns: Vec<String>,     // regex patterns for options
    pub positional_patterns: Vec<String>, // regex patterns for positional args
    pub subcommand_patterns: Vec<String>,
}

/// Path resolution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathResolutionConfig {
    pub resolve_relative: bool,
    pub expand_home: bool,
    pub validate_existence: bool,
    pub follow_symlinks: bool,
}

/// Quote handling strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuoteHandling {
    Preserve,
    Strip,
    Normalize,
    Smart, // Context-dependent
}

/// Entity types for natural language processing
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub enum EntityType {
    FilePath,
    DirectoryPath,
    Command,
    Flag,
    Option,
    Argument,
    Variable,
    Number,
    Date,
    Time,
    Url,
    Email,
    GitBranch,
    GitCommit,
    Language,
    Framework,
    Custom(String),
}

/// Communication message types for agent coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: Uuid,
    pub from_agent: String,
    pub to_agent: String,
    pub message_type: MessageType,
    pub payload: serde_json::Value,
    pub correlation_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
    pub priority: MessagePriority,
    pub ttl_seconds: Option<u64>,
}

/// Types of messages between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Request,
    Response,
    Notification,
    Delegation,
    Collaboration,
    Status,
    Error,
}

/// Message priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq)]
pub enum MessagePriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
    Emergency = 5,
}

/// Agent execution context
#[derive(Debug, Clone)]
pub struct AgentExecutionContext {
    pub workflow_id: Option<Uuid>,
    pub node_id: Option<String>,
    pub parent_context: Option<Box<AgentExecutionContext>>,
    pub variables: HashMap<String, serde_json::Value>,
    pub metadata: HashMap<String, String>,
    pub timeout: Option<DateTime<Utc>>,
    pub cancellation_token: Option<tokio_util::sync::CancellationToken>,
}

impl Default for ConcurrencyState {
    fn default() -> Self {
        Self {
            active_operations: Arc::new(RwLock::new(HashMap::new())),
            operation_locks: Arc::new(Mutex::new(HashMap::new())),
            resource_usage: Arc::new(RwLock::new(ResourceUsage::default())),
            max_concurrent_ops: 10,
        }
    }
}

impl ShellKind {
    /// Detect shell kind from environment or shell name
    pub fn detect() -> Self {
        if let Ok(shell) = std::env::var("SHELL") {
            if shell.contains("bash") {
                ShellKind::Bash
            } else if shell.contains("zsh") {
                ShellKind::Zsh
            } else if shell.contains("fish") {
                ShellKind::Fish
            } else {
                ShellKind::Unknown(shell)
            }
        } else if cfg!(windows) {
            ShellKind::PowerShell
        } else {
            ShellKind::Bash
        }
    }

    /// Get shell-specific command prefix
    pub fn command_prefix(&self) -> &'static str {
        match self {
            ShellKind::Bash | ShellKind::Zsh => "$",
            ShellKind::Fish => "❯",
            ShellKind::PowerShell => "PS>",
            ShellKind::Cmd => "C:\\>",
            ShellKind::Unknown(_) => "$",
        }
    }
}

impl Default for QualityConfig {
    fn default() -> Self {
        let mut enabled_checks = HashSet::new();
        enabled_checks.insert(QualityCheck::Security);
        enabled_checks.insert(QualityCheck::Style);
        enabled_checks.insert(QualityCheck::Performance);

        Self {
            enabled_checks,
            security_level: SecurityLevel::Standard,
            performance_thresholds: PerformanceThresholds {
                max_complexity: 10,
                max_function_length: 50,
                max_file_size_kb: 1000,
                min_test_coverage: 0.8,
            },
            style_rules: StyleRules {
                enforce_formatting: true,
                max_line_length: 100,
                indent_style: IndentStyle::Spaces(4),
                naming_conventions: HashMap::new(),
            },
            custom_rules: Vec::new(),
        }
    }
}

impl Default for NlpConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.7,
            entity_extraction: EntityExtractionConfig {
                enabled_types: [EntityType::FilePath, EntityType::Command, EntityType::Flag]
                    .iter()
                    .cloned()
                    .collect(),
                custom_patterns: HashMap::new(),
                case_sensitive: false,
                min_confidence: 0.6,
            },
            intent_classification: IntentClassificationConfig {
                model_type: IntentModelType::Hybrid,
                training_data: Vec::new(),
                fallback_intent: "unknown".to_string(),
                max_suggestions: 5,
            },
            parameter_extraction: ParameterExtractionConfig {
                cli_patterns: CliPatterns {
                    flag_patterns: vec![
                        r"--[a-zA-Z][a-zA-Z0-9-]*".to_string(),
                        r"-[a-zA-Z]".to_string(),
                    ],
                    option_patterns: vec![
                        r"--[a-zA-Z][a-zA-Z0-9-]*[=\s]+\S+".to_string(),
                        r"-[a-zA-Z]\s+\S+".to_string(),
                    ],
                    positional_patterns: vec![r"[^-]\S*".to_string()],
                    subcommand_patterns: vec![r"^[a-zA-Z][a-zA-Z0-9-]*".to_string()],
                },
                path_resolution: PathResolutionConfig {
                    resolve_relative: true,
                    expand_home: true,
                    validate_existence: false,
                    follow_symlinks: true,
                },
                variable_expansion: true,
                quote_handling: QuoteHandling::Smart,
            },
        }
    }
}

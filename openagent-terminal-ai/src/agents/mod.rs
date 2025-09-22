// Enhanced Agent System for OpenAgent Terminal
// Integrating selected Blitzy Platform AI capabilities

pub mod code_generation;
pub mod command;
pub mod communication_hub;
pub mod manager;
pub mod natural_language;
pub mod project_context;
pub mod quality;
pub mod types;
pub mod workflow_orchestration;

use crate::{AiProposal, AiRequest};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Core agent trait that all specialized agents must implement
#[async_trait]
pub trait AiAgent: Send + Sync {
    /// Unique identifier for this agent
    fn name(&self) -> &'static str;

    /// Agent version for compatibility checking
    fn version(&self) -> &'static str;

    /// Process a request and return responses
    async fn process(&self, request: AgentRequest) -> Result<AgentResponse, AgentError>;

    /// Check if this agent can handle the given request
    fn can_handle(&self, request: &AgentRequest) -> bool;

    /// Get agent capabilities and metadata
    fn capabilities(&self) -> AgentCapabilities;
}

/// Agent request types that extend basic AiRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentRequest {
    /// Basic terminal command generation (existing functionality)
    Command(AiRequest),

    /// Code generation and manipulation
    CodeGeneration {
        language: Option<String>,
        context: CodeContext,
        prompt: String,
        action: CodeAction,
    },

    /// Project analysis and context understanding
    ProjectContext { project_path: String, action: ContextAction },

    /// Code quality and security analysis
    Quality { code: String, language: Option<String>, action: QualityAction },

    /// Multi-agent collaboration request
    Collaboration { agents: Vec<String>, context: CollaborationContext, goal: String },
}

/// Agent response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentResponse {
    /// Basic command proposals (backward compatibility)
    Commands(Vec<AiProposal>),

    /// Code generation results
    Code { generated_code: String, language: String, explanation: String, suggestions: Vec<String> },

    /// Project context information
    Context { project_info: ProjectInfo, suggestions: Vec<ProjectSuggestion> },

    /// Quality analysis results
    QualityReport {
        score: f32,
        issues: Vec<QualityIssue>,
        suggestions: Vec<QualityFix>,
        security_warnings: Vec<SecurityIssue>,
    },

    /// Collaboration result
    CollaborationResult { participating_agents: Vec<String>, result: String, confidence: f32 },
}

/// Code-related action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeAction {
    Generate,
    Complete,
    Refactor,
    Explain,
    Optimize,
    Convert { target_language: String },
}

/// Project context action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextAction {
    Analyze,
    GetStructure,
    FindDependencies,
    SuggestImprovements,
}

/// Quality analysis action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualityAction {
    Analyze,
    SecurityScan,
    StyleCheck,
    Performance,
}

/// Code context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeContext {
    pub current_file: Option<String>,
    pub selection: Option<String>,
    pub cursor_position: Option<(usize, usize)>,
    pub project_files: Vec<String>,
    pub dependencies: Vec<String>,
}

/// Project information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub project_type: String,
    pub languages: Vec<String>,
    pub framework: Option<String>,
    pub structure: ProjectStructure,
    pub dependencies: Vec<Dependency>,
    pub git_info: Option<GitInfo>,
}

/// Project structure representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStructure {
    pub root: String,
    pub directories: Vec<DirectoryNode>,
    pub important_files: Vec<String>,
}

/// Directory node for project tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryNode {
    pub name: String,
    pub path: String,
    pub files: Vec<String>,
    pub subdirectories: Vec<DirectoryNode>,
}

/// Dependency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub dependency_type: DependencyType,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType {
    Runtime,
    Development,
    Build,
    Optional,
}

/// Git repository information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    pub branch: String,
    pub commit: String,
    pub status: GitStatus,
    pub remote: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub modified: Vec<String>,
    pub added: Vec<String>,
    pub deleted: Vec<String>,
    pub untracked: Vec<String>,
}

/// Project improvement suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSuggestion {
    pub category: String,
    pub title: String,
    pub description: String,
    pub priority: Priority,
    pub action: String,
}

/// Quality issues found in code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityIssue {
    pub severity: Severity,
    pub category: String,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub rule: String,
}

/// Quality fix suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityFix {
    pub description: String,
    pub suggested_code: String,
    pub confidence: f32,
}

/// Security issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityIssue {
    pub vulnerability_type: String,
    pub severity: Severity,
    pub description: String,
    pub cwe_id: Option<String>,
    pub line: Option<usize>,
    pub fix_suggestion: Option<String>,
}

/// Agent capabilities metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilities {
    pub supported_languages: Vec<String>,
    pub supported_frameworks: Vec<String>,
    pub features: Vec<String>,
    pub requires_internet: bool,
    pub privacy_level: PrivacyLevel,
}

/// Collaboration context for multi-agent workflows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationContext {
    pub project_context: Option<ProjectInfo>,
    pub code_context: Option<CodeContext>,
    pub user_context: HashMap<String, String>,
}

/// Priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// Severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Privacy level for agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrivacyLevel {
    Local,     // Processes everything locally
    CloudSafe, // Uses cloud APIs but sanitizes sensitive data
    CloudFull, // Uses cloud APIs with full context
}

/// Agent-specific errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentError {
    NotSupported(String),
    ProcessingError(String),
    InvalidRequest(String),
    AgentNotFound(String),
    CollaborationFailed(String),
    ConfigurationError(String),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::NotSupported(msg) => write!(f, "Not supported: {}", msg),
            AgentError::ProcessingError(msg) => write!(f, "Processing error: {}", msg),
            AgentError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            AgentError::AgentNotFound(msg) => write!(f, "Agent not found: {}", msg),
            AgentError::CollaborationFailed(msg) => write!(f, "Collaboration failed: {}", msg),
            AgentError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for AgentError {}

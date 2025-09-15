// Integration Points Module
// Interfaces for connecting AI agents with external systems

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::{Result, anyhow};
use uuid::Uuid;

pub mod lsp;
pub mod git;
pub mod terminal;
pub mod file_system;
pub mod ci_cd;

/// Core integration trait
#[async_trait]
pub trait Integration: Send + Sync {
    /// Unique identifier for this integration
    fn id(&self) -> &str;
    
    /// Human-readable name
    fn name(&self) -> &str;
    
    /// Description of what this integration provides
    fn description(&self) -> &str;
    
    /// Check if the integration is available/working
    async fn is_available(&self) -> bool;
    
    /// Initialize the integration
    async fn initialize(&mut self) -> Result<()>;
    
    /// Shutdown the integration
    async fn shutdown(&mut self) -> Result<()>;
    
    /// Get integration capabilities
    fn capabilities(&self) -> Vec<IntegrationCapability>;
}

/// Types of capabilities integrations can provide
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntegrationCapability {
    // Language Server Protocol
    LanguageCompletion,
    SymbolLookup,
    DiagnosticsReporting,
    CodeNavigation,
    Refactoring,
    
    // Version Control
    GitOperations,
    BranchManagement,
    CommitHistory,
    DiffGeneration,
    RemoteSync,
    
    // Terminal Operations
    CommandExecution,
    ProcessManagement,
    EnvironmentAccess,
    
    // File System
    FileOperations,
    DirectoryTraversal,
    PathResolution,
    FileWatching,
    
    // CI/CD
    BuildTriggers,
    DeploymentManagement,
    TestExecution,
    ArtifactManagement,
    
    // Custom
    Custom(String),
}

/// Context information for integration operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationContext {
    pub project_root: Option<PathBuf>,
    pub current_directory: PathBuf,
    pub environment: HashMap<String, String>,
    pub user_preferences: HashMap<String, String>,
    pub session_data: HashMap<String, serde_json::Value>,
}

/// Request to an integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationRequest {
    pub id: Uuid,
    pub integration_id: String,
    pub operation: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub context: IntegrationContext,
}

/// Response from an integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationResponse {
    pub request_id: Uuid,
    pub integration_id: String,
    pub success: bool,
    pub data: serde_json::Value,
    pub error_message: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Manager for all integrations
pub struct IntegrationManager {
    integrations: HashMap<String, Box<dyn Integration>>,
    initialized: bool,
}

impl IntegrationManager {
    pub fn new() -> Self {
        Self {
            integrations: HashMap::new(),
            initialized: false,
        }
    }

    /// Register a new integration
    pub fn register_integration(&mut self, integration: Box<dyn Integration>) -> Result<()> {
        let id = integration.id().to_string();
        
        if self.integrations.contains_key(&id) {
            return Err(anyhow!("Integration {} already registered", id));
        }
        
        self.integrations.insert(id.clone(), integration);
        tracing::info!("Registered integration: {}", id);
        Ok(())
    }

    /// Initialize all integrations
    pub async fn initialize_all(&mut self) -> Result<()> {
        for (id, integration) in self.integrations.iter_mut() {
            match integration.initialize().await {
                Ok(()) => {
                    tracing::info!("Initialized integration: {}", id);
                }
                Err(e) => {
                    tracing::error!("Failed to initialize integration {}: {}", id, e);
                }
            }
        }
        
        self.initialized = true;
        Ok(())
    }

    /// Get available integrations
    pub fn list_integrations(&self) -> Vec<String> {
        self.integrations.keys().cloned().collect()
    }

    /// Check if an integration is available
    pub async fn is_integration_available(&self, id: &str) -> bool {
        if let Some(integration) = self.integrations.get(id) {
            integration.is_available().await
        } else {
            false
        }
    }

    /// Find integrations by capability
    pub fn find_integrations_by_capability(&self, capability: IntegrationCapability) -> Vec<String> {
        let mut result = Vec::new();
        
        for (id, integration) in &self.integrations {
            if integration.capabilities().contains(&capability) {
                result.push(id.clone());
            }
        }
        
        result
    }

    /// Execute a request on an integration
    pub async fn execute_request(&self, request: IntegrationRequest) -> Result<IntegrationResponse> {
        if !self.initialized {
            return Err(anyhow!("Integration manager not initialized"));
        }

        // This is a placeholder - actual implementation would route to specific integrations
        // and handle their custom operations
        Err(anyhow!("Integration execution not yet implemented"))
    }

    /// Shutdown all integrations
    pub async fn shutdown_all(&mut self) -> Result<()> {
        for (id, integration) in self.integrations.iter_mut() {
            if let Err(e) = integration.shutdown().await {
                tracing::error!("Failed to shutdown integration {}: {}", id, e);
            }
        }
        
        self.initialized = false;
        tracing::info!("All integrations shut down");
        Ok(())
    }
}

/// LSP (Language Server Protocol) integration
pub struct LSPIntegration {
    id: String,
    name: String,
    language_servers: HashMap<String, LSPServerConfig>,
    initialized: bool,
}

#[derive(Debug, Clone)]
pub struct LSPServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub file_extensions: Vec<String>,
    pub initialization_options: serde_json::Value,
}

impl LSPIntegration {
    pub fn new() -> Self {
        Self {
            id: "lsp".to_string(),
            name: "Language Server Protocol".to_string(),
            language_servers: HashMap::new(),
            initialized: false,
        }
    }

    pub fn add_language_server(&mut self, language: String, config: LSPServerConfig) {
        self.language_servers.insert(language, config);
    }
}

#[async_trait]
impl Integration for LSPIntegration {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str { "Integration with Language Server Protocol for code intelligence" }

    async fn is_available(&self) -> bool {
        // Check if we have configured language servers
        !self.language_servers.is_empty()
    }

    async fn initialize(&mut self) -> Result<()> {
        // Initialize LSP connections for configured languages
        tracing::info!("Initializing LSP integration with {} language servers", self.language_servers.len());
        self.initialized = true;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Shutdown LSP connections
        self.initialized = false;
        Ok(())
    }

    fn capabilities(&self) -> Vec<IntegrationCapability> {
        vec![
            IntegrationCapability::LanguageCompletion,
            IntegrationCapability::SymbolLookup,
            IntegrationCapability::DiagnosticsReporting,
            IntegrationCapability::CodeNavigation,
            IntegrationCapability::Refactoring,
        ]
    }
}

/// Git integration for version control operations
pub struct GitIntegration {
    id: String,
    name: String,
    repository_path: Option<PathBuf>,
    initialized: bool,
}

impl GitIntegration {
    pub fn new() -> Self {
        Self {
            id: "git".to_string(),
            name: "Git Version Control".to_string(),
            repository_path: None,
            initialized: false,
        }
    }

    pub fn set_repository_path(&mut self, path: PathBuf) {
        self.repository_path = Some(path);
    }

    /// Check if current directory is a git repository
    pub async fn detect_repository(&mut self, path: &PathBuf) -> Result<bool> {
        let git_dir = path.join(".git");
        if git_dir.exists() {
            self.repository_path = Some(path.clone());
            Ok(true)
        } else {
            // Try parent directories
            if let Some(parent) = path.parent() {
                Box::pin(self.detect_repository(&parent.to_path_buf())).await
            } else {
                Ok(false)
            }
        }
    }
}

#[async_trait]
impl Integration for GitIntegration {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str { "Integration with Git for version control operations" }

    async fn is_available(&self) -> bool {
        // Check if git is available and we're in a repository
        self.repository_path.is_some()
    }

    async fn initialize(&mut self) -> Result<()> {
        // Try to detect git repository in current directory
        let current_dir = std::env::current_dir()?;
        self.detect_repository(&current_dir).await?;
        
        self.initialized = true;
        tracing::info!("Git integration initialized");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.initialized = false;
        Ok(())
    }

    fn capabilities(&self) -> Vec<IntegrationCapability> {
        vec![
            IntegrationCapability::GitOperations,
            IntegrationCapability::BranchManagement,
            IntegrationCapability::CommitHistory,
            IntegrationCapability::DiffGeneration,
            IntegrationCapability::RemoteSync,
        ]
    }
}

/// Terminal integration for command execution
pub struct TerminalIntegration {
    id: String,
    name: String,
    shell: String,
    initialized: bool,
}

impl TerminalIntegration {
    pub fn new() -> Self {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string());
        
        Self {
            id: "terminal".to_string(),
            name: "Terminal Operations".to_string(),
            shell,
            initialized: false,
        }
    }
}

#[async_trait]
impl Integration for TerminalIntegration {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str { "Integration with terminal for command execution and process management" }

    async fn is_available(&self) -> bool {
        // Terminal integration is always available
        true
    }

    async fn initialize(&mut self) -> Result<()> {
        self.initialized = true;
        tracing::info!("Terminal integration initialized with shell: {}", self.shell);
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.initialized = false;
        Ok(())
    }

    fn capabilities(&self) -> Vec<IntegrationCapability> {
        vec![
            IntegrationCapability::CommandExecution,
            IntegrationCapability::ProcessManagement,
            IntegrationCapability::EnvironmentAccess,
        ]
    }
}

/// File system integration
pub struct FileSystemIntegration {
    id: String,
    name: String,
    root_paths: Vec<PathBuf>,
    initialized: bool,
}

impl FileSystemIntegration {
    pub fn new() -> Self {
        Self {
            id: "filesystem".to_string(),
            name: "File System Operations".to_string(),
            root_paths: vec![],
            initialized: false,
        }
    }

    pub fn add_root_path(&mut self, path: PathBuf) {
        self.root_paths.push(path);
    }
}

#[async_trait]
impl Integration for FileSystemIntegration {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str { "Integration with file system for file operations and monitoring" }

    async fn is_available(&self) -> bool {
        true // File system is always available
    }

    async fn initialize(&mut self) -> Result<()> {
        // Add current directory as default root path
        let current_dir = std::env::current_dir()?;
        self.root_paths.push(current_dir);
        
        self.initialized = true;
        tracing::info!("File system integration initialized with {} root paths", self.root_paths.len());
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.initialized = false;
        Ok(())
    }

    fn capabilities(&self) -> Vec<IntegrationCapability> {
        vec![
            IntegrationCapability::FileOperations,
            IntegrationCapability::DirectoryTraversal,
            IntegrationCapability::PathResolution,
            IntegrationCapability::FileWatching,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_integration_manager() {
        let mut manager = IntegrationManager::new();
        
        // Register integrations
        let lsp_integration = Box::new(LSPIntegration::new());
        let git_integration = Box::new(GitIntegration::new());
        
        manager.register_integration(lsp_integration).unwrap();
        manager.register_integration(git_integration).unwrap();
        
        // Check integrations are listed
        let integrations = manager.list_integrations();
        assert!(integrations.contains(&"lsp".to_string()));
        assert!(integrations.contains(&"git".to_string()));
        
        // Initialize all
        manager.initialize_all().await.unwrap();
        
        // Find by capability
        let lsp_integrations = manager.find_integrations_by_capability(IntegrationCapability::LanguageCompletion);
        assert!(lsp_integrations.contains(&"lsp".to_string()));
    }

    #[tokio::test]
    async fn test_lsp_integration() {
        let mut lsp = LSPIntegration::new();
        
        assert_eq!(lsp.id(), "lsp");
        assert!(!lsp.is_available().await); // No language servers configured
        
        // Add a language server
        let config = LSPServerConfig {
            command: "rust-analyzer".to_string(),
            args: vec![],
            file_extensions: vec!["rs".to_string()],
            initialization_options: serde_json::json!({}),
        };
        lsp.add_language_server("rust".to_string(), config);
        
        assert!(lsp.is_available().await); // Now available
        
        lsp.initialize().await.unwrap();
        assert!(lsp.capabilities().contains(&IntegrationCapability::LanguageCompletion));
    }

    #[tokio::test]
    async fn test_terminal_integration() {
        let mut terminal = TerminalIntegration::new();
        
        assert_eq!(terminal.id(), "terminal");
        assert!(terminal.is_available().await);
        
        terminal.initialize().await.unwrap();
        assert!(terminal.capabilities().contains(&IntegrationCapability::CommandExecution));
    }
}
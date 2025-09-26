//! Terminal Event Bridge
//!
//! This module bridges terminal operations with the AI event integration system,
//! capturing terminal events and forwarding them to AI agents for processing.
//! Provides enterprise-grade event monitoring and AI response integration.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{Watcher, RecursiveMode, Event, EventKind};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::process::Command;
use tokio::sync::{mpsc, RwLock, Mutex, broadcast};
use tokio::time::{interval, sleep};
use tracing::{debug, info, warn, error, trace};

use crate::ai_event_integration::{
    AiEventIntegrator, TerminalEventType, FileChangeType, GitOperationType, AssistanceType
};
use crate::ai_runtime::{AiRuntime, AgentResponse};
use crate::blocks_v2::{ShellType, BlockId, BlockRecord};
use crate::event::{Event as WarpEvent, EventType};

/// Configuration for terminal event monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBridgeConfig {
    /// Enable command execution monitoring
    pub monitor_commands: bool,
    /// Enable directory change monitoring
    pub monitor_directory_changes: bool,
    /// Enable file system monitoring
    pub monitor_file_changes: bool,
    /// Enable Git operations monitoring
    pub monitor_git_operations: bool,
    /// Enable keystroke/typing monitoring
    pub monitor_typing: bool,
    /// Maximum number of recent commands to track
    pub max_recent_commands: usize,
    /// Debounce duration for file change events (ms)
    pub file_change_debounce_ms: u64,
    /// Directories to exclude from file monitoring
    pub excluded_directories: Vec<String>,
    /// File patterns to exclude from monitoring
    pub excluded_file_patterns: Vec<String>,
    /// Maximum command output length to capture (bytes)
    pub max_output_length: usize,
    /// Enable AI response forwarding to terminal
    pub forward_ai_responses: bool,
}

impl Default for EventBridgeConfig {
    fn default() -> Self {
        Self {
            monitor_commands: true,
            monitor_directory_changes: true,
            monitor_file_changes: true,
            monitor_git_operations: true,
            monitor_typing: false, // Disabled by default for privacy
            max_recent_commands: 50,
            file_change_debounce_ms: 500,
            excluded_directories: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".cargo".to_string(),
                "__pycache__".to_string(),
                ".venv".to_string(),
                "venv".to_string(),
            ],
            excluded_file_patterns: vec![
                "*.tmp".to_string(),
                "*.log".to_string(),
                "*.lock".to_string(),
                "*.swp".to_string(),
                "*.swo".to_string(),
                ".DS_Store".to_string(),
            ],
            max_output_length: 10 * 1024, // 10KB
            forward_ai_responses: true,
        }
    }
}

/// Terminal state tracking for context-aware AI responses
#[derive(Debug, Clone)]
pub struct TerminalState {
    pub current_directory: PathBuf,
    pub shell_type: ShellType,
    pub recent_commands: VecDeque<String>,
    pub last_command_result: Option<CommandResult>,
    pub git_repository_root: Option<PathBuf>,
    pub git_current_branch: Option<String>,
    pub active_processes: Vec<ProcessInfo>,
    pub environment_variables: std::collections::HashMap<String, String>,
    pub session_start_time: Instant,
    pub last_activity_time: Instant,
}

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub command: String,
    pub exit_code: i32,
    pub output: String,
    pub error_output: String,
    pub duration: Duration,
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub command_line: String,
    pub start_time: Instant,
}

impl TerminalState {
    /// Create new terminal state
    pub fn new(current_directory: PathBuf, shell_type: ShellType) -> Self {
        let now = Instant::now();
        Self {
            current_directory,
            shell_type,
            recent_commands: VecDeque::new(),
            last_command_result: None,
            git_repository_root: None,
            git_current_branch: None,
            active_processes: Vec::new(),
            environment_variables: std::collections::HashMap::new(),
            session_start_time: now,
            last_activity_time: now,
        }
    }

    /// Add a command to the history
    pub fn add_command(&mut self, command: String, max_commands: usize) {
        self.recent_commands.push_front(command);
        if self.recent_commands.len() > max_commands {
            self.recent_commands.pop_back();
        }
        self.last_activity_time = Instant::now();
    }

    /// Update current directory and refresh Git info
    pub async fn update_directory(&mut self, new_directory: PathBuf) -> Result<()> {
        self.current_directory = new_directory;
        self.refresh_git_info().await?;
        self.last_activity_time = Instant::now();
        Ok(())
    }

    /// Refresh Git repository information
    pub async fn refresh_git_info(&mut self) -> Result<()> {
        // Find Git repository root
        let mut current = self.current_directory.clone();
        self.git_repository_root = None;
        self.git_current_branch = None;

        loop {
            let git_dir = current.join(".git");
            if git_dir.exists() {
                self.git_repository_root = Some(current.clone());
                
                // Get current branch
                if let Ok(output) = Command::new("git")
                    .args(&["branch", "--show-current"])
                    .current_dir(&current)
                    .output()
                    .await
                {
                    if output.status.success() {
                        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if !branch.is_empty() {
                            self.git_current_branch = Some(branch);
                        }
                    }
                }
                break;
            }

            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                break;
            }
        }

        Ok(())
    }

    /// Update command result
    pub fn set_command_result(&mut self, result: CommandResult) {
        self.last_command_result = Some(result);
        self.last_activity_time = Instant::now();
    }
}

/// File system event debouncer to avoid spam
#[derive(Debug, Clone)]
struct FileEventDebouncer {
    pending_events: Arc<RwLock<std::collections::HashMap<PathBuf, (FileChangeType, Instant)>>>,
    debounce_duration: Duration,
}

impl FileEventDebouncer {
    fn new(debounce_duration: Duration) -> Self {
        Self {
            pending_events: Arc::new(RwLock::new(std::collections::HashMap::new())),
            debounce_duration,
        }
    }

    /// Add an event to the debouncer
    async fn add_event(&self, path: PathBuf, change_type: FileChangeType) {
        let mut events = self.pending_events.write().await;
        events.insert(path, (change_type, Instant::now()));
    }

    /// Get and remove debounced events
    async fn get_debounced_events(&self) -> Vec<(PathBuf, FileChangeType)> {
        let mut events = self.pending_events.write().await;
        let now = Instant::now();
        
        let ready_events: Vec<_> = events
            .iter()
            .filter_map(|(path, (change_type, timestamp))| {
                if now.duration_since(*timestamp) >= self.debounce_duration {
                    Some((path.clone(), change_type.clone()))
                } else {
                    None
                }
            })
            .collect();

        // Remove processed events
        for (path, _) in &ready_events {
            events.remove(path);
        }

        ready_events
    }
}

/// Main terminal event bridge
pub struct TerminalEventBridge {
    /// Configuration
    config: EventBridgeConfig,
    
    /// AI event integrator
    ai_integrator: Arc<Mutex<AiEventIntegrator>>,
    
    /// Current terminal state
    terminal_state: Arc<RwLock<TerminalState>>,
    
    /// File system watcher
    _file_watcher: Option<Box<dyn Watcher + Send>>,
    
    /// File event debouncer
    file_debouncer: FileEventDebouncer,
    
    /// AI response receiver
    ai_response_receiver: Option<broadcast::Receiver<AgentResponse>>,
    
    /// Response forwarding sender (to terminal UI)
    response_forwarder: Option<mpsc::UnboundedSender<String>>,
    
    /// Background task handles
    task_handles: Vec<tokio::task::JoinHandle<()>>,
}

impl TerminalEventBridge {
    /// Create a new terminal event bridge
    pub fn new(
        config: EventBridgeConfig,
        ai_integrator: Arc<Mutex<AiEventIntegrator>>,
        initial_directory: PathBuf,
        shell_type: ShellType,
    ) -> Result<Self> {
        let terminal_state = Arc::new(RwLock::new(TerminalState::new(initial_directory, shell_type)));
        let file_debouncer = FileEventDebouncer::new(Duration::from_millis(config.file_change_debounce_ms));
        
        Ok(Self {
            config,
            ai_integrator,
            terminal_state,
            _file_watcher: None,
            file_debouncer,
            ai_response_receiver: None,
            response_forwarder: None,
            task_handles: Vec::new(),
        })
    }

    /// Set up response forwarding to terminal UI
    pub fn set_response_forwarder(&mut self, sender: mpsc::UnboundedSender<String>) {
        self.response_forwarder = Some(sender);
    }

    /// Start monitoring terminal events
    pub async fn start_monitoring(&mut self) -> Result<()> {
        info!("Starting terminal event monitoring");

        // Initialize AI response receiver
        {
            let integrator = self.ai_integrator.lock().await;
            self.ai_response_receiver = Some(integrator.get_response_receiver());
        }

        // Start file system monitoring if enabled
        if self.config.monitor_file_changes {
            self.start_file_monitoring().await?;
        }

        // Start AI response processing
        if self.config.forward_ai_responses {
            self.start_response_processing().await?;
        }

        // Start periodic tasks
        self.start_periodic_tasks().await?;

        // Send session started event
        self.send_session_started_event().await?;

        info!("Terminal event monitoring started successfully");
        Ok(())
    }

    /// Stop monitoring and cleanup
    pub async fn stop_monitoring(&mut self) {
        info!("Stopping terminal event monitoring");
        
        // Abort all background tasks
        for handle in &self.task_handles {
            handle.abort();
        }
        self.task_handles.clear();

        info!("Terminal event monitoring stopped");
    }

    /// Handle command execution event
    pub async fn on_command_executed(
        &self,
        command: String,
        exit_code: i32,
        output: String,
        error_output: String,
        duration: Duration,
    ) -> Result<()> {
        if !self.config.monitor_commands {
            return Ok(());
        }

        let working_directory = {
            let state = self.terminal_state.read().await;
            state.current_directory.clone()
        };

        // Truncate output if too long
        let truncated_output = if output.len() > self.config.max_output_length {
            let truncated = &output[..self.config.max_output_length];
            format!("{}... [truncated]", truncated)
        } else {
            output.clone()
        };

        let truncated_error = if error_output.len() > self.config.max_output_length {
            let truncated = &error_output[..self.config.max_output_length];
            format!("{}... [truncated]", truncated)
        } else {
            error_output.clone()
        };

        // Update terminal state
        {
            let mut state = self.terminal_state.write().await;
            state.add_command(command.clone(), self.config.max_recent_commands);
            state.set_command_result(CommandResult {
                command: command.clone(),
                exit_code,
                output: output.clone(),
                error_output: error_output.clone(),
                duration,
                timestamp: Instant::now(),
            });

            // Update Git info if this was a Git command
            if command.starts_with("git") {
                if let Err(e) = state.refresh_git_info().await {
                    warn!("Failed to refresh git info: {}", e);
                }
            }
        }

        // Get shell type before moving command
        let shell_type = {
            let state = self.terminal_state.read().await;
            state.shell_type
        };
        
        // Create and send event
        let event = if exit_code == 0 {
            TerminalEventType::CommandExecuted {
                command: command.clone(),
                exit_code,
                output: truncated_output,
                error_output: truncated_error,
                duration_ms: duration.as_millis() as u64,
                working_directory,
                shell: shell_type,
            }
        } else {
            TerminalEventType::CommandFailed {
                command: command.clone(),
                error: if !truncated_error.is_empty() { truncated_error } else { "Command failed".to_string() },
                exit_code,
                working_directory,
            }
        };

        self.send_ai_event(event).await?;

        // Detect Git operations
        if self.config.monitor_git_operations {
            self.detect_git_operation(&command).await?;
        }

        Ok(())
    }

    /// Handle directory change event
    pub async fn on_directory_changed(&self, new_directory: PathBuf) -> Result<()> {
        if !self.config.monitor_directory_changes {
            return Ok(());
        }

        let old_directory = {
            let mut state = self.terminal_state.write().await;
            let old = state.current_directory.clone();
            state.update_directory(new_directory.clone()).await?;
            old
        };

        if old_directory != new_directory {
            let event = TerminalEventType::DirectoryChanged {
                old_path: old_directory,
                new_path: new_directory,
            };

            self.send_ai_event(event).await?;
        }

        Ok(())
    }

    /// Handle command typing event
    pub async fn on_command_typed(&self, partial_command: String, cursor_position: usize) -> Result<()> {
        if !self.config.monitor_typing || partial_command.trim().is_empty() {
            return Ok(());
        }

        let working_directory = {
            let state = self.terminal_state.read().await;
            state.current_directory.clone()
        };

        let event = TerminalEventType::CommandTyped {
            partial_command,
            cursor_position,
            working_directory,
        };

        self.send_ai_event(event).await?;
        Ok(())
    }

    /// Handle AI assistance request
    pub async fn on_ai_assistance_requested(&self, context: String, assistance_type: AssistanceType) -> Result<()> {
        let event = TerminalEventType::AiAssistanceRequested {
            context,
            assistance_type,
        };

        self.send_ai_event(event).await?;
        Ok(())
    }

    /// Start file system monitoring
    async fn start_file_monitoring(&mut self) -> Result<()> {
        use notify::{Watcher, RecommendedWatcher};

        let (tx, mut rx) = mpsc::unbounded_channel();
        
        let mut watcher = RecommendedWatcher::new(
            move |result: notify::Result<Event>| {
                if let Ok(event) = result {
                    if let Err(e) = tx.send(event) {
                        error!("Failed to send file system event: {}", e);
                    }
                }
            },
            notify::Config::default(),
        )?;

        // Watch current directory and common project directories
        let current_dir = {
            let state = self.terminal_state.read().await;
            state.current_directory.clone()
        };

        watcher.watch(&current_dir, RecursiveMode::Recursive)?;

        self._file_watcher = Some(Box::new(watcher));

        // Start file event processing task
        let file_debouncer = self.file_debouncer.clone();
        let config = self.config.clone();
        let ai_integrator = Arc::clone(&self.ai_integrator);

        let handle = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Err(e) = Self::process_file_event(event, &file_debouncer, &config).await {
                    error!("Failed to process file event: {}", e);
                }
            }
        });

        self.task_handles.push(handle);

        // Start debounced event processing
        let file_debouncer2 = self.file_debouncer.clone();
        let ai_integrator2 = Arc::clone(&self.ai_integrator);
        
        let handle2 = tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(100));
            
            loop {
                interval.tick().await;
                
                let events = file_debouncer2.get_debounced_events().await;
                for (path, change_type) in events {
                    let event = TerminalEventType::FileChanged { path, change_type };
                    
                    let integrator = ai_integrator2.lock().await;
                    if let Err(e) = integrator.send_event(event) {
                        error!("Failed to send file change event: {}", e);
                    }
                }
            }
        });

        self.task_handles.push(handle2);
        Ok(())
    }

    /// Process file system events
    async fn process_file_event(
        event: notify::Event,
        debouncer: &FileEventDebouncer,
        config: &EventBridgeConfig,
    ) -> Result<()> {
        for path in &event.paths {
            // Skip excluded directories
            if config.excluded_directories.iter().any(|excluded| {
                path.components().any(|c| c.as_os_str() == excluded.as_str())
            }) {
                continue;
            }

            // Skip excluded file patterns
            if let Some(file_name) = path.file_name() {
                let file_str = file_name.to_string_lossy();
                if config.excluded_file_patterns.iter().any(|pattern| {
                    // Simple pattern matching - in production would use glob
                    if pattern.contains('*') {
                        let prefix = pattern.trim_end_matches('*');
                        file_str.starts_with(prefix)
                    } else {
                        file_str == *pattern
                    }
                }) {
                    continue;
                }
            }

            let change_type = match event.kind {
                EventKind::Create(_) => FileChangeType::Created,
                EventKind::Modify(_) => FileChangeType::Modified,
                EventKind::Remove(_) => FileChangeType::Deleted,
                _ => continue,
            };

            debouncer.add_event(path.clone(), change_type).await;
        }

        Ok(())
    }

    /// Start AI response processing
    async fn start_response_processing(&mut self) -> Result<()> {
        if let Some(mut receiver) = self.ai_response_receiver.take() {
            let forwarder = self.response_forwarder.clone();

            let handle = tokio::spawn(async move {
                while let Ok(response) = receiver.recv().await {
                    // Forward response to terminal UI if configured
                    if let Some(ref sender) = forwarder {
                        let formatted_response = format!(
                            "\n🤖 AI Assistant ({}): {}\n",
                            response.metadata.get("agent_id").unwrap_or(&"unknown".to_string()),
                            response.content
                        );

                        if let Err(e) = sender.send(formatted_response) {
                            warn!("Failed to forward AI response to terminal: {}", e);
                        }
                    }
                    
                    debug!("Processed AI response: {}", response.content);
                }
            });

            self.task_handles.push(handle);
        }

        Ok(())
    }

    /// Start periodic maintenance tasks
    async fn start_periodic_tasks(&mut self) -> Result<()> {
        let terminal_state = Arc::clone(&self.terminal_state);

        // Periodic Git info refresh
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                let mut state = terminal_state.write().await;
                if let Err(e) = state.refresh_git_info().await {
                    debug!("Failed to refresh git info: {}", e);
                }
            }
        });

        self.task_handles.push(handle);
        Ok(())
    }

    /// Send session started event
    async fn send_session_started_event(&self) -> Result<()> {
        let (shell, working_directory) = {
            let state = self.terminal_state.read().await;
            (state.shell_type, state.current_directory.clone())
        };

        let event = TerminalEventType::SessionStarted {
            shell,
            working_directory,
        };

        self.send_ai_event(event).await?;
        Ok(())
    }

    /// Detect Git operations from commands
    async fn detect_git_operation(&self, command: &str) -> Result<()> {
        if !command.starts_with("git ") {
            return Ok(());
        }

        let args: Vec<&str> = command.split_whitespace().collect();
        if args.len() < 2 {
            return Ok(());
        }

        let operation = match args[1] {
            "commit" => GitOperationType::Commit,
            "push" => GitOperationType::Push,
            "pull" => GitOperationType::Pull,
            "checkout" => GitOperationType::Checkout,
            "merge" => GitOperationType::Merge,
            "rebase" => GitOperationType::Rebase,
            _ => return Ok(()), // Skip other commands
        };

        let (branch, working_directory) = {
            let state = self.terminal_state.read().await;
            (state.git_current_branch.clone(), state.current_directory.clone())
        };

        let event = TerminalEventType::GitOperation {
            operation,
            branch,
            working_directory,
        };

        self.send_ai_event(event).await?;
        Ok(())
    }

    /// Send event to AI integration system
    async fn send_ai_event(&self, event: TerminalEventType) -> Result<()> {
        let integrator = self.ai_integrator.lock().await;
        integrator.send_event(event).context("Failed to send event to AI integrator")?;
        Ok(())
    }

    /// Get current terminal state (for debugging/monitoring)
    pub async fn get_terminal_state(&self) -> TerminalState {
        self.terminal_state.read().await.clone()
    }

    /// Update configuration
    pub async fn update_config(&mut self, new_config: EventBridgeConfig) -> Result<()> {
        info!("Updating terminal event bridge configuration");
        self.config = new_config;
        
        // Restart monitoring with new config if needed
        // In a full implementation, this would selectively restart components
        
        Ok(())
    }

    /// Get monitoring statistics
    pub async fn get_statistics(&self) -> BridgeStatistics {
        let integrator = self.ai_integrator.lock().await;
        let ai_stats = integrator.get_stats().await;
        
        let state = self.terminal_state.read().await;
        
        BridgeStatistics {
            session_duration: state.session_start_time.elapsed(),
            total_commands: state.recent_commands.len(),
            last_activity: state.last_activity_time.elapsed(),
            ai_events_processed: ai_stats.events_processed,
            ai_responses_generated: ai_stats.responses_generated,
            current_directory: state.current_directory.clone(),
            git_repository: state.git_repository_root.clone(),
            git_branch: state.git_current_branch.clone(),
        }
    }
}

/// Statistics for monitoring bridge performance
#[derive(Debug, Clone)]
pub struct BridgeStatistics {
    pub session_duration: Duration,
    pub total_commands: usize,
    pub last_activity: Duration,
    pub ai_events_processed: u64,
    pub ai_responses_generated: u64,
    pub current_directory: PathBuf,
    pub git_repository: Option<PathBuf>,
    pub git_branch: Option<String>,
}

impl Drop for TerminalEventBridge {
    fn drop(&mut self) {
        // Ensure background tasks are cleaned up
        for handle in &self.task_handles {
            handle.abort();
        }
    }
}

/// Helper trait for integrating with different terminal implementations
pub trait TerminalIntegration {
    /// Called when a command is about to be executed
    fn on_command_starting(&self, command: &str) -> Result<()>;
    
    /// Called when a command completes
    fn on_command_completed(&self, command: &str, exit_code: i32, output: &str, error_output: &str, duration: Duration) -> Result<()>;
    
    /// Called when directory changes
    fn on_directory_change(&self, new_directory: &Path) -> Result<()>;
    
    /// Called when user types (optional, for command completion)
    fn on_user_input(&self, partial_command: &str, cursor_position: usize) -> Result<()>;
    
    /// Called when AI assistance is explicitly requested
    fn on_ai_help_requested(&self, context: &str, assistance_type: AssistanceType) -> Result<()>;
}

/// Default implementation that can be used with any terminal
pub struct DefaultTerminalIntegration {
    bridge: Arc<Mutex<TerminalEventBridge>>,
}

impl DefaultTerminalIntegration {
    pub fn new(bridge: Arc<Mutex<TerminalEventBridge>>) -> Self {
        Self { bridge }
    }
}

#[async_trait::async_trait]
impl TerminalIntegration for DefaultTerminalIntegration {
    fn on_command_starting(&self, _command: &str) -> Result<()> {
        // Could be used for pre-command processing
        Ok(())
    }
    
    fn on_command_completed(&self, command: &str, exit_code: i32, output: &str, error_output: &str, duration: Duration) -> Result<()> {
        let bridge = self.bridge.clone();
        let command = command.to_string();
        let output = output.to_string();
        let error_output = error_output.to_string();
        
        tokio::spawn(async move {
            let bridge_guard = bridge.lock().await;
            if let Err(e) = bridge_guard.on_command_executed(command, exit_code, output, error_output, duration).await {
                error!("Failed to handle command completion: {}", e);
            }
        });
        
        Ok(())
    }
    
    fn on_directory_change(&self, new_directory: &Path) -> Result<()> {
        let bridge = self.bridge.clone();
        let new_directory = new_directory.to_path_buf();
        
        tokio::spawn(async move {
            let bridge_guard = bridge.lock().await;
            if let Err(e) = bridge_guard.on_directory_changed(new_directory).await {
                error!("Failed to handle directory change: {}", e);
            }
        });
        
        Ok(())
    }
    
    fn on_user_input(&self, partial_command: &str, cursor_position: usize) -> Result<()> {
        let bridge = self.bridge.clone();
        let partial_command = partial_command.to_string();
        
        tokio::spawn(async move {
            let bridge_guard = bridge.lock().await;
            if let Err(e) = bridge_guard.on_command_typed(partial_command, cursor_position).await {
                error!("Failed to handle user input: {}", e);
            }
        });
        
        Ok(())
    }
    
    fn on_ai_help_requested(&self, context: &str, assistance_type: AssistanceType) -> Result<()> {
        let bridge = self.bridge.clone();
        let context = context.to_string();
        
        tokio::spawn(async move {
            let bridge_guard = bridge.lock().await;
            if let Err(e) = bridge_guard.on_ai_assistance_requested(context, assistance_type).await {
                error!("Failed to handle AI assistance request: {}", e);
            }
        });
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_terminal_state_updates() {
        let temp_dir = TempDir::new().unwrap();
        let mut state = TerminalState::new(temp_dir.path().to_path_buf(), ShellType::Bash);
        
        // Test adding commands
        state.add_command("ls -la".to_string(), 5);
        state.add_command("pwd".to_string(), 5);
        
        assert_eq!(state.recent_commands.len(), 2);
        assert_eq!(state.recent_commands[0], "pwd");
        assert_eq!(state.recent_commands[1], "ls -la");
    }

    #[tokio::test]
    async fn test_file_event_debouncer() {
        let debouncer = FileEventDebouncer::new(Duration::from_millis(100));
        
        let path = PathBuf::from("/test/file.txt");
        debouncer.add_event(path.clone(), FileChangeType::Modified).await;
        
        // Should be empty immediately
        let events = debouncer.get_debounced_events().await;
        assert!(events.is_empty());
        
        // Wait for debounce period
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        let events = debouncer.get_debounced_events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, path);
    }
}
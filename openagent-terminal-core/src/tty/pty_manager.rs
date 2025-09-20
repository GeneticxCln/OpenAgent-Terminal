use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::event::WindowSize;
use crate::tty::{ChildEvent, EventedPty, Options, Shell};

/// Unique identifier for a pane's PTY process
pub type PtyId = u64;

/// Shell type detection for context awareness
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Sh,
    Dash,
    Unknown,
}

impl ShellKind {
    /// Detect shell kind from shell program name
    pub fn from_shell_name(shell_program: &str) -> Self {
        match shell_program {
            s if s.contains("bash") => ShellKind::Bash,
            s if s.contains("zsh") => ShellKind::Zsh,
            s if s.contains("fish") => ShellKind::Fish,
            s if s.contains("powershell") || s.contains("pwsh") => ShellKind::PowerShell,
            s if s.contains("cmd") => ShellKind::Cmd,
            "sh" => ShellKind::Sh,
            s if s.contains("dash") => ShellKind::Dash,
            _ => ShellKind::Unknown,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            ShellKind::Bash => "bash",
            ShellKind::Zsh => "zsh",
            ShellKind::Fish => "fish",
            ShellKind::PowerShell => "powershell",
            ShellKind::Cmd => "cmd",
            ShellKind::Sh => "sh",
            ShellKind::Dash => "dash",
            ShellKind::Unknown => "unknown",
        }
    }
}

/// Context information for a PTY process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyContext {
    /// Current working directory
    pub working_directory: PathBuf,
    /// Shell type
    pub shell_kind: ShellKind,
    /// Last executed command (if available)
    pub last_command: Option<String>,
    /// Environment variables
    pub environment: HashMap<String, String>,
    /// Shell-specific configuration
    pub shell_config: ShellConfig,
    /// Creation time
    #[serde(skip, default = "Instant::now")]
    pub created_at: Instant,
    /// Last activity time
    #[serde(skip, default = "Instant::now")]
    pub last_activity: Instant,
}

/// Shell-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    /// Shell executable path
    pub executable: String,
    /// Shell arguments
    pub args: Vec<String>,
    /// Shell-specific environment variables
    pub env_vars: HashMap<String, String>,
    /// Prompt detection pattern (for command boundary detection)
    pub prompt_pattern: Option<String>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            executable: "bash".to_string(),
            args: vec!["-l".to_string()],
            env_vars: HashMap::new(),
            prompt_pattern: None,
        }
    }
}

/// PTY process manager for a single terminal pane
pub struct PtyManager {
    /// Unique identifier
    pub id: PtyId,
    /// PTY process context
    pub context: PtyContext,
    /// Process status
    pub status: PtyStatus,
    /// PTY interface (boxed to avoid generic complications)
    pty: Option<Box<dyn EventedPty<Reader = File, Writer = File> + Send>>,
    /// Performance metrics
    metrics: PtyMetrics,
    /// Child PID if available (Unix)
    child_pid: Option<u32>,
}

/// Status of a PTY process
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtyStatus {
    /// PTY is starting up
    Starting,
    /// PTY is active and running
    Active,
    /// PTY process has exited
    Exited { exit_code: Option<i32> },
    /// PTY encountered an error
    Error { message: String },
}

/// Performance metrics for PTY monitoring
#[derive(Debug, Default)]
pub struct PtyMetrics {
    /// Time to startup in milliseconds
    pub startup_time_ms: Option<u64>,
    /// Total bytes read
    pub bytes_read: u64,
    /// Total bytes written
    pub bytes_written: u64,
    /// Last I/O activity
    pub last_io_activity: Option<Instant>,
    /// Command count (approximation based on newlines)
    pub command_count: u64,
}

impl PtyManager {
    /// Create a new PTY manager for a pane
    pub fn new(
        pane_id: PtyId,
        working_directory: PathBuf,
        shell_config: ShellConfig,
        environment: HashMap<String, String>,
    ) -> Result<Self, PtyManagerError> {
        let shell_kind = ShellKind::from_shell_name(&shell_config.executable);

        let context = PtyContext {
            working_directory: working_directory.clone(),
            shell_kind,
            last_command: None,
            environment: environment.clone(),
            shell_config: shell_config.clone(),
            created_at: Instant::now(),
            last_activity: Instant::now(),
        };

        debug!("Creating PTY manager for pane {} in {}", pane_id, working_directory.display());

        Ok(Self {
            id: pane_id,
            context,
            status: PtyStatus::Starting,
            pty: None, // Will be set when PTY is created
            metrics: PtyMetrics::default(),
            child_pid: None,
        })
    }

    /// Create PTY process with the configured context
    pub fn create_pty(
        &mut self,
        window_size: WindowSize,
        window_id: u64,
    ) -> Result<(), PtyManagerError> {
        let start_time = Instant::now();

        // Build PTY options from context
        let shell = Shell::new(
            self.context.shell_config.executable.clone(),
            self.context.shell_config.args.clone(),
        );

        let mut pty_options = Options {
            shell: Some(shell),
            working_directory: Some(self.context.working_directory.clone()),
            env: self.context.environment.clone(),
            drain_on_exit: true,
            #[cfg(target_os = "windows")]
            escape_args: true,
        };

        // Add shell-specific environment variables
        for (key, value) in &self.context.shell_config.env_vars {
            pty_options.env.insert(key.clone(), value.clone());
        }

        // Create the PTY
        match crate::tty::new(&pty_options, window_size, window_id) {
            Ok(pty) => {
                // Capture PID on Unix before boxing
                #[cfg(not(windows))]
                {
                    self.child_pid = Some(pty.child().id());
                }

                self.pty = Some(Box::new(pty));
                self.status = PtyStatus::Active;
                self.metrics.startup_time_ms = Some(start_time.elapsed().as_millis() as u64);

                info!(
                    "PTY created successfully for pane {} in {}ms",
                    self.id,
                    self.metrics.startup_time_ms.unwrap()
                );

                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to create PTY: {}", e);
                error!("{}", error_msg);
                self.status = PtyStatus::Error { message: error_msg.clone() };
                Err(PtyManagerError::PtyCreation(error_msg))
            }
        }
    }

    /// Update working directory context
    pub fn update_working_directory(&mut self, new_dir: PathBuf) {
        debug!("Updating working directory for pane {} to {}", self.id, new_dir.display());
        self.context.working_directory = new_dir;
        self.context.last_activity = Instant::now();
    }

    /// Update last command for context awareness
    pub fn update_last_command(&mut self, command: String) {
        debug!("Updating last command for pane {}: {}", self.id, command);
        self.context.last_command = Some(command);
        self.context.last_activity = Instant::now();
        self.metrics.command_count += 1;
    }

    /// Check if PTY is still active
    pub fn is_active(&self) -> bool {
        matches!(self.status, PtyStatus::Active)
    }

    /// Check if PTY is available
    pub fn has_pty(&self) -> bool {
        self.pty.is_some()
    }

    /// Poll for child events (process exit, etc.)
    pub fn poll_child_events(&mut self) -> Vec<ChildEvent> {
        let mut events = Vec::new();

        if let Some(ref mut pty) = self.pty {
            while let Some(event) = pty.next_child_event() {
                match event {
                    ChildEvent::Exited(exit_code) => {
                        info!("PTY process exited for pane {}: {:?}", self.id, exit_code);
                        self.status = PtyStatus::Exited { exit_code };
                    }
                }
                events.push(event);
            }
        }

        events
    }

    /// Update I/O metrics
    pub fn record_io_activity(&mut self, bytes_read: u64, bytes_written: u64) {
        self.metrics.bytes_read += bytes_read;
        self.metrics.bytes_written += bytes_written;
        self.metrics.last_io_activity = Some(Instant::now());
        self.context.last_activity = Instant::now();
    }

    /// Get performance metrics
    pub fn metrics(&self) -> &PtyMetrics {
        &self.metrics
    }

    /// Get child PID if available
    pub fn child_pid(&self) -> Option<u32> {
        self.child_pid
    }

    /// Try a non-blocking read of available PTY bytes
    pub fn read_nonblocking(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use std::io::{Error, ErrorKind, Read};
        if let Some(ref mut pty) = self.pty {
            match pty.reader().read(buf) {
                Ok(n) => Ok(n),
                Err(e) => Err(e),
            }
        } else {
            Err(Error::new(ErrorKind::NotConnected, "PTY not initialized"))
        }
    }

    /// Check if PTY is idle (no activity for specified duration)
    pub fn is_idle(&self, threshold: Duration) -> bool {
        self.context.last_activity.elapsed() > threshold
    }

    /// Cleanup PTY resources
    pub fn cleanup(&mut self) {
        debug!("Cleaning up PTY manager for pane {}", self.id);
        self.pty = None;
        if matches!(self.status, PtyStatus::Active | PtyStatus::Starting) {
            self.status = PtyStatus::Exited { exit_code: None };
        }
    }

    /// Get context for AI integration
    pub fn get_ai_context(&self) -> PtyAiContext {
        PtyAiContext {
            working_directory: self.context.working_directory.clone(),
            shell_kind: self.context.shell_kind,
            last_command: self.context.last_command.clone(),
            shell_executable: self.context.shell_config.executable.clone(),
        }
    }
}

/// AI context extracted from PTY manager
#[derive(Debug, Clone)]
pub struct PtyAiContext {
    pub working_directory: PathBuf,
    pub shell_kind: ShellKind,
    pub last_command: Option<String>,
    pub shell_executable: String,
}

impl PtyAiContext {
    /// Convert to strings for AI provider consumption
    pub fn to_strings(&self) -> (String, String) {
        let working_dir = self.working_directory.to_string_lossy().to_string();
        let shell_kind = self.shell_kind.to_str().to_string();
        (working_dir, shell_kind)
    }
}

/// Collection of PTY managers for multiple terminals
pub struct PtyManagerCollection {
    managers: HashMap<PtyId, Arc<parking_lot::Mutex<PtyManager>>>,
    next_id: PtyId,
}

impl PtyManagerCollection {
    /// Create new collection
    pub fn new() -> Self {
        Self { managers: HashMap::new(), next_id: 1 }
    }

    /// Create a new PTY manager and return its ID
    pub fn create_pty_manager(
        &mut self,
        working_directory: PathBuf,
        shell_config: ShellConfig,
        environment: HashMap<String, String>,
    ) -> Result<PtyId, PtyManagerError> {
        let pty_id = self.next_id;
        self.next_id += 1;

        let manager = PtyManager::new(pty_id, working_directory, shell_config, environment)?;
        self.managers.insert(pty_id, Arc::new(parking_lot::Mutex::new(manager)));

        debug!("Created PTY manager with ID: {}", pty_id);
        Ok(pty_id)
    }

    /// Get PTY manager by ID
    pub fn get_manager(&self, pty_id: PtyId) -> Option<Arc<parking_lot::Mutex<PtyManager>>> {
        self.managers.get(&pty_id).cloned()
    }

    /// Remove PTY manager
    pub fn remove_manager(&mut self, pty_id: PtyId) -> Option<Arc<parking_lot::Mutex<PtyManager>>> {
        debug!("Removing PTY manager: {}", pty_id);
        self.managers.remove(&pty_id)
    }

    /// Get all active PTY IDs
    pub fn active_pty_ids(&self) -> Vec<PtyId> {
        self.managers.keys().copied().collect()
    }

    /// Get total number of managed PTYs
    pub fn count(&self) -> usize {
        self.managers.len()
    }

    /// Cleanup all inactive PTY managers
    pub fn cleanup_inactive(&mut self) {
        let inactive_ids: Vec<PtyId> = self
            .managers
            .iter()
            .filter_map(|(&id, manager)| {
                let manager_guard = manager.lock();
                if matches!(
                    manager_guard.status,
                    PtyStatus::Exited { .. } | PtyStatus::Error { .. }
                ) {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();

        for id in inactive_ids {
            self.remove_manager(id);
        }
    }

    /// Get context from a specific PTY for AI integration
    pub fn get_ai_context(&self, pty_id: PtyId) -> Option<PtyAiContext> {
        self.get_manager(pty_id)?.lock().get_ai_context().into()
    }

    /// Get aggregated metrics for all PTY processes
    pub fn get_aggregate_metrics(&self) -> PtyCollectionMetrics {
        let mut total_bytes_read = 0;
        let mut total_bytes_written = 0;
        let mut total_commands = 0;
        let mut active_count = 0;
        let mut avg_startup_time = 0u64;
        let mut startup_times = Vec::new();

        for manager in self.managers.values() {
            let manager_guard = manager.lock();
            let metrics = manager_guard.metrics();

            total_bytes_read += metrics.bytes_read;
            total_bytes_written += metrics.bytes_written;
            total_commands += metrics.command_count;

            if manager_guard.is_active() {
                active_count += 1;
            }

            if let Some(startup_time) = metrics.startup_time_ms {
                startup_times.push(startup_time);
            }
        }

        if !startup_times.is_empty() {
            avg_startup_time = startup_times.iter().sum::<u64>() / startup_times.len() as u64;
        }

        PtyCollectionMetrics {
            total_pty_count: self.managers.len(),
            active_pty_count: active_count,
            total_bytes_read,
            total_bytes_written,
            total_command_count: total_commands,
            avg_startup_time_ms: avg_startup_time,
        }
    }
}

impl Default for PtyManagerCollection {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregated metrics for the PTY collection
#[derive(Debug, Clone)]
pub struct PtyCollectionMetrics {
    pub total_pty_count: usize,
    pub active_pty_count: usize,
    pub total_bytes_read: u64,
    pub total_bytes_written: u64,
    pub total_command_count: u64,
    pub avg_startup_time_ms: u64,
}

/// Errors that can occur in PTY management
#[derive(Debug, thiserror::Error)]
pub enum PtyManagerError {
    #[error("Failed to create PTY: {0}")]
    PtyCreation(String),

    #[error("PTY not found: {0}")]
    PtyNotFound(PtyId),

    #[error("PTY not active: {0}")]
    PtyNotActive(PtyId),

    #[error("Invalid working directory: {0}")]
    InvalidWorkingDirectory(String),

    #[error("Shell configuration error: {0}")]
    ShellConfigError(String),

    #[error("Context error: {0}")]
    ContextError(String),
}

impl Default for PtyContext {
    fn default() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            shell_kind: ShellKind::Unknown,
            last_command: None,
            environment: HashMap::new(),
            shell_config: ShellConfig::default(),
            created_at: Instant::now(),
            last_activity: Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_kind_detection() {
        assert_eq!(ShellKind::from_shell_name("bash"), ShellKind::Bash);
        assert_eq!(ShellKind::from_shell_name("/usr/bin/zsh"), ShellKind::Zsh);
        assert_eq!(ShellKind::from_shell_name("fish"), ShellKind::Fish);
        assert_eq!(ShellKind::from_shell_name("powershell.exe"), ShellKind::PowerShell);
        assert_eq!(ShellKind::from_shell_name("pwsh"), ShellKind::PowerShell);
        assert_eq!(ShellKind::from_shell_name("unknown_shell"), ShellKind::Unknown);
    }

    #[test]
    fn test_pty_context_creation() {
        let context = PtyContext::default();
        assert_eq!(context.shell_kind, ShellKind::Unknown);
        assert!(
            context.working_directory.is_absolute()
                || context.working_directory == std::path::Path::new("/")
        );
        assert!(context.last_command.is_none());
    }

    #[test]
    fn test_pty_manager_collection() {
        let mut collection = PtyManagerCollection::new();
        assert_eq!(collection.count(), 0);

        let shell_config = ShellConfig::default();
        let env = HashMap::new();
        let working_dir = PathBuf::from("/tmp");

        let pty_id = collection.create_pty_manager(working_dir, shell_config, env).unwrap();
        assert_eq!(collection.count(), 1);
        assert!(collection.get_manager(pty_id).is_some());

        collection.remove_manager(pty_id);
        assert_eq!(collection.count(), 0);
    }
}

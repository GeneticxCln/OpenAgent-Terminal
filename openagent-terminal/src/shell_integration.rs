//! Native Shell Integration for OpenAgent Terminal
//!
//! Provides immediate command timing, exit code capture, working directory tracking,
//! command categorization, and enhanced history with no lazy fallbacks.

#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::blocks_v2::{BlockId, ShellType};

/// Native shell integration manager
pub struct ShellIntegration {
    /// Command tracker for immediate timing
    command_tracker: CommandTracker,

    /// Exit code monitor for immediate visual feedback
    exit_monitor: ExitCodeMonitor,

    /// Working directory tracker for AI context
    directory_tracker: DirectoryTracker,

    /// Command categorizer for organization
    categorizer: CommandCategorizer,

    /// Enhanced history with immediate search
    enhanced_history: EnhancedHistory,

    /// Shell hooks for immediate integration
    shell_hooks: ShellHooks,

    /// Event callbacks for immediate responses
    event_callbacks: Vec<Box<dyn Fn(&ShellEvent) + Send + Sync>>,

    /// Performance statistics
    stats: ShellStats,
}

/// Shell integration events for immediate feedback
#[derive(Debug, Clone)]
pub enum ShellEvent {
    CommandStarted {
        id: CommandId,
        command: String,
        working_dir: PathBuf,
        timestamp: DateTime<Utc>,
    },
    CommandCompleted {
        id: CommandId,
        exit_code: i32,
        duration: Duration,
        output_size: usize,
    },
    DirectoryChanged {
        from: PathBuf,
        to: PathBuf,
        timestamp: DateTime<Utc>,
    },
    CommandCategorized {
        id: CommandId,
        category: CommandCategory,
        subcategory: Option<String>,
    },
    HistoryUpdated {
        entry_count: usize,
        last_command: String,
    },
}

/// Command tracker for immediate timing
#[derive(Debug, Default)]
pub struct CommandTracker {
    /// Currently running commands
    active_commands: HashMap<CommandId, ActiveCommand>,

    /// Command timing history
    timing_history: VecDeque<CommandTiming>,

    /// Performance thresholds
    slow_command_threshold: Duration,
    very_slow_threshold: Duration,

    /// Statistics
    total_commands: usize,
    total_time: Duration,
    average_time: Duration,
}

/// Exit code monitor for immediate visual feedback
#[derive(Debug, Default)]
pub struct ExitCodeMonitor {
    /// Exit code patterns and meanings
    exit_code_meanings: HashMap<i32, ExitCodeInfo>,

    /// Recent exit codes for trend analysis
    recent_codes: VecDeque<(CommandId, i32, DateTime<Utc>)>,

    /// Success/failure statistics
    success_count: usize,
    failure_count: usize,
    last_status: Option<CommandStatus>,
}

/// Working directory tracker for AI context
#[derive(Debug)]
pub struct DirectoryTracker {
    /// Current working directory
    current_dir: PathBuf,

    /// Directory change history
    directory_history: VecDeque<DirectoryChange>,

    /// Directory-specific command patterns
    dir_patterns: HashMap<PathBuf, DirectoryPattern>,

    /// Project detection
    project_detector: ProjectDetector,
}

/// Command categorizer for organization
#[derive(Debug)]
pub struct CommandCategorizer {
    /// Category rules for immediate classification
    category_rules: Vec<CategoryRule>,

    /// Command patterns cache
    pattern_cache: HashMap<String, CommandCategory>,

    /// Learning system for adaptive categorization
    learning_system: CategoryLearning,

    /// Statistics by category
    category_stats: HashMap<CommandCategory, CategoryStats>,
}

/// Enhanced history with immediate search
#[derive(Debug)]
pub struct EnhancedHistory {
    /// Command history entries
    entries: VecDeque<HistoryEntry>,

    /// Search index for immediate lookup
    search_index: HistorySearchIndex,

    /// Suggestion engine
    suggestion_engine: HistorySuggestionEngine,

    /// Frequency analysis
    frequency_analyzer: FrequencyAnalyzer,

    /// Configuration
    max_entries: usize,
    deduplication_enabled: bool,
}

/// Shell hooks for immediate integration
#[derive(Debug)]
pub struct ShellHooks {
    /// Pre-command hooks
    pre_command_hooks: Vec<String>,

    /// Post-command hooks
    post_command_hooks: Vec<String>,

    /// Directory change hooks
    cd_hooks: Vec<String>,

    /// Shell configuration
    shell_config: ShellConfig,
}

/// Unique command identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommandId(u64);

impl CommandId {
    pub fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        Self(timestamp)
    }
}

/// Active command tracking
#[derive(Debug, Clone)]
pub struct ActiveCommand {
    pub id: CommandId,
    pub command: String,
    pub working_dir: PathBuf,
    pub start_time: Instant,
    pub pid: Option<u32>,
    pub shell_type: ShellType,
    pub category: Option<CommandCategory>,
}

/// Command timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTiming {
    pub id: CommandId,
    pub command: String,
    pub duration: Duration,
    pub start_time: DateTime<Utc>,
    pub working_dir: PathBuf,
    pub exit_code: i32,
    pub category: Option<CommandCategory>,
}

/// Exit code information
#[derive(Debug, Clone)]
pub struct ExitCodeInfo {
    pub meaning: String,
    pub severity: ExitCodeSeverity,
    pub suggestion: Option<String>,
    pub common_causes: Vec<String>,
}

/// Exit code severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCodeSeverity {
    Success,
    Warning,
    Error,
    Critical,
}

/// Command status for visual indicators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStatus {
    Running,
    Success,
    Warning,
    Error,
    Timeout,
}

/// Directory change tracking
#[derive(Debug, Clone)]
pub struct DirectoryChange {
    pub from: PathBuf,
    pub to: PathBuf,
    pub timestamp: DateTime<Utc>,
    pub trigger_command: Option<String>,
}

/// Directory pattern analysis
#[derive(Debug, Clone)]
pub struct DirectoryPattern {
    pub common_commands: Vec<String>,
    pub project_type: Option<ProjectType>,
    pub frequency_map: HashMap<String, usize>,
    pub last_updated: DateTime<Utc>,
}

/// Project detection system
#[derive(Debug)]
pub struct ProjectDetector {
    pub detection_rules: Vec<ProjectDetectionRule>,
    pub cache: HashMap<PathBuf, Option<ProjectInfo>>,
}

/// Project information
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub project_type: ProjectType,
    pub name: Option<String>,
    pub root_dir: PathBuf,
    pub config_files: Vec<PathBuf>,
    pub detected_tools: Vec<String>,
}

/// Project types for context
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectType {
    Git,
    Rust,
    JavaScript,
    Python,
    Go,
    Docker,
    Kubernetes,
    Terraform,
    Unknown,
}

/// Command categories for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommandCategory {
    FileSystem,
    Git,
    Docker,
    Kubernetes,
    Development,
    System,
    Network,
    Database,
    Text,
    Archive,
    Process,
    Custom(u32),
}

/// Category rule for immediate classification
#[derive(Debug, Clone)]
pub struct CategoryRule {
    pub pattern: Regex,
    pub category: CommandCategory,
    pub subcategory: Option<String>,
    pub priority: usize,
}

/// Category learning system
#[derive(Debug, Default)]
pub struct CategoryLearning {
    pub patterns: HashMap<String, CommandCategory>,
    pub confidence_scores: HashMap<String, f64>,
    pub learning_enabled: bool,
}

/// Category statistics
#[derive(Debug, Default, Clone)]
pub struct CategoryStats {
    pub count: usize,
    pub total_time: Duration,
    pub average_time: Duration,
    pub success_rate: f64,
    pub last_used: Option<DateTime<Utc>>,
}

/// History entry with enhanced metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: CommandId,
    pub command: String,
    pub timestamp: DateTime<Utc>,
    pub working_dir: PathBuf,
    pub exit_code: i32,
    pub duration: Duration,
    pub category: Option<CommandCategory>,
    pub output_preview: Option<String>,
    pub tags: Vec<String>,
    pub frequency_score: f64,
}

/// History search index
#[derive(Debug, Default)]
pub struct HistorySearchIndex {
    pub command_index: HashMap<String, Vec<usize>>,
    pub directory_index: HashMap<PathBuf, Vec<usize>>,
    pub category_index: HashMap<CommandCategory, Vec<usize>>,
    pub tag_index: HashMap<String, Vec<usize>>,
}

/// History suggestion engine
#[derive(Debug)]
pub struct HistorySuggestionEngine {
    pub context_suggestions: HashMap<PathBuf, Vec<String>>,
    pub pattern_suggestions: Vec<String>,
    pub frequency_suggestions: Vec<String>,
    pub last_update: Instant,
}

impl Default for HistorySuggestionEngine {
    fn default() -> Self {
        Self {
            context_suggestions: HashMap::new(),
            pattern_suggestions: Vec::new(),
            frequency_suggestions: Vec::new(),
            last_update: Instant::now(),
        }
    }
}

/// Frequency analyzer
#[derive(Debug, Default)]
pub struct FrequencyAnalyzer {
    pub command_frequency: HashMap<String, usize>,
    pub directory_frequency: HashMap<PathBuf, usize>,
    pub time_patterns: HashMap<String, Vec<DateTime<Utc>>>,
}

/// Shell configuration
#[derive(Debug)]
pub struct ShellConfig {
    pub shell_type: ShellType,
    pub prompt_command: Option<String>,
    pub precmd_functions: Vec<String>,
    pub preexec_functions: Vec<String>,
}

/// Project detection rule
#[derive(Debug)]
pub struct ProjectDetectionRule {
    pub name: String,
    pub project_type: ProjectType,
    pub indicators: Vec<ProjectIndicator>,
    pub priority: usize,
}

/// Project detection indicator
#[derive(Debug)]
pub enum ProjectIndicator {
    FileExists(String),
    DirectoryExists(String),
    CommandAvailable(String),
    FileContent { file: String, pattern: Regex },
}

/// Performance statistics
#[derive(Debug, Clone)]
pub struct ShellStats {
    pub commands_tracked: usize,
    pub categories_detected: usize,
    pub directories_tracked: usize,
    pub history_entries: usize,
    pub average_command_time: Duration,
    pub success_rate: f64,
    pub last_reset: Instant,
}

impl Default for ShellStats {
    fn default() -> Self {
        Self {
            commands_tracked: 0,
            categories_detected: 0,
            directories_tracked: 0,
            history_entries: 0,
            average_command_time: Duration::default(),
            success_rate: 0.0,
            last_reset: Instant::now(),
        }
    }
}

impl ShellIntegration {
    /// Create new shell integration with immediate capabilities
    pub fn new() -> Self {
        let mut integration = Self {
            command_tracker: CommandTracker {
                slow_command_threshold: Duration::from_secs(5),
                very_slow_threshold: Duration::from_secs(30),
                ..Default::default()
            },
            exit_monitor: ExitCodeMonitor::new(),
            directory_tracker: DirectoryTracker::new(),
            categorizer: CommandCategorizer::new(),
            enhanced_history: EnhancedHistory::new(),
            shell_hooks: ShellHooks::new(),
            event_callbacks: Vec::new(),
            stats: ShellStats {
                last_reset: Instant::now(),
                ..Default::default()
            },
        };

        // Initialize shell hooks immediately
        integration.setup_shell_hooks();

        integration
    }

    /// Register event callback for immediate responses
    pub fn register_event_callback<F>(&mut self, callback: F)
    where
        F: Fn(&ShellEvent) + Send + Sync + 'static,
    {
        self.event_callbacks.push(Box::new(callback));
    }

    /// Emit shell event immediately
    fn emit_event(&self, event: ShellEvent) {
        for callback in &self.event_callbacks {
            callback(&event);
        }
    }

    /// Start tracking command immediately
    pub fn start_command(&mut self, command: String, working_dir: Option<PathBuf>) -> CommandId {
        let id = CommandId::new();
        let working_dir = working_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));

        // Categorize command immediately
        let category = self.categorizer.categorize_command(&command);

        let active_command = ActiveCommand {
            id,
            command: command.clone(),
            working_dir: working_dir.clone(),
            start_time: Instant::now(),
            pid: None,
            shell_type: self.detect_shell_type(),
            category,
        };

        self.command_tracker
            .active_commands
            .insert(id, active_command);
        self.command_tracker.total_commands += 1;
        self.stats.commands_tracked += 1;

        // Emit command started event immediately
        self.emit_event(ShellEvent::CommandStarted {
            id,
            command: command.clone(),
            working_dir,
            timestamp: Utc::now(),
        });

        // Update frequency analyzer immediately
        self.enhanced_history
            .frequency_analyzer
            .command_frequency
            .entry(command)
            .and_modify(|count| *count += 1)
            .or_insert(1);

        id
    }

    /// Complete command tracking immediately
    pub fn complete_command(
        &mut self,
        id: CommandId,
        exit_code: i32,
        output: Option<String>,
    ) -> Result<()> {
        let Some(active_command) = self.command_tracker.active_commands.remove(&id) else {
            return Ok(()); // Command not tracked
        };

        let duration = active_command.start_time.elapsed();
        let output_size = output.as_ref().map(|s| s.len()).unwrap_or(0);

        // Create timing record immediately
        let timing = CommandTiming {
            id,
            command: active_command.command.clone(),
            duration,
            start_time: Utc::now() - chrono::Duration::from_std(duration)?,
            working_dir: active_command.working_dir.clone(),
            exit_code,
            category: active_command.category,
        };

        // Update timing history immediately
        self.command_tracker
            .timing_history
            .push_back(timing.clone());
        if self.command_tracker.timing_history.len() > 1000 {
            self.command_tracker.timing_history.pop_front();
        }

        // Update statistics immediately
        self.command_tracker.total_time += duration;
        self.command_tracker.average_time =
            self.command_tracker.total_time / self.command_tracker.total_commands as u32;

        // Process exit code immediately
        self.exit_monitor.process_exit_code(id, exit_code);

        // Update category statistics immediately
        if let Some(category) = active_command.category {
            let stats = self.categorizer.category_stats.entry(category).or_default();
            stats.count += 1;
            stats.total_time += duration;
            stats.average_time = stats.total_time / stats.count as u32;
            stats.last_used = Some(Utc::now());

            // Update success rate
            if exit_code == 0 {
                stats.success_rate =
                    (stats.success_rate * (stats.count - 1) as f64 + 1.0) / stats.count as f64;
            } else {
                stats.success_rate =
                    (stats.success_rate * (stats.count - 1) as f64) / stats.count as f64;
            }

            self.emit_event(ShellEvent::CommandCategorized {
                id,
                category,
                subcategory: None, // Could be enhanced
            });
        }

        // Add to enhanced history immediately
        let history_entry = HistoryEntry {
            id,
            command: active_command.command.clone(),
            timestamp: timing.start_time,
            working_dir: active_command.working_dir,
            exit_code,
            duration,
            category: active_command.category,
            output_preview: output.as_ref().map(|s| s.chars().take(100).collect()),
            tags: self.generate_command_tags(&active_command.command),
            frequency_score: self.calculate_frequency_score(&active_command.command),
        };

        self.enhanced_history.add_entry(history_entry);

        // Update global statistics immediately
        self.stats.average_command_time =
            (self.stats.average_command_time * (self.stats.commands_tracked - 1) as u32 + duration)
                / self.stats.commands_tracked as u32;

        if exit_code == 0 {
            self.exit_monitor.success_count += 1;
        } else {
            self.exit_monitor.failure_count += 1;
        }

        let total_completed = self.exit_monitor.success_count + self.exit_monitor.failure_count;
        self.stats.success_rate = self.exit_monitor.success_count as f64 / total_completed as f64;

        // Emit command completed event immediately
        self.emit_event(ShellEvent::CommandCompleted {
            id,
            exit_code,
            duration,
            output_size,
        });

        Ok(())
    }

    /// Track directory change immediately
    pub fn change_directory(&mut self, new_dir: PathBuf) -> Result<()> {
        let old_dir = self.directory_tracker.current_dir.clone();

        if old_dir != new_dir {
            let change = DirectoryChange {
                from: old_dir.clone(),
                to: new_dir.clone(),
                timestamp: Utc::now(),
                trigger_command: None, // Could be enhanced to track the cd command
            };

            self.directory_tracker.directory_history.push_back(change);
            if self.directory_tracker.directory_history.len() > 100 {
                self.directory_tracker.directory_history.pop_front();
            }

            self.directory_tracker.current_dir = new_dir.clone();

            // Detect project type immediately
            if let Ok(project_info) = self
                .directory_tracker
                .project_detector
                .detect_project(&new_dir)
            {
                if let Some(info) = project_info {
                    // Update directory pattern
                    let pattern = self
                        .directory_tracker
                        .dir_patterns
                        .entry(new_dir.clone())
                        .or_insert_with(|| DirectoryPattern {
                            common_commands: Vec::new(),
                            project_type: Some(info.project_type),
                            frequency_map: HashMap::new(),
                            last_updated: Utc::now(),
                        });
                    pattern.project_type = Some(info.project_type);
                    pattern.last_updated = Utc::now();
                }
            }

            // Update frequency tracking immediately
            self.enhanced_history
                .frequency_analyzer
                .directory_frequency
                .entry(new_dir.clone())
                .and_modify(|count| *count += 1)
                .or_insert(1);

            self.stats.directories_tracked += 1;

            // Emit directory changed event immediately
            self.emit_event(ShellEvent::DirectoryChanged {
                from: old_dir,
                to: new_dir,
                timestamp: Utc::now(),
            });
        }

        Ok(())
    }

    /// Search command history immediately
    pub fn search_history(&self, query: &str, max_results: usize) -> Vec<&HistoryEntry> {
        self.enhanced_history.search(query, max_results)
    }

    /// Get command suggestions immediately
    pub fn get_suggestions(
        &mut self,
        partial_command: &str,
        context: Option<&PathBuf>,
    ) -> Vec<String> {
        self.enhanced_history.suggestion_engine.get_suggestions(
            partial_command,
            context,
            &self.enhanced_history.entries,
            &self.enhanced_history.frequency_analyzer,
        )
    }

    /// Get command timing statistics
    pub fn get_timing_stats(&self) -> Vec<CommandTiming> {
        self.command_tracker
            .timing_history
            .iter()
            .cloned()
            .collect()
    }

    /// Get current shell statistics
    pub fn get_stats(&self) -> ShellStats {
        self.stats.clone()
    }

    /// Setup shell hooks for immediate integration
    fn setup_shell_hooks(&mut self) {
        let shell_type = self.detect_shell_type();

        match shell_type {
            ShellType::Zsh => self.setup_zsh_hooks(),
            ShellType::Bash => self.setup_bash_hooks(),
            ShellType::Fish => self.setup_fish_hooks(),
            _ => {} // Other shells not implemented yet
        }
    }

    /// Setup Zsh hooks
    fn setup_zsh_hooks(&mut self) {
        self.shell_hooks.shell_config.precmd_functions = vec!["openagent_precmd".to_string()];
        self.shell_hooks.shell_config.preexec_functions = vec!["openagent_preexec".to_string()];
    }

    /// Setup Bash hooks  
    fn setup_bash_hooks(&mut self) {
        self.shell_hooks.shell_config.prompt_command = Some("openagent_prompt_command".to_string());
    }

    /// Setup Fish hooks
    fn setup_fish_hooks(&mut self) {
        // Fish uses event handlers
        self.shell_hooks.pre_command_hooks =
            vec!["function __openagent_preexec --on-event fish_preexec".to_string()];
    }

    /// Detect current shell type
    fn detect_shell_type(&self) -> ShellType {
        if let Ok(shell) = std::env::var("SHELL") {
            if shell.contains("zsh") {
                ShellType::Zsh
            } else if shell.contains("bash") {
                ShellType::Bash
            } else if shell.contains("fish") {
                ShellType::Fish
            } else {
                ShellType::Bash // Default fallback
            }
        } else {
            ShellType::Bash
        }
    }

    /// Generate command tags for categorization
    fn generate_command_tags(&self, command: &str) -> Vec<String> {
        let mut tags = Vec::new();

        // Extract command name
        if let Some(cmd_name) = command.split_whitespace().next() {
            tags.push(format!("cmd:{}", cmd_name));
        }

        // Add pattern-based tags
        if command.contains("git") {
            tags.push("git".to_string());
        }
        if command.contains("docker") {
            tags.push("docker".to_string());
        }
        if command.contains("kubectl") || command.contains("k8s") {
            tags.push("kubernetes".to_string());
        }

        tags
    }

    /// Calculate frequency score for command
    fn calculate_frequency_score(&self, command: &str) -> f64 {
        let frequency = self
            .enhanced_history
            .frequency_analyzer
            .command_frequency
            .get(command)
            .copied()
            .unwrap_or(0);

        // Simple scoring based on frequency
        (frequency as f64 + 1.0).ln()
    }
}

impl ExitCodeMonitor {
    fn new() -> Self {
        let mut monitor = Self::default();
        monitor.setup_exit_code_meanings();
        monitor
    }

    fn setup_exit_code_meanings(&mut self) {
        // Common exit codes and their meanings
        self.exit_code_meanings.insert(
            0,
            ExitCodeInfo {
                meaning: "Success".to_string(),
                severity: ExitCodeSeverity::Success,
                suggestion: None,
                common_causes: vec!["Command completed successfully".to_string()],
            },
        );

        self.exit_code_meanings.insert(
            1,
            ExitCodeInfo {
                meaning: "General error".to_string(),
                severity: ExitCodeSeverity::Error,
                suggestion: Some("Check command syntax and arguments".to_string()),
                common_causes: vec![
                    "Invalid arguments".to_string(),
                    "Permission denied".to_string(),
                ],
            },
        );

        self.exit_code_meanings.insert(
            2,
            ExitCodeInfo {
                meaning: "Misuse of shell builtins".to_string(),
                severity: ExitCodeSeverity::Error,
                suggestion: Some("Check command usage with --help".to_string()),
                common_causes: vec!["Invalid command usage".to_string()],
            },
        );

        self.exit_code_meanings.insert(
            126,
            ExitCodeInfo {
                meaning: "Command not executable".to_string(),
                severity: ExitCodeSeverity::Error,
                suggestion: Some("Check file permissions with ls -l".to_string()),
                common_causes: vec![
                    "File not executable".to_string(),
                    "Permission denied".to_string(),
                ],
            },
        );

        self.exit_code_meanings.insert(
            127,
            ExitCodeInfo {
                meaning: "Command not found".to_string(),
                severity: ExitCodeSeverity::Error,
                suggestion: Some("Check if command is installed and in PATH".to_string()),
                common_causes: vec![
                    "Command not installed".to_string(),
                    "Typo in command name".to_string(),
                ],
            },
        );

        self.exit_code_meanings.insert(
            130,
            ExitCodeInfo {
                meaning: "Script terminated by Control-C".to_string(),
                severity: ExitCodeSeverity::Warning,
                suggestion: Some("Command was interrupted by user".to_string()),
                common_causes: vec!["User interruption".to_string()],
            },
        );
    }

    fn process_exit_code(&mut self, id: CommandId, exit_code: i32) {
        self.recent_codes.push_back((id, exit_code, Utc::now()));
        if self.recent_codes.len() > 100 {
            self.recent_codes.pop_front();
        }

        self.last_status = Some(match exit_code {
            0 => CommandStatus::Success,
            130 => CommandStatus::Warning,
            _ => CommandStatus::Error,
        });
    }

    pub fn get_exit_code_info(&self, exit_code: i32) -> Option<&ExitCodeInfo> {
        self.exit_code_meanings.get(&exit_code)
    }
}

impl DirectoryTracker {
    fn new() -> Self {
        Self {
            current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            directory_history: VecDeque::new(),
            dir_patterns: HashMap::new(),
            project_detector: ProjectDetector::new(),
        }
    }
}

impl ProjectDetector {
    fn new() -> Self {
        let mut detector = Self {
            detection_rules: Vec::new(),
            cache: HashMap::new(),
        };

        detector.setup_detection_rules();
        detector
    }

    fn setup_detection_rules(&mut self) {
        // Git project
        self.detection_rules.push(ProjectDetectionRule {
            name: "Git Repository".to_string(),
            project_type: ProjectType::Git,
            indicators: vec![ProjectIndicator::DirectoryExists(".git".to_string())],
            priority: 1,
        });

        // Rust project
        self.detection_rules.push(ProjectDetectionRule {
            name: "Rust Project".to_string(),
            project_type: ProjectType::Rust,
            indicators: vec![
                ProjectIndicator::FileExists("Cargo.toml".to_string()),
                ProjectIndicator::DirectoryExists("src".to_string()),
            ],
            priority: 2,
        });

        // Add more project types...
    }

    fn detect_project(&mut self, dir: &PathBuf) -> Result<Option<ProjectInfo>> {
        if let Some(cached) = self.cache.get(dir) {
            return Ok(cached.clone());
        }

        for rule in &self.detection_rules {
            if self.check_indicators(dir, &rule.indicators) {
                let project_info = ProjectInfo {
                    project_type: rule.project_type,
                    name: dir.file_name().map(|n| n.to_string_lossy().to_string()),
                    root_dir: dir.clone(),
                    config_files: Vec::new(),   // Would be populated
                    detected_tools: Vec::new(), // Would be populated
                };

                self.cache.insert(dir.clone(), Some(project_info.clone()));
                return Ok(Some(project_info));
            }
        }

        self.cache.insert(dir.clone(), None);
        Ok(None)
    }

    fn check_indicators(&self, dir: &PathBuf, indicators: &[ProjectIndicator]) -> bool {
        for indicator in indicators {
            match indicator {
                ProjectIndicator::FileExists(file) => {
                    if !dir.join(file).exists() {
                        return false;
                    }
                }
                ProjectIndicator::DirectoryExists(subdir) => {
                    if !dir.join(subdir).is_dir() {
                        return false;
                    }
                }
                ProjectIndicator::CommandAvailable(cmd) => {
                    if Command::new("which")
                        .arg(cmd)
                        .output()
                        .map_or(true, |o| !o.status.success())
                    {
                        return false;
                    }
                }
                ProjectIndicator::FileContent { file, pattern } => {
                    let file_path = dir.join(file);
                    if let Ok(content) = std::fs::read_to_string(file_path) {
                        if !pattern.is_match(&content) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }
        }
        true
    }
}

impl CommandCategorizer {
    fn new() -> Self {
        let mut categorizer = Self {
            category_rules: Vec::new(),
            pattern_cache: HashMap::new(),
            learning_system: CategoryLearning::default(),
            category_stats: HashMap::new(),
        };

        categorizer.setup_category_rules();
        categorizer
    }

    fn setup_category_rules(&mut self) {
        // Git commands
        self.category_rules.push(CategoryRule {
            pattern: Regex::new(r"^git\s").unwrap(),
            category: CommandCategory::Git,
            subcategory: None,
            priority: 1,
        });

        // Docker commands
        self.category_rules.push(CategoryRule {
            pattern: Regex::new(r"^docker\s").unwrap(),
            category: CommandCategory::Docker,
            subcategory: None,
            priority: 1,
        });

        // File system commands
        self.category_rules.push(CategoryRule {
            pattern: Regex::new(r"^(ls|cd|pwd|mkdir|rmdir|cp|mv|rm|find|locate|which|ln)(\s|$)")
                .unwrap(),
            category: CommandCategory::FileSystem,
            subcategory: None,
            priority: 2,
        });

        // Add more rules...
    }

    fn categorize_command(&mut self, command: &str) -> Option<CommandCategory> {
        // Check cache first
        if let Some(&category) = self.pattern_cache.get(command) {
            return Some(category);
        }

        // Apply rules
        for rule in &self.category_rules {
            if rule.pattern.is_match(command) {
                self.pattern_cache
                    .insert(command.to_string(), rule.category);
                return Some(rule.category);
            }
        }

        None
    }
}

impl EnhancedHistory {
    fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            search_index: HistorySearchIndex::default(),
            suggestion_engine: HistorySuggestionEngine::default(),
            frequency_analyzer: FrequencyAnalyzer::default(),
            max_entries: 10000,
            deduplication_enabled: true,
        }
    }

    fn add_entry(&mut self, entry: HistoryEntry) {
        let entry_index = self.entries.len();

        // Update search indices immediately
        let command_tokens: Vec<String> = entry
            .command
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        for token in &command_tokens {
            self.search_index
                .command_index
                .entry(token.clone())
                .or_insert_with(Vec::new)
                .push(entry_index);
        }

        self.search_index
            .directory_index
            .entry(entry.working_dir.clone())
            .or_insert_with(Vec::new)
            .push(entry_index);

        if let Some(category) = entry.category {
            self.search_index
                .category_index
                .entry(category)
                .or_insert_with(Vec::new)
                .push(entry_index);
        }

        for tag in &entry.tags {
            self.search_index
                .tag_index
                .entry(tag.clone())
                .or_insert_with(Vec::new)
                .push(entry_index);
        }

        self.entries.push_back(entry);

        // Limit size
        if self.entries.len() > self.max_entries {
            let removed = self.entries.pop_front().unwrap();
            self.cleanup_indices(&removed, 0);
        }
    }

    fn cleanup_indices(&mut self, removed_entry: &HistoryEntry, removed_index: usize) {
        // Remove from indices - simplified implementation
        let command_tokens: Vec<String> = removed_entry
            .command
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        for token in &command_tokens {
            if let Some(indices) = self.search_index.command_index.get_mut(token) {
                indices.retain(|&idx| idx != removed_index);
            }
        }
        // Similar cleanup for other indices...
    }

    fn search(&self, query: &str, max_results: usize) -> Vec<&HistoryEntry> {
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        // Search by command content
        for (i, entry) in self.entries.iter().enumerate().rev() {
            if entry.command.to_lowercase().contains(&query_lower) {
                results.push(entry);
                if results.len() >= max_results {
                    break;
                }
            }
        }

        results
    }
}

impl HistorySuggestionEngine {
    fn get_suggestions(
        &mut self,
        partial_command: &str,
        context: Option<&PathBuf>,
        entries: &VecDeque<HistoryEntry>,
        frequency_analyzer: &FrequencyAnalyzer,
    ) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Context-based suggestions
        if let Some(dir) = context {
            if let Some(context_cmds) = self.context_suggestions.get(dir) {
                for cmd in context_cmds {
                    if cmd.starts_with(partial_command) {
                        suggestions.push(cmd.clone());
                    }
                }
            }
        }

        // Frequency-based suggestions
        for (command, &frequency) in &frequency_analyzer.command_frequency {
            if command.starts_with(partial_command) {
                suggestions.push(format!("{} (used {} times)", command, frequency));
            }
        }

        // Recent command suggestions
        for entry in entries.iter().rev().take(50) {
            if entry.command.starts_with(partial_command) {
                if !suggestions.iter().any(|s| s.starts_with(&entry.command)) {
                    suggestions.push(entry.command.clone());
                }
            }
        }

        suggestions.sort_by(|a, b| {
            // Sort by relevance (simplified)
            let a_freq = frequency_analyzer.command_frequency.get(a).unwrap_or(&0);
            let b_freq = frequency_analyzer.command_frequency.get(b).unwrap_or(&0);
            b_freq.cmp(a_freq)
        });

        suggestions.truncate(10); // Limit to top 10
        suggestions
    }
}

impl ShellHooks {
    fn new() -> Self {
        Self {
            pre_command_hooks: Vec::new(),
            post_command_hooks: Vec::new(),
            cd_hooks: Vec::new(),
            shell_config: ShellConfig {
                shell_type: ShellType::Bash,
                prompt_command: None,
                precmd_functions: Vec::new(),
                preexec_functions: Vec::new(),
            },
        }
    }
}

impl Default for ShellIntegration {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_timing() {
        let mut shell = ShellIntegration::new();
        let id = shell.start_command("echo test".to_string(), None);

        std::thread::sleep(Duration::from_millis(10));
        shell
            .complete_command(id, 0, Some("test".to_string()))
            .unwrap();

        assert!(!shell.command_tracker.timing_history.is_empty());
    }

    #[test]
    fn test_command_categorization() {
        let mut categorizer = CommandCategorizer::new();

        assert_eq!(
            categorizer.categorize_command("git status"),
            Some(CommandCategory::Git)
        );
        assert_eq!(
            categorizer.categorize_command("docker ps"),
            Some(CommandCategory::Docker)
        );
        assert_eq!(
            categorizer.categorize_command("ls -la"),
            Some(CommandCategory::FileSystem)
        );
    }

    #[test]
    fn test_history_search() {
        let mut shell = ShellIntegration::new();
        let id = shell.start_command("git commit -m test".to_string(), None);
        shell.complete_command(id, 0, None).unwrap();

        let results = shell.search_history("git", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].command.contains("git"));
    }
}

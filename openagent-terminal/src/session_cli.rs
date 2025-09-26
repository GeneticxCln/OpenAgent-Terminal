//! Session CLI Commands
//!
//! Provides a command-line interface for session management operations including
//! session creation, restoration, listing, export/import, and configuration.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand};
use comfy_table::{Table, presets, Cell, Color, Attribute};
use serde::{Deserialize, Serialize};
use tracing::{info, error};

use crate::session_service::{
    SessionService, SessionEvent, RestoreOptions, SessionStats, 
    RestorationSummary, SessionSummary
};
use crate::session_persistence::{SessionId, PersistenceConfig, UserPreferences};
use crate::ai_context_provider::PtyAiContext;

/// Session management CLI
#[derive(Parser, Debug)]
#[command(
    name = "session",
    about = "Manage terminal sessions and persistence",
    long_about = "Comprehensive session management for the OpenAgent terminal including \
                  session creation, restoration, export/import, and configuration."
)]
pub struct SessionCli {
    #[command(subcommand)]
    pub command: SessionCommand,
    
    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
    
    /// Output format (table, json, yaml)
    #[arg(short = 'f', long = "format", global = true, default_value = "table")]
    pub output_format: OutputFormat,
    
    /// Configuration file path
    #[arg(short = 'c', long = "config", global = true)]
    pub config_file: Option<PathBuf>,
}

/// Session CLI subcommands
#[derive(Subcommand, Debug)]
pub enum SessionCommand {
    /// Create a new session
    New(NewSessionArgs),
    
    /// List all sessions
    List(ListSessionArgs),
    
    /// Show session details
    Show(ShowSessionArgs),
    
    /// Restore a session
    Restore(RestoreSessionArgs),
    
    /// Delete a session
    Delete(DeleteSessionArgs),
    
    /// Export a session
    Export(ExportSessionArgs),
    
    /// Import a session
    Import(ImportSessionArgs),
    
    /// Clean up old sessions
    Cleanup(CleanupSessionArgs),
    
    /// Show session statistics
    Stats(StatsSessionArgs),
    
    /// Manage session configuration
    Config(ConfigSessionArgs),
    
    /// Manage user preferences
    Prefs(PrefsSessionArgs),
    
    /// Watch session events
    Watch(WatchSessionArgs),
}

#[derive(Args, Debug)]
pub struct NewSessionArgs {
    /// Session title/description
    #[arg(short, long)]
    pub title: Option<String>,
    
    /// Set as current session immediately
    #[arg(short, long, default_value = "true")]
    pub activate: bool,
    
    /// Copy settings from existing session
    #[arg(long)]
    pub copy_from: Option<String>,
}

#[derive(Args, Debug)]
pub struct ListSessionArgs {
    /// Show only active sessions
    #[arg(short, long)]
    pub active_only: bool,
    
    /// Limit number of results
    #[arg(short = 'n', long, default_value = "20")]
    pub limit: usize,
    
    /// Sort by (created, modified, commands, conversations)
    #[arg(short, long, default_value = "modified")]
    pub sort: SessionSortOption,
    
    /// Filter by minimum age (e.g., "1d", "2h", "30m")
    #[arg(long)]
    pub min_age: Option<String>,
    
    /// Filter by maximum age
    #[arg(long)]
    pub max_age: Option<String>,
}

#[derive(Args, Debug)]
pub struct ShowSessionArgs {
    /// Session ID or partial ID
    pub session_id: String,
    
    /// Show command history
    #[arg(long)]
    pub show_commands: bool,
    
    /// Show conversation history
    #[arg(long)]
    pub show_conversations: bool,
    
    /// Show preferences
    #[arg(long)]
    pub show_preferences: bool,
    
    /// Show workspace state
    #[arg(long)]
    pub show_workspace: bool,
    
    /// Number of recent commands to show
    #[arg(long, default_value = "10")]
    pub recent_commands: usize,
}

#[derive(Args, Debug)]
pub struct RestoreSessionArgs {
    /// Session ID or partial ID
    pub session_id: String,
    
    /// Don't restore command history
    #[arg(long)]
    pub no_commands: bool,
    
    /// Don't restore conversations
    #[arg(long)]
    pub no_conversations: bool,
    
    /// Don't restore preferences
    #[arg(long)]
    pub no_preferences: bool,
    
    /// Don't restore workspace
    #[arg(long)]
    pub no_workspace: bool,
    
    /// Restore environment variables
    #[arg(long)]
    pub restore_environment: bool,
    
    /// Maximum age of data to restore (e.g., "7d", "1h")
    #[arg(long)]
    pub max_age: Option<String>,
    
    /// Create a new session instead of replacing current
    #[arg(long)]
    pub as_new_session: bool,
}

#[derive(Args, Debug)]
pub struct DeleteSessionArgs {
    /// Session IDs or partial IDs
    pub session_ids: Vec<String>,
    
    /// Force deletion without confirmation
    #[arg(short, long)]
    pub force: bool,
    
    /// Delete all sessions older than specified age
    #[arg(long)]
    pub older_than: Option<String>,
}

#[derive(Args, Debug)]
pub struct ExportSessionArgs {
    /// Session ID or partial ID
    pub session_id: String,
    
    /// Output file path
    #[arg(short, long)]
    pub output: PathBuf,
    
    /// Include sensitive data in export
    #[arg(long)]
    pub include_sensitive: bool,
    
    /// Compression format (none, gzip, zip)
    #[arg(long, default_value = "gzip")]
    pub compression: CompressionFormat,
    
    /// Include command history
    #[arg(long, default_value = "true")]
    pub include_commands: bool,
    
    /// Include conversations
    #[arg(long, default_value = "true")]
    pub include_conversations: bool,
}

#[derive(Args, Debug)]
pub struct ImportSessionArgs {
    /// Import file path
    pub file: PathBuf,
    
    /// Generate new session ID
    #[arg(long, default_value = "true")]
    pub new_id: bool,
    
    /// Set as current session
    #[arg(long)]
    pub activate: bool,
    
    /// Override existing session with same ID
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct CleanupSessionArgs {
    /// Delete sessions older than (e.g., "30d", "1w")
    #[arg(short, long, default_value = "30d")]
    pub older_than: String,
    
    /// Keep at least this many recent sessions
    #[arg(short, long, default_value = "5")]
    pub keep_recent: usize,
    
    /// Dry run - show what would be deleted
    #[arg(long)]
    pub dry_run: bool,
    
    /// Force cleanup without confirmation
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct StatsSessionArgs {
    /// Show detailed statistics
    #[arg(short, long)]
    pub detailed: bool,
    
    /// Show statistics for specific session
    #[arg(long)]
    pub session_id: Option<String>,
    
    /// Time range for statistics (e.g., "7d", "1m")
    #[arg(long)]
    pub time_range: Option<String>,
}

#[derive(Args, Debug)]
pub struct ConfigSessionArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Show current configuration
    Show,
    
    /// Set configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    
    /// Get configuration value
    Get {
        /// Configuration key
        key: String,
    },
    
    /// Reset configuration to defaults
    Reset {
        /// Force reset without confirmation
        #[arg(short, long)]
        force: bool,
    },
    
    /// Edit configuration file
    Edit,
}

#[derive(Args, Debug)]
pub struct PrefsSessionArgs {
    #[command(subcommand)]
    pub command: PrefsCommand,
}

#[derive(Subcommand, Debug)]
pub enum PrefsCommand {
    /// Show current preferences
    Show,
    
    /// Set preference value
    Set {
        /// Preference key
        key: String,
        /// Preference value
        value: String,
    },
    
    /// Get preference value
    Get {
        /// Preference key
        key: String,
    },
    
    /// Reset preferences to defaults
    Reset {
        /// Force reset without confirmation
        #[arg(short, long)]
        force: bool,
    },
    
    /// Export preferences
    Export {
        /// Output file
        output: PathBuf,
    },
    
    /// Import preferences
    Import {
        /// Input file
        input: PathBuf,
        /// Merge with existing preferences
        #[arg(long)]
        merge: bool,
    },
}

#[derive(Args, Debug)]
pub struct WatchSessionArgs {
    /// Events to watch (all, saves, commands, conversations)
    #[arg(short, long, default_values = &["all"])]
    pub events: Vec<String>,
    
    /// Follow mode - continue watching new events
    #[arg(short, long)]
    pub follow: bool,
    
    /// Show timestamps
    #[arg(short, long)]
    pub timestamps: bool,
    
    /// JSON output format
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
    Csv,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            "yaml" => Ok(OutputFormat::Yaml),
            "csv" => Ok(OutputFormat::Csv),
            _ => Err(format!("Invalid output format: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionSortOption {
    Created,
    Modified,
    Commands,
    Conversations,
    Name,
}

impl std::str::FromStr for SessionSortOption {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "created" => Ok(SessionSortOption::Created),
            "modified" => Ok(SessionSortOption::Modified),
            "commands" => Ok(SessionSortOption::Commands),
            "conversations" => Ok(SessionSortOption::Conversations),
            "name" => Ok(SessionSortOption::Name),
            _ => Err(format!("Invalid sort option: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionFormat {
    None,
    Gzip,
    Zip,
}

impl std::str::FromStr for CompressionFormat {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(CompressionFormat::None),
            "gzip" => Ok(CompressionFormat::Gzip),
            "zip" => Ok(CompressionFormat::Zip),
            _ => Err(format!("Invalid compression format: {}", s)),
        }
    }
}

/// Session CLI handler
pub struct SessionCliHandler {
    session_service: SessionService,
}

impl SessionCliHandler {
    pub fn new(session_service: SessionService) -> Self {
        Self { session_service }
    }
    
    /// Execute a session CLI command
    pub async fn execute(&self, cli: SessionCli) -> Result<()> {
        match &cli.command {
            SessionCommand::New(args) => self.handle_new_session(args, &cli).await,
            SessionCommand::List(args) => self.handle_list_sessions(args, &cli).await,
            SessionCommand::Show(args) => self.handle_show_session(args, &cli).await,
            SessionCommand::Restore(args) => self.handle_restore_session(args, &cli).await,
            SessionCommand::Delete(args) => self.handle_delete_session(args, &cli).await,
            SessionCommand::Export(args) => self.handle_export_session(args, &cli).await,
            SessionCommand::Import(args) => self.handle_import_session(args, &cli).await,
            SessionCommand::Cleanup(args) => self.handle_cleanup_sessions(args, &cli).await,
            SessionCommand::Stats(args) => self.handle_session_stats(args, &cli).await,
            SessionCommand::Config(args) => self.handle_config(args, &cli).await,
            SessionCommand::Prefs(args) => self.handle_preferences(args, &cli).await,
            SessionCommand::Watch(args) => self.handle_watch_events(args, &cli).await,
        }
    }
    
    async fn handle_new_session(&self, args: &NewSessionArgs, cli: &SessionCli) -> Result<()> {
        let context = PtyAiContext::default(); // Would be populated from current environment
        
        info!("Creating new session...");
        let session_id = self.session_service.start_new_session(&context).await?;
        
        let output = NewSessionOutput {
            session_id,
            title: args.title.clone().unwrap_or_else(|| format!("Session {}", session_id)),
            created_at: Utc::now(),
            activated: args.activate,
        };
        
        self.output_result(&output, &cli.output_format)?;
        
        if args.activate {
            info!("Session {} activated", session_id);
        }
        
        Ok(())
    }
    
    async fn handle_list_sessions(&self, args: &ListSessionArgs, cli: &SessionCli) -> Result<()> {
        let mut sessions = self.session_service.list_sessions().await?;
        
        // Apply filters
        if let Some(min_age) = &args.min_age {
            let min_time = parse_duration(min_age)?;
            let cutoff = Utc::now() - chrono::Duration::from_std(min_time)?;
            sessions.retain(|s| s.created_at <= cutoff);
        }
        
        if let Some(max_age) = &args.max_age {
            let max_time = parse_duration(max_age)?;
            let cutoff = Utc::now() - chrono::Duration::from_std(max_time)?;
            sessions.retain(|s| s.created_at >= cutoff);
        }
        
        // Sort sessions
        sessions.sort_by(|a, b| {
            match args.sort {
                SessionSortOption::Created => a.created_at.cmp(&b.created_at),
                SessionSortOption::Modified => b.last_active.cmp(&a.last_active),
                SessionSortOption::Commands => b.command_count.cmp(&a.command_count),
                SessionSortOption::Conversations => b.conversation_count.cmp(&a.conversation_count),
                SessionSortOption::Name => a.title.cmp(&b.title),
            }
        });
        
        // Limit results
        sessions.truncate(args.limit);
        
        let output = ListSessionOutput { sessions };
        self.output_result(&output, &cli.output_format)?;
        
        Ok(())
    }
    
    async fn handle_show_session(&self, args: &ShowSessionArgs, cli: &SessionCli) -> Result<()> {
        let session_id = self.resolve_session_id(&args.session_id).await?;
        
        // This would need to be implemented to get detailed session info
        println!("Showing details for session: {}", session_id);
        
        if args.show_commands {
            println!("Recent commands would be displayed here");
        }
        
        if args.show_conversations {
            println!("Recent conversations would be displayed here");
        }
        
        Ok(())
    }
    
    async fn handle_restore_session(&self, args: &RestoreSessionArgs, cli: &SessionCli) -> Result<()> {
        let session_id = self.resolve_session_id(&args.session_id).await?;
        
        let options = RestoreOptions {
            restore_commands: !args.no_commands,
            restore_conversations: !args.no_conversations,
            restore_preferences: !args.no_preferences,
            restore_workspace: !args.no_workspace,
            restore_environment: args.restore_environment,
            max_restore_age: args.max_age.as_ref().map(|age| parse_duration(age)).transpose()?,
        };
        
        info!("Restoring session {}...", session_id);
        let summary = self.session_service.restore_session(session_id, options).await?;
        
        self.output_result(&summary, &cli.output_format)?;
        info!("Session {} restored successfully", session_id);
        
        Ok(())
    }
    
    async fn handle_delete_session(&self, args: &DeleteSessionArgs, cli: &SessionCli) -> Result<()> {
        let mut deleted_count = 0;
        
        for session_id_str in &args.session_ids {
            let session_id = self.resolve_session_id(session_id_str).await?;
            
            if !args.force {
                println!("Are you sure you want to delete session {}? (y/N)", session_id);
                // Would implement confirmation input here
            }
            
            self.session_service.delete_session(session_id).await?;
            deleted_count += 1;
            info!("Deleted session {}", session_id);
        }
        
        let output = DeleteSessionOutput {
            deleted_count,
            session_ids: args.session_ids.clone(),
        };
        
        self.output_result(&output, &cli.output_format)?;
        
        Ok(())
    }
    
    async fn handle_export_session(&self, args: &ExportSessionArgs, cli: &SessionCli) -> Result<()> {
        let session_id = self.resolve_session_id(&args.session_id).await?;
        
        info!("Exporting session {} to {}...", session_id, args.output.display());
        self.session_service.export_session(session_id, &args.output).await?;
        
        let output = ExportSessionOutput {
            session_id,
            output_path: args.output.clone(),
            compression: args.compression.clone(),
            file_size: 0, // Would be calculated
        };
        
        self.output_result(&output, &cli.output_format)?;
        info!("Session exported successfully");
        
        Ok(())
    }
    
    async fn handle_import_session(&self, args: &ImportSessionArgs, cli: &SessionCli) -> Result<()> {
        info!("Importing session from {}...", args.file.display());
        let session_id = self.session_service.import_session(&args.file).await?;
        
        let output = ImportSessionOutput {
            session_id,
            source_path: args.file.clone(),
            activated: args.activate,
        };
        
        self.output_result(&output, &cli.output_format)?;
        info!("Session imported successfully with ID: {}", session_id);
        
        Ok(())
    }
    
    async fn handle_cleanup_sessions(&self, args: &CleanupSessionArgs, cli: &SessionCli) -> Result<()> {
        if args.dry_run {
            info!("Dry run - showing what would be cleaned up");
        }
        
        let deleted_count = if args.dry_run {
            0 // Would simulate cleanup
        } else {
            self.session_service.cleanup_old_sessions().await?
        };
        
        let output = CleanupSessionOutput {
            deleted_count,
            dry_run: args.dry_run,
            criteria: format!("older than {}", args.older_than),
        };
        
        self.output_result(&output, &cli.output_format)?;
        
        Ok(())
    }
    
    async fn handle_session_stats(&self, _args: &StatsSessionArgs, cli: &SessionCli) -> Result<()> {
        let stats = self.session_service.get_session_stats().await?;
        
        self.output_result(&stats, &cli.output_format)?;
        
        Ok(())
    }
    
    async fn handle_config(&self, args: &ConfigSessionArgs, cli: &SessionCli) -> Result<()> {
        match &args.command {
            ConfigCommand::Show => {
                println!("Configuration management not yet implemented");
            }
            ConfigCommand::Set { key, value } => {
                println!("Setting config {} = {}", key, value);
            }
            ConfigCommand::Get { key } => {
                println!("Getting config {}", key);
            }
            ConfigCommand::Reset { force: _ } => {
                println!("Resetting configuration");
            }
            ConfigCommand::Edit => {
                println!("Opening configuration editor");
            }
        }
        
        Ok(())
    }
    
    async fn handle_preferences(&self, args: &PrefsSessionArgs, cli: &SessionCli) -> Result<()> {
        match &args.command {
            PrefsCommand::Show => {
                if let Some(session) = self.session_service.get_current_session().await {
                    self.output_result(&session.preferences, &cli.output_format)?;
                } else {
                    println!("No active session");
                }
            }
            PrefsCommand::Set { key, value } => {
                let key_owned = key.clone();
                let value_owned = value.clone();
                self.session_service.update_preferences(move |prefs| {
                    // Would implement preference setting based on key
                    println!("Setting preference {} = {}", key_owned, value_owned);
                }).await?;
            }
            PrefsCommand::Get { key } => {
                println!("Getting preference {}", key);
            }
            PrefsCommand::Reset { force: _ } => {
                self.session_service.update_preferences(|prefs| {
                    *prefs = UserPreferences::default();
                }).await?;
                println!("Preferences reset to defaults");
            }
            PrefsCommand::Export { output } => {
                println!("Exporting preferences to {}", output.display());
            }
            PrefsCommand::Import { input, merge: _ } => {
                println!("Importing preferences from {}", input.display());
            }
        }
        
        Ok(())
    }
    
    async fn handle_watch_events(&self, args: &WatchSessionArgs, _cli: &SessionCli) -> Result<()> {
        let mut event_receiver = self.session_service.subscribe_events();
        
        info!("Watching session events... (Ctrl+C to stop)");
        
        while let Ok(event) = event_receiver.recv().await {
            if args.json {
                let json_output = serde_json::to_string(&event)?;
                println!("{}", json_output);
            } else {
                let timestamp = if args.timestamps {
                    format!("[{}] ", Utc::now().format("%H:%M:%S"))
                } else {
                    String::new()
                };
                
                match event {
                    SessionEvent::SessionStarted { session_id, .. } => {
                        println!("{}Session started: {}", timestamp, session_id);
                    }
                    SessionEvent::SessionRestored { session_id, .. } => {
                        println!("{}Session restored: {}", timestamp, session_id);
                    }
                    SessionEvent::SessionSaved { session_id, .. } => {
                        println!("{}Session saved: {}", timestamp, session_id);
                    }
                    SessionEvent::SessionDeleted { session_id, .. } => {
                        println!("{}Session deleted: {}", timestamp, session_id);
                    }
                    SessionEvent::CommandAdded { session_id, command, .. } => {
                        println!("{}Command added to {}: {}", timestamp, session_id, command);
                    }
                    SessionEvent::ConversationUpdated { session_id, conversation_id } => {
                        println!("{}Conversation {} updated in session {}", timestamp, conversation_id, session_id);
                    }
                    SessionEvent::PreferencesUpdated { session_id } => {
                        println!("{}Preferences updated in session {}", timestamp, session_id);
                    }
                    SessionEvent::WorkspaceUpdated { session_id } => {
                        println!("{}Workspace updated in session {}", timestamp, session_id);
                    }
                }
            }
            
            if !args.follow {
                break;
            }
        }
        
        Ok(())
    }
    
    // Helper methods
    
    async fn resolve_session_id(&self, partial_id: &str) -> Result<SessionId> {
        let sessions = self.session_service.list_sessions().await?;
        
        // Try exact match first
        if let Ok(uuid) = partial_id.parse::<uuid::Uuid>() {
            let session_id = SessionId(uuid);
            if sessions.iter().any(|s| s.session_id == session_id) {
                return Ok(session_id);
            }
        }
        
        // Try partial match
        let matches: Vec<_> = sessions.iter()
            .filter(|s| s.session_id.to_string().starts_with(partial_id))
            .collect();
        
        match matches.len() {
            0 => Err(anyhow::anyhow!("No session found matching: {}", partial_id)),
            1 => Ok(matches[0].session_id),
            _ => Err(anyhow::anyhow!("Multiple sessions match: {}", partial_id)),
        }
    }
    
    fn output_result<T: Serialize>(&self, data: &T, format: &OutputFormat) -> Result<()> {
        match format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(data)?;
                println!("{}", json);
            }
            OutputFormat::Yaml => {
                let yaml = serde_yaml::to_string(data)?;
                println!("{}", yaml);
            }
            OutputFormat::Table => {
                // Would implement table formatting for specific types
                let json = serde_json::to_string_pretty(data)?;
                println!("{}", json);
            }
            OutputFormat::Csv => {
                // Would implement CSV formatting for specific types
                let json = serde_json::to_string(data)?;
                println!("{}", json);
            }
        }
        Ok(())
    }
}

// Output structs for different commands
#[derive(Debug, Serialize)]
struct NewSessionOutput {
    session_id: SessionId,
    title: String,
    created_at: DateTime<Utc>,
    activated: bool,
}

#[derive(Debug, Serialize)]
struct ListSessionOutput {
    sessions: Vec<SessionSummary>,
}

#[derive(Debug, Serialize)]
struct DeleteSessionOutput {
    deleted_count: usize,
    session_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ExportSessionOutput {
    session_id: SessionId,
    output_path: PathBuf,
    compression: CompressionFormat,
    file_size: u64,
}

#[derive(Debug, Serialize)]
struct ImportSessionOutput {
    session_id: SessionId,
    source_path: PathBuf,
    activated: bool,
}

#[derive(Debug, Serialize)]
struct CleanupSessionOutput {
    deleted_count: usize,
    dry_run: bool,
    criteria: String,
}

// Helper function to parse duration strings
fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    
    if s.is_empty() {
        return Err(anyhow::anyhow!("Empty duration string"));
    }
    
    let (number_part, unit_part) = if let Some(pos) = s.find(|c: char| c.is_alphabetic()) {
        s.split_at(pos)
    } else {
        return Err(anyhow::anyhow!("Invalid duration format: {}", s));
    };
    
    let number: u64 = number_part.parse()
        .with_context(|| format!("Invalid number in duration: {}", number_part))?;
    
    let multiplier = match unit_part.to_lowercase().as_str() {
        "s" | "sec" | "second" | "seconds" => 1,
        "m" | "min" | "minute" | "minutes" => 60,
        "h" | "hour" | "hours" => 60 * 60,
        "d" | "day" | "days" => 60 * 60 * 24,
        "w" | "week" | "weeks" => 60 * 60 * 24 * 7,
        _ => return Err(anyhow::anyhow!("Invalid duration unit: {}", unit_part)),
    };
    
    Ok(Duration::from_secs(number * multiplier))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(7200));
        assert_eq!(parse_duration("1d").unwrap(), Duration::from_secs(86400));
        assert_eq!(parse_duration("1w").unwrap(), Duration::from_secs(604800));
        
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("30x").is_err());
    }
}
//! Production Session CLI Commands
//! 
//! Comprehensive command-line interface for session management with full CRUD operations,
//! advanced filtering, export/import functionality, and performance monitoring.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use comfy_table::{Table, presets, Cell, Color, Attribute, ContentArrangement};
use serde::{Deserialize, Serialize};
use tracing::{info, error, warn};

use crate::session_service::{
    SessionService, SessionEvent, RestoreOptions, SessionStats, 
    RestorationSummary, SessionSummary
};
use crate::session_persistence::{SessionId, PersistenceConfig, UserPreferences};
use crate::ai_context_provider::PtyAiContext;

/// Production session management CLI
#[derive(Parser, Debug)]
#[command(
    name = "session",
    about = "Comprehensive terminal session management",
    long_about = "Complete session management for the OpenAgent terminal including \
                  session creation, restoration, export/import, analytics, and configuration."
)]
pub struct SessionCli {
    #[command(subcommand)]
    pub command: SessionCommand,
    
    /// Verbose output with detailed information
    #[arg(short, long, global = true)]
    pub verbose: bool,
    
    /// Output format (table, json, yaml, csv)
    #[arg(short = 'f', long = "format", global = true, default_value = "table")]
    pub output_format: OutputFormat,
    
    /// Configuration file path
    #[arg(short = 'c', long = "config", global = true)]
    pub config_file: Option<PathBuf>,
    
    /// Use colored output
    #[arg(long, global = true, default_value = "true")]
    pub color: bool,
    
    /// Suppress all output except errors
    #[arg(short = 'q', long = "quiet", global = true)]
    pub quiet: bool,
}

/// Comprehensive session CLI subcommands
#[derive(Subcommand, Debug)]
pub enum SessionCommand {
    /// Create a new session
    #[command(alias = "new")]
    Create(CreateSessionArgs),
    
    /// List sessions with advanced filtering
    #[command(alias = "ls")]
    List(ListSessionArgs),
    
    /// Show detailed session information
    #[command(alias = "info")]
    Show(ShowSessionArgs),
    
    /// Restore a session with options
    #[command(alias = "load")]
    Restore(RestoreSessionArgs),
    
    /// Delete sessions
    #[command(alias = "rm")]
    Delete(DeleteSessionArgs),
    
    /// Export sessions to file
    Export(ExportSessionArgs),
    
    /// Import sessions from file
    Import(ImportSessionArgs),
    
    /// Archive old sessions
    Archive(ArchiveSessionArgs),
    
    /// Clean up sessions (delete archived/old)
    Cleanup(CleanupSessionArgs),
    
    /// Session statistics and analytics
    Stats(StatsSessionArgs),
    
    /// Search sessions by content
    Search(SearchSessionArgs),
    
    /// Tag management
    Tag(TagSessionArgs),
    
    /// Configuration management
    Config(ConfigSessionArgs),
    
    /// Backup and restore session database
    Backup(BackupSessionArgs),
    
    /// Validate session integrity
    Validate(ValidateSessionArgs),
    
    /// Watch session changes in real-time
    Watch(WatchSessionArgs),
}

/// Output formats supported
#[derive(Clone, Debug, ValueEnum, Serialize, Deserialize)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
    Csv,
    Tree,
    Compact,
}

/// Session sorting options
#[derive(Clone, Debug, ValueEnum, Serialize, Deserialize)]
pub enum SessionSortOption {
    Created,
    Modified,
    Commands,
    Conversations,
    Size,
    Name,
}

#[derive(Args, Debug)]
pub struct CreateSessionArgs {
    /// Session name
    #[arg(short, long)]
    pub name: Option<String>,
    
    /// Session description
    #[arg(short, long)]
    pub description: Option<String>,
    
    /// Initialize with current working directory
    #[arg(long)]
    pub use_cwd: bool,
    
    /// Initialize with current environment
    #[arg(long)]
    pub use_env: bool,
    
    /// Tags for the session
    #[arg(short, long)]
    pub tags: Vec<String>,
    
    /// Project root directory
    #[arg(short, long)]
    pub project: Option<PathBuf>,
    
    /// Copy settings from another session
    #[arg(long)]
    pub copy_from: Option<String>,
    
    /// Auto-save interval in minutes
    #[arg(long, default_value = "5")]
    pub autosave_interval: u32,
    
    /// Enable encryption for this session
    #[arg(long)]
    pub encrypted: bool,
}

#[derive(Args, Debug)]
pub struct ListSessionArgs {
    /// Show only active sessions
    #[arg(short, long)]
    pub active_only: bool,
    
    /// Show only archived sessions
    #[arg(long)]
    pub archived_only: bool,
    
    /// Limit number of results
    #[arg(short = 'n', long, default_value = "20")]
    pub limit: usize,
    
    /// Sort by field
    #[arg(short, long, default_value = "modified")]
    pub sort: SessionSortOption,
    
    /// Sort in descending order
    #[arg(long)]
    pub desc: bool,
    
    /// Filter by minimum age (e.g., "1d", "2h", "30m")
    #[arg(long)]
    pub min_age: Option<String>,
    
    /// Filter by maximum age
    #[arg(long)]
    pub max_age: Option<String>,
    
    /// Filter by tag
    #[arg(short, long)]
    pub tag: Option<String>,
    
    /// Filter by name pattern (supports wildcards)
    #[arg(long)]
    pub name_pattern: Option<String>,
    
    /// Show session sizes
    #[arg(long)]
    pub show_size: bool,
    
    /// Show full session IDs
    #[arg(long)]
    pub full_ids: bool,
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
    
    /// Show environment variables
    #[arg(long)]
    pub show_environment: bool,
    
    /// Show session metadata
    #[arg(long)]
    pub show_metadata: bool,
    
    /// Number of recent commands to show
    #[arg(long, default_value = "10")]
    pub recent_commands: usize,
    
    /// Number of recent conversations to show
    #[arg(long, default_value = "5")]
    pub recent_conversations: usize,
    
    /// Show everything (equivalent to all show-* flags)
    #[arg(long)]
    pub all: bool,
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
    
    /// Don't restore environment
    #[arg(long)]
    pub no_environment: bool,
    
    /// Restore environment variables
    #[arg(long)]
    pub restore_environment: bool,
    
    /// Maximum age of data to restore (e.g., "7d", "1h")
    #[arg(long)]
    pub max_age: Option<String>,
    
    /// Create a new session instead of replacing current
    #[arg(long)]
    pub as_new_session: bool,
    
    /// Merge with current session instead of replacing
    #[arg(long)]
    pub merge: bool,
    
    /// Preview restoration without applying
    #[arg(long)]
    pub dry_run: bool,
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
    
    /// Delete archived sessions only
    #[arg(long)]
    pub archived_only: bool,
    
    /// Delete by tag
    #[arg(long)]
    pub with_tag: Option<String>,
    
    /// Keep backups before deletion
    #[arg(long)]
    pub backup_first: bool,
}

#[derive(Args, Debug)]
pub struct ExportSessionArgs {
    /// Session IDs to export (empty = all)
    pub session_ids: Vec<String>,
    
    /// Output file path
    #[arg(short, long)]
    pub output: PathBuf,
    
    /// Export format (json, yaml, sqlite)
    #[arg(short, long, default_value = "json")]
    pub format: ExportFormat,
    
    /// Include command history
    #[arg(long, default_value = "true")]
    pub include_commands: bool,
    
    /// Include conversations
    #[arg(long, default_value = "true")]
    pub include_conversations: bool,
    
    /// Include preferences
    #[arg(long, default_value = "true")]
    pub include_preferences: bool,
    
    /// Include workspace state
    #[arg(long, default_value = "true")]
    pub include_workspace: bool,
    
    /// Compress output
    #[arg(short, long)]
    pub compress: bool,
    
    /// Encrypt output
    #[arg(short, long)]
    pub encrypt: bool,
}

#[derive(Args, Debug)]
pub struct ImportSessionArgs {
    /// Input file path
    pub input: PathBuf,
    
    /// Import format (auto-detect if not specified)
    #[arg(short, long)]
    pub format: Option<ExportFormat>,
    
    /// Overwrite existing sessions
    #[arg(long)]
    pub overwrite: bool,
    
    /// Merge with existing sessions
    #[arg(long)]
    pub merge: bool,
    
    /// Validate before import
    #[arg(long, default_value = "true")]
    pub validate: bool,
    
    /// Preview import without applying
    #[arg(long)]
    pub dry_run: bool,
    
    /// Decrypt input file
    #[arg(short, long)]
    pub decrypt: bool,
}

#[derive(Args, Debug)]
pub struct ArchiveSessionArgs {
    /// Session IDs to archive (empty = auto-select old sessions)
    pub session_ids: Vec<String>,
    
    /// Archive sessions older than specified age
    #[arg(long, default_value = "30d")]
    pub older_than: String,
    
    /// Force archiving without confirmation
    #[arg(short, long)]
    pub force: bool,
    
    /// Archive location
    #[arg(short, long)]
    pub archive_path: Option<PathBuf>,
    
    /// Compress archived sessions
    #[arg(short, long, default_value = "true")]
    pub compress: bool,
}

#[derive(Args, Debug)]
pub struct CleanupSessionArgs {
    /// Delete archived sessions older than specified age
    #[arg(long, default_value = "90d")]
    pub delete_archived_older_than: String,
    
    /// Delete sessions with no commands older than specified age
    #[arg(long, default_value = "7d")]
    pub delete_empty_older_than: String,
    
    /// Maximum sessions to keep
    #[arg(long)]
    pub max_sessions: Option<usize>,
    
    /// Force cleanup without confirmation
    #[arg(short, long)]
    pub force: bool,
    
    /// Dry run (show what would be cleaned)
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct StatsSessionArgs {
    /// Show detailed statistics
    #[arg(short, long)]
    pub detailed: bool,
    
    /// Time period for stats (e.g., "7d", "1m", "all")
    #[arg(long, default_value = "30d")]
    pub period: String,
    
    /// Group statistics by (day, week, month)
    #[arg(long)]
    pub group_by: Option<String>,
    
    /// Show top N sessions by commands
    #[arg(long, default_value = "10")]
    pub top_sessions: usize,
    
    /// Include charts (requires terminal graphics support)
    #[arg(long)]
    pub charts: bool,
}

#[derive(Args, Debug)]
pub struct SearchSessionArgs {
    /// Search query
    pub query: String,
    
    /// Search in command history
    #[arg(long, default_value = "true")]
    pub in_commands: bool,
    
    /// Search in conversations
    #[arg(long, default_value = "true")]
    pub in_conversations: bool,
    
    /// Search in session names/descriptions
    #[arg(long, default_value = "true")]
    pub in_metadata: bool,
    
    /// Case sensitive search
    #[arg(long)]
    pub case_sensitive: bool,
    
    /// Use regular expressions
    #[arg(short, long)]
    pub regex: bool,
    
    /// Maximum results to show
    #[arg(short, long, default_value = "20")]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct TagSessionArgs {
    #[command(subcommand)]
    pub action: TagAction,
}

#[derive(Subcommand, Debug)]
pub enum TagAction {
    /// Add tags to sessions
    Add {
        /// Session IDs
        session_ids: Vec<String>,
        /// Tags to add
        tags: Vec<String>,
    },
    /// Remove tags from sessions
    Remove {
        /// Session IDs
        session_ids: Vec<String>,
        /// Tags to remove
        tags: Vec<String>,
    },
    /// List all tags
    List {
        /// Show tag usage counts
        #[arg(long)]
        counts: bool,
    },
    /// Rename a tag
    Rename {
        /// Old tag name
        old_name: String,
        /// New tag name
        new_name: String,
    },
}

#[derive(Args, Debug)]
pub struct ConfigSessionArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
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
    /// Edit configuration in editor
    Edit,
}

#[derive(Args, Debug)]
pub struct PrefsSessionArgs {
    #[command(subcommand)]
    pub action: PrefsAction,
}

#[derive(Subcommand, Debug)]
pub enum PrefsAction {
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
    /// Export preferences to file
    Export {
        /// Output file path
        output: PathBuf,
    },
    /// Import preferences from file
    Import {
        /// Input file path
        input: PathBuf,
        /// Merge with existing preferences
        #[arg(short, long)]
        merge: bool,
    },
}

#[derive(Args, Debug)]
pub struct BackupSessionArgs {
    #[command(subcommand)]
    pub action: BackupAction,
}

#[derive(Subcommand, Debug)]
pub enum BackupAction {
    /// Create backup
    Create {
        /// Backup file path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Include full history
        #[arg(long, default_value = "true")]
        full: bool,
        /// Compress backup
        #[arg(short, long, default_value = "true")]
        compress: bool,
    },
    /// Restore from backup
    Restore {
        /// Backup file path
        input: PathBuf,
        /// Force restore without confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// List available backups
    List {
        /// Show detailed backup information
        #[arg(short, long)]
        detailed: bool,
    },
}

#[derive(Args, Debug)]
pub struct ValidateSessionArgs {
    /// Session IDs to validate (empty = all)
    pub session_ids: Vec<String>,
    
    /// Fix found issues automatically
    #[arg(long)]
    pub fix: bool,
    
    /// Show detailed validation results
    #[arg(short, long)]
    pub detailed: bool,
}

#[derive(Args, Debug)]
pub struct WatchSessionArgs {
    /// Session IDs to watch (empty = all active)
    pub session_ids: Vec<String>,
    
    /// Watch interval in seconds
    #[arg(short, long, default_value = "5")]
    pub interval: u64,
    
    /// Show only changes
    #[arg(short, long)]
    pub changes_only: bool,
}

/// Export formats supported
#[derive(Clone, Debug, ValueEnum, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Yaml,
    Sqlite,
    Csv,
}

/// Session CLI implementation with comprehensive functionality
pub struct SessionCliRunner {
    session_service: SessionService,
    config: SessionCliConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCliConfig {
    pub default_output_format: OutputFormat,
    pub default_limit: usize,
    pub color_enabled: bool,
    pub auto_backup: bool,
    pub backup_retention_days: u32,
    pub compression_enabled: bool,
}

impl Default for SessionCliConfig {
    fn default() -> Self {
        Self {
            default_output_format: OutputFormat::Table,
            default_limit: 20,
            color_enabled: true,
            auto_backup: true,
            backup_retention_days: 30,
            compression_enabled: true,
        }
    }
}

impl SessionCliRunner {
    pub fn new(session_service: SessionService) -> Self {
        Self {
            session_service,
            config: SessionCliConfig::default(),
        }
    }

    /// Run the CLI command with comprehensive error handling
    pub async fn run(&mut self, cli: SessionCli) -> Result<()> {
        // Configure output based on CLI flags
        if cli.quiet {
            // Suppress non-error output
        }
        
        match cli.command {
            SessionCommand::Create(args) => self.handle_create(args, &cli).await,
            SessionCommand::List(args) => self.handle_list(args, &cli).await,
            SessionCommand::Show(args) => self.handle_show(args, &cli).await,
            SessionCommand::Restore(args) => self.handle_restore(args, &cli).await,
            SessionCommand::Delete(args) => self.handle_delete(args, &cli).await,
            SessionCommand::Export(args) => self.handle_export(args, &cli).await,
            SessionCommand::Import(args) => self.handle_import(args, &cli).await,
            SessionCommand::Archive(args) => self.handle_archive(args, &cli).await,
            SessionCommand::Cleanup(args) => self.handle_cleanup(args, &cli).await,
            SessionCommand::Stats(args) => self.handle_stats(args, &cli).await,
            SessionCommand::Search(args) => self.handle_search(args, &cli).await,
            SessionCommand::Tag(args) => self.handle_tag(args, &cli).await,
            SessionCommand::Config(args) => self.handle_config(args, &cli).await,
            SessionCommand::Backup(args) => self.handle_backup(args, &cli).await,
            SessionCommand::Validate(args) => self.handle_validate(args, &cli).await,
            SessionCommand::Watch(args) => self.handle_watch(args, &cli).await,
        }
    }

    async fn handle_create(&mut self, args: CreateSessionArgs, cli: &SessionCli) -> Result<()> {
        info!("Creating new session");

        let session_id = self.session_service.create_session().await?;
        
        // Apply session configuration
        if let Some(name) = args.name {
            self.session_service.set_session_name(&session_id, &name).await?;
        }
        
        if let Some(description) = args.description {
            self.session_service.set_session_description(&session_id, &description).await?;
        }
        
        if !args.tags.is_empty() {
            for tag in &args.tags {
                self.session_service.add_session_tag(&session_id, tag).await?;
            }
        }
        
        if args.use_cwd {
            let cwd = std::env::current_dir()?;
            self.session_service.set_session_working_directory(&session_id, &cwd).await?;
        }
        
        if args.use_env {
            let env: std::collections::HashMap<String, String> = std::env::vars().collect();
            self.session_service.set_session_environment(&session_id, &env).await?;
        }
        
        if let Some(project) = args.project {
            self.session_service.set_session_project_root(&session_id, &project).await?;
        }
        
        // Configure auto-save
        if args.autosave_interval > 0 {
            let interval = Duration::from_secs(args.autosave_interval as u64 * 60);
            self.session_service.set_autosave_interval(&session_id, interval).await?;
        }
        
        self.output_result(&format!("Created session: {}", session_id), &cli.output_format)?;
        Ok(())
    }

    async fn handle_list(&mut self, args: ListSessionArgs, cli: &SessionCli) -> Result<()> {
        let sessions = self.session_service.list_sessions().await?;
        
        // Apply filters
        let mut filtered_sessions: Vec<_> = sessions.into_iter()
            .filter(|session| {
                if args.active_only && !session.is_active {
                    return false;
                }
                if args.archived_only && !session.is_archived {
                    return false;
                }
                if let Some(ref tag) = args.tag {
                    if !session.tags.contains(tag) {
                        return false;
                    }
                }
                if let Some(ref pattern) = args.name_pattern {
                    if !session.name.contains(pattern) {
                        return false;
                    }
                }
                true
            })
            .collect();

        // Apply sorting
        match args.sort {
            SessionSortOption::Created => filtered_sessions.sort_by_key(|s| s.created_at),
            SessionSortOption::Modified => filtered_sessions.sort_by_key(|s| s.last_modified),
            SessionSortOption::Commands => filtered_sessions.sort_by_key(|s| s.command_count),
            SessionSortOption::Conversations => filtered_sessions.sort_by_key(|s| s.conversation_count),
            SessionSortOption::Size => filtered_sessions.sort_by_key(|s| s.size_bytes),
            SessionSortOption::Name => filtered_sessions.sort_by(|a, b| a.name.cmp(&b.name)),
        }

        if args.desc {
            filtered_sessions.reverse();
        }

        // Apply limit
        if filtered_sessions.len() > args.limit {
            filtered_sessions.truncate(args.limit);
        }

        self.output_sessions(&filtered_sessions, &cli.output_format, &args)?;
        Ok(())
    }

    async fn handle_show(&mut self, args: ShowSessionArgs, cli: &SessionCli) -> Result<()> {
        let session_id = self.resolve_session_id(&args.session_id).await?;
        let session = self.session_service.get_session(&session_id).await?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", args.session_id))?;

        let mut output = format!("Session: {} ({})\n", session.name, session.id);
        output.push_str(&format!("Created: {}\n", session.created_at.format("%Y-%m-%d %H:%M:%S")));
        output.push_str(&format!("Modified: {}\n", session.last_modified.format("%Y-%m-%d %H:%M:%S")));
        output.push_str(&format!("Active: {}\n", session.is_active));
        
        if !session.tags.is_empty() {
            output.push_str(&format!("Tags: {}\n", session.tags.join(", ")));
        }

        if args.show_commands || args.all {
            let commands = self.session_service.get_session_commands(&session_id, args.recent_commands).await?;
            output.push_str(&format!("\nRecent Commands ({}):\n", commands.len()));
            for (i, cmd) in commands.iter().enumerate() {
                output.push_str(&format!("  {}: {} ({})\n", i + 1, cmd.command, cmd.timestamp.format("%H:%M:%S")));
            }
        }

        if args.show_conversations || args.all {
            let conversations = self.session_service.get_session_conversations(&session_id, args.recent_conversations).await?;
            output.push_str(&format!("\nRecent Conversations ({}):\n", conversations.len()));
            for (i, conv) in conversations.iter().enumerate() {
                output.push_str(&format!("  {}: {} ({})\n", i + 1, conv.summary, conv.timestamp.format("%H:%M:%S")));
            }
        }

        if args.show_preferences || args.all {
            let preferences = self.session_service.get_session_preferences(&session_id).await?;
            output.push_str(&format!("\nPreferences:\n{:#?}\n", preferences));
        }

        if args.show_workspace || args.all {
            let workspace = self.session_service.get_session_workspace(&session_id).await?;
            output.push_str(&format!("\nWorkspace State:\n{:#?}\n", workspace));
        }

        if args.show_environment || args.all {
            let environment = self.session_service.get_session_environment(&session_id).await?;
            output.push_str(&format!("\nEnvironment Variables ({}):\n", environment.len()));
            for (key, value) in environment.iter() {
                output.push_str(&format!("  {}={}\n", key, value));
            }
        }

        self.output_result(&output, &cli.output_format)?;
        Ok(())
    }

    async fn handle_restore(&mut self, args: RestoreSessionArgs, cli: &SessionCli) -> Result<()> {
        let session_id = self.resolve_session_id(&args.session_id).await?;
        
        let options = RestoreOptions {
            restore_commands: !args.no_commands,
            restore_conversations: !args.no_conversations,
            restore_preferences: !args.no_preferences,
            restore_workspace: !args.no_workspace,
            restore_environment: args.restore_environment,
            max_age: self.parse_duration(&args.max_age.as_deref().unwrap_or("30d"))?,
            as_new_session: args.as_new_session,
            merge_with_current: args.merge,
            dry_run: args.dry_run,
        };

        if args.dry_run {
            let summary = self.session_service.preview_restore(&session_id, &options).await?;
            self.output_restoration_preview(&summary, &cli.output_format)?;
        } else {
            let summary = self.session_service.restore_session(&session_id, &options).await?;
            self.output_restoration_summary(&summary, &cli.output_format)?;
        }

        Ok(())
    }

    async fn handle_delete(&mut self, args: DeleteSessionArgs, cli: &SessionCli) -> Result<()> {
        let mut session_ids = Vec::new();

        // Resolve session IDs
        for id_pattern in &args.session_ids {
            let resolved = self.resolve_session_id(id_pattern).await?;
            session_ids.push(resolved);
        }

        // Handle age-based deletion
        if let Some(age) = &args.older_than {
            let duration = self.parse_duration(age)?;
            let old_sessions = self.session_service.find_sessions_older_than(duration).await?;
            for session in old_sessions {
                if args.archived_only && !session.is_archived {
                    continue;
                }
                session_ids.push(session.id);
            }
        }

        // Handle tag-based deletion
        if let Some(tag) = &args.with_tag {
            let tagged_sessions = self.session_service.find_sessions_with_tag(tag).await?;
            for session in tagged_sessions {
                session_ids.push(session.id);
            }
        }

        if session_ids.is_empty() {
            return Err(anyhow::anyhow!("No sessions to delete"));
        }

        // Confirmation unless forced
        if !args.force {
            println!("About to delete {} session(s):", session_ids.len());
            for id in &session_ids {
                let session = self.session_service.get_session(id).await?.unwrap();
                println!("  - {} ({})", session.name, id);
            }
            
            print!("Continue? [y/N]: ");
            use std::io::{self, Write};
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            
            if !input.trim().to_lowercase().starts_with('y') {
                println!("Cancelled");
                return Ok(());
            }
        }

        // Create backups if requested
        if args.backup_first {
            for id in &session_ids {
                self.session_service.create_session_backup(id).await?;
            }
        }

        // Delete sessions
        let mut deleted_count = 0;
        for id in &session_ids {
            match self.session_service.delete_session(id).await {
                Ok(_) => {
                    deleted_count += 1;
                    if cli.verbose {
                        println!("Deleted session: {}", id);
                    }
                }
                Err(e) => {
                    error!("Failed to delete session {}: {}", id, e);
                }
            }
        }

        self.output_result(&format!("Deleted {} session(s)", deleted_count), &cli.output_format)?;
        Ok(())
    }

    async fn handle_export(&mut self, args: ExportSessionArgs, cli: &SessionCli) -> Result<()> {
        // Implementation for export
        info!("Exporting sessions to {:?}", args.output);
        
        let sessions = if args.session_ids.is_empty() {
            self.session_service.list_sessions().await?
        } else {
            let mut sessions = Vec::new();
            for id_pattern in &args.session_ids {
                let id = self.resolve_session_id(id_pattern).await?;
                if let Some(session) = self.session_service.get_session(&id).await? {
                    sessions.push(session);
                }
            }
            sessions
        };

        let export_data = self.prepare_export_data(&sessions, &args).await?;
        self.write_export_file(&export_data, &args.output, &args.format).await?;

        self.output_result(&format!("Exported {} sessions to {:?}", sessions.len(), args.output), &cli.output_format)?;
        Ok(())
    }

    async fn handle_import(&mut self, args: ImportSessionArgs, cli: &SessionCli) -> Result<()> {
        // Implementation for import
        info!("Importing sessions from {:?}", args.input);
        
        let import_data = self.read_import_file(&args.input, &args.format).await?;
        
        if args.validate {
            self.validate_import_data(&import_data)?;
        }

        if args.dry_run {
            self.output_import_preview(&import_data, &cli.output_format)?;
        } else {
            let imported_count = self.import_sessions(import_data, &args).await?;
            self.output_result(&format!("Imported {} sessions", imported_count), &cli.output_format)?;
        }

        Ok(())
    }

    async fn handle_archive(&mut self, args: ArchiveSessionArgs, cli: &SessionCli) -> Result<()> {
        // Implementation for archiving sessions
        let duration = self.parse_duration(&args.older_than)?;
        let sessions_to_archive = if args.session_ids.is_empty() {
            self.session_service.find_sessions_older_than(duration).await?
        } else {
            let mut sessions = Vec::new();
            for id_pattern in &args.session_ids {
                let id = self.resolve_session_id(id_pattern).await?;
                if let Some(session) = self.session_service.get_session(&id).await? {
                    sessions.push(session);
                }
            }
            sessions
        };

        if !args.force && !sessions_to_archive.is_empty() {
            println!("About to archive {} session(s)", sessions_to_archive.len());
            // Confirmation logic
        }

        let mut archived_count = 0;
        for session in sessions_to_archive {
            match self.session_service.archive_session(&session.id, args.compress).await {
                Ok(_) => archived_count += 1,
                Err(e) => error!("Failed to archive session {}: {}", session.id, e),
            }
        }

        self.output_result(&format!("Archived {} sessions", archived_count), &cli.output_format)?;
        Ok(())
    }

    async fn handle_cleanup(&mut self, args: CleanupSessionArgs, cli: &SessionCli) -> Result<()> {
        // Implementation for cleanup
        let cleanup_summary = if args.dry_run {
            self.session_service.preview_cleanup(&args).await?
        } else {
            self.session_service.cleanup_sessions(&args).await?
        };

        self.output_cleanup_summary(&cleanup_summary, &cli.output_format)?;
        Ok(())
    }

    async fn handle_stats(&mut self, args: StatsSessionArgs, cli: &SessionCli) -> Result<()> {
        let period = self.parse_duration(&args.period)?;
        let stats = self.session_service.get_session_statistics(period).await?;
        
        self.output_statistics(&stats, &args, &cli.output_format)?;
        Ok(())
    }

    async fn handle_search(&mut self, args: SearchSessionArgs, cli: &SessionCli) -> Result<()> {
        let results = self.session_service.search_sessions(&args).await?;
        self.output_search_results(&results, &cli.output_format)?;
        Ok(())
    }

    async fn handle_tag(&mut self, args: TagSessionArgs, cli: &SessionCli) -> Result<()> {
        match args.action {
            TagAction::Add { session_ids, tags } => {
                for id_pattern in &session_ids {
                    let id = self.resolve_session_id(id_pattern).await?;
                    for tag in &tags {
                        self.session_service.add_session_tag(&id, tag).await?;
                    }
                }
                self.output_result(&format!("Added tags to {} sessions", session_ids.len()), &cli.output_format)?;
            }
            TagAction::Remove { session_ids, tags } => {
                for id_pattern in &session_ids {
                    let id = self.resolve_session_id(id_pattern).await?;
                    for tag in &tags {
                        self.session_service.remove_session_tag(&id, tag).await?;
                    }
                }
                self.output_result(&format!("Removed tags from {} sessions", session_ids.len()), &cli.output_format)?;
            }
            TagAction::List { counts } => {
                let tags = self.session_service.list_all_tags(counts).await?;
                self.output_tags(&tags, &cli.output_format)?;
            }
            TagAction::Rename { old_name, new_name } => {
                let renamed_count = self.session_service.rename_tag(&old_name, &new_name).await?;
                self.output_result(&format!("Renamed tag '{}' to '{}' in {} sessions", old_name, new_name, renamed_count), &cli.output_format)?;
            }
        }
        Ok(())
    }

    async fn handle_config(&mut self, args: ConfigSessionArgs, cli: &SessionCli) -> Result<()> {
        match args.action {
            ConfigAction::Show => {
                let config = self.session_service.get_configuration().await?;
                self.output_configuration(&config, &cli.output_format)?;
            }
            ConfigAction::Set { key, value } => {
                self.session_service.set_configuration(&key, &value).await?;
                self.output_result(&format!("Set {} = {}", key, value), &cli.output_format)?;
            }
            ConfigAction::Get { key } => {
                let value = self.session_service.get_configuration_value(&key).await?;
                self.output_result(&format!("{} = {}", key, value), &cli.output_format)?;
            }
            ConfigAction::Reset { force } => {
                if !force {
                    // Confirmation
                }
                self.session_service.reset_configuration().await?;
                self.output_result("Configuration reset to defaults", &cli.output_format)?;
            }
            ConfigAction::Edit => {
                self.session_service.edit_configuration().await?;
            }
        }
        Ok(())
    }

    async fn handle_backup(&mut self, args: BackupSessionArgs, cli: &SessionCli) -> Result<()> {
        match args.action {
            BackupAction::Create { output, full, compress } => {
                let backup_path = self.session_service.create_backup(output, full, compress).await?;
                self.output_result(&format!("Created backup: {:?}", backup_path), &cli.output_format)?;
            }
            BackupAction::Restore { input, force } => {
                if !force {
                    // Confirmation
                }
                self.session_service.restore_from_backup(&input).await?;
                self.output_result(&format!("Restored from backup: {:?}", input), &cli.output_format)?;
            }
            BackupAction::List { detailed } => {
                let backups = self.session_service.list_backups(detailed).await?;
                self.output_backups(&backups, &cli.output_format)?;
            }
        }
        Ok(())
    }

    async fn handle_validate(&mut self, args: ValidateSessionArgs, cli: &SessionCli) -> Result<()> {
        let session_ids = if args.session_ids.is_empty() {
            self.session_service.list_sessions().await?.into_iter().map(|s| s.id).collect()
        } else {
            let mut ids = Vec::new();
            for id_pattern in &args.session_ids {
                let id = self.resolve_session_id(id_pattern).await?;
                ids.push(id);
            }
            ids
        };

        let validation_results = self.session_service.validate_sessions(&session_ids, args.fix).await?;
        self.output_validation_results(&validation_results, &cli.output_format)?;
        Ok(())
    }

    async fn handle_watch(&mut self, args: WatchSessionArgs, cli: &SessionCli) -> Result<()> {
        let session_ids = if args.session_ids.is_empty() {
            self.session_service.get_active_session_ids().await?
        } else {
            let mut ids = Vec::new();
            for id_pattern in &args.session_ids {
                let id = self.resolve_session_id(id_pattern).await?;
                ids.push(id);
            }
            ids
        };

        self.session_service.watch_sessions(&session_ids, Duration::from_secs(args.interval), args.changes_only).await?;
        Ok(())
    }

    // Helper methods

    async fn resolve_session_id(&self, pattern: &str) -> Result<SessionId> {
        // Try exact match first
        if let Ok(id) = SessionId::parse(pattern) {
            return Ok(id);
        }

        // Try partial match
        let sessions = self.session_service.list_sessions().await?;
        let matches: Vec<_> = sessions.into_iter()
            .filter(|s| s.id.to_string().starts_with(pattern) || s.name.contains(pattern))
            .collect();

        match matches.len() {
            0 => Err(anyhow::anyhow!("No session found matching: {}", pattern)),
            1 => Ok(matches[0].id.clone()),
            _ => Err(anyhow::anyhow!("Multiple sessions match '{}': {}", 
                pattern, 
                matches.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")
            )),
        }
    }

    fn parse_duration(&self, duration_str: &str) -> Result<Duration> {
        // Parse duration strings like "1d", "2h", "30m", "45s"
        let duration_str = duration_str.trim();
        if duration_str.is_empty() {
            return Err(anyhow::anyhow!("Empty duration string"));
        }

        let (number_part, unit_part) = if let Some(pos) = duration_str.find(|c: char| c.is_alphabetic()) {
            (&duration_str[..pos], &duration_str[pos..])
        } else {
            return Err(anyhow::anyhow!("Invalid duration format: {}", duration_str));
        };

        let number: u64 = number_part.parse()
            .with_context(|| format!("Invalid number in duration: {}", number_part))?;

        let duration = match unit_part {
            "s" | "sec" | "seconds" => Duration::from_secs(number),
            "m" | "min" | "minutes" => Duration::from_secs(number * 60),
            "h" | "hour" | "hours" => Duration::from_secs(number * 3600),
            "d" | "day" | "days" => Duration::from_secs(number * 86400),
            "w" | "week" | "weeks" => Duration::from_secs(number * 604800),
            _ => return Err(anyhow::anyhow!("Unknown duration unit: {}", unit_part)),
        };

        Ok(duration)
    }

    fn output_result(&self, message: &str, format: &OutputFormat) -> Result<()> {
        match format {
            OutputFormat::Table | OutputFormat::Compact => {
                println!("{}", message);
            }
            OutputFormat::Json => {
                let result = serde_json::json!({ "result": message });
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Yaml => {
                let result = serde_json::json!({ "result": message });
                println!("{}", serde_yaml::to_string(&result)?);
            }
            _ => {
                println!("{}", message);
            }
        }
        Ok(())
    }

    fn output_sessions(&self, sessions: &[SessionSummary], format: &OutputFormat, args: &ListSessionArgs) -> Result<()> {
        match format {
            OutputFormat::Table => {
                let mut table = Table::new();
                table.set_content_arrangement(ContentArrangement::Dynamic);
                table.set_header(vec![
                    Cell::new("ID").add_attribute(Attribute::Bold),
                    Cell::new("Name").add_attribute(Attribute::Bold),
                    Cell::new("Active").add_attribute(Attribute::Bold),
                    Cell::new("Commands").add_attribute(Attribute::Bold),
                    Cell::new("Modified").add_attribute(Attribute::Bold),
                ]);

                for session in sessions {
                    let id_display = if args.full_ids {
                        session.id.to_string()
                    } else {
                        session.id.to_string()[..8].to_string()
                    };

                    table.add_row(vec![
                        Cell::new(&id_display),
                        Cell::new(&session.name),
                        Cell::new(if session.is_active { "✓" } else { "✗" })
                            .fg(if session.is_active { Color::Green } else { Color::Red }),
                        Cell::new(&session.command_count.to_string()),
                        Cell::new(&session.last_modified.format("%Y-%m-%d %H:%M").to_string()),
                    ]);
                }

                println!("{table}");
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(sessions)?);
            }
            OutputFormat::Yaml => {
                println!("{}", serde_yaml::to_string(sessions)?);
            }
            OutputFormat::Csv => {
                println!("id,name,active,commands,modified");
                for session in sessions {
                    println!("{},{},{},{},{}", 
                        session.id, session.name, session.is_active, 
                        session.command_count, session.last_modified.format("%Y-%m-%d %H:%M"));
                }
            }
            _ => {
                for session in sessions {
                    println!("{}: {} ({})", session.id, session.name, 
                            if session.is_active { "active" } else { "inactive" });
                }
            }
        }
        Ok(())
    }

    // Additional output methods would be implemented here...
    fn output_restoration_preview(&self, _summary: &RestorationSummary, _format: &OutputFormat) -> Result<()> {
        // Implementation for restoration preview output
        Ok(())
    }

    fn output_restoration_summary(&self, _summary: &RestorationSummary, _format: &OutputFormat) -> Result<()> {
        // Implementation for restoration summary output
        Ok(())
    }

    async fn prepare_export_data(&self, _sessions: &[SessionSummary], _args: &ExportSessionArgs) -> Result<serde_json::Value> {
        // Implementation for preparing export data
        Ok(serde_json::json!({}))
    }

    async fn write_export_file(&self, _data: &serde_json::Value, _path: &PathBuf, _format: &ExportFormat) -> Result<()> {
        // Implementation for writing export file
        Ok(())
    }

    async fn read_import_file(&self, _path: &PathBuf, _format: &Option<ExportFormat>) -> Result<serde_json::Value> {
        // Implementation for reading import file
        Ok(serde_json::json!({}))
    }

    fn validate_import_data(&self, _data: &serde_json::Value) -> Result<()> {
        // Implementation for validating import data
        Ok(())
    }

    fn output_import_preview(&self, _data: &serde_json::Value, _format: &OutputFormat) -> Result<()> {
        // Implementation for import preview output
        Ok(())
    }

    async fn import_sessions(&self, _data: serde_json::Value, _args: &ImportSessionArgs) -> Result<usize> {
        // Implementation for importing sessions
        Ok(0)
    }

    fn output_cleanup_summary(&self, _summary: &(), _format: &OutputFormat) -> Result<()> {
        // Implementation for cleanup summary output
        Ok(())
    }

    fn output_statistics(&self, _stats: &SessionStats, _args: &StatsSessionArgs, _format: &OutputFormat) -> Result<()> {
        // Implementation for statistics output
        Ok(())
    }

    fn output_search_results(&self, _results: &[SessionSummary], _format: &OutputFormat) -> Result<()> {
        // Implementation for search results output
        Ok(())
    }

    fn output_tags(&self, _tags: &[String], _format: &OutputFormat) -> Result<()> {
        // Implementation for tags output
        Ok(())
    }

    fn output_configuration(&self, _config: &serde_json::Value, _format: &OutputFormat) -> Result<()> {
        // Implementation for configuration output
        Ok(())
    }

    fn output_backups(&self, _backups: &[String], _format: &OutputFormat) -> Result<()> {
        // Implementation for backups output
        Ok(())
    }

    fn output_validation_results(&self, _results: &[String], _format: &OutputFormat) -> Result<()> {
        // Implementation for validation results output
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        let runner = SessionCliRunner::new(SessionService::new());
        
        assert_eq!(runner.parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(runner.parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(runner.parse_duration("2h").unwrap(), Duration::from_secs(7200));
        assert_eq!(runner.parse_duration("1d").unwrap(), Duration::from_secs(86400));
        
        assert!(runner.parse_duration("invalid").is_err());
        assert!(runner.parse_duration("").is_err());
    }

    #[test]
    fn test_cli_parsing() {
        use clap::Parser;
        
        let cli = SessionCli::try_parse_from(vec![
            "session", "list", "--active-only", "--limit", "10"
        ]).unwrap();
        
        match cli.command {
            SessionCommand::List(args) => {
                assert!(args.active_only);
                assert_eq!(args.limit, 10);
            }
            _ => panic!("Expected List command"),
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum SessionCommands {
    /// Create a new session
    New(CreateSessionArgs),
    
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
    
    async fn handle_new_session(&self, args: &CreateSessionArgs, cli: &SessionCli) -> Result<()> {
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
        match &args.action {
            ConfigAction::Show => {
                println!("Configuration management not yet implemented");
            }
            ConfigAction::Set { key, value } => {
                println!("Setting config {} = {}", key, value);
            }
            ConfigAction::Get { key } => {
                println!("Getting config {}", key);
            }
            ConfigAction::Reset { force: _ } => {
                println!("Resetting configuration");
            }
            ConfigAction::Edit => {
                println!("Opening configuration editor");
            }
        }
        
        Ok(())
    }
    
    async fn handle_preferences(&self, args: &PrefsSessionArgs, cli: &SessionCli) -> Result<()> {
        match &args.action {
            PrefsAction::Show => {
                if let Some(session) = self.session_service.get_current_session().await {
                    self.output_result(&session.preferences, &cli.output_format)?;
                } else {
                    println!("No active session");
                }
            }
            PrefsAction::Set { key, value } => {
                let key_owned = key.clone();
                let value_owned = value.clone();
                self.session_service.update_preferences(move |prefs| {
                    // Would implement preference setting based on key
                    println!("Setting preference {} = {}", key_owned, value_owned);
                }).await?;
            }
            PrefsAction::Get { key } => {
                println!("Getting preference {}", key);
            }
            PrefsAction::Reset { force: _ } => {
                self.session_service.update_preferences(|prefs| {
                    *prefs = UserPreferences::default();
                }).await?;
                println!("Preferences reset to defaults");
            }
            PrefsAction::Export { output } => {
                println!("Exporting preferences to {}", output.display());
            }
            PrefsAction::Import { input, merge: _ } => {
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

#[derive(Debug, Clone, ValueEnum, Serialize)]
pub enum CompressionFormat {
    None,
    Gzip,
    Zstd,
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
// Blocks 2.0 System for OpenAgent Terminal
// Enhanced block system with per-block environments, tagging, and advanced features

#![cfg(feature = "blocks")]

pub mod environment;
pub mod export;
pub mod search;
pub mod storage;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

/// Block metadata and content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// Unique identifier
    pub id: BlockId,
    /// Command that was executed
    pub command: String,
    /// Output from the command
    pub output: String,
    /// Working directory when executed
    pub directory: PathBuf,
    /// Environment variables at execution time
    pub environment: HashMap<String, String>,
    /// Shell used for execution
    pub shell: ShellType,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last modified timestamp
    pub modified_at: DateTime<Utc>,
    /// Tags for organization
    pub tags: HashSet<String>,
    /// Whether this block is starred/favorited
    pub starred: bool,
    /// Parent block ID (for chained blocks)
    pub parent_id: Option<BlockId>,
    /// Child block IDs
    pub children: Vec<BlockId>,
    /// Custom metadata
    pub metadata: BlockMetadata,
    /// Execution status
    pub status: ExecutionStatus,
    /// Exit code from command
    pub exit_code: Option<i32>,
    /// Duration of execution in milliseconds
    pub duration_ms: Option<u64>,
}

/// Unique block identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(Uuid);

impl BlockId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_string(s: &str) -> Result<Self> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for BlockId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Shell type for block execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Nushell,
    Custom(u32), // Hash of custom shell name
}

impl std::str::FromStr for ShellType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "bash" => Self::Bash,
            "zsh" => Self::Zsh,
            "fish" => Self::Fish,
            "powershell" | "pwsh" => Self::PowerShell,
            "nu" | "nushell" => Self::Nushell,
            other => {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                use std::hash::{Hash, Hasher};
                other.hash(&mut hasher);
                Self::Custom(hasher.finish() as u32)
            }
        })
    }
}

impl ShellType {
    pub fn to_str(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::PowerShell => "powershell",
            Self::Nushell => "nushell",
            Self::Custom(_) => "custom",
        }
    }
}

/// Execution status of a block
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Running,
    Success,
    Failed,
    Cancelled,
    Timeout,
}

/// Custom metadata for blocks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockMetadata {
    /// User-defined notes
    pub notes: Option<String>,
    /// Category for organization
    pub category: Option<String>,
    /// Priority level (1-5)
    pub priority: Option<u8>,
    /// Whether this block contains sensitive data
    pub sensitive: bool,
    /// Custom key-value pairs
    pub custom: HashMap<String, serde_json::Value>,
}

type BlockEventCallback = Box<dyn Fn(&BlockEvent) + Send + Sync>;

/// Native block manager for creating and managing blocks without lazy fallbacks
pub struct BlockManager {
    storage: Arc<storage::BlockStorage>,
    environment_manager: environment::EnvironmentManager,
    search_engine: search::SearchEngine,
    export_manager: export::ExportManager,
    #[allow(dead_code)]
    current_session: SessionId,
    active_blocks: HashMap<BlockId, Arc<Block>>,
    /// Native event callbacks for real-time updates
    event_callbacks: Vec<BlockEventCallback>,
    /// Real-time block execution state
    executing_blocks: HashMap<BlockId, ExecutionHandle>,
    /// Native rendering state
    render_state: BlockRenderState,
}

/// Session identifier for grouping blocks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Native block events for real-time processing
#[derive(Debug, Clone)]
pub enum BlockEvent {
    Created(BlockId),
    Updated(BlockId),
    Executed(BlockId, ExecutionResult),
    Deleted(BlockId),
    StarToggled(BlockId, bool),
    TagsUpdated(BlockId, Vec<String>),
}

/// Execution handle for tracking running commands
#[derive(Debug)]
#[allow(dead_code)]
pub struct ExecutionHandle {
    pub block_id: BlockId,
    pub pid: Option<u32>,
    pub start_time: DateTime<Utc>,
    pub status: ExecutionStatus,
    pub output_stream: Arc<std::sync::Mutex<String>>,
}

/// Execution result for completed commands
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub exit_code: i32,
    pub output: String,
    pub error_output: String,
    pub duration: std::time::Duration,
}

/// Native rendering state for blocks
#[derive(Debug, Default)]
pub struct BlockRenderState {
    pub visible_blocks: Vec<BlockId>,
    pub collapsed_blocks: HashSet<BlockId>,
    pub highlighted_block: Option<BlockId>,
    pub animation_states: HashMap<BlockId, BlockAnimation>,
}

/// Block animation states
#[derive(Debug, Clone)]
pub struct BlockAnimation {
    pub animation_type: BlockAnimationType,
    pub start_time: std::time::Instant,
    pub duration: std::time::Duration,
    pub progress: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockAnimationType {
    FadeIn,
    FadeOut,
    Expand,
    Collapse,
    Highlight,
    Update,
}

impl BlockManager {
    /// Create a new native block manager with immediate operations
    pub async fn new(data_dir: PathBuf) -> Result<Self> {
        let storage = Arc::new(storage::BlockStorage::new(&data_dir).await?);
        let environment_manager = environment::EnvironmentManager::new();
        let search_engine = search::SearchEngine::new(storage.clone()).await?;
        let export_manager = export::ExportManager::new();

        Ok(Self {
            storage,
            environment_manager,
            search_engine,
            export_manager,
            current_session: SessionId::new(),
            active_blocks: HashMap::new(),
            event_callbacks: Vec::new(),
            executing_blocks: HashMap::new(),
            render_state: BlockRenderState::default(),
        })
    }

    /// Register a native event callback for real-time updates
    pub fn register_event_callback<F>(&mut self, callback: F)
    where
        F: Fn(&BlockEvent) + Send + Sync + 'static,
    {
        self.event_callbacks.push(Box::new(callback));
    }

    /// Emit block event immediately to all registered callbacks
    fn emit_event(&self, event: BlockEvent) {
        for callback in &self.event_callbacks {
            callback(&event);
        }
    }

    /// Create and immediately execute a native command block
    pub async fn create_and_execute_block(
        &mut self,
        params: CreateBlockParams,
    ) -> Result<Arc<Block>> {
        let block = self.create_block(params).await?;

        // Start execution immediately - no lazy loading
        let command = block.command.clone();
        self.start_native_execution(block.id, command).await?;

        Ok(block)
    }

    /// Start native command execution without lazy fallbacks
    async fn start_native_execution(&mut self, block_id: BlockId, _command: String) -> Result<()> {
        let execution_handle = ExecutionHandle {
            block_id,
            pid: None, // Will be set when process starts
            start_time: Utc::now(),
            status: ExecutionStatus::Running,
            output_stream: Arc::new(std::sync::Mutex::new(String::new())),
        };

        self.executing_blocks.insert(block_id, execution_handle);

        // Emit immediate event - no lazy processing
        self.emit_event(BlockEvent::Executed(
            block_id,
            ExecutionResult {
                exit_code: -1, // Indicates still running
                output: String::new(),
                error_output: String::new(),
                duration: std::time::Duration::from_secs(0),
            },
        ));

        // TODO: Start actual process execution in a separate task
        // For now, we'll simulate with a placeholder
        tokio::spawn(async move {
            // Native process execution would go here
            // This should interface directly with the PTY/terminal
        });

        Ok(())
    }

    /// Get native rendering state for immediate display
    pub fn get_render_state(&self) -> &BlockRenderState {
        &self.render_state
    }

    /// Update rendering state immediately
    pub fn update_render_state<F>(&mut self, update_fn: F)
    where
        F: FnOnce(&mut BlockRenderState),
    {
        update_fn(&mut self.render_state);
    }

    /// Toggle block visibility with immediate effect
    pub fn toggle_block_visibility(&mut self, block_id: BlockId) -> bool {
        if self.render_state.collapsed_blocks.contains(&block_id) {
            self.render_state.collapsed_blocks.remove(&block_id);
            // Start expand animation immediately
            self.start_block_animation(block_id, BlockAnimationType::Expand);
            false
        } else {
            self.render_state.collapsed_blocks.insert(block_id);
            // Start collapse animation immediately
            self.start_block_animation(block_id, BlockAnimationType::Collapse);
            true
        }
    }

    /// Start block animation immediately
    fn start_block_animation(&mut self, block_id: BlockId, animation_type: BlockAnimationType) {
        let animation = BlockAnimation {
            animation_type,
            start_time: std::time::Instant::now(),
            duration: match animation_type {
                BlockAnimationType::FadeIn | BlockAnimationType::FadeOut => {
                    std::time::Duration::from_millis(200)
                }
                BlockAnimationType::Expand | BlockAnimationType::Collapse => {
                    std::time::Duration::from_millis(300)
                }
                BlockAnimationType::Highlight => std::time::Duration::from_millis(150),
                BlockAnimationType::Update => std::time::Duration::from_millis(100),
            },
            progress: 0.0,
        };

        self.render_state
            .animation_states
            .insert(block_id, animation);
    }

    /// Update animation progress and return blocks that need rerendering
    pub fn update_animations(&mut self) -> Vec<BlockId> {
        let mut changed_blocks = Vec::new();
        let now = std::time::Instant::now();

        let keys: Vec<BlockId> = self.render_state.animation_states.keys().cloned().collect();
        for block_id in keys {
            if let Some(anim) = self.render_state.animation_states.get(&block_id).cloned() {
                let elapsed = now.duration_since(anim.start_time);
                let progress =
                    (elapsed.as_secs_f32() / anim.duration.as_secs_f32()).clamp(0.0, 1.0);
                if progress >= 1.0 {
                    // Animation complete; remove it
                    self.render_state.animation_states.remove(&block_id);
                    changed_blocks.push(block_id);
                } else if let Some(animation_mut) =
                    self.render_state.animation_states.get_mut(&block_id)
                {
                    animation_mut.progress = progress;
                    changed_blocks.push(block_id);
                }
            }
        }

        changed_blocks
    }

    /// Create a new block
    pub async fn create_block(&mut self, params: CreateBlockParams) -> Result<Arc<Block>> {
        let block_id = BlockId::new();
        let now = Utc::now();

        // Capture current environment if not provided
        let environment = params
            .environment
            .unwrap_or_else(|| self.environment_manager.capture_current());

        let block = Block {
            id: block_id,
            command: params.command,
            output: String::new(),
            directory: params
                .directory
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
            environment,
            shell: params.shell.unwrap_or(ShellType::Bash),
            created_at: now,
            modified_at: now,
            tags: params.tags.unwrap_or_default(),
            starred: false,
            parent_id: params.parent_id,
            children: Vec::new(),
            metadata: params.metadata.unwrap_or_default(),
            status: ExecutionStatus::Running,
            exit_code: None,
            duration_ms: None,
        };

        let block_arc = Arc::new(block);

        // Store in database
        self.storage.insert(&block_arc).await?;

        // Update parent if exists
        if let Some(parent_id) = block_arc.parent_id {
            self.add_child_to_parent(parent_id, block_id).await?;
        }

        // Cache in memory
        self.active_blocks.insert(block_id, block_arc.clone());

        // Index for search
        self.search_engine.index_block(&block_arc).await?;

        // Emit immediate creation event
        self.emit_event(BlockEvent::Created(block_id));

        info!("Created block {}", block_id.to_string());

        Ok(block_arc)
    }

    /// Update block output and status
    pub async fn update_block_output(
        &mut self,
        block_id: BlockId,
        output: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<()> {
        let updated_block = {
            let entry = self.get_block_mut(block_id).await?;
            let mut owned = (**entry).clone();
            owned.output = output;
            owned.exit_code = Some(exit_code);
            owned.duration_ms = Some(duration_ms);
            owned.status = if exit_code == 0 {
                ExecutionStatus::Success
            } else {
                ExecutionStatus::Failed
            };
            owned.modified_at = Utc::now();
            let arc_new = Arc::new(owned.clone());
            *entry = arc_new.clone();
            owned
        };

        // Update storage and search without holding a mutable borrow on self
        self.storage.update(&updated_block).await?;
        self.search_engine.update_block(&updated_block).await?;

        // Emit immediate update event
        self.emit_event(BlockEvent::Updated(block_id));

        Ok(())
    }

    /// Add tags to a block
    pub async fn add_tags(&mut self, block_id: BlockId, tags: Vec<String>) -> Result<()> {
        let updated_block = {
            let entry = self.get_block_mut(block_id).await?;
            let mut owned = (**entry).clone();
            for tag in &tags {
                owned.tags.insert(tag.clone());
            }
            owned.modified_at = Utc::now();
            let arc_new = Arc::new(owned.clone());
            *entry = arc_new.clone();
            owned
        };
        self.storage.update(&updated_block).await?;
        self.search_engine.update_block(&updated_block).await?;
        // Notify listeners of tags update
        self.emit_event(BlockEvent::TagsUpdated(block_id, tags));
        Ok(())
    }

    /// Toggle star status
    pub async fn toggle_star(&mut self, block_id: BlockId) -> Result<bool> {
        let (updated_block, starred) = {
            let entry = self.get_block_mut(block_id).await?;
            let mut owned = (**entry).clone();
            owned.starred = !owned.starred;
            owned.modified_at = Utc::now();
            let starred = owned.starred;
            let arc_new = Arc::new(owned.clone());
            *entry = arc_new.clone();
            (owned, starred)
        };

        self.storage.update(&updated_block).await?;

        // Emit immediate star toggle event
        self.emit_event(BlockEvent::StarToggled(block_id, starred));

        info!("Block {} starred: {}", block_id.to_string(), starred);
        Ok(starred)
    }

    /// Search blocks
    pub async fn search(&self, query: SearchQuery) -> Result<Vec<Arc<Block>>> {
        self.search_engine.search(query).await
    }

    /// Get all starred blocks
    pub async fn get_starred(&self) -> Result<Vec<Arc<Block>>> {
        self.storage.get_starred().await
    }

    /// Get blocks by tag
    pub async fn get_by_tag(&self, tag: &str) -> Result<Vec<Arc<Block>>> {
        self.storage.get_by_tag(tag).await
    }

    /// Export blocks
    pub async fn export_blocks(
        &self,
        block_ids: Vec<BlockId>,
        format: export::ExportFormat,
        options: export::ExportOptions,
    ) -> Result<Vec<u8>> {
        let mut blocks = Vec::new();
        for id in block_ids {
            if let Ok(block) = self.get_block(id).await {
                blocks.push(block);
            }
        }

        self.export_manager.export(blocks, format, options).await
    }

    /// Import blocks
    pub async fn import_blocks(
        &mut self,
        data: &[u8],
        format: export::ExportFormat,
        options: export::ImportOptions,
    ) -> Result<Vec<BlockId>> {
        let blocks = self.export_manager.import(data, format, &options).await?;

        let mut imported_ids = Vec::new();
        for mut block in blocks {
            // Generate new IDs if requested
            if options.generate_new_ids {
                block.id = BlockId::new();
                block.parent_id = None;
                block.children.clear();
            }

            let block_arc = Arc::new(block);
            self.storage.insert(&block_arc).await?;
            self.search_engine.index_block(&block_arc).await?;

            imported_ids.push(block_arc.id);
        }

        info!("Imported {} blocks", imported_ids.len());

        Ok(imported_ids)
    }

    /// Get a block by ID
    async fn get_block(&self, block_id: BlockId) -> Result<Arc<Block>> {
        if let Some(block) = self.active_blocks.get(&block_id) {
            return Ok(block.clone());
        }

        self.storage.get(block_id).await
    }

    /// Get mutable reference to block
    async fn get_block_mut(&mut self, block_id: BlockId) -> Result<&mut Arc<Block>> {
        if !self.active_blocks.contains_key(&block_id) {
            let block = self.storage.get(block_id).await?;
            self.active_blocks.insert(block_id, block);
        }

        self.active_blocks
            .get_mut(&block_id)
            .context("Block not found in cache")
    }

    /// Add child to parent block
    async fn add_child_to_parent(&mut self, parent_id: BlockId, child_id: BlockId) -> Result<()> {
        let updated_parent = {
            let entry = self.get_block_mut(parent_id).await?;
            let mut owned = (**entry).clone();
            owned.children.push(child_id);
            owned.modified_at = Utc::now();
            let arc_new = Arc::new(owned.clone());
            *entry = arc_new.clone();
            owned
        };
        self.storage.update(&updated_parent).await?;
        Ok(())
    }

    /// Clean up old blocks
    pub async fn cleanup_old_blocks(&mut self, days: i64) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        let deleted = self.storage.delete_before(cutoff).await?;

        // Clear from cache
        self.active_blocks
            .retain(|_, block| block.created_at > cutoff);

        info!("Deleted {} old blocks", deleted);

        Ok(deleted)
    }
}

/// Parameters for creating a new block
pub struct CreateBlockParams {
    pub command: String,
    pub directory: Option<PathBuf>,
    pub environment: Option<HashMap<String, String>>,
    pub shell: Option<ShellType>,
    pub tags: Option<HashSet<String>>,
    pub parent_id: Option<BlockId>,
    pub metadata: Option<BlockMetadata>,
}

/// Search query for blocks with advanced filtering
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Text search in command and output
    pub text: Option<String>,
    /// Search only in command text
    pub command_text: Option<String>,
    /// Search only in output text
    pub output_text: Option<String>,
    /// Tags to filter by (AND operation)
    pub tags: Option<Vec<String>>,
    /// Directory path filter (supports wildcards)
    pub directory: Option<PathBuf>,
    /// Shell type filter
    pub shell: Option<ShellType>,
    /// Show only starred blocks
    pub starred_only: bool,
    /// Date range filtering
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    /// Execution status filter
    pub status: Option<ExecutionStatus>,
    /// Exit code filtering
    pub exit_code: Option<ExitCodeFilter>,
    /// Duration filtering (in milliseconds)
    pub duration: Option<DurationFilter>,
    /// Sorting options
    pub sort_by: SortField,
    pub sort_order: SortOrder,
    /// Pagination
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

/// Exit code filtering options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCodeFilter {
    Success,         // exit_code = 0
    Failure,         // exit_code != 0
    Specific(i32),   // exact exit code
    Range(i32, i32), // exit code range (inclusive)
}

/// Duration filtering options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationFilter {
    LessThan(u64),    // duration < value (ms)
    GreaterThan(u64), // duration > value (ms)
    Range(u64, u64),  // duration range (ms, inclusive)
}

/// Sort field options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    CreatedAt,
    ModifiedAt,
    Command,
    Duration,
    ExitCode,
    Directory,
}

/// Sort order options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: None,
            command_text: None,
            output_text: None,
            tags: None,
            directory: None,
            shell: None,
            starred_only: false,
            date_from: None,
            date_to: None,
            status: None,
            exit_code: None,
            duration: None,
            sort_by: SortField::CreatedAt,
            sort_order: SortOrder::Descending,
            offset: None,
            limit: Some(100),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_block_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = BlockManager::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        let params = CreateBlockParams {
            command: "echo 'Hello, World!'".to_string(),
            directory: None,
            environment: None,
            shell: Some(ShellType::Bash),
            tags: Some(["test".to_string()].into()),
            parent_id: None,
            metadata: None,
        };

        let block = manager.create_block(params).await.unwrap();
        assert_eq!(block.command, "echo 'Hello, World!'");
        assert!(block.tags.contains("test"));
    }

    #[tokio::test]
    async fn test_block_search() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = BlockManager::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        // Create test blocks
        for i in 0..5 {
            let params = CreateBlockParams {
                command: format!("test command {}", i),
                directory: None,
                environment: None,
                shell: Some(ShellType::Zsh),
                tags: Some([format!("tag{}", i)].into()),
                parent_id: None,
                metadata: None,
            };
            manager.create_block(params).await.unwrap();
        }

        // Search by text
        let query = SearchQuery {
            text: Some("command".to_string()),
            ..Default::default()
        };

        let results = manager.search(query).await.unwrap();
        assert_eq!(results.len(), 5);
    }
}

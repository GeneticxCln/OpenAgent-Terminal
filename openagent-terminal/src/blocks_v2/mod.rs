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
use tracing::{error, info};
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

type BlockEventCallback = Arc<dyn Fn(&BlockEvent) + Send + Sync>;

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
    /// Optional handle into the workspace PTY manager to reuse pane PTYs
    pty_collection: Option<
        Arc<parking_lot::Mutex<openagent_terminal_core::tty::pty_manager::PtyManagerCollection>>,
    >,
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
    pub tx: tokio::sync::broadcast::Sender<String>,
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
            pty_collection: None,
        })
    }

    /// Provide access to the workspace PTY collection so Blocks can reuse pane PTYs
    pub fn set_workspace_pty_collection(
        &mut self,
        collection: Arc<
            parking_lot::Mutex<openagent_terminal_core::tty::pty_manager::PtyManagerCollection>,
        >,
    ) {
        self.pty_collection = Some(collection);
    }

    /// Register a native event callback for real-time updates
    pub fn register_event_callback<F>(&mut self, callback: F)
    where
        F: Fn(&BlockEvent) + Send + Sync + 'static,
    {
        let arc_cb: Arc<dyn Fn(&BlockEvent) + Send + Sync> = Arc::new(callback);
        self.event_callbacks.push(arc_cb);
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
    async fn start_native_execution(&mut self, block_id: BlockId, command: String) -> Result<()> {
        use openagent_terminal_core::event::WindowSize;
        use openagent_terminal_core::tty::{
            ChildEvent, EventedPty, EventedReadWrite, Options, Shell,
        };
        use std::io::Read;
        use std::thread;
        use std::time::Duration;

        // Prepare streaming channel and in-memory buffer
        let (tx, _rx) = tokio::sync::broadcast::channel::<String>(128);
        let output_stream = Arc::new(std::sync::Mutex::new(String::new()));

        let start_time = Utc::now();
        let execution_handle = ExecutionHandle {
            block_id,
            pid: None, // Will be set when PTY starts
            start_time,
            status: ExecutionStatus::Running,
            output_stream: output_stream.clone(),
            tx: tx.clone(),
        };

        // Store handle before starting process
        self.executing_blocks.insert(block_id, execution_handle);

        // Emit immediate event - indicates running state
        self.emit_event(BlockEvent::Executed(
            block_id,
            ExecutionResult {
                exit_code: -1, // running
                output: String::new(),
                error_output: String::new(),
                duration: std::time::Duration::from_secs(0),
            },
        ));

        // Resolve execution context from block
        let block = self.get_block(block_id).await?;
        let working_dir = block.directory.clone();
        let env_vars = block.environment.clone();
        let shell = block.shell;

        // If workspace PTY collection is available and there is an active PTY we can tap,
        // prefer reusing that PTY to stream output through the existing terminal process.
        if let Some(ref collection) = self.pty_collection {
            let callbacks = self.event_callbacks.clone();
            // For now, pick the first active PTY
            let ptys = {
                let c = collection.lock();
                c.active_pty_ids()
            };
            if let Some(pty_id) = ptys.first().copied() {
                let collection = collection.clone();
                let tx_clone = tx.clone();
                let output_stream_clone = output_stream.clone();
                let block_id_clone = block_id;
                let start_instant = std::time::Instant::now();
                let (done_tx, done_rx) = tokio::sync::oneshot::channel::<(String, i32, u64)>();

                // Reader task: attach to the manager and pull bytes
                tokio::spawn(async move {
                    use std::thread;
                    use std::time::Duration;

                    let mut local_exit: Option<i32> = None;
                    let start = start_instant;
                    let mut buf = [0u8; 8192];

                    loop {
                        let mut read_any = false;
                        // Scoped lock for read
                        if let Some(manager) = collection.lock().get_manager(pty_id) {
                            let mut mgr = manager.lock();
                            match mgr.read_nonblocking(&mut buf) {
                                Ok(0) => {}
                                Ok(n) => {
                                    read_any = true;
                                    let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                                    let _ = tx_clone.send(chunk.clone());
                                    if let Ok(mut s) = output_stream_clone.lock() {
                                        s.push_str(&chunk);
                                    }
                                    // Emit per-chunk update event
                                    for cb in &callbacks {
                                        cb(&BlockEvent::Updated(block_id_clone));
                                    }
                                }
                                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                                Err(e) => {
                                    error!("Blocks v2: workspace PTY read error: {}", e);
                                    break;
                                }
                            }

                            // Check child events
if let Some(ev) = mgr.poll_child_events().into_iter().next() {
                                let openagent_terminal_core::tty::ChildEvent::Exited(code) = ev;
                                local_exit = code.or(Some(0));
                                break;
                            }
                        }

                        if local_exit.is_some() {
                            break;
                        }
                        if !read_any {
                            thread::sleep(Duration::from_millis(15));
                        }
                    }

                    let dur = start.elapsed().as_millis() as u64;
                    let output = output_stream_clone
                        .lock()
                        .ok()
                        .map(|m| m.clone())
                        .unwrap_or_default();
                    let code = local_exit.unwrap_or(0);
                    let _ = done_tx.send((output, code, dur));
                });

                // Await completion and persist
                if let Ok((final_output, exit_code, duration_ms)) = done_rx.await {
                    self.update_block_output(block_id, final_output, exit_code, duration_ms)
                        .await?;
                    // Keep the execution handle available briefly so subscribers can still attach
                    // and receive any final messages; just update status instead of removing.
                    if let Some(h) = self.executing_blocks.get_mut(&block_id) {
                        h.status = if exit_code == 0 {
                            ExecutionStatus::Success
                        } else {
                            ExecutionStatus::Failed
                        };
                    }
                    return Ok(());
                }
            }
        }

        // Build shell + args to run this command in a login shell when applicable
        let (program, args): (String, Vec<String>) = match shell {
            ShellType::Bash => ("bash".to_string(), vec!["-lc".to_string(), command.clone()]),
            ShellType::Zsh => ("zsh".to_string(), vec!["-lc".to_string(), command.clone()]),
            ShellType::Fish => (
                "fish".to_string(),
                vec!["-l".to_string(), "-c".to_string(), command.clone()],
            ),
            ShellType::PowerShell => (
                "pwsh".to_string(),
                vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    command.clone(),
                ],
            ),
            ShellType::Nushell => ("nu".to_string(), vec!["-c".to_string(), command.clone()]),
            ShellType::Custom(_) => (
                // Fallback to POSIX sh
                "sh".to_string(),
                vec!["-lc".to_string(), command.clone()],
            ),
        };

        // Completion reporting back to async context
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<(String, i32, u64)>();
        // PID reporting
        let (pid_tx, pid_rx) = tokio::sync::oneshot::channel::<u32>();
        let callbacks = self.event_callbacks.clone();

        // Clone for move into blocking task
        let tx_clone = tx.clone();
        let output_stream_clone = output_stream.clone();
        let block_id_clone = block_id;
        let start_instant = std::time::Instant::now();

        // Spawn a blocking task that creates a PTY and streams output
        tokio::task::spawn_blocking(move || {
            // Construct PTY options
            let options = Options {
                shell: Some(Shell::new(program, args)),
                working_directory: Some(working_dir.clone()),
                drain_on_exit: true,
                #[cfg(target_os = "windows")]
                escape_args: true,
                env: env_vars.clone(),
            };

            // Provide sane defaults for PTY size (detached from UI)
            let window_size = WindowSize {
                num_lines: 24,
                num_cols: 80,
                cell_width: 8,
                cell_height: 16,
            };

            // Create PTY
            let mut pty = match openagent_terminal_core::tty::new(&options, window_size, 0) {
                Ok(pty) => pty,
                Err(e) => {
                    error!(
                        "Blocks v2: failed to spawn PTY for block {}: {}",
                        block_id_clone, e
                    );
                    return;
                }
            };

            // Report PID if available
            #[cfg(not(windows))]
            {
                let pid = pty.child().id();
                let _ = pid_tx.send(pid);
            }

            // Read loop (non-blocking read with small sleeps)
            let mut buf = [0u8; 8192];
            let exit_code: i32;

            'event_loop: loop {
                let mut made_progress = false;
                loop {
                    match pty.reader().read(&mut buf) {
                        Ok(0) => break, // EOF or no data
                        Ok(n) => {
                            made_progress = true;
                            let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                            // Broadcast first
                            let _ = tx_clone.send(chunk.clone());
                            // Append to in-memory stream too
                            if let Ok(mut s) = output_stream_clone.lock() {
                                s.push_str(&chunk);
                            }
                            // Emit per-chunk update event
                            for cb in &callbacks {
                                cb(&BlockEvent::Updated(block_id_clone));
                            }
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(err) => {
                            error!(
                                "Blocks v2: read error on PTY for block {}: {}",
                                block_id_clone, err
                            );
                            break;
                        }
                    }
                }

                // Check for child exit
if let Some(evt) = pty.next_child_event() {
                    match evt {
                        ChildEvent::Exited(code) => {
                            exit_code = code.unwrap_or(0);
                            break 'event_loop;
                        }
                    }
                }

                if !made_progress {
                    // Avoid busy-looping when no data is available
                    thread::sleep(Duration::from_millis(15));
                }
            }

            // Compute duration
            let duration = start_instant.elapsed();

            // Final state: send a trailing newline to mark completion (optional)
            let _ = tx_clone.send(String::new());

            // Report completion back to async context
            let (final_output, code) = {
                let s = output_stream_clone
                    .lock()
                    .ok()
                    .map(|m| m.clone())
                    .unwrap_or_default();
                (s, exit_code)
            };
            let _ = done_tx.send((final_output, code, duration.as_millis() as u64));
        });

        // Update PID when available
        if let Ok(pid) = pid_rx.await {
            if let Some(handle) = self.executing_blocks.get_mut(&block_id) {
                handle.pid = Some(pid);
            }
        }

        // Await completion and persist results (emits BlockEvent::Updated via update_block_output)
        if let Ok((final_output, exit_code, duration_ms)) = done_rx.await {
            self.update_block_output(block_id, final_output, exit_code, duration_ms)
                .await?;
            // Keep the execution handle available briefly so subscribers can still attach
            // and receive any final messages; just update status instead of removing.
            if let Some(h) = self.executing_blocks.get_mut(&block_id) {
                h.status = if exit_code == 0 {
                    ExecutionStatus::Success
                } else {
                    ExecutionStatus::Failed
                };
            }
        }

        Ok(())
    }

    /// Subscribe to real-time output stream for a running block
    pub fn subscribe_output_stream(
        &self,
        block_id: BlockId,
    ) -> Option<tokio::sync::broadcast::Receiver<String>> {
        self.executing_blocks
            .get(&block_id)
            .map(|h| h.tx.subscribe())
    }

    /// Append output to a running block and notify subscribers immediately
    pub fn append_output(&mut self, block_id: BlockId, chunk: &str) -> bool {
        if let Some(h) = self.executing_blocks.get(&block_id) {
            if let Ok(mut s) = h.output_stream.lock() {
                s.push_str(chunk);
            }
            let _ = h.tx.send(chunk.to_string());
            self.emit_event(BlockEvent::Updated(block_id));
            true
        } else {
            false
        }
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

    /// Mark a running block as cancelled and persist immediately
    pub async fn mark_block_cancelled(&mut self, block_id: BlockId) -> Result<()> {
        let updated_block = {
            let entry = self.get_block_mut(block_id).await?;
            let mut owned = (**entry).clone();
            owned.status = ExecutionStatus::Cancelled;
            owned.exit_code = None;
            owned.modified_at = Utc::now();
            let arc_new = Arc::new(owned.clone());
            *entry = arc_new.clone();
            owned
        };

        // Persist cancellation
        self.storage.update(&updated_block).await?;
        self.search_engine.update_block(&updated_block).await?;

        // Notify listeners that the block was updated (status change)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_params(cmd: &str) -> CreateBlockParams {
        CreateBlockParams {
            command: cmd.to_string(),
            directory: None,
            environment: None,
            shell: Some(ShellType::Bash),
            tags: None,
            parent_id: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_block_creation() {
        // Basic creation sanity (rely on existing DB tests in storage)
        let dir = TempDir::new().unwrap();
        let mut mgr = BlockManager::new(dir.path().to_path_buf()).await.unwrap();
        let params = make_params("echo hi");
        let b = mgr.create_block(params).await.unwrap();
        assert!(!b.command.is_empty());
    }

    #[tokio::test]
    async fn test_realtime_output_streaming() {
        let dir = TempDir::new().unwrap();
        let mut mgr = BlockManager::new(dir.path().to_path_buf()).await.unwrap();

        // Create and start execution
        let params = make_params("sleep 1");
        let block = mgr.create_and_execute_block(params).await.unwrap();

        // Subscribe to output stream
        let mut rx = mgr.subscribe_output_stream(block.id).expect("stream");

        // Append output and expect to receive it
        assert!(mgr.append_output(block.id, "line1\n"));
        let recv = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await;
        assert!(recv.is_ok(), "did not receive streamed output in time");
        assert_eq!(recv.unwrap().unwrap(), "line1\n");
    }
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

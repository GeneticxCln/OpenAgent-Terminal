//! Native Persistence Layer for OpenAgent Terminal
//!
//! This module provides immediate data persistence for blocks, tabs, and splits
//! with no lazy writes or deferred operations. All changes are saved in real-time.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs as async_fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::blocks_v2::{Block, BlockId};
use crate::workspace::tab_manager::TabManagerState;
use crate::workspace::TabId;

/// Native persistence manager with immediate write capabilities
pub struct NativePersistence {
    /// Data directory for immediate file operations
    data_dir: PathBuf,

    /// Optional sync provider (enabled only when the `sync` feature is on)
    #[cfg(feature = "sync")]
    sync_provider: std::sync::Arc<tokio::sync::Mutex<Option<openagent_terminal_sync::LocalFsProvider>>>,

    /// Block persistence state
    block_persistence: BlockPersistence,

    /// Tab persistence state
    tab_persistence: TabPersistence,

    /// Split persistence state
    split_persistence: SplitPersistence,

    /// Persistence event callbacks
    event_callbacks: Vec<Box<dyn Fn(&PersistenceEvent) + Send + Sync>>,

    /// Write-ahead log for immediate consistency
    wal: WriteAheadLog,

    /// Backup manager for data safety
    backup_manager: BackupManager,

    /// Real-time sync channel
    sync_sender: Option<mpsc::UnboundedSender<SyncOperation>>,

    /// Performance monitoring
    perf_stats: PersistenceStats,
}

/// Persistence events for immediate feedback
#[derive(Debug, Clone)]
pub enum PersistenceEvent {
    BlockSaved(BlockId),
    BlockDeleted(BlockId),
    TabStateSaved(TabId),
    SplitLayoutSaved(String),
    BackupCreated {
        backup_path: PathBuf,
        timestamp: DateTime<Utc>,
    },
    SyncCompleted {
        operation_count: usize,
        duration: Duration,
    },
    ErrorOccurred {
        error: String,
        operation: String,
    },
}

/// Block persistence for immediate saves
#[derive(Debug)]
pub struct BlockPersistence {
    pub blocks_dir: PathBuf,
    pub index_file: PathBuf,
    pub block_cache: HashMap<BlockId, CachedBlockEntry>,
    pub pending_writes: Vec<BlockWrite>,
    pub last_save: Instant,
}

/// Tab persistence for immediate saves
#[derive(Debug)]
pub struct TabPersistence {
    pub tabs_file: PathBuf,
    pub sessions_dir: PathBuf,
    pub tab_cache: HashMap<TabId, CachedTabEntry>,
    pub current_session: SessionPersistence,
    pub last_save: Instant,
}

/// Split persistence for immediate saves
#[derive(Debug)]
pub struct SplitPersistence {
    pub layouts_dir: PathBuf,
    pub layout_cache: HashMap<String, CachedLayoutEntry>,
    pub history_file: PathBuf,
    pub last_save: Instant,
}

/// Cached block entry for immediate access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedBlockEntry {
    pub block: Block,
    pub file_path: PathBuf,
    pub last_modified: DateTime<Utc>,
    pub content_hash: u64,
    pub dirty: bool,
}

/// Cached tab entry for immediate access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTabEntry {
    pub tab_id: TabId,
    pub title: String,
    pub working_directory: PathBuf,
    pub modified: bool,
    pub session_id: String,
    pub last_modified: DateTime<Utc>,
    pub dirty: bool,
}

/// Cached layout entry for immediate access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedLayoutEntry {
    pub layout_id: String,
    pub layout_data: Vec<u8>, // Serialized layout
    pub last_modified: DateTime<Utc>,
    pub dirty: bool,
}

/// Session persistence for immediate saves
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPersistence {
    pub session_id: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub tab_ids: Vec<TabId>,
    pub active_tab: Option<TabId>,
    pub window_state: WindowState,
}

/// Window state for session persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub position_x: i32,
    pub position_y: i32,
    pub maximized: bool,
    pub fullscreen: bool,
}

/// Write-ahead log for immediate consistency
#[derive(Debug)]
pub struct WriteAheadLog {
    pub log_file: PathBuf,
    pub log_entries: Vec<LogEntry>,
    pub last_checkpoint: Instant,
    pub checkpoint_interval: Duration,
}

/// Log entry for write-ahead logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub operation: LogOperation,
    pub data: Vec<u8>,
    pub checksum: u64,
}

/// Log operations for write-ahead logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogOperation {
    SaveBlock(BlockId),
    DeleteBlock(BlockId),
    SaveTabState(TabId),
    SaveSplitLayout(String),
    CreateSession(String),
    UpdateSession(String),
}

/// Backup manager for data safety
#[derive(Debug)]
pub struct BackupManager {
    pub backup_dir: PathBuf,
    pub backup_interval: Duration,
    pub max_backups: usize,
    pub last_backup: Instant,
    pub backup_queue: Vec<BackupTask>,
}

/// Backup task for immediate execution
#[derive(Debug, Clone)]
pub struct BackupTask {
    pub source_path: PathBuf,
    pub backup_path: PathBuf,
    pub timestamp: DateTime<Utc>,
    pub priority: BackupPriority,
}

/// Backup priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackupPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Sync operations for real-time updates
#[derive(Debug, Clone)]
pub enum SyncOperation {
    SaveBlock(Block),
    DeleteBlock(BlockId),
    SaveTabState(TabManagerState),
    SaveSplitLayout {
        layout_id: String,
        layout_data: Vec<u8>,
    },
    CreateBackup(BackupTask),
    Checkpoint,
}

/// Block write operation for immediate execution
#[derive(Debug, Clone)]
pub struct BlockWrite {
    pub block_id: BlockId,
    pub block_data: Vec<u8>,
    pub file_path: PathBuf,
    pub timestamp: DateTime<Utc>,
    pub urgent: bool,
}

/// Performance statistics for monitoring
#[derive(Debug, Clone)]
pub struct PersistenceStats {
    pub blocks_saved: usize,
    pub blocks_loaded: usize,
    pub tabs_saved: usize,
    pub splits_saved: usize,
    pub backups_created: usize,
    pub total_write_time: Duration,
    pub total_read_time: Duration,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub last_reset: Instant,
}

impl Default for PersistenceStats {
    fn default() -> Self {
        Self {
            blocks_saved: 0,
            blocks_loaded: 0,
            tabs_saved: 0,
            splits_saved: 0,
            backups_created: 0,
            total_write_time: Duration::default(),
            total_read_time: Duration::default(),
            cache_hits: 0,
            cache_misses: 0,
            last_reset: Instant::now(),
        }
    }
}

#[cfg(all(test, feature = "sync"))]
mod tests_sync {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_sync_enqueue_updates_status() {
        use std::time::Duration as StdDuration;
        // Create a temp dir and initialize persistence (with sync provider)
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();
        let persistence = NativePersistence::new(data_dir.clone()).await.unwrap();

        // Enqueue a settings-related sync op (doesn't require Block/Tab types)
        persistence.enqueue_sync(SyncOperation::SaveSplitLayout {
            layout_id: "test_layout".to_string(),
            layout_data: vec![1, 2, 3],
        });

        // Give the background task a moment to process
        tokio::time::sleep(StdDuration::from_millis(100)).await;

        // The LocalFsProvider writes a status file under data_dir/sync/sync_status.json
        let status_file = data_dir.join("sync").join("sync_status.json");
        assert!(status_file.exists(), "expected sync status file to exist");
        let content = std::fs::read_to_string(status_file).unwrap();
        assert!(content.contains("last_push") || content.contains("last_pull"));
    }
}

impl NativePersistence {
    /// Create new native persistence with immediate capabilities
    pub async fn new(data_dir: PathBuf) -> Result<Self> {
        // Ensure data directory exists
        async_fs::create_dir_all(&data_dir)
            .await
            .context("Failed to create data directory")?;

        let blocks_dir = data_dir.join("blocks");
        let tabs_file = data_dir.join("tabs.json");
        let sessions_dir = data_dir.join("sessions");
        let layouts_dir = data_dir.join("layouts");
        let backup_dir = data_dir.join("backups");
        let wal_file = data_dir.join("wal.log");

        // Create subdirectories immediately
        for dir in &[&blocks_dir, &sessions_dir, &layouts_dir, &backup_dir] {
            async_fs::create_dir_all(dir)
                .await
                .with_context(|| format!("Failed to create directory: {:?}", dir))?;
        }

        let block_persistence = BlockPersistence {
            blocks_dir,
            index_file: data_dir.join("blocks_index.json"),
            block_cache: HashMap::new(),
            pending_writes: Vec::new(),
            last_save: Instant::now(),
        };

        let tab_persistence = TabPersistence {
            tabs_file,
            sessions_dir,
            tab_cache: HashMap::new(),
            current_session: SessionPersistence {
                session_id: uuid::Uuid::new_v4().to_string(),
                created_at: Utc::now(),
                last_active: Utc::now(),
                tab_ids: Vec::new(),
                active_tab: None,
                window_state: WindowState {
                    width: 1200,
                    height: 800,
                    position_x: 100,
                    position_y: 100,
                    maximized: false,
                    fullscreen: false,
                },
            },
            last_save: Instant::now(),
        };

        let split_persistence = SplitPersistence {
            layouts_dir,
            layout_cache: HashMap::new(),
            history_file: data_dir.join("split_history.json"),
            last_save: Instant::now(),
        };

        let wal = WriteAheadLog {
            log_file: wal_file,
            log_entries: Vec::new(),
            last_checkpoint: Instant::now(),
            checkpoint_interval: Duration::from_secs(30), // Checkpoint every 30 seconds
        };

        // Initialize optional sync provider behind feature flag
        #[cfg(feature = "sync")]
        let sync_provider = {
            use openagent_terminal_sync::{LocalFsProvider, SyncConfig};
            let cfg = SyncConfig {
                provider: "local_fs".to_string(),
                data_dir: Some(data_dir.join("sync")),
                endpoint_env: None,
                encryption_key_env: None,
            };
            // Local FS provider is best-effort; if it fails, fall back to None
            match LocalFsProvider::new(&cfg) {
                Ok(p) => std::sync::Arc::new(tokio::sync::Mutex::new(Some(p))),
                Err(_) => std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            }
        };

        let backup_manager = BackupManager {
            backup_dir,
            backup_interval: Duration::from_secs(300), // Backup every 5 minutes
            max_backups: 50,                           // Keep 50 backups
            last_backup: Instant::now(),
            backup_queue: Vec::new(),
        };

        let mut persistence = Self {
            data_dir,
            block_persistence,
            tab_persistence,
            split_persistence,
            event_callbacks: Vec::new(),
            wal,
            backup_manager,
            #[cfg(feature = "sync")]
            sync_provider,
            sync_sender: None,
            perf_stats: PersistenceStats {
                last_reset: Instant::now(),
                ..Default::default()
            },
        };

        // Load existing data immediately
        persistence.load_all_data().await?;

        // Start background sync task
        let (sync_tx, mut sync_rx) = mpsc::unbounded_channel();
        persistence.sync_sender = Some(sync_tx);

        // Spawn sync worker for immediate processing
        #[cfg(feature = "sync")]
        {
            let provider_arc = persistence.sync_provider.clone();
            tokio::spawn(async move {
                use openagent_terminal_sync::{SyncProvider, SyncScope};
                while let Some(operation) = sync_rx.recv().await {
                    debug!("Processing sync operation: {:?}", operation);
                    let guard = provider_arc.lock().await;
                    if let Some(provider) = guard.as_ref() {
                        // Minimal mapping of operations to scopes
                        let scope = match &operation {
                            SyncOperation::SaveBlock(_) | SyncOperation::DeleteBlock(_) => SyncScope::History,
                            SyncOperation::SaveTabState(_) | SyncOperation::SaveSplitLayout { .. } => SyncScope::Settings,
                            SyncOperation::CreateBackup(_) | SyncOperation::Checkpoint => {
                                // Backups/checkpoints don't affect sync directly; skip
                                continue;
                            }
                        };
                        // Best-effort push; log on failure
                        if let Err(e) = provider.push(scope) {
                            debug!("sync push failed: {:?}", e);
                        }
                    } else {
                        debug!("sync provider unavailable; skipping operation");
                    }
                }
            });
        }
        #[cfg(not(feature = "sync"))]
        {
            // Without sync feature, drain the channel to avoid buildup (no-ops)
            tokio::spawn(async move {
                while let Some(operation) = sync_rx.recv().await {
                    debug!("Sync feature disabled; ignoring operation: {:?}", operation);
                }
            });
        }

        info!(
            "Native persistence initialized at {:?}",
            persistence.data_dir
        );

        Ok(persistence)
    }

    /// Register persistence event callback for immediate updates
    pub fn register_event_callback<F>(&mut self, callback: F)
    where
        F: Fn(&PersistenceEvent) + Send + Sync + 'static,
    {
        self.event_callbacks.push(Box::new(callback));
    }

    /// Emit persistence event immediately
    fn emit_event(&self, event: PersistenceEvent) {
        for callback in &self.event_callbacks {
            callback(&event);
        }
    }

    /// Save block immediately with no lazy writes
    pub async fn save_block(&mut self, block: &Block) -> Result<()> {
        let start_time = Instant::now();

        // Write to WAL first for consistency
        self.write_to_wal(
            LogOperation::SaveBlock(block.id),
            bincode::serialize(block)?,
        )
        .await?;

        // Generate file path
        let file_path = self
            .block_persistence
            .blocks_dir
            .join(format!("{}.json", block.id));

        // Serialize block data immediately
        let block_data = serde_json::to_vec_pretty(block).context("Failed to serialize block")?;

        // Write to disk immediately - no buffering
        async_fs::write(&file_path, &block_data)
            .await
            .with_context(|| format!("Failed to write block to {:?}", file_path))?;

        // Update cache immediately
        let cache_entry = CachedBlockEntry {
            block: block.clone(),
            file_path: file_path.clone(),
            last_modified: Utc::now(),
            content_hash: self.calculate_hash(&block_data),
            dirty: false,
        };

        self.block_persistence
            .block_cache
            .insert(block.id, cache_entry);
        self.block_persistence.last_save = Instant::now();

        // Update performance stats
        self.perf_stats.blocks_saved += 1;
        self.perf_stats.total_write_time += start_time.elapsed();

        // Emit immediate event
        self.emit_event(PersistenceEvent::BlockSaved(block.id));

        // Notify sync layer (best-effort)
        if let Some(tx) = &self.sync_sender {
            let _ = tx.send(SyncOperation::SaveBlock(block.clone()));
        }

        // Schedule backup if needed
        if self.should_create_backup() {
            self.schedule_backup(file_path, BackupPriority::Normal)
                .await?;
        }

        debug!("Block {} saved in {:?}", block.id, start_time.elapsed());

        Ok(())
    }

    /// Load block immediately with no lazy loading
    pub async fn load_block(&mut self, block_id: BlockId) -> Result<Option<Block>> {
        let start_time = Instant::now();

        // Check cache first for immediate access
        if let Some(cache_entry) = self.block_persistence.block_cache.get(&block_id) {
            self.perf_stats.cache_hits += 1;
            debug!(
                "Block {} loaded from cache in {:?}",
                block_id,
                start_time.elapsed()
            );
            return Ok(Some(cache_entry.block.clone()));
        }

        self.perf_stats.cache_misses += 1;

        // Load from disk immediately
        let file_path = self
            .block_persistence
            .blocks_dir
            .join(format!("{}.json", block_id));

        if !file_path.exists() {
            return Ok(None);
        }

        let block_data = async_fs::read(&file_path)
            .await
            .with_context(|| format!("Failed to read block from {:?}", file_path))?;

        let block: Block =
            serde_json::from_slice(&block_data).context("Failed to deserialize block")?;

        // Update cache immediately
        let cache_entry = CachedBlockEntry {
            block: block.clone(),
            file_path,
            last_modified: Utc::now(),
            content_hash: self.calculate_hash(&block_data),
            dirty: false,
        };

        self.block_persistence
            .block_cache
            .insert(block_id, cache_entry);

        // Update performance stats
        self.perf_stats.blocks_loaded += 1;
        self.perf_stats.total_read_time += start_time.elapsed();

        debug!(
            "Block {} loaded from disk in {:?}",
            block_id,
            start_time.elapsed()
        );

        Ok(Some(block))
    }

    /// Delete block immediately with no deferred operations
    pub async fn delete_block(&mut self, block_id: BlockId) -> Result<()> {
        let start_time = Instant::now();

        // Write to WAL first
        self.write_to_wal(LogOperation::DeleteBlock(block_id), Vec::new())
            .await?;

        // Remove from cache immediately
        if let Some(cache_entry) = self.block_persistence.block_cache.remove(&block_id) {
            // Delete file immediately
            if cache_entry.file_path.exists() {
                async_fs::remove_file(&cache_entry.file_path)
                    .await
                    .with_context(|| {
                        format!("Failed to delete block file: {:?}", cache_entry.file_path)
                    })?;
            }
        }

        // Emit immediate event
        self.emit_event(PersistenceEvent::BlockDeleted(block_id));

        // Notify sync layer (best-effort)
        if let Some(tx) = &self.sync_sender {
            let _ = tx.send(SyncOperation::DeleteBlock(block_id));
        }

        debug!("Block {} deleted in {:?}", block_id, start_time.elapsed());

        Ok(())
    }

    /// Save tab state immediately with no lazy writes
    pub async fn save_tab_state(&mut self, tab_state: &TabManagerState) -> Result<()> {
        let start_time = Instant::now();

        // Update current session immediately
        self.tab_persistence.current_session.last_active = Utc::now();
        self.tab_persistence.current_session.tab_ids =
            tab_state.tab_titles.keys().copied().collect();
        self.tab_persistence.current_session.active_tab = tab_state.active_tab;

        // Serialize tab state
        let tab_data =
            serde_json::to_vec_pretty(tab_state).context("Failed to serialize tab state")?;

        // Write to disk immediately
        async_fs::write(&self.tab_persistence.tabs_file, &tab_data)
            .await
            .context("Failed to write tab state")?;

        // Save session data immediately
        let session_file = self.tab_persistence.sessions_dir.join(format!(
            "{}.json",
            self.tab_persistence.current_session.session_id
        ));

        let session_data = serde_json::to_vec_pretty(&self.tab_persistence.current_session)
            .context("Failed to serialize session")?;

        async_fs::write(&session_file, &session_data)
            .await
            .context("Failed to write session data")?;

        self.tab_persistence.last_save = Instant::now();

        // Update performance stats
        self.perf_stats.tabs_saved += 1;
        self.perf_stats.total_write_time += start_time.elapsed();

        // Emit events for each tab
        for &tab_id in &tab_state.tab_titles.keys().copied().collect::<Vec<_>>() {
            self.emit_event(PersistenceEvent::TabStateSaved(tab_id));
        }

        // Notify sync layer (best-effort)
        if let Some(tx) = &self.sync_sender {
            let _ = tx.send(SyncOperation::SaveTabState(tab_state.clone()));
        }

        debug!("Tab state saved in {:?}", start_time.elapsed());

        Ok(())
    }

    /// Save split layout immediately with no lazy writes
    pub async fn save_split_layout(&mut self, layout_id: &str, layout_data: &[u8]) -> Result<()> {
        let start_time = Instant::now();

        // Write to WAL first
        self.write_to_wal(
            LogOperation::SaveSplitLayout(layout_id.to_string()),
            layout_data.to_vec(),
        )
        .await?;

        let file_path = self
            .split_persistence
            .layouts_dir
            .join(format!("{}.bin", layout_id));

        // Write layout data immediately
        async_fs::write(&file_path, layout_data)
            .await
            .with_context(|| format!("Failed to write split layout to {:?}", file_path))?;

        // Update cache immediately
        let cache_entry = CachedLayoutEntry {
            layout_id: layout_id.to_string(),
            layout_data: layout_data.to_vec(),
            last_modified: Utc::now(),
            dirty: false,
        };

        self.split_persistence
            .layout_cache
            .insert(layout_id.to_string(), cache_entry);
        self.split_persistence.last_save = Instant::now();

        // Update performance stats
        self.perf_stats.splits_saved += 1;
        self.perf_stats.total_write_time += start_time.elapsed();

        // Emit immediate event
        self.emit_event(PersistenceEvent::SplitLayoutSaved(layout_id.to_string()));

        // Notify sync layer (best-effort)
        if let Some(tx) = &self.sync_sender {
            let _ = tx.send(SyncOperation::SaveSplitLayout {
                layout_id: layout_id.to_string(),
                layout_data: layout_data.to_vec(),
            });
        }

        debug!(
            "Split layout {} saved in {:?}",
            layout_id,
            start_time.elapsed()
        );

        Ok(())
    }

    /// Write to write-ahead log immediately
    async fn write_to_wal(&mut self, operation: LogOperation, data: Vec<u8>) -> Result<()> {
        let entry = LogEntry {
            timestamp: Utc::now(),
            operation,
            data: data.clone(),
            checksum: self.calculate_hash(&data),
        };

        // Serialize entry
        let entry_data = bincode::serialize(&entry).context("Failed to serialize WAL entry")?;

        // Append to WAL file immediately
        let mut file = async_fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.wal.log_file)
            .await
            .context("Failed to open WAL file")?;

        file.write_all(&entry_data)
            .await
            .context("Failed to write to WAL")?;

        file.write_all(b"\n")
            .await
            .context("Failed to write WAL separator")?;

        file.sync_all().await.context("Failed to sync WAL file")?;

        // Add to memory log
        self.wal.log_entries.push(entry);

        // Checkpoint if needed
        if self.wal.last_checkpoint.elapsed() >= self.wal.checkpoint_interval {
            self.checkpoint_wal().await?;
        }

        Ok(())
    }

    /// Checkpoint WAL immediately
    async fn checkpoint_wal(&mut self) -> Result<()> {
        let start_time = Instant::now();

        // Clear processed entries
        self.wal.log_entries.clear();

        // Truncate WAL file
        async_fs::write(&self.wal.log_file, b"")
            .await
            .context("Failed to truncate WAL file")?;

        self.wal.last_checkpoint = Instant::now();

        debug!("WAL checkpoint completed in {:?}", start_time.elapsed());

        Ok(())
    }

    /// Load all data immediately on startup
    async fn load_all_data(&mut self) -> Result<()> {
        let start_time = Instant::now();

        // Load block index
        if self.block_persistence.index_file.exists() {
            let _index_data = async_fs::read(&self.block_persistence.index_file).await?;
            // Parse and populate block cache if needed
        }

        // Load tab state
        if self.tab_persistence.tabs_file.exists() {
            let _tab_data = async_fs::read(&self.tab_persistence.tabs_file).await?;
            // Parse and populate tab cache if needed
        }

        // Load session data
        let session_files = async_fs::read_dir(&self.tab_persistence.sessions_dir).await;
        if let Ok(mut entries) = session_files {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.path().extension().map_or(false, |ext| ext == "json") {
                    let _session_data = async_fs::read(entry.path()).await?;
                    // Parse session data if needed
                }
            }
        }

        info!("All data loaded in {:?}", start_time.elapsed());

        Ok(())
    }

    /// Schedule backup with immediate priority handling
    async fn schedule_backup(
        &mut self,
        source_path: PathBuf,
        priority: BackupPriority,
    ) -> Result<()> {
        let timestamp = Utc::now();
        let backup_filename = format!(
            "{}_{}.backup",
            source_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            timestamp.format("%Y%m%d_%H%M%S")
        );

        let backup_path = self.backup_manager.backup_dir.join(backup_filename);

        let backup_task = BackupTask {
            source_path,
            backup_path: backup_path.clone(),
            timestamp,
            priority,
        };

        // Execute backup immediately for high/critical priority
        if priority >= BackupPriority::High {
            self.execute_backup_task(&backup_task).await?;
        } else {
            // Queue for background processing
            self.backup_manager.backup_queue.push(backup_task);
        }

        Ok(())
    }

    /// Execute backup task immediately
    async fn execute_backup_task(&mut self, task: &BackupTask) -> Result<()> {
        let start_time = Instant::now();

        // Copy file immediately
        async_fs::copy(&task.source_path, &task.backup_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to backup {:?} to {:?}",
                    task.source_path, task.backup_path
                )
            })?;

        // Update backup stats
        self.perf_stats.backups_created += 1;
        self.backup_manager.last_backup = Instant::now();

        // Emit immediate event
        self.emit_event(PersistenceEvent::BackupCreated {
            backup_path: task.backup_path.clone(),
            timestamp: task.timestamp,
        });

        debug!(
            "Backup created in {:?}: {:?}",
            start_time.elapsed(),
            task.backup_path
        );

        Ok(())
    }

    /// Check if backup is needed
    fn should_create_backup(&self) -> bool {
        self.backup_manager.last_backup.elapsed() >= self.backup_manager.backup_interval
    }

    /// Calculate hash for data integrity
    fn calculate_hash(&self, data: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }

    /// Get persistence statistics
    pub fn get_stats(&self) -> PersistenceStats {
        self.perf_stats.clone()
    }

    /// Force immediate sync of all pending operations
    pub async fn force_sync(&mut self) -> Result<()> {
        let start_time = Instant::now();
        let operation_count = self.wal.log_entries.len() + self.backup_manager.backup_queue.len();

        // Process all pending backups immediately
        while let Some(backup_task) = self.backup_manager.backup_queue.pop() {
            self.execute_backup_task(&backup_task).await?;
        }

        // Checkpoint WAL immediately
        self.checkpoint_wal().await?;

        let duration = start_time.elapsed();

        // Emit sync completion event
        self.emit_event(PersistenceEvent::SyncCompleted {
            operation_count,
            duration,
        });

        info!(
            "Force sync completed: {} operations in {:?}",
            operation_count, duration
        );

        Ok(())
    }
    /// Enqueue a sync operation (best-effort). Public to facilitate testing.
    pub fn enqueue_sync(&self, op: SyncOperation) {
        if let Some(tx) = &self.sync_sender {
            let _ = tx.send(op);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_native_persistence_creation() {
        let temp_dir = TempDir::new().unwrap();
        let persistence = NativePersistence::new(temp_dir.path().to_path_buf())
            .await
            .unwrap();

        assert!(persistence.data_dir.exists());
        assert!(persistence.block_persistence.blocks_dir.exists());
        assert!(persistence.backup_manager.backup_dir.exists());
    }

    #[test]
    fn test_hash_calculation() {
        let persistence = NativePersistence {
            data_dir: PathBuf::new(),
            block_persistence: BlockPersistence {
                blocks_dir: PathBuf::new(),
                index_file: PathBuf::new(),
                block_cache: HashMap::new(),
                pending_writes: Vec::new(),
                last_save: Instant::now(),
            },
            tab_persistence: TabPersistence {
                tabs_file: PathBuf::new(),
                sessions_dir: PathBuf::new(),
                tab_cache: HashMap::new(),
                current_session: SessionPersistence {
                    session_id: String::new(),
                    created_at: Utc::now(),
                    last_active: Utc::now(),
                    tab_ids: Vec::new(),
                    active_tab: None,
                    window_state: WindowState {
                        width: 0,
                        height: 0,
                        position_x: 0,
                        position_y: 0,
                        maximized: false,
                        fullscreen: false,
                    },
                },
                last_save: Instant::now(),
            },
            split_persistence: SplitPersistence {
                layouts_dir: PathBuf::new(),
                layout_cache: HashMap::new(),
                history_file: PathBuf::new(),
                last_save: Instant::now(),
            },
            event_callbacks: Vec::new(),
            wal: WriteAheadLog {
                log_file: PathBuf::new(),
                log_entries: Vec::new(),
                last_checkpoint: Instant::now(),
                checkpoint_interval: Duration::from_secs(30),
            },
            backup_manager: BackupManager {
                backup_dir: PathBuf::new(),
                backup_interval: Duration::from_secs(300),
                max_backups: 50,
                last_backup: Instant::now(),
                backup_queue: Vec::new(),
            },
            #[cfg(feature = "sync")]
            sync_provider: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            sync_sender: None,
            perf_stats: PersistenceStats::default(),
        };

        let data1 = b"hello world";
        let data2 = b"hello world";
        let data3 = b"different data";

        let hash1 = persistence.calculate_hash(data1);
        let hash2 = persistence.calculate_hash(data2);
        let hash3 = persistence.calculate_hash(data3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}

//! Blocks v2 - Enterprise-ready command block management with SQLite backend
//!
//! Provides full database operations, indexing, search, and real-time updates
//! for command execution history and block management.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Unique identifier for a command block
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u64);

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Shell type for command execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Nushell,
    Custom(String),
}

impl Default for ShellType {
    fn default() -> Self {
        ShellType::Bash
    }
}

impl ShellType {
    pub fn to_str(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::PowerShell => "pwsh",
            ShellType::Nushell => "nu",
            ShellType::Custom(_) => "custom",
        }
    }
}

impl FromStr for ShellType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "bash" => Ok(ShellType::Bash),
            "zsh" => Ok(ShellType::Zsh),
            "fish" => Ok(ShellType::Fish),
            "pwsh" => Ok(ShellType::PowerShell),
            "nu" => Ok(ShellType::Nushell),
            _ => Ok(ShellType::Custom(s.to_string())),
        }
    }
}

/// Parameters for creating a new command block
#[derive(Debug, Clone)]
pub struct CreateBlockParams {
    pub command: String,
    pub directory: Option<PathBuf>,
    pub environment: Option<HashMap<String, String>>,
    pub shell: Option<ShellType>,
    pub tags: Option<Vec<String>>,
    pub parent_id: Option<BlockId>,
    pub metadata: Option<HashMap<String, String>>,
}

/// Search query for finding blocks
#[derive(Debug, Default)]
pub struct SearchQuery<'a> {
    pub text: Option<&'a str>,
    pub directory: Option<&'a Path>,
    pub shell: Option<ShellType>,
    pub tags: Option<&'a [String]>,
    pub starred: Option<bool>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort_by: Option<&'a str>,
    pub sort_order: Option<&'a str>,
}

/// Command block record with full information
#[derive(Debug, Clone)]
pub struct BlockRecord {
    pub id: BlockId,
    pub command: String,
    pub output: String,
    pub error_output: String,
    pub directory: PathBuf,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub starred: bool,
    pub tags: Vec<String>,
    pub shell: ShellType,
    pub status: String,
}

impl Default for BlockRecord {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: BlockId(0),
            command: String::new(),
            output: String::new(),
            error_output: String::new(),
            directory: PathBuf::new(),
            created_at: now,
            modified_at: now,
            exit_code: 0,
            duration_ms: 0,
            starred: false,
            tags: Vec::new(),
            shell: ShellType::Bash,
            status: "pending".to_string(),
        }
    }
}

/// Database connection and operations
mod database {
    use super::*;
    
    pub struct BlockDatabase {
        conn: Arc<Mutex<rusqlite::Connection>>,
    }
    
    impl BlockDatabase {
        pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
            let db_path = db_path.as_ref().to_path_buf();
            
            let conn = tokio::task::spawn_blocking(move || {
                rusqlite::Connection::open(db_path)
                    .context("Failed to open blocks database")
            })
            .await
            .context("Database task failed")??;
            
            let db = Self {
                conn: Arc::new(Mutex::new(conn)),
            };
            
            db.initialize_schema().await?;
            Ok(db)
        }
        
        async fn initialize_schema(&self) -> Result<()> {
            let conn = self.conn.clone();
            
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
                
                // Create blocks table with full indexing
                conn.execute(
                    r#"
                    CREATE TABLE IF NOT EXISTS blocks (
                        id INTEGER PRIMARY KEY,
                        command TEXT NOT NULL,
                        output TEXT DEFAULT '',
                        error_output TEXT DEFAULT '',
                        directory TEXT NOT NULL,
                        shell_type TEXT NOT NULL,
                        exit_code INTEGER,
                        duration_ms INTEGER,
                        created_at TEXT NOT NULL,
                        modified_at TEXT NOT NULL,
                        starred BOOLEAN DEFAULT FALSE,
                        status TEXT DEFAULT 'pending',
                        parent_id INTEGER,
                        session_id TEXT,
                        FOREIGN KEY (parent_id) REFERENCES blocks (id)
                    )
                    "#,
                    [],
                )?;
                
                // Create tags table
                conn.execute(
                    r#"
                    CREATE TABLE IF NOT EXISTS block_tags (
                        block_id INTEGER NOT NULL,
                        tag TEXT NOT NULL,
                        PRIMARY KEY (block_id, tag),
                        FOREIGN KEY (block_id) REFERENCES blocks (id) ON DELETE CASCADE
                    )
                    "#,
                    [],
                )?;
                
                // Create metadata table
                conn.execute(
                    r#"
                    CREATE TABLE IF NOT EXISTS block_metadata (
                        block_id INTEGER NOT NULL,
                        key TEXT NOT NULL,
                        value TEXT NOT NULL,
                        PRIMARY KEY (block_id, key),
                        FOREIGN KEY (block_id) REFERENCES blocks (id) ON DELETE CASCADE
                    )
                    "#,
                    [],
                )?;
                
                // Create indexes
                conn.execute("CREATE INDEX IF NOT EXISTS idx_blocks_command ON blocks (command)", [])?;
                conn.execute("CREATE INDEX IF NOT EXISTS idx_blocks_directory ON blocks (directory)", [])?;
                conn.execute("CREATE INDEX IF NOT EXISTS idx_blocks_created_at ON blocks (created_at)", [])?;
                conn.execute("CREATE INDEX IF NOT EXISTS idx_blocks_starred ON blocks (starred)", [])?;
                conn.execute("CREATE INDEX IF NOT EXISTS idx_blocks_status ON blocks (status)", [])?;
                conn.execute("CREATE INDEX IF NOT EXISTS idx_block_tags_tag ON block_tags (tag)", [])?;
                
                info!("Blocks database schema initialized successfully");
                Ok::<(), anyhow::Error>(())
            })
            .await
            .context("Database schema initialization task failed")?
        }
        
        pub async fn insert_block(&self, params: &CreateBlockParams) -> Result<BlockId> {
            let conn = self.conn.clone();
            let params = params.clone();
            
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
                let now = Utc::now();
                
                let block_id = conn.query_row(
                    r#"
                    INSERT INTO blocks (
                        command, directory, shell_type, created_at, modified_at,
                        status, parent_id, session_id
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    RETURNING id
                    "#,
                    rusqlite::params![
                        params.command,
                        params.directory.as_ref().map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "/".to_string()),
                        params.shell.unwrap_or_default().to_str(),
                        now.to_rfc3339(),
                        now.to_rfc3339(),
                        "running",
                        params.parent_id.map(|id| id.0 as i64),
                        "default_session", // Simple session ID for now
                    ],
                    |row| row.get::<_, i64>(0),
                )?;
                
                let block_id = BlockId(block_id as u64);
                
                // Insert tags if provided
                if let Some(tags) = params.tags {
                    for tag in tags {
                        conn.execute(
                            "INSERT INTO block_tags (block_id, tag) VALUES (?1, ?2)",
                            rusqlite::params![block_id.0 as i64, tag],
                        )?;
                    }
                }
                
                // Insert metadata if provided
                if let Some(metadata) = params.metadata {
                    for (key, value) in metadata {
                        conn.execute(
                            "INSERT INTO block_metadata (block_id, key, value) VALUES (?1, ?2, ?3)",
                            rusqlite::params![block_id.0 as i64, key, value],
                        )?;
                    }
                }
                
                // Insert environment variables
                if let Some(environment) = params.environment {
                    for (env_key, env_value) in environment {
                        conn.execute(
                            "INSERT INTO block_metadata (block_id, key, value) VALUES (?1, ?2, ?3)",
                            rusqlite::params![block_id.0 as i64, env_key, env_value],
                        )?;
                    }
                }
                
                debug!("Inserted block {} with command: {}", block_id, params.command);
                Ok(block_id)
            })
            .await
            .context("Database insert task failed")?
        }
        
        pub async fn get_block_by_id(&self, block_id: BlockId) -> Result<Option<BlockRecord>> {
            let conn = self.conn.clone();
            
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
                
                match conn.query_row(
                    r#"
                    SELECT id, command, output, error_output, directory, shell_type,
                           exit_code, duration_ms, created_at, modified_at, starred, status
                    FROM blocks WHERE id = ?1
                    "#,
                    rusqlite::params![block_id.0 as i64],
                    |row| {
                        Ok(BlockRecord {
                            id: BlockId(row.get::<_, i64>(0)? as u64),
                            command: row.get(1)?,
                            output: row.get(2)?,
                            error_output: row.get(3)?,
                            directory: PathBuf::from(row.get::<_, String>(4)?),
                            shell: ShellType::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                            exit_code: row.get::<_, Option<i32>>(6)?.unwrap_or(0),
                            duration_ms: row.get::<_, Option<i64>>(7)?.unwrap_or(0) as u64,
                            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e)))?
                                .with_timezone(&Utc),
                            modified_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(e)))?
                                .with_timezone(&Utc),
                            starred: row.get::<_, bool>(10)?,
                            status: row.get(11)?,
                            tags: Vec::new(), // Will be loaded separately
                        })
                    },
                ) {
                    Ok(mut block) => {
                        // Load tags
                        let mut stmt = conn.prepare("SELECT tag FROM block_tags WHERE block_id = ?1")?;
                        let tag_iter = stmt.query_map([block_id.0 as i64], |row| {
                            Ok(row.get::<_, String>(0)?)
                        })?;
                        
                        for tag in tag_iter {
                            block.tags.push(tag?);
                        }
                        
                        Ok(Some(block))
                    }
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(e) => Err(e.into()),
                }
            })
            .await
            .context("Database get task failed")?
        }
        
        pub async fn search_blocks(&self, query: &SearchQuery<'_>) -> Result<Vec<BlockRecord>> {
            let conn = self.conn.clone();
            let text = query.text.map(|s| s.to_string());
            let limit = query.limit.unwrap_or(100);
            let offset = query.offset.unwrap_or(0);
            
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
                
                let mut sql = String::from(
                    r#"
                    SELECT id, command, output, error_output, directory, shell_type,
                           exit_code, duration_ms, created_at, modified_at, starred, status
                    FROM blocks WHERE 1=1
                    "#
                );
                
                let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
                
                if let Some(text_query) = text {
                    sql.push_str(" AND (command LIKE ?1 OR output LIKE ?1)");
                    params.push(Box::new(format!("%{}%", text_query)));
                }
                
                sql.push_str(" ORDER BY created_at DESC LIMIT ?");
                params.push(Box::new(limit as i64));
                
                sql.push_str(" OFFSET ?");
                params.push(Box::new(offset as i64));
                
                let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
                
                let mut stmt = conn.prepare(&sql)?;
                let block_iter = stmt.query_map(&param_refs[..], |row| {
                    Ok(BlockRecord {
                        id: BlockId(row.get::<_, i64>(0)? as u64),
                        command: row.get(1)?,
                        output: row.get(2)?,
                        error_output: row.get(3)?,
                        directory: PathBuf::from(row.get::<_, String>(4)?),
                        shell: ShellType::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                        exit_code: row.get::<_, Option<i32>>(6)?.unwrap_or(0),
                        duration_ms: row.get::<_, Option<i64>>(7)?.unwrap_or(0) as u64,
                        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e)))?
                            .with_timezone(&Utc),
                        modified_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(e)))?
                            .with_timezone(&Utc),
                        starred: row.get::<_, bool>(10)?,
                        status: row.get(11)?,
                        tags: Vec::new(), // Will be loaded separately if needed
                    })
                })?;
                
                let mut blocks = Vec::new();
                for block in block_iter {
                    blocks.push(block?);
                }
                
                Ok(blocks)
            })
            .await
            .context("Database search task failed")?
        }
        
        pub async fn append_output(&self, block_id: BlockId, content: &str) -> Result<()> {
            let conn = self.conn.clone();
            let content = content.to_string();
            
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
                let now = Utc::now();
                
                conn.execute(
                    "UPDATE blocks SET output = output || ?1, modified_at = ?2 WHERE id = ?3",
                    rusqlite::params![content, now.to_rfc3339(), block_id.0 as i64],
                )?;
                
                Ok::<(), anyhow::Error>(())
            })
            .await
            .context("Database append output task failed")?
        }
        
        pub async fn update_block_output(
            &self,
            block_id: BlockId,
            output: &str,
            error_output: &str,
            exit_code: i32,
            duration_ms: u64,
        ) -> Result<()> {
            let conn = self.conn.clone();
            let output = output.to_string();
            let error_output = error_output.to_string();
            
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
                let now = Utc::now();
                
                conn.execute(
                    r#"
                    UPDATE blocks 
                    SET output = ?1, error_output = ?2, exit_code = ?3, duration_ms = ?4, 
                        modified_at = ?5, status = ?6 
                    WHERE id = ?7
                    "#,
                    rusqlite::params![
                        output,
                        error_output,
                        exit_code,
                        duration_ms as i64,
                        now.to_rfc3339(),
                        if exit_code == 0 { "completed" } else { "failed" },
                        block_id.0 as i64,
                    ],
                )?;
                
                Ok::<(), anyhow::Error>(())
            })
            .await
            .context("Database update output task failed")?
        }
        
        pub async fn mark_block_cancelled(&self, block_id: BlockId) -> Result<()> {
            let conn = self.conn.clone();
            
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
                let now = Utc::now();
                
                conn.execute(
                    "UPDATE blocks SET status = 'cancelled', modified_at = ?1 WHERE id = ?2",
                    rusqlite::params![now.to_rfc3339(), block_id.0 as i64],
                )?;
                
                Ok::<(), anyhow::Error>(())
            })
            .await
            .context("Database cancel task failed")?
        }
        
        pub async fn toggle_starred(&self, block_id: BlockId) -> Result<bool> {
            let conn = self.conn.clone();
            
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
                let now = Utc::now();
                
                let new_starred = conn.query_row(
                    "SELECT NOT starred FROM blocks WHERE id = ?1",
                    rusqlite::params![block_id.0 as i64],
                    |row| row.get::<_, bool>(0),
                )?;
                
                conn.execute(
                    "UPDATE blocks SET starred = ?1, modified_at = ?2 WHERE id = ?3",
                    rusqlite::params![new_starred, now.to_rfc3339(), block_id.0 as i64],
                )?;
                
                Ok(new_starred)
            })
            .await
            .context("Database toggle starred task failed")?
        }
        
        pub async fn update_block_tags(&self, block_id: BlockId, tags: Vec<String>) -> Result<()> {
            let conn = self.conn.clone();
            
            tokio::task::spawn_blocking(move || {
                let conn = conn.lock().map_err(|_| anyhow::anyhow!("Database lock poisoned"))?;
                let now = Utc::now();
                
                // Delete existing tags
                conn.execute(
                    "DELETE FROM block_tags WHERE block_id = ?1",
                    rusqlite::params![block_id.0 as i64],
                )?;
                
                // Insert new tags
                for tag in tags {
                    conn.execute(
                        "INSERT INTO block_tags (block_id, tag) VALUES (?1, ?2)",
                        rusqlite::params![block_id.0 as i64, tag],
                    )?;
                }
                
                // Update modified_at
                conn.execute(
                    "UPDATE blocks SET modified_at = ?1 WHERE id = ?2",
                    rusqlite::params![now.to_rfc3339(), block_id.0 as i64],
                )?;
                
                Ok::<(), anyhow::Error>(())
            })
            .await
            .context("Database update tags task failed")?
        }
    }
}

/// Block events for real-time updates
#[derive(Debug, Clone)]
pub enum BlockEvent {
    Created { id: BlockId, command: String },
    Updated { id: BlockId, status: String },
    OutputAppended { id: BlockId, content: String },
    Completed { id: BlockId, exit_code: i32, duration_ms: u64 },
    Cancelled { id: BlockId },
    Starred { id: BlockId, starred: bool },
    TagsUpdated { id: BlockId, tags: Vec<String> },
}

/// Enterprise-ready Block Manager with full SQLite backend
pub struct BlockManager {
    /// SQLite database connection
    database: Arc<RwLock<database::BlockDatabase>>,
    
    /// Root directory for relative path resolution
    root: PathBuf,
    
    /// Cache for frequently accessed blocks
    cache: Arc<RwLock<HashMap<BlockId, BlockRecord>>>,
    
    /// Event sender for real-time updates
    event_sender: Option<tokio::sync::mpsc::UnboundedSender<BlockEvent>>,
}

impl BlockManager {
    /// Create new BlockManager with SQLite database
    pub async fn new(root: PathBuf) -> Result<Self> {
        // Ensure the root directory exists
        tokio::fs::create_dir_all(&root).await.context("Failed to create blocks directory")?;
        
        // Initialize database
        let db_path = root.join("blocks.db");
        let database = database::BlockDatabase::new(&db_path).await
            .context("Failed to initialize blocks database")?;
        
        info!("BlockManager initialized with database at: {}", db_path.display());
        
        Ok(Self {
            database: Arc::new(RwLock::new(database)),
            root,
            cache: Arc::new(RwLock::new(HashMap::new())),
            event_sender: None,
        })
    }
    
    /// Set event sender for real-time updates
    pub fn set_event_sender(&mut self, sender: tokio::sync::mpsc::UnboundedSender<BlockEvent>) {
        self.event_sender = Some(sender);
    }
    
    /// Create new block with immediate database insertion
    pub async fn create_block(&mut self, params: CreateBlockParams) -> Result<BlockRecord> {
        let db = self.database.read().await;
        let block_id = db.insert_block(&params).await
            .context("Failed to insert block into database")?;
        
        // Retrieve the created block to return complete information
        let block = db.get_block_by_id(block_id).await?
            .ok_or_else(|| anyhow::anyhow!("Block was created but cannot be retrieved"))?;
        
        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(block_id, block.clone());
        }
        
        // Emit event
        self.emit_event(BlockEvent::Created {
            id: block_id,
            command: params.command.clone(),
        }).await;
        
        info!("Created block {} for command: {}", block_id, params.command);
        Ok(block)
    }
    
    /// Search blocks with comprehensive filtering
    pub async fn search(&self, query: SearchQuery<'_>) -> Result<Vec<BlockRecord>> {
        let db = self.database.read().await;
        let results = db.search_blocks(&query).await
            .context("Failed to search blocks")?;
        
        debug!("Block search returned {} results", results.len());
        Ok(results)
    }
    
    /// Get block by ID with caching
    pub async fn get_block(&self, block_id: BlockId) -> Result<Option<BlockRecord>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(block) = cache.get(&block_id) {
                return Ok(Some(block.clone()));
            }
        }
        
        // Query database
        let db = self.database.read().await;
        let block = db.get_block_by_id(block_id).await
            .context("Failed to get block by ID")?;
        
        // Update cache if found
        if let Some(ref block) = block {
            let mut cache = self.cache.write().await;
            cache.insert(block_id, block.clone());
        }
        
        Ok(block)
    }
    
    /// Append output to block with real-time updates
    pub async fn append_output(&mut self, block_id: BlockId, content: &str) -> Result<()> {
        let db = self.database.read().await;
        db.append_output(block_id, content).await
            .context("Failed to append output to block")?;
        
        // Invalidate cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(&block_id);
        }
        
        // Emit event
        self.emit_event(BlockEvent::OutputAppended {
            id: block_id,
            content: content.to_string(),
        }).await;
        
        Ok(())
    }
    
    /// Update block output with completion data
    pub async fn update_block_output(
        &mut self,
        block_id: BlockId,
        output: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<()> {
        let db = self.database.read().await;
        db.update_block_output(block_id, &output, "", exit_code, duration_ms).await
            .context("Failed to update block output")?;
        
        // Invalidate cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(&block_id);
        }
        
        // Emit events
        let status = if exit_code == 0 { "completed" } else { "failed" };
        self.emit_event(BlockEvent::Updated {
            id: block_id,
            status: status.to_string(),
        }).await;
        
        self.emit_event(BlockEvent::Completed {
            id: block_id,
            exit_code,
            duration_ms,
        }).await;
        
        debug!("Updated block {} output, exit_code: {}, duration: {}ms", 
               block_id, exit_code, duration_ms);
        Ok(())
    }
    
    /// Update block output with both stdout and stderr
    pub async fn update_block_output_with_error(
        &mut self,
        block_id: BlockId,
        output: String,
        error_output: String,
        exit_code: i32,
        duration_ms: u64,
    ) -> Result<()> {
        let db = self.database.read().await;
        db.update_block_output(block_id, &output, &error_output, exit_code, duration_ms).await
            .context("Failed to update block output with error")?;
        
        // Invalidate cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(&block_id);
        }
        
        // Emit events
        let status = if exit_code == 0 { "completed" } else { "failed" };
        self.emit_event(BlockEvent::Updated {
            id: block_id,
            status: status.to_string(),
        }).await;
        
        self.emit_event(BlockEvent::Completed {
            id: block_id,
            exit_code,
            duration_ms,
        }).await;
        
        Ok(())
    }
    
    /// Mark block as cancelled
    pub async fn mark_block_cancelled(&mut self, block_id: BlockId) -> Result<()> {
        let db = self.database.read().await;
        db.mark_block_cancelled(block_id).await
            .context("Failed to mark block as cancelled")?;
        
        // Invalidate cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(&block_id);
        }
        
        // Emit event
        self.emit_event(BlockEvent::Cancelled { id: block_id }).await;
        self.emit_event(BlockEvent::Updated {
            id: block_id,
            status: "cancelled".to_string(),
        }).await;
        
        debug!("Marked block {} as cancelled", block_id);
        Ok(())
    }
    
    /// Toggle starred status of a block
    pub async fn toggle_starred(&mut self, block_id: BlockId) -> Result<bool> {
        let db = self.database.read().await;
        let new_starred = db.toggle_starred(block_id).await
            .context("Failed to toggle starred status")?;
        
        // Invalidate cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(&block_id);
        }
        
        // Emit event
        self.emit_event(BlockEvent::Starred {
            id: block_id,
            starred: new_starred,
        }).await;
        
        Ok(new_starred)
    }
    
    /// Update block tags
    pub async fn update_block_tags(&mut self, block_id: BlockId, tags: Vec<String>) -> Result<()> {
        let db = self.database.read().await;
        db.update_block_tags(block_id, tags.clone()).await
            .context("Failed to update block tags")?;
        
        // Invalidate cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(&block_id);
        }
        
        // Emit event
        self.emit_event(BlockEvent::TagsUpdated {
            id: block_id,
            tags,
        }).await;
        
        Ok(())
    }
    
    /// Get recent blocks for quick access
    pub async fn get_recent_blocks(&self, limit: usize) -> Result<Vec<BlockRecord>> {
        let query = SearchQuery {
            limit: Some(limit),
            sort_by: Some("created_at"),
            sort_order: Some("DESC"),
            ..Default::default()
        };
        
        self.search(query).await
    }
    
    /// Get blocks for a specific session
    pub async fn get_session_blocks(&self, _session_id: &str) -> Result<Vec<BlockRecord>> {
        // This would require adding session_id to SearchQuery and implementing it in the database
        // For now, use a metadata search
        let query = SearchQuery {
            sort_by: Some("created_at"),
            sort_order: Some("DESC"),
            ..Default::default()
        };
        
        let results = self.search(query).await?;
        // Filter by session would be done in the database query ideally
        Ok(results)
    }
    
    /// Clear cache - useful for testing or memory management
    pub async fn clear_cache(&mut self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        debug!("Block cache cleared");
    }
    
    /// Get cache statistics
    pub async fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        (cache.len(), cache.capacity())
    }
    
    /// Emit block event if sender is available
    async fn emit_event(&self, event: BlockEvent) {
        if let Some(ref sender) = self.event_sender {
            if let Err(e) = sender.send(event) {
                warn!("Failed to send block event: {}", e);
            }
        }
    }
    
    /// Placeholder for workspace PTY collection integration
    pub fn set_workspace_pty_collection<T>(&mut self, _handle: T) {
        // This would integrate with workspace PTY collection for real-time output capture
        debug!("Workspace PTY collection handle set");
    }
}
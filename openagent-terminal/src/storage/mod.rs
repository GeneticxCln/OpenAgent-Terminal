// Lightweight storage module exposing the Blocks storage wrapper implemented over blocks_v2.
// The previous SQLx-powered storage manager is intentionally not compiled in this build.

pub mod blocks;
pub mod migrations;
pub mod plugins;

use anyhow::Result;
use std::path::Path;

/// Storage error types
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] anyhow::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Migration error: {0}")]
    Migration(String),
    #[error("Not found: {0}")]
    NotFound(String),
}

/// Result type for storage operations
pub type StorageResult<T> = std::result::Result<T, StorageError>;

/// Main storage abstraction for OpenAgent Terminal
#[derive(Debug)]
pub struct Storage {
    db_path: std::path::PathBuf,
}

impl Storage {
    /// Create a new Storage instance
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        Ok(Self { db_path })
    }
    
    /// Initialize storage (run migrations, etc.)
    pub async fn initialize(&self) -> Result<()> {
        // Initialize database schema if needed
        Ok(())
    }
    
    /// Get the database path
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
    
    /// Close storage connections
    pub async fn close(&self) -> Result<()> {
        Ok(())
    }
    
    /// Perform storage cleanup operations
    pub async fn cleanup(&self) -> Result<()> {
        Ok(())
    }
    
    /// Get storage statistics
    pub async fn stats(&self) -> Result<StorageStats> {
        Ok(StorageStats::default())
    }
}

/// Storage statistics
#[derive(Debug, Default)]
pub struct StorageStats {
    pub total_blocks: u64,
    pub total_sessions: u64,
    pub db_size_bytes: u64,
    pub last_cleanup: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            db_path: std::env::temp_dir().join("openagent-terminal.db"),
        }
    }
}

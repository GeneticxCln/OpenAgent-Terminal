use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Error as SqlxError, Pool, Sqlite};
use std::path::Path;
use thiserror::Error;

pub mod blocks;
pub mod migrations;

/// Storage error types
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] SqlxError),
    #[error("Migration error: {0}")]
    Migration(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Storage result type
pub type StorageResult<T> = Result<T, StorageError>;

/// Main storage manager for persisting terminal data
pub struct Storage {
    pub pool: Pool<Sqlite>,
}

impl Storage {
    /// Create a new storage instance with SQLite database at the specified path
    pub async fn new<P: AsRef<Path>>(db_path: P) -> StorageResult<Self> {
        let db_path = db_path.as_ref();

        // Create parent directories if they don't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        let storage = Self { pool };

        // Run migrations
        storage.migrate().await?;

        Ok(storage)
    }

    /// Run database migrations
    async fn migrate(&self) -> StorageResult<()> {
        migrations::run_migrations(&self.pool).await
    }

    /// Get block storage interface
    pub fn blocks(&self) -> blocks::BlockStorage {
        blocks::BlockStorage::new(self.pool.clone())
    }
}

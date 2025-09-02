// Block storage implementation using SQLite

#![allow(dead_code)]

use super::{Block, BlockId, ExecutionStatus};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{sqlite::{SqlitePoolOptions, SqliteConnectOptions}, SqlitePool};
use std::path::Path;
use std::fs::File;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, info};

/// Block storage using SQLite
pub struct BlockStorage {
    pool: SqlitePool,
}

impl BlockStorage {
    /// Create new block storage
    pub async fn new(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;

        let db_path = data_dir.join("blocks.db");
        if !db_path.exists() {
            File::create(&db_path)?;
        }
        let db_url = format!("sqlite://{}", db_path.display());

        let connect_options = SqliteConnectOptions::from_str(&db_url)?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await
            .context("Failed to connect to database")?;

        // Create tables
        Self::initialize_schema(&pool).await?;

        Ok(Self { pool })
    }

    /// Initialize database schema
    async fn initialize_schema(pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS blocks (
                id TEXT PRIMARY KEY,
                command TEXT NOT NULL,
                output TEXT NOT NULL,
                directory TEXT NOT NULL,
                environment TEXT NOT NULL,
                shell TEXT NOT NULL,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL,
                tags TEXT NOT NULL,
                starred INTEGER NOT NULL DEFAULT 0,
                parent_id TEXT,
                children TEXT NOT NULL,
                metadata TEXT NOT NULL,
                status TEXT NOT NULL,
                exit_code INTEGER,
                duration_ms INTEGER
            );
            
            CREATE INDEX IF NOT EXISTS idx_blocks_created_at ON blocks(created_at);
            CREATE INDEX IF NOT EXISTS idx_blocks_starred ON blocks(starred);
            CREATE INDEX IF NOT EXISTS idx_blocks_parent_id ON blocks(parent_id);
            CREATE INDEX IF NOT EXISTS idx_blocks_status ON blocks(status);
            "#,
        )
        .execute(pool)
        .await?;

        // Create full-text search table
        sqlx::query(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS blocks_fts USING fts5(
                id UNINDEXED,
                command,
                output,
                tags,
                content=blocks,
                content_rowid=rowid
            );
            
            -- Maintain FTS index with external content table semantics
            CREATE TRIGGER IF NOT EXISTS blocks_ai AFTER INSERT ON blocks BEGIN
                INSERT INTO blocks_fts(rowid, id, command, output, tags)
                VALUES (new.rowid, new.id, new.command, new.output, new.tags);
            END;
            
            CREATE TRIGGER IF NOT EXISTS blocks_au AFTER UPDATE ON blocks BEGIN
                INSERT INTO blocks_fts(blocks_fts, rowid) VALUES('delete', old.rowid);
                INSERT INTO blocks_fts(rowid, id, command, output, tags)
                VALUES (new.rowid, new.id, new.command, new.output, new.tags);
            END;
            
            CREATE TRIGGER IF NOT EXISTS blocks_ad AFTER DELETE ON blocks BEGIN
                INSERT INTO blocks_fts(blocks_fts, rowid) VALUES('delete', old.rowid);
            END;
            "#,
        )
        .execute(pool)
        .await?;

        info!("Block storage initialized");
        Ok(())
    }

    /// Insert a new block
    pub async fn insert(&self, block: &Arc<Block>) -> Result<()> {
        let environment_json = serde_json::to_string(&block.environment)?;
        let tags_json = serde_json::to_string(&block.tags)?;
        let children_json = serde_json::to_string(&block.children)?;
        let metadata_json = serde_json::to_string(&block.metadata)?;

        sqlx::query(
            r#"
            INSERT INTO blocks (
                id, command, output, directory, environment, shell,
                created_at, modified_at, tags, starred, parent_id,
                children, metadata, status, exit_code, duration_ms
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(block.id.to_string())
        .bind(&block.command)
        .bind(&block.output)
        .bind(block.directory.to_string_lossy())
        .bind(environment_json)
        .bind(block.shell.to_str())
        .bind(block.created_at.to_rfc3339())
        .bind(block.modified_at.to_rfc3339())
        .bind(tags_json)
        .bind(block.starred as i32)
        .bind(block.parent_id.map(|id| id.to_string()))
        .bind(children_json)
        .bind(metadata_json)
        .bind(format!("{:?}", block.status))
        .bind(block.exit_code)
        .bind(block.duration_ms.map(|d| d as i64))
        .execute(&self.pool)
        .await?;

        debug!("Inserted block {}", block.id.to_string());
        Ok(())
    }

    /// Update an existing block
    pub async fn update(&self, block: &Block) -> Result<()> {
        let environment_json = serde_json::to_string(&block.environment)?;
        let tags_json = serde_json::to_string(&block.tags)?;
        let children_json = serde_json::to_string(&block.children)?;
        let metadata_json = serde_json::to_string(&block.metadata)?;

        sqlx::query(
            r#"
            UPDATE blocks SET
                command = ?, output = ?, directory = ?, environment = ?,
                shell = ?, modified_at = ?, tags = ?, starred = ?,
                parent_id = ?, children = ?, metadata = ?, status = ?,
                exit_code = ?, duration_ms = ?
            WHERE id = ?
            "#,
        )
        .bind(&block.command)
        .bind(&block.output)
        .bind(block.directory.to_string_lossy())
        .bind(environment_json)
        .bind(block.shell.to_str())
        .bind(block.modified_at.to_rfc3339())
        .bind(tags_json)
        .bind(block.starred as i32)
        .bind(block.parent_id.map(|id| id.to_string()))
        .bind(children_json)
        .bind(metadata_json)
        .bind(format!("{:?}", block.status))
        .bind(block.exit_code)
        .bind(block.duration_ms.map(|d| d as i64))
        .bind(block.id.to_string())
        .execute(&self.pool)
        .await?;

        debug!("Updated block {}", block.id.to_string());
        Ok(())
    }

    /// Get a block by ID
    pub async fn get(&self, id: BlockId) -> Result<Arc<Block>> {
        let row = sqlx::query_as::<_, BlockRow>(
            r#"
            SELECT * FROM blocks WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(Arc::new(row.into_block()?))
    }

    /// Get all blocks
    pub async fn get_all_blocks(&self) -> Result<Vec<Block>> {
        let rows = sqlx::query_as::<_, BlockRow>(
            r#"
            SELECT * FROM blocks
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|row| row.into_block()).collect()
    }

    /// Get all starred blocks
    pub async fn get_starred(&self) -> Result<Vec<Arc<Block>>> {
        let rows = sqlx::query_as::<_, BlockRow>(
            r#"
            SELECT * FROM blocks WHERE starred = 1
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|row| row.into_block().map(Arc::new)).collect()
    }

    /// Get blocks by tag
    pub async fn get_by_tag(&self, tag: &str) -> Result<Vec<Arc<Block>>> {
        let pattern = format!("%\"{}\"%", tag);

        let rows = sqlx::query_as::<_, BlockRow>(
            r#"
            SELECT * FROM blocks WHERE tags LIKE ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(pattern)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|row| row.into_block().map(Arc::new)).collect()
    }

    /// Basic search with FTS and simple filters
    pub async fn search(&self, query: &super::SearchQuery) -> Result<Vec<Arc<Block>>> {
        // Build dynamic SQL
        let mut sql = String::from(
            "SELECT b.* FROM blocks b\n"
        );

        let mut where_clauses: Vec<String> = Vec::new();
        let mut binds: Vec<(usize, String)> = Vec::new();
        let mut bind_ix = 1;

        // If text provided, join FTS table
        if let Some(text) = &query.text {
            sql.push_str("JOIN blocks_fts f ON f.rowid = b.rowid\n");
            where_clauses.push("blocks_fts MATCH ?".into());
            binds.push((bind_ix, text.clone()));
            bind_ix += 1;
        }

        if query.starred_only {
            where_clauses.push("b.starred = 1".into());
        }

        if let Some(status) = query.status {
            where_clauses.push("b.status = ?".into());
            binds.push((bind_ix, format!("{:?}", status)));
            bind_ix += 1;
        }

        if let Some(dir) = &query.directory {
            where_clauses.push("b.directory LIKE ?".into());
            binds.push((bind_ix, format!("%{}%", dir.display())));
            bind_ix += 1;
        }

        if let Some(tags) = &query.tags {
            for tag in tags {
                where_clauses.push("b.tags LIKE ?".into());
                binds.push((bind_ix, format!("%\"{}\"%", tag)));
                bind_ix += 1;
            }
        }

        if let Some(from) = query.date_from {
            where_clauses.push("b.created_at >= ?".into());
            binds.push((bind_ix, from.to_rfc3339()));
            bind_ix += 1;
        }
        if let Some(to) = query.date_to {
            where_clauses.push("b.created_at <= ?".into());
            binds.push((bind_ix, to.to_rfc3339()));
            bind_ix += 1;
        }

        if !where_clauses.is_empty() {
            sql.push_str("WHERE ");
            sql.push_str(&where_clauses.join(" AND "));
            sql.push('\n');
        }

        sql.push_str("ORDER BY b.created_at DESC\n");
        let limit = query.limit.unwrap_or(100).to_string();
        sql.push_str("LIMIT ");
        sql.push_str(&limit);

        // Build the query dynamically
        let mut q = sqlx::query_as::<_, BlockRow>(&sql);
        for (_idx, val) in binds {
            q = q.bind(val);
        }

        let rows = q.fetch_all(&self.pool).await?;
        let mut out = Vec::new();
        for row in rows {
            out.push(Arc::new(row.into_block()?));
        }
        Ok(out)
    }

    /// Delete blocks before a certain date
    pub async fn delete_before(&self, cutoff: DateTime<Utc>) -> Result<usize> {
        let result = sqlx::query(
            r#"
            DELETE FROM blocks WHERE created_at < ?
            "#,
        )
        .bind(cutoff.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as usize)
    }
}

/// Database row representation
#[derive(sqlx::FromRow)]
struct BlockRow {
    id: String,
    command: String,
    output: String,
    directory: String,
    environment: String,
    shell: String,
    created_at: String,
    modified_at: String,
    tags: String,
    starred: i32,
    parent_id: Option<String>,
    children: String,
    metadata: String,
    status: String,
    exit_code: Option<i32>,
    duration_ms: Option<i64>,
}

impl BlockRow {
    fn into_block(self) -> Result<Block> {
        use super::ShellType;
        use std::path::PathBuf;

        Ok(Block {
            id: BlockId::from_string(&self.id)?,
            command: self.command,
            output: self.output,
            directory: PathBuf::from(self.directory),
            environment: serde_json::from_str(&self.environment)?,
            shell: ShellType::from_str(&self.shell),
            created_at: DateTime::parse_from_rfc3339(&self.created_at)?.with_timezone(&Utc),
            modified_at: DateTime::parse_from_rfc3339(&self.modified_at)?.with_timezone(&Utc),
            tags: serde_json::from_str(&self.tags)?,
            starred: self.starred != 0,
            parent_id: self.parent_id.and_then(|id| BlockId::from_string(&id).ok()),
            children: serde_json::from_str(&self.children)?,
            metadata: serde_json::from_str(&self.metadata)?,
            status: parse_execution_status(&self.status),
            exit_code: self.exit_code,
            duration_ms: self.duration_ms.map(|d| d as u64),
        })
    }
}

fn parse_execution_status(s: &str) -> ExecutionStatus {
    match s {
        "Running" => ExecutionStatus::Running,
        "Success" => ExecutionStatus::Success,
        "Failed" => ExecutionStatus::Failed,
        "Cancelled" => ExecutionStatus::Cancelled,
        "Timeout" => ExecutionStatus::Timeout,
        _ => ExecutionStatus::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks_v2::{Block, BlockId, BlockMetadata, ExecutionStatus, ShellType};
    use std::collections::{HashMap, HashSet};
    use tempfile::TempDir;

    #[tokio::test]
    async fn search_basic() {
        let dir = TempDir::new().unwrap();
        let storage = BlockStorage::new(dir.path()).await.unwrap();

        // Insert sample block
        let now = Utc::now();
        let block = Block {
            id: BlockId::new(),
            command: "echo test command".to_string(),
            output: "output line".to_string(),
            directory: dir.path().to_path_buf(),
            environment: HashMap::new(),
            shell: ShellType::Bash,
            created_at: now,
            modified_at: now,
            tags: HashSet::from(["tag1".to_string()]),
            starred: true,
            parent_id: None,
            children: Vec::new(),
            metadata: BlockMetadata::default(),
            status: ExecutionStatus::Success,
            exit_code: Some(0),
            duration_ms: Some(10),
        };
        storage.insert(&Arc::new(block)).await.unwrap();

        // Search text
        let mut q = crate::blocks_v2::SearchQuery::default();
        q.text = Some("test".to_string());
        q.limit = Some(10);
        let results = storage.search(&q).await.unwrap();
        assert!(!results.is_empty());

        // Search by tag
        let mut q2 = crate::blocks_v2::SearchQuery::default();
        q2.tags = Some(vec!["tag1".to_string()]);
        let r2 = storage.search(&q2).await.unwrap();
        assert!(!r2.is_empty());

        // Search starred only
        let mut q3 = crate::blocks_v2::SearchQuery::default();
        q3.starred_only = true;
        let r3 = storage.search(&q3).await.unwrap();
        assert!(!r3.is_empty());
    }
}

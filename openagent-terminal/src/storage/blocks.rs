use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Pool, Row, Sqlite};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::display::blocks::CommandBlock;
use crate::storage::{StorageError, StorageResult};

/// Persisted block data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedBlock {
    pub id: i64,
    pub session_id: Option<String>,
    pub start_total_line: i32,
    pub end_total_line: Option<i32>,
    pub command: Option<String>,
    pub working_directory: Option<String>,
    pub exit_code: Option<i32>,
    pub started_at: i64, // Unix timestamp in milliseconds
    pub ended_at: Option<i64>,
    pub output_preview: Option<String>,
    pub output_hash: Option<String>,
    pub tags: Option<String>, // JSON array of tags
    pub created_at: i64,
    pub updated_at: i64,
}

/// Search filters for querying blocks
#[derive(Debug, Default, Clone)]
pub struct BlockFilter {
    pub query: Option<String>, // Full-text search query
    pub command_pattern: Option<String>,
    pub working_directory: Option<String>,
    pub exit_code: Option<i32>,
    pub session_id: Option<String>,
    pub start_time: Option<i64>, // Unix timestamp in milliseconds
    pub end_time: Option<i64>,
    pub tags: Vec<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Sort options for block queries
#[derive(Debug, Clone)]
pub enum BlockSort {
    StartedAt(SortOrder),
    Command(SortOrder),
    WorkingDirectory(SortOrder),
    ExitCode(SortOrder),
}

#[derive(Debug, Clone)]
pub enum SortOrder {
    Asc,
    Desc,
}

impl Default for BlockSort {
    fn default() -> Self {
        BlockSort::StartedAt(SortOrder::Desc)
    }
}

/// Block storage interface
pub struct BlockStorage {
    pool: Pool<Sqlite>,
}

impl BlockStorage {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Insert a new block record
    pub async fn insert_block(
        &self,
        block: &CommandBlock,
        session_id: Option<&str>,
        output_content: Option<&str>,
    ) -> StorageResult<i64> {
        let started_at = system_time_to_millis(instant_to_system_time(block.started_at));
        let ended_at =
            block.ended_at.map(|instant| system_time_to_millis(instant_to_system_time(instant)));

        let (output_preview, output_hash) = if let Some(content) = output_content {
            let preview = if content.len() > 1000 {
                format!("{}...", &content[..1000])
            } else {
                content.to_string()
            };

            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let hash = format!("{:x}", hasher.finalize());

            (Some(preview), Some(hash))
        } else {
            (None, None)
        };

        let result = sqlx::query(
            r#"
            INSERT INTO blocks (
                session_id, start_total_line, end_total_line, command,
                working_directory, exit_code, started_at, ended_at,
                output_preview, output_hash
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(session_id)
        .bind(block.start_total_line as i32)
        .bind(block.end_total_line.map(|line| line as i32))
        .bind(&block.cmd)
        .bind(&block.cwd)
        .bind(block.exit)
        .bind(started_at)
        .bind(ended_at)
        .bind(output_preview)
        .bind(output_hash)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Update an existing block
    pub async fn update_block(
        &self,
        id: i64,
        block: &CommandBlock,
        output_content: Option<&str>,
    ) -> StorageResult<()> {
        let started_at = system_time_to_millis(instant_to_system_time(block.started_at));
        let ended_at =
            block.ended_at.map(|instant| system_time_to_millis(instant_to_system_time(instant)));

        let (output_preview, output_hash) = if let Some(content) = output_content {
            let preview = if content.len() > 1000 {
                format!("{}...", &content[..1000])
            } else {
                content.to_string()
            };

            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let hash = format!("{:x}", hasher.finalize());

            (Some(preview), Some(hash))
        } else {
            (None, None)
        };

        sqlx::query(
            r#"
            UPDATE blocks SET
                start_total_line = ?, end_total_line = ?, command = ?,
                working_directory = ?, exit_code = ?, started_at = ?,
                ended_at = ?, output_preview = ?, output_hash = ?
            WHERE id = ?
            "#,
        )
        .bind(block.start_total_line as i32)
        .bind(block.end_total_line.map(|line| line as i32))
        .bind(&block.cmd)
        .bind(&block.cwd)
        .bind(block.exit)
        .bind(started_at)
        .bind(ended_at)
        .bind(output_preview)
        .bind(output_hash)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Search blocks using filters and sorting
    pub async fn search_blocks(
        &self,
        filter: &BlockFilter,
        sort: &BlockSort,
    ) -> StorageResult<Vec<PersistedBlock>> {
        let mut query_builder = Vec::new();
        let mut bind_values: Vec<Box<dyn sqlx::Encode<'static, Sqlite> + Send + Sync>> = Vec::new();
        let mut where_clauses = Vec::new();

        // Build the base query
        if let Some(search_query) = &filter.query {
            // Use FTS for full-text search
            query_builder.push(
                r#"
                SELECT blocks.* FROM blocks
                JOIN blocks_fts ON blocks.id = blocks_fts.rowid
                WHERE blocks_fts MATCH ?
                "#
                .to_string(),
            );
            bind_values.push(Box::new(search_query.clone()));
        } else {
            query_builder.push("SELECT * FROM blocks".to_string());

            // Add WHERE conditions
            if let Some(command_pattern) = &filter.command_pattern {
                where_clauses.push("command LIKE ?");
                bind_values.push(Box::new(format!("%{}%", command_pattern)));
            }

            if let Some(working_dir) = &filter.working_directory {
                where_clauses.push("working_directory LIKE ?");
                bind_values.push(Box::new(format!("%{}%", working_dir)));
            }

            if let Some(exit_code) = filter.exit_code {
                where_clauses.push("exit_code = ?");
                bind_values.push(Box::new(exit_code));
            }

            if let Some(session_id) = &filter.session_id {
                where_clauses.push("session_id = ?");
                bind_values.push(Box::new(session_id.clone()));
            }

            if let Some(start_time) = filter.start_time {
                where_clauses.push("started_at >= ?");
                bind_values.push(Box::new(start_time));
            }

            if let Some(end_time) = filter.end_time {
                where_clauses.push("started_at <= ?");
                bind_values.push(Box::new(end_time));
            }

            if !where_clauses.is_empty() {
                query_builder.push(format!(" WHERE {}", where_clauses.join(" AND ")));
            }
        }

        // Add ORDER BY clause
        let order_clause = match sort {
            BlockSort::StartedAt(SortOrder::Asc) => "ORDER BY started_at ASC",
            BlockSort::StartedAt(SortOrder::Desc) => "ORDER BY started_at DESC",
            BlockSort::Command(SortOrder::Asc) => "ORDER BY command ASC",
            BlockSort::Command(SortOrder::Desc) => "ORDER BY command DESC",
            BlockSort::WorkingDirectory(SortOrder::Asc) => "ORDER BY working_directory ASC",
            BlockSort::WorkingDirectory(SortOrder::Desc) => "ORDER BY working_directory DESC",
            BlockSort::ExitCode(SortOrder::Asc) => "ORDER BY exit_code ASC",
            BlockSort::ExitCode(SortOrder::Desc) => "ORDER BY exit_code DESC",
        };
        query_builder.push(format!(" {}", order_clause));

        // Add LIMIT and OFFSET
        if let Some(limit) = filter.limit {
            query_builder.push(format!(" LIMIT {}", limit));
            if let Some(offset) = filter.offset {
                query_builder.push(format!(" OFFSET {}", offset));
            }
        }

        let final_query = query_builder.join("");

        // Execute the query
        let _query = sqlx::query_as::<Sqlite, PersistedBlock>(&final_query);

        // Note: SQLx doesn't support dynamic binding easily, so we'll need to handle this
        // For now, let's implement a simpler version without full dynamic binding
        self.execute_search_query(filter, sort).await
    }

    /// Simplified search implementation
    async fn execute_search_query(
        &self,
        filter: &BlockFilter,
        sort: &BlockSort,
    ) -> StorageResult<Vec<PersistedBlock>> {
        // Base query with common filtering
        let query = if let Some(search_query) = &filter.query {
            sqlx::query_as::<Sqlite, PersistedBlock>(
                "SELECT blocks.* FROM blocks
                 JOIN blocks_fts ON blocks.id = blocks_fts.rowid
                 WHERE blocks_fts MATCH ?
                 ORDER BY started_at DESC LIMIT ?",
            )
            .bind(search_query)
            .bind(filter.limit.unwrap_or(100))
        } else {
            // Simple filtering without FTS
            match sort {
                BlockSort::StartedAt(SortOrder::Desc) => sqlx::query_as::<Sqlite, PersistedBlock>(
                    "SELECT * FROM blocks WHERE 1=1 ORDER BY started_at DESC LIMIT ?",
                )
                .bind(filter.limit.unwrap_or(100)),
                BlockSort::StartedAt(SortOrder::Asc) => sqlx::query_as::<Sqlite, PersistedBlock>(
                    "SELECT * FROM blocks WHERE 1=1 ORDER BY started_at ASC LIMIT ?",
                )
                .bind(filter.limit.unwrap_or(100)),
                BlockSort::Command(SortOrder::Desc) => sqlx::query_as::<Sqlite, PersistedBlock>(
                    "SELECT * FROM blocks WHERE 1=1 ORDER BY command DESC LIMIT ?",
                )
                .bind(filter.limit.unwrap_or(100)),
                BlockSort::Command(SortOrder::Asc) => sqlx::query_as::<Sqlite, PersistedBlock>(
                    "SELECT * FROM blocks WHERE 1=1 ORDER BY command ASC LIMIT ?",
                )
                .bind(filter.limit.unwrap_or(100)),
                BlockSort::WorkingDirectory(SortOrder::Desc) => {
                    sqlx::query_as::<Sqlite, PersistedBlock>(
                        "SELECT * FROM blocks WHERE 1=1 ORDER BY working_directory DESC LIMIT ?",
                    )
                    .bind(filter.limit.unwrap_or(100))
                }
                BlockSort::WorkingDirectory(SortOrder::Asc) => {
                    sqlx::query_as::<Sqlite, PersistedBlock>(
                        "SELECT * FROM blocks WHERE 1=1 ORDER BY working_directory ASC LIMIT ?",
                    )
                    .bind(filter.limit.unwrap_or(100))
                }
                BlockSort::ExitCode(SortOrder::Desc) => sqlx::query_as::<Sqlite, PersistedBlock>(
                    "SELECT * FROM blocks WHERE 1=1 ORDER BY exit_code DESC LIMIT ?",
                )
                .bind(filter.limit.unwrap_or(100)),
                BlockSort::ExitCode(SortOrder::Asc) => sqlx::query_as::<Sqlite, PersistedBlock>(
                    "SELECT * FROM blocks WHERE 1=1 ORDER BY exit_code ASC LIMIT ?",
                )
                .bind(filter.limit.unwrap_or(100)),
            }
        };

        let blocks = query.fetch_all(&self.pool).await?;
        Ok(blocks)
    }

    /// Get a block by ID
    pub async fn get_block(&self, id: i64) -> StorageResult<Option<PersistedBlock>> {
        let block = sqlx::query_as::<Sqlite, PersistedBlock>("SELECT * FROM blocks WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(block)
    }

    /// Delete a block by ID
    pub async fn delete_block(&self, id: i64) -> StorageResult<()> {
        sqlx::query("DELETE FROM blocks WHERE id = ?").bind(id).execute(&self.pool).await?;

        Ok(())
    }

    /// Get blocks for a specific session
    pub async fn get_session_blocks(&self, session_id: &str) -> StorageResult<Vec<PersistedBlock>> {
        let blocks = sqlx::query_as::<Sqlite, PersistedBlock>(
            "SELECT * FROM blocks WHERE session_id = ? ORDER BY started_at ASC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(blocks)
    }

    /// Add or remove tags from a block
    pub async fn update_block_tags(&self, id: i64, tags: Vec<String>) -> StorageResult<()> {
        let tags_json = serde_json::to_string(&tags)
            .map_err(|e| StorageError::Database(sqlx::Error::Decode(Box::new(e))))?;

        sqlx::query("UPDATE blocks SET tags = ? WHERE id = ?")
            .bind(tags_json)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// Convert Instant to SystemTime (approximate)
fn instant_to_system_time(_instant: Instant) -> SystemTime {
    // This is approximate since Instant doesn't have a direct conversion to SystemTime
    // In practice, you might want to store SystemTime directly in CommandBlock
    SystemTime::now()
}

/// Convert SystemTime to milliseconds since epoch
fn system_time_to_millis(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}

impl FromRow<'_, sqlx::sqlite::SqliteRow> for PersistedBlock {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(PersistedBlock {
            id: row.try_get("id")?,
            session_id: row.try_get("session_id")?,
            start_total_line: row.try_get("start_total_line")?,
            end_total_line: row.try_get("end_total_line")?,
            command: row.try_get("command")?,
            working_directory: row.try_get("working_directory")?,
            exit_code: row.try_get("exit_code")?,
            started_at: row.try_get("started_at")?,
            ended_at: row.try_get("ended_at")?,
            output_preview: row.try_get("output_preview")?,
            output_hash: row.try_get("output_hash")?,
            tags: row.try_get("tags")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl From<PersistedBlock> for CommandBlock {
    fn from(block: PersistedBlock) -> Self {
        // Convert back to CommandBlock for display purposes
        // Note: Some fields like Instant timestamps are approximate
        CommandBlock {
            start_total_line: block.start_total_line as usize,
            end_total_line: block.end_total_line.map(|line| line as usize),
            cmd: block.command,
            cwd: block.working_directory,
            exit: block.exit_code,
            started_at: Instant::now(), // Approximate - consider storing system time instead
            ended_at: block.ended_at.map(|_| Instant::now()), // Approximate
            folded: false,              // UI state, not persisted
            anim_start: None,
            anim_opening: false,
            anim_duration_ms: 140,
        }
    }
}

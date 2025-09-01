// Blocks 2.0 Database Module - SQLite storage for terminal blocks

use sqlx::{SqlitePool, Row, migrate::MigrateDatabase};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::Path;
use anyhow::Result;
use tokio::sync::RwLock;
use std::sync::Arc;

/// Block structure representing a terminal command and its output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    pub command: String,
    pub output: String,
    pub exit_code: i32,
    pub directory: String,
    pub environment: HashMap<String, String>,
    pub shell: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub duration_ms: i64,
    pub tags: Vec<String>,
    pub starred: bool,
    pub metadata: BlockMetadata,
}

/// Additional metadata for blocks
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlockMetadata {
    pub hostname: String,
    pub username: String,
    pub session_id: String,
    pub terminal_size: (u16, u16),
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
    pub custom: HashMap<String, serde_json::Value>,
}

/// Search parameters for block queries
#[derive(Debug, Clone, Default)]
pub struct BlockQuery {
    pub text: Option<String>,
    pub command: Option<String>,
    pub tags: Vec<String>,
    pub directory: Option<String>,
    pub starred_only: bool,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Export format options
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Json,
    Markdown,
    ShellScript,
    Html,
}

/// Main database manager for Blocks 2.0
pub struct BlocksDatabase {
    pool: Arc<SqlitePool>,
    cache: Arc<RwLock<lru::LruCache<String, Block>>>,
}

impl BlocksDatabase {
    /// Create or open a blocks database
    pub async fn new(db_path: &Path) -> Result<Self> {
        let db_url = format!("sqlite://{}", db_path.display());
        
        // Create database if it doesn't exist
        if !sqlx::Sqlite::database_exists(&db_url).await? {
            sqlx::Sqlite::create_database(&db_url).await?;
        }
        
        // Connect to database
        let pool = SqlitePool::connect(&db_url).await?;
        
        // Run migrations
        Self::run_migrations(&pool).await?;
        
        // Initialize cache
        let cache = Arc::new(RwLock::new(
            lru::LruCache::new(std::num::NonZeroUsize::new(100).unwrap())
        ));
        
        Ok(Self {
            pool: Arc::new(pool),
            cache,
        })
    }
    
    /// Run database migrations
    async fn run_migrations(pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS blocks (
                id TEXT PRIMARY KEY NOT NULL,
                command TEXT NOT NULL,
                output TEXT NOT NULL,
                exit_code INTEGER NOT NULL,
                directory TEXT NOT NULL,
                environment TEXT NOT NULL,  -- JSON
                shell TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                tags TEXT NOT NULL,         -- JSON array
                starred INTEGER NOT NULL DEFAULT 0,
                metadata TEXT NOT NULL      -- JSON
            );
            
            CREATE INDEX IF NOT EXISTS idx_blocks_created_at ON blocks(created_at);
            CREATE INDEX IF NOT EXISTS idx_blocks_directory ON blocks(directory);
            CREATE INDEX IF NOT EXISTS idx_blocks_starred ON blocks(starred);
            CREATE INDEX IF NOT EXISTS idx_blocks_exit_code ON blocks(exit_code);
            
            -- Full-text search
            CREATE VIRTUAL TABLE IF NOT EXISTS blocks_fts USING fts5(
                id UNINDEXED,
                command,
                output,
                tags,
                content=blocks,
                content_rowid=rowid
            );
            
            -- Triggers to keep FTS index updated
            CREATE TRIGGER IF NOT EXISTS blocks_ai AFTER INSERT ON blocks BEGIN
                INSERT INTO blocks_fts(id, command, output, tags)
                VALUES (new.id, new.command, new.output, new.tags);
            END;
            
            CREATE TRIGGER IF NOT EXISTS blocks_ad AFTER DELETE ON blocks BEGIN
                DELETE FROM blocks_fts WHERE id = old.id;
            END;
            
            CREATE TRIGGER IF NOT EXISTS blocks_au AFTER UPDATE ON blocks BEGIN
                UPDATE blocks_fts 
                SET command = new.command, output = new.output, tags = new.tags
                WHERE id = new.id;
            END;
            
            -- Tags table for autocomplete
            CREATE TABLE IF NOT EXISTS tags (
                name TEXT PRIMARY KEY NOT NULL,
                usage_count INTEGER NOT NULL DEFAULT 1,
                last_used TEXT NOT NULL
            );
            
            -- Block groups/collections
            CREATE TABLE IF NOT EXISTS collections (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS collection_blocks (
                collection_id TEXT NOT NULL,
                block_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                PRIMARY KEY (collection_id, block_id),
                FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
                FOREIGN KEY (block_id) REFERENCES blocks(id) ON DELETE CASCADE
            );
            "#
        )
        .execute(pool)
        .await?;
        
        Ok(())
    }
    
    /// Insert a new block
    pub async fn insert_block(&self, block: Block) -> Result<()> {
        let env_json = serde_json::to_string(&block.environment)?;
        let tags_json = serde_json::to_string(&block.tags)?;
        let metadata_json = serde_json::to_string(&block.metadata)?;
        
        sqlx::query(
            r#"
            INSERT INTO blocks (
                id, command, output, exit_code, directory, environment,
                shell, created_at, updated_at, duration_ms, tags, starred, metadata
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&block.id)
        .bind(&block.command)
        .bind(&block.output)
        .bind(block.exit_code)
        .bind(&block.directory)
        .bind(env_json)
        .bind(&block.shell)
        .bind(block.created_at.to_rfc3339())
        .bind(block.updated_at.to_rfc3339())
        .bind(block.duration_ms)
        .bind(tags_json)
        .bind(block.starred as i32)
        .bind(metadata_json)
        .execute(&*self.pool)
        .await?;
        
        // Update tags table
        for tag in &block.tags {
            self.update_tag_usage(tag).await?;
        }
        
        // Add to cache
        self.cache.write().await.put(block.id.clone(), block);
        
        Ok(())
    }
    
    /// Get a block by ID
    pub async fn get_block(&self, id: &str) -> Result<Option<Block>> {
        // Check cache first
        if let Some(block) = self.cache.read().await.peek(id) {
            return Ok(Some(block.clone()));
        }
        
        let row = sqlx::query(
            "SELECT * FROM blocks WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await?;
        
        if let Some(row) = row {
            let block = self.row_to_block(row)?;
            
            // Update cache
            self.cache.write().await.put(block.id.clone(), block.clone());
            
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }
    
    /// Search blocks with various filters
    pub async fn search_blocks(&self, query: BlockQuery) -> Result<Vec<Block>> {
        let mut sql = String::from("SELECT * FROM blocks WHERE 1=1");
        let mut bindings = Vec::new();
        
        // Full-text search
        if let Some(text) = &query.text {
            sql.push_str(" AND id IN (SELECT id FROM blocks_fts WHERE blocks_fts MATCH ?)");
            bindings.push(text.clone());
        }
        
        // Command filter
        if let Some(command) = &query.command {
            sql.push_str(" AND command LIKE ?");
            bindings.push(format!("%{}%", command));
        }
        
        // Tags filter
        if !query.tags.is_empty() {
            let tags_json = serde_json::to_string(&query.tags)?;
            sql.push_str(" AND tags LIKE ?");
            bindings.push(format!("%{}%", tags_json));
        }
        
        // Directory filter
        if let Some(directory) = &query.directory {
            sql.push_str(" AND directory = ?");
            bindings.push(directory.clone());
        }
        
        // Starred filter
        if query.starred_only {
            sql.push_str(" AND starred = 1");
        }
        
        // Date range filters
        if let Some(date_from) = &query.date_from {
            sql.push_str(" AND created_at >= ?");
            bindings.push(date_from.to_rfc3339());
        }
        
        if let Some(date_to) = &query.date_to {
            sql.push_str(" AND created_at <= ?");
            bindings.push(date_to.to_rfc3339());
        }
        
        // Exit code filter
        if let Some(exit_code) = query.exit_code {
            sql.push_str(" AND exit_code = ?");
            bindings.push(exit_code.to_string());
        }
        
        // Order and pagination
        sql.push_str(" ORDER BY created_at DESC");
        
        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        
        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }
        
        // Build and execute query
        let mut query_builder = sqlx::query(&sql);
        for binding in bindings {
            query_builder = query_builder.bind(binding);
        }
        
        let rows = query_builder.fetch_all(&*self.pool).await?;
        
        let mut blocks = Vec::new();
        for row in rows {
            blocks.push(self.row_to_block(row)?);
        }
        
        Ok(blocks)
    }
    
    /// Update a block
    pub async fn update_block(&self, block: Block) -> Result<()> {
        let env_json = serde_json::to_string(&block.environment)?;
        let tags_json = serde_json::to_string(&block.tags)?;
        let metadata_json = serde_json::to_string(&block.metadata)?;
        
        sqlx::query(
            r#"
            UPDATE blocks SET
                command = ?, output = ?, exit_code = ?, directory = ?,
                environment = ?, shell = ?, updated_at = ?, duration_ms = ?,
                tags = ?, starred = ?, metadata = ?
            WHERE id = ?
            "#
        )
        .bind(&block.command)
        .bind(&block.output)
        .bind(block.exit_code)
        .bind(&block.directory)
        .bind(env_json)
        .bind(&block.shell)
        .bind(block.updated_at.to_rfc3339())
        .bind(block.duration_ms)
        .bind(tags_json)
        .bind(block.starred as i32)
        .bind(metadata_json)
        .bind(&block.id)
        .execute(&*self.pool)
        .await?;
        
        // Update cache
        self.cache.write().await.put(block.id.clone(), block);
        
        Ok(())
    }
    
    /// Delete a block
    pub async fn delete_block(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM blocks WHERE id = ?")
            .bind(id)
            .execute(&*self.pool)
            .await?;
        
        // Remove from cache
        self.cache.write().await.pop(id);
        
        Ok(())
    }
    
    /// Star/unstar a block
    pub async fn toggle_star(&self, id: &str) -> Result<bool> {
        let current = sqlx::query("SELECT starred FROM blocks WHERE id = ?")
            .bind(id)
            .fetch_one(&*self.pool)
            .await?
            .get::<i32, _>(0);
        
        let new_starred = if current == 0 { 1 } else { 0 };
        
        sqlx::query("UPDATE blocks SET starred = ? WHERE id = ?")
            .bind(new_starred)
            .bind(id)
            .execute(&*self.pool)
            .await?;
        
        Ok(new_starred == 1)
    }
    
    /// Add tags to a block
    pub async fn add_tags(&self, id: &str, tags: Vec<String>) -> Result<()> {
        if let Some(mut block) = self.get_block(id).await? {
            for tag in tags {
                if !block.tags.contains(&tag) {
                    block.tags.push(tag.clone());
                    self.update_tag_usage(&tag).await?;
                }
            }
            
            block.updated_at = Utc::now();
            self.update_block(block).await?;
        }
        
        Ok(())
    }
    
    /// Remove tags from a block
    pub async fn remove_tags(&self, id: &str, tags: Vec<String>) -> Result<()> {
        if let Some(mut block) = self.get_block(id).await? {
            block.tags.retain(|t| !tags.contains(t));
            block.updated_at = Utc::now();
            self.update_block(block).await?;
        }
        
        Ok(())
    }
    
    /// Get all tags with usage counts
    pub async fn get_all_tags(&self) -> Result<Vec<(String, i32)>> {
        let rows = sqlx::query("SELECT name, usage_count FROM tags ORDER BY usage_count DESC")
            .fetch_all(&*self.pool)
            .await?;
        
        let mut tags = Vec::new();
        for row in rows {
            tags.push((
                row.get::<String, _>(0),
                row.get::<i32, _>(1),
            ));
        }
        
        Ok(tags)
    }
    
    /// Export blocks in various formats
    pub async fn export_blocks(&self, blocks: Vec<Block>, format: ExportFormat) -> Result<String> {
        match format {
            ExportFormat::Json => {
                Ok(serde_json::to_string_pretty(&blocks)?)
            }
            
            ExportFormat::Markdown => {
                let mut md = String::from("# Terminal Blocks Export\n\n");
                
                for block in blocks {
                    md.push_str(&format!("## Block: {}\n\n", block.id));
                    md.push_str(&format!("**Created:** {}\n", block.created_at));
                    md.push_str(&format!("**Directory:** `{}`\n", block.directory));
                    md.push_str(&format!("**Shell:** {}\n", block.shell));
                    
                    if !block.tags.is_empty() {
                        md.push_str(&format!("**Tags:** {}\n", block.tags.join(", ")));
                    }
                    
                    md.push_str("\n### Command\n```bash\n");
                    md.push_str(&block.command);
                    md.push_str("\n```\n\n");
                    
                    md.push_str("### Output\n```\n");
                    md.push_str(&block.output);
                    md.push_str("\n```\n\n");
                    
                    md.push_str(&format!("**Exit Code:** {}\n", block.exit_code));
                    md.push_str(&format!("**Duration:** {}ms\n\n", block.duration_ms));
                    md.push_str("---\n\n");
                }
                
                Ok(md)
            }
            
            ExportFormat::ShellScript => {
                let mut script = String::from("#!/bin/bash\n");
                script.push_str("# Terminal Blocks Export - Shell Script\n\n");
                
                for block in blocks {
                    script.push_str(&format!("# Block: {}\n", block.id));
                    script.push_str(&format!("# Created: {}\n", block.created_at));
                    script.push_str(&format!("# Directory: {}\n", block.directory));
                    
                    if !block.tags.is_empty() {
                        script.push_str(&format!("# Tags: {}\n", block.tags.join(", ")));
                    }
                    
                    script.push_str(&format!("cd '{}'\n", block.directory));
                    
                    // Export environment variables
                    for (key, value) in &block.environment {
                        script.push_str(&format!("export {}='{}'\n", key, value));
                    }
                    
                    script.push_str(&block.command);
                    script.push_str("\n\n");
                }
                
                Ok(script)
            }
            
            ExportFormat::Html => {
                let mut html = String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>Terminal Blocks Export</title>
    <style>
        body { font-family: monospace; background: #1e1e1e; color: #d4d4d4; }
        .block { margin: 20px; padding: 15px; background: #2d2d2d; border-radius: 5px; }
        .command { background: #1e1e1e; padding: 10px; border-left: 3px solid #007acc; }
        .output { background: #1e1e1e; padding: 10px; margin-top: 10px; }
        .metadata { color: #808080; font-size: 0.9em; }
        .tag { background: #007acc; color: white; padding: 2px 5px; border-radius: 3px; margin: 0 2px; }
        .success { color: #4ec9b0; }
        .error { color: #f48771; }
    </style>
</head>
<body>
    <h1>Terminal Blocks Export</h1>
"#);
                
                for block in blocks {
                    html.push_str("<div class='block'>");
                    html.push_str(&format!("<div class='metadata'>"));
                    html.push_str(&format!("ID: {} | ", block.id));
                    html.push_str(&format!("Created: {} | ", block.created_at));
                    html.push_str(&format!("Directory: {} | ", block.directory));
                    html.push_str(&format!("Duration: {}ms", block.duration_ms));
                    
                    if !block.tags.is_empty() {
                        html.push_str(" | Tags: ");
                        for tag in &block.tags {
                            html.push_str(&format!("<span class='tag'>{}</span>", tag));
                        }
                    }
                    
                    html.push_str("</div>");
                    
                    html.push_str("<div class='command'><pre>");
                    html.push_str(&html_escape(&block.command));
                    html.push_str("</pre></div>");
                    
                    html.push_str("<div class='output'><pre>");
                    html.push_str(&html_escape(&block.output));
                    html.push_str("</pre></div>");
                    
                    let status_class = if block.exit_code == 0 { "success" } else { "error" };
                    html.push_str(&format!("<div class='metadata {}'>Exit Code: {}</div>", status_class, block.exit_code));
                    
                    html.push_str("</div>");
                }
                
                html.push_str("</body></html>");
                Ok(html)
            }
        }
    }
    
    /// Import blocks from JSON
    pub async fn import_blocks(&self, json: &str) -> Result<usize> {
        let blocks: Vec<Block> = serde_json::from_str(json)?;
        let count = blocks.len();
        
        for block in blocks {
            self.insert_block(block).await?;
        }
        
        Ok(count)
    }
    
    /// Update tag usage count
    async fn update_tag_usage(&self, tag: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO tags (name, usage_count, last_used)
            VALUES (?, 1, ?)
            ON CONFLICT(name) DO UPDATE SET
                usage_count = usage_count + 1,
                last_used = excluded.last_used
            "#
        )
        .bind(tag)
        .bind(Utc::now().to_rfc3339())
        .execute(&*self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Convert database row to Block
    fn row_to_block(&self, row: sqlx::sqlite::SqliteRow) -> Result<Block> {
        Ok(Block {
            id: row.get("id"),
            command: row.get("command"),
            output: row.get("output"),
            exit_code: row.get("exit_code"),
            directory: row.get("directory"),
            environment: serde_json::from_str(&row.get::<String, _>("environment"))?,
            shell: row.get("shell"),
            created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?.into(),
            updated_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))?.into(),
            duration_ms: row.get("duration_ms"),
            tags: serde_json::from_str(&row.get::<String, _>("tags"))?,
            starred: row.get::<i32, _>("starred") == 1,
            metadata: serde_json::from_str(&row.get::<String, _>("metadata"))?,
        })
    }
}

/// HTML escape utility
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_block_crud() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        
        let db = BlocksDatabase::new(&db_path).await.unwrap();
        
        // Create a test block
        let block = Block {
            id: "test-123".to_string(),
            command: "echo hello".to_string(),
            output: "hello\n".to_string(),
            exit_code: 0,
            directory: "/home/user".to_string(),
            environment: HashMap::new(),
            shell: "bash".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            duration_ms: 100,
            tags: vec!["test".to_string()],
            starred: false,
            metadata: BlockMetadata::default(),
        };
        
        // Insert
        db.insert_block(block.clone()).await.unwrap();
        
        // Read
        let retrieved = db.get_block("test-123").await.unwrap().unwrap();
        assert_eq!(retrieved.command, "echo hello");
        
        // Update
        let mut updated = retrieved.clone();
        updated.starred = true;
        db.update_block(updated).await.unwrap();
        
        // Verify update
        let verified = db.get_block("test-123").await.unwrap().unwrap();
        assert!(verified.starred);
        
        // Delete
        db.delete_block("test-123").await.unwrap();
        assert!(db.get_block("test-123").await.unwrap().is_none());
    }
    
    #[tokio::test]
    async fn test_search() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        
        let db = BlocksDatabase::new(&db_path).await.unwrap();
        
        // Insert test blocks
        for i in 0..5 {
            let block = Block {
                id: format!("block-{}", i),
                command: format!("echo test{}", i),
                output: format!("test{}\n", i),
                exit_code: 0,
                directory: "/home/user".to_string(),
                environment: HashMap::new(),
                shell: "bash".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                duration_ms: 100,
                tags: vec!["test".to_string()],
                starred: i % 2 == 0,
                metadata: BlockMetadata::default(),
            };
            
            db.insert_block(block).await.unwrap();
        }
        
        // Search starred blocks
        let query = BlockQuery {
            starred_only: true,
            ..Default::default()
        };
        
        let results = db.search_blocks(query).await.unwrap();
        assert_eq!(results.len(), 3); // blocks 0, 2, 4
    }
}

use crate::storage::{StorageError, StorageResult};
use sqlx::{Pool, Row, Sqlite};

/// Database schema version
#[allow(dead_code)]
const CURRENT_VERSION: i32 = 2;

/// Migration definition
#[allow(dead_code)]
struct Migration {
    version: i32,
    name: &'static str,
    up_sql: &'static str,
}

/// All database migrations in order
#[allow(dead_code)]
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_blocks_schema",
        up_sql: r#"
            -- Table for storing terminal command blocks
            CREATE TABLE IF NOT EXISTS blocks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT,
                start_total_line INTEGER NOT NULL,
                end_total_line INTEGER,
                command TEXT,
                working_directory TEXT,
                exit_code INTEGER,
                started_at INTEGER NOT NULL, -- Unix timestamp in milliseconds
                ended_at INTEGER,
                output_preview TEXT, -- First ~1000 chars of output for search
                output_hash TEXT, -- Hash of full output for deduplication
                tags TEXT, -- JSON array of user tags
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
            );

            -- Indexes for efficient querying
            CREATE INDEX IF NOT EXISTS idx_blocks_session ON blocks(session_id);
            CREATE INDEX IF NOT EXISTS idx_blocks_started_at ON blocks(started_at);
            CREATE INDEX IF NOT EXISTS idx_blocks_command ON blocks(command);
            CREATE INDEX IF NOT EXISTS idx_blocks_exit_code ON blocks(exit_code);
            CREATE INDEX IF NOT EXISTS idx_blocks_working_directory ON blocks(working_directory);
            CREATE INDEX IF NOT EXISTS idx_blocks_output_hash ON blocks(output_hash);

            -- Full-text search for commands and output
            CREATE VIRTUAL TABLE IF NOT EXISTS blocks_fts USING fts5(
                command,
                working_directory,
                output_preview,
                content='blocks',
                content_rowid='id'
            );

            -- Triggers to keep FTS table in sync
            CREATE TRIGGER IF NOT EXISTS blocks_fts_insert AFTER INSERT ON blocks BEGIN
                INSERT INTO blocks_fts(rowid, command, working_directory, output_preview)
                VALUES (new.id, new.command, new.working_directory, new.output_preview);
            END;

            CREATE TRIGGER IF NOT EXISTS blocks_fts_delete AFTER DELETE ON blocks BEGIN
                INSERT INTO blocks_fts(blocks_fts, rowid, command, working_directory, output_preview)
                VALUES ('delete', old.id, old.command, old.working_directory, old.output_preview);
            END;

            CREATE TRIGGER IF NOT EXISTS blocks_fts_update AFTER UPDATE ON blocks BEGIN
                INSERT INTO blocks_fts(blocks_fts, rowid, command, working_directory, output_preview)
                VALUES ('delete', old.id, old.command, old.working_directory, old.output_preview);
                INSERT INTO blocks_fts(rowid, command, working_directory, output_preview)
                VALUES (new.id, new.command, new.working_directory, new.output_preview);
            END;

            -- Update timestamps trigger
            CREATE TRIGGER IF NOT EXISTS blocks_updated_at AFTER UPDATE ON blocks BEGIN
                UPDATE blocks SET updated_at = strftime('%s', 'now') * 1000 WHERE id = new.id;
            END;
        "#,
    },
    Migration {
        version: 2,
        name: "plugin_kv_docs_and_ai_transcripts",
        up_sql: r#"
            -- Namespaces for plugins (optional but useful for indexing)
            CREATE TABLE IF NOT EXISTS plugin_namespaces (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                plugin_id TEXT NOT NULL,
                namespace TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
                UNIQUE(plugin_id, namespace)
            );

            -- Key/Value storage for plugins (namespaced)
            CREATE TABLE IF NOT EXISTS plugin_kv (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                plugin_id TEXT NOT NULL,
                namespace TEXT NOT NULL,
                key TEXT NOT NULL,
                value BLOB NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
                UNIQUE(plugin_id, namespace, key)
            );
            CREATE INDEX IF NOT EXISTS idx_plugin_kv_plugin_ns ON plugin_kv(plugin_id, namespace);
            CREATE TRIGGER IF NOT EXISTS plugin_kv_updated_at AFTER UPDATE ON plugin_kv BEGIN
                UPDATE plugin_kv SET updated_at = strftime('%s', 'now') * 1000 WHERE id = new.id;
            END;

            -- Document storage for plugins (JSON documents)
            CREATE TABLE IF NOT EXISTS plugin_docs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                plugin_id TEXT NOT NULL,
                namespace TEXT NOT NULL,
                doc_id TEXT NOT NULL,
                doc_json TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
                UNIQUE(plugin_id, namespace, doc_id)
            );
            CREATE INDEX IF NOT EXISTS idx_plugin_docs_plugin_ns ON plugin_docs(plugin_id, namespace);
            CREATE TRIGGER IF NOT EXISTS plugin_docs_updated_at AFTER UPDATE ON plugin_docs BEGIN
                UPDATE plugin_docs SET updated_at = strftime('%s', 'now') * 1000 WHERE id = new.id;
            END;

            -- Optional per-plugin quota configuration (null namespace => default for plugin)
            CREATE TABLE IF NOT EXISTS plugin_quotas (
                plugin_id TEXT NOT NULL,
                namespace TEXT,
                max_total_bytes INTEGER,
                max_value_bytes INTEGER,
                max_keys INTEGER,
                max_docs INTEGER,
                PRIMARY KEY (plugin_id, namespace)
            );

            -- AI conversations and turns (transcripts)
            CREATE TABLE IF NOT EXISTS ai_conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                last_active INTEGER NOT NULL,
                status TEXT NOT NULL,
                metadata TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_ai_conversations_last_active ON ai_conversations(last_active);

            CREATE TABLE IF NOT EXISTS ai_turns (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                intent_json TEXT,
                entities_json TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_ai_turns_session_ts ON ai_turns(session_id, timestamp);
        "#,
    },
];

/// Get current schema version from database
#[allow(dead_code)]
async fn get_schema_version(pool: &Pool<Sqlite>) -> StorageResult<i32> {
    // Create schema_version table if it doesn't exist
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Get current version
    let row =
        sqlx::query("SELECT MAX(version) as version FROM schema_version").fetch_one(pool).await?;

    Ok(row.get::<Option<i32>, _>("version").unwrap_or(0))
}

/// Set schema version in database
#[allow(dead_code)]
async fn set_schema_version(pool: &Pool<Sqlite>, version: i32) -> StorageResult<()> {
    sqlx::query("INSERT OR REPLACE INTO schema_version (version) VALUES (?)")
        .bind(version)
        .execute(pool)
        .await?;

    Ok(())
}

/// Run all pending migrations
#[allow(dead_code)]
pub async fn run_migrations(pool: &Pool<Sqlite>) -> StorageResult<()> {
    let current_version = get_schema_version(pool).await?;

    log::info!("Current database schema version: {}", current_version);

    // Apply migrations that are newer than current version
    for migration in MIGRATIONS.iter().filter(|m| m.version > current_version) {
        log::info!("Applying migration {}: {}", migration.version, migration.name);

        // Start a transaction for the migration
        let mut tx = pool.begin().await?;

        // Execute migration SQL
        sqlx::query(migration.up_sql).execute(&mut *tx).await.map_err(|e| {
            StorageError::Migration(format!("Failed to apply migration {}: {}", migration.name, e))
        })?;

        // Update schema version
        sqlx::query("INSERT OR REPLACE INTO schema_version (version) VALUES (?)")
            .bind(migration.version)
            .execute(&mut *tx)
            .await?;

        // Commit transaction
        tx.commit().await?;

        log::info!("Successfully applied migration {}", migration.version);
    }

    if current_version < CURRENT_VERSION {
        log::info!("Database migrations completed. Schema version: {}", CURRENT_VERSION);
    } else {
        log::debug!("Database schema is up to date");
    }

    Ok(())
}

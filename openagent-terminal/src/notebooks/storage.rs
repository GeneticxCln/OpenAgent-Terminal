// Notebook storage using SQLite (sqlx)

#![cfg(feature = "blocks")]

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

use super::{CellId, CellType, Notebook, NotebookCell, NotebookId};
use crate::blocks_v2::{ExecutionStatus, ShellType};

pub struct NotebookStorage {
    pool: SqlitePool,
}

impl NotebookStorage {
    pub async fn new(data_dir: &Path) -> Result<Self> {
        tokio::fs::create_dir_all(data_dir).await?;
        let db_path = data_dir.join("notebooks.db");
        if !db_path.exists() {
            tokio::fs::File::create(&db_path).await?;
        }
        let db_url = format!("sqlite://{}", db_path.display());

        let connect_options = SqliteConnectOptions::from_str(&db_url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .idle_timeout(Duration::from_secs(30))
            .connect_with(connect_options)
            .await
            .context("Failed to connect to notebooks database")?;
        Self::initialize_schema(&pool).await?;
        info!("Notebook storage initialized at {}", db_path.display());
        Ok(Self { pool })
    }

    async fn initialize_schema(pool: &SqlitePool) -> Result<()> {
        // Enable foreign keys
        sqlx::query("PRAGMA foreign_keys = ON;").execute(pool).await?;

        // Main tables
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS notebooks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                tags TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                default_directory TEXT,
                env_overrides TEXT,
                params TEXT
            );

            CREATE TABLE IF NOT EXISTS notebook_cells (
                id TEXT PRIMARY KEY,
                notebook_id TEXT NOT NULL,
                idx INTEGER NOT NULL,
                cell_type TEXT NOT NULL,
                content TEXT NOT NULL,
                directory TEXT,
                shell TEXT,
                output TEXT,
                error_output TEXT,
                exit_code INTEGER,
                duration_ms INTEGER,
                block_id TEXT,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (notebook_id) REFERENCES notebooks(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_cells_notebook ON notebook_cells(notebook_id);
            CREATE INDEX IF NOT EXISTS idx_cells_notebook_idx ON notebook_cells(notebook_id, idx);
            "#,
        )
        .execute(pool)
        .await?;

        // Try to add new columns if upgrading from an earlier schema (ignore errors)
        let _ = sqlx::query("ALTER TABLE notebooks ADD COLUMN default_directory TEXT")
            .execute(pool)
            .await;
        let _ =
            sqlx::query("ALTER TABLE notebooks ADD COLUMN env_overrides TEXT").execute(pool).await;
        let _ = sqlx::query("ALTER TABLE notebooks ADD COLUMN params TEXT").execute(pool).await;

        Ok(())
    }

    pub async fn insert_notebook(&self, nb: &Notebook) -> Result<()> {
        let tags = serde_json::to_string(&nb.tags)?;
        let env_json = serde_json::to_string(&nb.env_overrides)?;
        let params_json = serde_json::to_string(&nb.params)?;
        sqlx::query(
            r#"
            INSERT INTO notebooks (id, name, description, tags, created_at, updated_at, default_directory, env_overrides, params)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(nb.id.to_string())
        .bind(&nb.name)
        .bind(&nb.description)
        .bind(tags)
        .bind(nb.created_at.to_rfc3339())
        .bind(nb.updated_at.to_rfc3339())
        .bind(nb.default_directory.as_ref().map(|p| p.to_string_lossy().to_string()))
        .bind(env_json)
        .bind(params_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_notebook(&self, id: NotebookId) -> Result<Notebook> {
        let row = sqlx::query_as::<_, NotebookRow>("SELECT * FROM notebooks WHERE id = ?")
            .bind(id.to_string())
            .fetch_one(&self.pool)
            .await?;
        row.into_notebook()
    }

    pub async fn list_notebooks(&self) -> Result<Vec<Notebook>> {
        let rows =
            sqlx::query_as::<_, NotebookRow>("SELECT * FROM notebooks ORDER BY updated_at DESC")
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter().map(|r| r.into_notebook()).collect()
    }

    pub async fn next_index_for_notebook(&self, nb: NotebookId) -> Result<i64> {
        let row: (Option<i64>,) =
            sqlx::query_as("SELECT MAX(idx) FROM notebook_cells WHERE notebook_id = ?")
                .bind(nb.to_string())
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0.map(|v| v + 1).unwrap_or(0))
    }

    pub async fn insert_cell(&self, cell: &NotebookCell) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO notebook_cells (
                id, notebook_id, idx, cell_type, content, directory, shell, output,
                error_output, exit_code, duration_ms, block_id, status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(cell.id.to_string())
        .bind(cell.notebook_id.to_string())
        .bind(cell.idx)
        .bind(cell_type_to_str(cell.cell_type))
        .bind(&cell.content)
        .bind(cell.directory.as_ref().map(|p| p.to_string_lossy().to_string()))
        .bind(cell.shell.map(|s| s.to_str().to_string()))
        .bind(&cell.output)
        .bind(&cell.error_output)
        .bind(cell.exit_code)
        .bind(cell.duration_ms.map(|d| d as i64))
        .bind(cell.block_id.map(|b| b.to_string()))
        .bind(format!("{:?}", cell.status))
        .bind(cell.created_at.to_rfc3339())
        .bind(cell.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_cell(&self, cell: &NotebookCell) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE notebook_cells SET
                idx = ?, cell_type = ?, content = ?, directory = ?, shell = ?, output = ?,
                error_output = ?, exit_code = ?, duration_ms = ?, block_id = ?, status = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(cell.idx)
        .bind(cell_type_to_str(cell.cell_type))
        .bind(&cell.content)
        .bind(cell.directory.as_ref().map(|p| p.to_string_lossy().to_string()))
        .bind(cell.shell.map(|s| s.to_str().to_string()))
        .bind(&cell.output)
        .bind(&cell.error_output)
        .bind(cell.exit_code)
        .bind(cell.duration_ms.map(|d| d as i64))
        .bind(cell.block_id.map(|b| b.to_string()))
        .bind(format!("{:?}", cell.status))
        .bind(cell.updated_at.to_rfc3339())
        .bind(cell.id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_cell(&self, id: CellId) -> Result<NotebookCell> {
        let row = sqlx::query_as::<_, CellRow>("SELECT * FROM notebook_cells WHERE id = ?")
            .bind(id.to_string())
            .fetch_one(&self.pool)
            .await?;
        row.into_cell()
    }

    pub async fn list_cells(&self, notebook_id: NotebookId) -> Result<Vec<NotebookCell>> {
        let rows = sqlx::query_as::<_, CellRow>(
            "SELECT * FROM notebook_cells WHERE notebook_id = ? ORDER BY idx ASC",
        )
        .bind(notebook_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(|r| r.into_cell()).collect()
    }

    pub async fn delete_cell(&self, id: CellId) -> Result<()> {
        sqlx::query("DELETE FROM notebook_cells WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct NotebookRow {
    id: String,
    name: String,
    description: Option<String>,
    tags: String,
    created_at: String,
    updated_at: String,
    default_directory: Option<String>,
    env_overrides: Option<String>,
    params: Option<String>,
}

impl NotebookRow {
    fn into_notebook(self) -> Result<Notebook> {
        Ok(Notebook {
            id: self.id.parse()?,
            name: self.name,
            description: self.description,
            tags: serde_json::from_str(&self.tags)?,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)?.with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)?.with_timezone(&Utc),
            default_directory: self.default_directory.map(std::path::PathBuf::from),
            env_overrides: match &self.env_overrides {
                Some(s) => serde_json::from_str(s)?,
                None => Default::default(),
            },
            params: match &self.params {
                Some(s) => serde_json::from_str(s)?,
                None => Vec::new(),
            },
        })
    }
}

#[derive(sqlx::FromRow)]
struct CellRow {
    id: String,
    notebook_id: String,
    idx: i64,
    cell_type: String,
    content: String,
    directory: Option<String>,
    shell: Option<String>,
    output: Option<String>,
    error_output: Option<String>,
    exit_code: Option<i32>,
    duration_ms: Option<i64>,
    block_id: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

impl CellRow {
    fn into_cell(self) -> Result<NotebookCell> {
        Ok(NotebookCell {
            id: self.id.parse()?,
            notebook_id: self.notebook_id.parse()?,
            idx: self.idx,
            cell_type: cell_type_from_str(&self.cell_type)?,
            content: self.content,
            directory: self.directory.map(std::path::PathBuf::from),
            shell: self.shell.and_then(|s| ShellType::from_str(&s).ok()),
            output: self.output,
            error_output: self.error_output,
            exit_code: self.exit_code,
            duration_ms: self.duration_ms.map(|d| d as u64),
            block_id: self.block_id.and_then(|s| crate::blocks_v2::BlockId::from_string(&s).ok()),
            status: parse_status(&self.status),
            created_at: DateTime::parse_from_rfc3339(&self.created_at)?.with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)?.with_timezone(&Utc),
        })
    }
}

fn cell_type_to_str(ct: CellType) -> &'static str {
    match ct {
        CellType::Command => "Command",
        CellType::Markdown => "Markdown",
    }
}
fn cell_type_from_str(s: &str) -> Result<CellType> {
    match s {
        "Command" => Ok(CellType::Command),
        "Markdown" => Ok(CellType::Markdown),
        _ => anyhow::bail!("unknown cell type: {}", s),
    }
}

fn parse_status(s: &str) -> ExecutionStatus {
    match s {
        "Running" => ExecutionStatus::Running,
        "Success" => ExecutionStatus::Success,
        "Failed" => ExecutionStatus::Failed,
        "Cancelled" => ExecutionStatus::Cancelled,
        "Timeout" => ExecutionStatus::Timeout,
        _ => ExecutionStatus::Failed,
    }
}

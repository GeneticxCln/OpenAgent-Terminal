//! Workflow Engine Persistence Integration
//!
//! This module provides persistence capabilities for the workflow engine,
//! allowing workflow execution history to be stored and retrieved.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub use super::{WorkflowDefinition, WorkflowStatus};

/// Re-export types for convenience
pub type WorkflowParameters = HashMap<String, serde_json::Value>;

/// Workflow execution record for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedWorkflowExecution {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub status: WorkflowExecutionStatus,
    pub parameters: WorkflowParameters,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub outputs: HashMap<String, String>,
    pub logs: Vec<String>,
    pub error_message: Option<String>,
    pub artifacts: Vec<String>,
    pub created_by: String,
}

/// Workflow execution status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl From<&WorkflowStatus> for WorkflowExecutionStatus {
    fn from(status: &WorkflowStatus) -> Self {
        match status {
            WorkflowStatus::Pending => Self::Pending,
            WorkflowStatus::Running => Self::Running,
            WorkflowStatus::Success => Self::Success,
            WorkflowStatus::Failed => Self::Failed,
            WorkflowStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<WorkflowExecutionStatus> for WorkflowStatus {
    fn from(status: WorkflowExecutionStatus) -> Self {
        match status {
            WorkflowExecutionStatus::Pending => Self::Pending,
            WorkflowExecutionStatus::Running => Self::Running,
            WorkflowExecutionStatus::Success => Self::Success,
            WorkflowExecutionStatus::Failed => Self::Failed,
            WorkflowExecutionStatus::Cancelled => Self::Cancelled,
        }
    }
}

/// Workflow execution summary for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionSummary {
    pub id: String,
    pub workflow_name: String,
    pub status: WorkflowExecutionStatus,
    pub started_at: DateTime<Utc>,
    pub duration_ms: Option<i64>,
    pub parameters_count: usize,
    pub has_outputs: bool,
    pub error_summary: Option<String>,
}

/// Search filters for workflow executions
#[derive(Debug, Clone, Default)]
pub struct WorkflowSearchFilters {
    pub workflow_name: Option<String>,
    pub status: Option<WorkflowExecutionStatus>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Basic workflow persistence interface
pub trait WorkflowPersistenceInterface: Send + Sync {
    /// Save workflow execution
    fn save_execution(&mut self, execution: &PersistedWorkflowExecution) -> Result<()>;
    
    /// Update execution status
    fn update_execution_status(
        &mut self,
        execution_id: &str,
        status: WorkflowExecutionStatus,
        error_message: Option<&str>,
    ) -> Result<()>;
    
    /// Get execution by ID
    fn get_execution(&self, execution_id: &str) -> Result<Option<PersistedWorkflowExecution>>;
    
    /// Search executions
    fn search_executions(&self, filters: &WorkflowSearchFilters) -> Result<Vec<WorkflowExecutionSummary>>;
    
    /// Add execution log
    fn add_execution_log(
        &mut self,
        execution_id: &str,
        level: &str,
        step_id: Option<&str>,
        message: &str,
    ) -> Result<()>;
}

/// Null persistence implementation (no-op)
pub struct NullWorkflowPersistence;

impl WorkflowPersistenceInterface for NullWorkflowPersistence {
    fn save_execution(&mut self, _execution: &PersistedWorkflowExecution) -> Result<()> {
        Ok(())
    }
    
    fn update_execution_status(
        &mut self,
        _execution_id: &str,
        _status: WorkflowExecutionStatus,
        _error_message: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }
    
    fn get_execution(&self, _execution_id: &str) -> Result<Option<PersistedWorkflowExecution>> {
        Ok(None)
    }
    
    fn search_executions(&self, _filters: &WorkflowSearchFilters) -> Result<Vec<WorkflowExecutionSummary>> {
        Ok(Vec::new())
    }
    
    fn add_execution_log(
        &mut self,
        _execution_id: &str,
        _level: &str,
        _step_id: Option<&str>,
        _message: &str,
    ) -> Result<()> {
        Ok(())
    }
}

/// SQLite-based workflow persistence
#[cfg(feature = "sqlite")]
pub struct SqliteWorkflowPersistence {
    conn: rusqlite::Connection,
}

#[cfg(feature = "sqlite")]
impl SqliteWorkflowPersistence {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        use rusqlite::{params, Connection};
        
        let conn = Connection::open(db_path.as_ref())
            .with_context(|| format!("Failed to open database at {:?}", db_path.as_ref()))?;

        // Enable optimizations
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        conn.execute("PRAGMA journal_mode = WAL", [])?;
        conn.execute("PRAGMA synchronous = NORMAL", [])?;

        let mut persistence = Self { conn };
        persistence.initialize_schema()?;
        Ok(persistence)
    }

    fn initialize_schema(&mut self) -> Result<()> {
        use rusqlite::params;
        
        // Main executions table
        self.conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS workflow_executions (
                id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                workflow_name TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                parameters TEXT NOT NULL DEFAULT '{}',
                started_at INTEGER NOT NULL,
                finished_at INTEGER,
                duration_ms INTEGER,
                outputs TEXT DEFAULT '{}',
                error_message TEXT,
                created_by TEXT NOT NULL DEFAULT 'unknown'
            )
            "#,
            [],
        )?;

        // Logs table
        self.conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS workflow_execution_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                execution_id TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                level TEXT NOT NULL DEFAULT 'info',
                step_id TEXT,
                message TEXT NOT NULL,
                FOREIGN KEY (execution_id) REFERENCES workflow_executions(id) ON DELETE CASCADE
            )
            "#,
            [],
        )?;

        // Artifacts table
        self.conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS workflow_execution_artifacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                execution_id TEXT NOT NULL,
                artifact_path TEXT NOT NULL,
                artifact_name TEXT NOT NULL,
                FOREIGN KEY (execution_id) REFERENCES workflow_executions(id) ON DELETE CASCADE
            )
            "#,
            [],
        )?;

        // Indexes
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_workflow_executions_workflow_name ON workflow_executions(workflow_name)",
            [],
        )?;
        
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_workflow_executions_status ON workflow_executions(status)",
            [],
        )?;
        
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_workflow_executions_started_at ON workflow_executions(started_at)",
            [],
        )?;

        Ok(())
    }
}

#[cfg(feature = "sqlite")]
impl WorkflowPersistenceInterface for SqliteWorkflowPersistence {
    fn save_execution(&mut self, execution: &PersistedWorkflowExecution) -> Result<()> {
        use rusqlite::params;
        
        let parameters_json = serde_json::to_string(&execution.parameters)?;
        let outputs_json = serde_json::to_string(&execution.outputs)?;

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO workflow_executions 
            (id, workflow_id, workflow_name, status, parameters, started_at, finished_at, 
             duration_ms, outputs, error_message, created_by)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                execution.id,
                execution.workflow_id,
                execution.workflow_name,
                execution.status.to_string(),
                parameters_json,
                execution.started_at.timestamp(),
                execution.finished_at.map(|dt| dt.timestamp()),
                execution.duration_ms,
                outputs_json,
                execution.error_message,
                execution.created_by,
            ],
        )?;

        // Save logs
        for log_entry in &execution.logs {
            self.add_execution_log(&execution.id, "info", None, log_entry)?;
        }

        Ok(())
    }

    fn update_execution_status(
        &mut self,
        execution_id: &str,
        status: WorkflowExecutionStatus,
        error_message: Option<&str>,
    ) -> Result<()> {
        use rusqlite::params;
        
        let finished_at = if matches!(
            status,
            WorkflowExecutionStatus::Success | WorkflowExecutionStatus::Failed | WorkflowExecutionStatus::Cancelled
        ) {
            Some(Utc::now().timestamp())
        } else {
            None
        };

        self.conn.execute(
            r#"
            UPDATE workflow_executions 
            SET status = ?1, finished_at = ?2, error_message = ?3
            WHERE id = ?4
            "#,
            params![status.to_string(), finished_at, error_message, execution_id],
        )?;

        Ok(())
    }

    fn get_execution(&self, execution_id: &str) -> Result<Option<PersistedWorkflowExecution>> {
        use rusqlite::{params, OptionalExtension};
        
        let execution = self.conn.query_row(
            r#"
            SELECT id, workflow_id, workflow_name, status, parameters, started_at, finished_at,
                   duration_ms, outputs, error_message, created_by
            FROM workflow_executions WHERE id = ?1
            "#,
            params![execution_id],
            |row| {
                let parameters_json: String = row.get(4)?;
                let outputs_json: String = row.get(8)?;

                Ok(PersistedWorkflowExecution {
                    id: row.get(0)?,
                    workflow_id: row.get(1)?,
                    workflow_name: row.get(2)?,
                    status: WorkflowExecutionStatus::from_string(&row.get::<_, String>(3)?),
                    parameters: serde_json::from_str(&parameters_json).unwrap_or_default(),
                    started_at: DateTime::from_timestamp(row.get(5)?, 0).unwrap_or_else(|| Utc::now()),
                    finished_at: row.get::<_, Option<i64>>(6)?.and_then(|ts| DateTime::from_timestamp(ts, 0)),
                    duration_ms: row.get(7)?,
                    outputs: serde_json::from_str(&outputs_json).unwrap_or_default(),
                    logs: Vec::new(), // Load separately
                    error_message: row.get(9)?,
                    artifacts: Vec::new(), // Load separately
                    created_by: row.get(10)?,
                })
            },
        ).optional()?;

        if let Some(mut exec) = execution {
            // Load logs
            exec.logs = self.get_execution_logs(&exec.id)?;
            // Load artifacts
            exec.artifacts = self.get_execution_artifacts(&exec.id)?;
            Ok(Some(exec))
        } else {
            Ok(None)
        }
    }

    fn search_executions(&self, filters: &WorkflowSearchFilters) -> Result<Vec<WorkflowExecutionSummary>> {
        use rusqlite::params;
        
        let mut query = r#"
            SELECT id, workflow_name, status, started_at, duration_ms, parameters, outputs, error_message
            FROM workflow_executions WHERE 1=1
        "#.to_string();

        let mut sql_params = Vec::new();
        let mut param_index = 1;

        if let Some(name) = &filters.workflow_name {
            query.push_str(&format!(" AND workflow_name LIKE ?{}", param_index));
            sql_params.push(format!("%{}%", name));
            param_index += 1;
        }

        if let Some(status) = &filters.status {
            query.push_str(&format!(" AND status = ?{}", param_index));
            sql_params.push(status.to_string());
            param_index += 1;
        }

        query.push_str(" ORDER BY started_at DESC");

        if let Some(limit) = filters.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = self.conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = sql_params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(&param_refs[..], |row| {
            let parameters_json: String = row.get(5)?;
            let outputs_json: String = row.get(6)?;

            let parameters: HashMap<String, serde_json::Value> =
                serde_json::from_str(&parameters_json).unwrap_or_default();
            let outputs: HashMap<String, String> =
                serde_json::from_str(&outputs_json).unwrap_or_default();

            Ok(WorkflowExecutionSummary {
                id: row.get(0)?,
                workflow_name: row.get(1)?,
                status: WorkflowExecutionStatus::from_string(&row.get::<_, String>(2)?),
                started_at: DateTime::from_timestamp(row.get(3)?, 0).unwrap_or_else(|| Utc::now()),
                duration_ms: row.get(4)?,
                parameters_count: parameters.len(),
                has_outputs: !outputs.is_empty(),
                error_summary: row.get::<_, Option<String>>(7)?.map(|e| {
                    if e.len() > 100 { format!("{}...", &e[..97]) } else { e }
                }),
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }

    fn add_execution_log(
        &mut self,
        execution_id: &str,
        level: &str,
        step_id: Option<&str>,
        message: &str,
    ) -> Result<()> {
        use rusqlite::params;
        
        self.conn.execute(
            r#"
            INSERT INTO workflow_execution_logs (execution_id, timestamp, level, step_id, message)
            VALUES (?1, strftime('%s', 'now'), ?2, ?3, ?4)
            "#,
            params![execution_id, level, step_id, message],
        )?;

        Ok(())
    }
}

#[cfg(feature = "sqlite")]
impl SqliteWorkflowPersistence {
    fn get_execution_logs(&self, execution_id: &str) -> Result<Vec<String>> {
        use rusqlite::params;
        
        let mut stmt = self.conn.prepare(
            "SELECT message FROM workflow_execution_logs WHERE execution_id = ?1 ORDER BY timestamp ASC",
        )?;

        let logs = stmt.query_map(params![execution_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(logs)
    }

    fn get_execution_artifacts(&self, execution_id: &str) -> Result<Vec<String>> {
        use rusqlite::params;
        
        let mut stmt = self.conn.prepare(
            "SELECT artifact_path FROM workflow_execution_artifacts WHERE execution_id = ?1",
        )?;

        let artifacts = stmt.query_map(params![execution_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(artifacts)
    }
}

impl WorkflowExecutionStatus {
    pub fn to_string(&self) -> String {
        match self {
            Self::Pending => "pending".to_string(),
            Self::Running => "running".to_string(),
            Self::Success => "success".to_string(),
            Self::Failed => "failed".to_string(),
            Self::Cancelled => "cancelled".to_string(),
        }
    }

    pub fn from_string(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "running" => Self::Running,
            "success" => Self::Success,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Failed,
        }
    }
}

/// Create a workflow persistence implementation
pub fn create_workflow_persistence<P: AsRef<Path>>(db_path: Option<P>) -> Result<Box<dyn WorkflowPersistenceInterface>> {
    if let Some(_path) = db_path {
        #[cfg(feature = "sqlite")]
        {
            let persistence = SqliteWorkflowPersistence::new(_path)?;
            Ok(Box::new(persistence))
        }
        #[cfg(not(feature = "sqlite"))]
        {
            Ok(Box::new(NullWorkflowPersistence))
        }
    } else {
        Ok(Box::new(NullWorkflowPersistence))
    }
}

/// Create a workflow execution from workflow state
pub fn create_persisted_execution(
    workflow_id: String,
    workflow_name: String,
    parameters: WorkflowParameters,
    created_by: String,
) -> PersistedWorkflowExecution {
    PersistedWorkflowExecution {
        id: uuid::Uuid::new_v4().to_string(),
        workflow_id,
        workflow_name,
        status: WorkflowExecutionStatus::Pending,
        parameters,
        started_at: Utc::now(),
        finished_at: None,
        duration_ms: None,
        outputs: HashMap::new(),
        logs: Vec::new(),
        error_message: None,
        artifacts: Vec::new(),
        created_by,
    }
}
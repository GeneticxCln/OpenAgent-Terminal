//! Workflow Run Persistence and History Management
//!
//! This module handles persistent storage of workflow executions, parameters,
//! results, and provides re-run capabilities with full history tracking.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info};
use uuid::Uuid;

/// Workflow execution status for persistence
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
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
            _ => Self::Failed, // Default to failed for unknown statuses
        }
    }
}

/// Persisted workflow execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecution {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub status: WorkflowExecutionStatus,
    pub parameters: HashMap<String, serde_json::Value>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub outputs: HashMap<String, String>,
    pub logs: Vec<String>,
    pub error_message: Option<String>,
    pub artifacts: Vec<String>,
    pub created_by: String, // User/session identifier
}

/// Workflow execution summary for UI display
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

/// Search filters for workflow execution history
#[derive(Debug, Clone, Default)]
pub struct WorkflowSearchFilters {
    pub workflow_name: Option<String>,
    pub status: Option<WorkflowExecutionStatus>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Workflow persistence manager
pub struct WorkflowPersistence {
    conn: Connection,
}

impl WorkflowPersistence {
    /// Initialize workflow persistence with database setup
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let conn = Connection::open(db_path.as_ref())
            .with_context(|| format!("Failed to open database at {:?}", db_path.as_ref()))?;

        // Enable foreign keys and WAL mode for better performance
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        conn.execute("PRAGMA journal_mode = WAL", [])?;
        conn.execute("PRAGMA synchronous = NORMAL", [])?;
        conn.execute("PRAGMA temp_store = memory", [])?;
        conn.execute("PRAGMA mmap_size = 268435456", [])?; // 256MB mmap

        let mut persistence = Self { conn };
        persistence.initialize_schema()?;
        Ok(persistence)
    }

    /// Initialize database schema for workflow persistence
    fn initialize_schema(&mut self) -> Result<()> {
        // Main workflow executions table
        self.conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS workflow_executions (
                id TEXT PRIMARY KEY NOT NULL,
                workflow_id TEXT NOT NULL,
                workflow_name TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                parameters TEXT NOT NULL DEFAULT '{}', -- JSON
                started_at INTEGER NOT NULL, -- Unix timestamp
                finished_at INTEGER, -- Unix timestamp
                duration_ms INTEGER,
                outputs TEXT DEFAULT '{}', -- JSON
                error_message TEXT,
                created_by TEXT NOT NULL DEFAULT 'unknown',
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            )
            "#,
            [],
        )?;

        // Workflow execution logs table
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

        // Workflow execution artifacts table
        self.conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS workflow_execution_artifacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                execution_id TEXT NOT NULL,
                artifact_path TEXT NOT NULL,
                artifact_name TEXT NOT NULL,
                artifact_size INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                FOREIGN KEY (execution_id) REFERENCES workflow_executions(id) ON DELETE CASCADE
            )
            "#,
            [],
        )?;

        // Create indexes for better query performance
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

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_workflow_execution_logs_execution_id ON workflow_execution_logs(execution_id)",
            [],
        )?;

        info!("Workflow persistence schema initialized successfully");
        Ok(())
    }

    /// Save a workflow execution record
    pub fn save_execution(&mut self, execution: &WorkflowExecution) -> Result<()> {
        let parameters_json = serde_json::to_string(&execution.parameters)
            .context("Failed to serialize workflow parameters")?;
        let outputs_json = serde_json::to_string(&execution.outputs)
            .context("Failed to serialize workflow outputs")?;

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO workflow_executions 
            (id, workflow_id, workflow_name, status, parameters, started_at, finished_at, 
             duration_ms, outputs, error_message, created_by, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, strftime('%s', 'now'))
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

        // Save artifacts
        for artifact in &execution.artifacts {
            self.add_execution_artifact(&execution.id, artifact, artifact)?;
        }

        debug!("Saved workflow execution: {}", execution.id);
        Ok(())
    }

    /// Update workflow execution status
    pub fn update_execution_status(
        &mut self,
        execution_id: &str,
        status: WorkflowExecutionStatus,
        error_message: Option<&str>,
    ) -> Result<()> {
        let finished_at = if matches!(
            status,
            WorkflowExecutionStatus::Success
                | WorkflowExecutionStatus::Failed
                | WorkflowExecutionStatus::Cancelled
        ) {
            Some(Utc::now().timestamp())
        } else {
            None
        };

        // Calculate duration if workflow is finishing
        let duration_ms = if finished_at.is_some() {
            // Get started_at to calculate duration
            let started_at: Option<i64> = self
                .conn
                .query_row(
                    "SELECT started_at FROM workflow_executions WHERE id = ?1",
                    params![execution_id],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(start_timestamp) = started_at {
                Some((finished_at.unwrap() - start_timestamp) * 1000) // Convert to milliseconds
            } else {
                None
            }
        } else {
            None
        };

        self.conn.execute(
            r#"
            UPDATE workflow_executions 
            SET status = ?1, finished_at = ?2, duration_ms = ?3, error_message = ?4
            WHERE id = ?5
            "#,
            params![status.to_string(), finished_at, duration_ms, error_message, execution_id],
        )?;

        debug!("Updated workflow execution status: {} -> {:?}", execution_id, status);
        Ok(())
    }

    /// Add log entry to workflow execution
    pub fn add_execution_log(
        &mut self,
        execution_id: &str,
        level: &str,
        step_id: Option<&str>,
        message: &str,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO workflow_execution_logs (execution_id, timestamp, level, step_id, message)
            VALUES (?1, strftime('%s', 'now'), ?2, ?3, ?4)
            "#,
            params![execution_id, level, step_id, message],
        )?;

        Ok(())
    }

    /// Add artifact to workflow execution
    pub fn add_execution_artifact(
        &mut self,
        execution_id: &str,
        artifact_path: &str,
        artifact_name: &str,
    ) -> Result<()> {
        // Get file size if the artifact path exists
        let artifact_size = if let Ok(metadata) = std::fs::metadata(artifact_path) {
            metadata.len() as i64
        } else {
            0
        };

        self.conn.execute(
            r#"
            INSERT INTO workflow_execution_artifacts (execution_id, artifact_path, artifact_name, artifact_size)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![execution_id, artifact_path, artifact_name, artifact_size],
        )?;

        Ok(())
    }

    /// Get workflow execution by ID
    pub fn get_execution(&self, execution_id: &str) -> Result<Option<WorkflowExecution>> {
        let execution = self
            .conn
            .query_row(
                r#"
                SELECT id, workflow_id, workflow_name, status, parameters, started_at, finished_at,
                       duration_ms, outputs, error_message, created_by
                FROM workflow_executions WHERE id = ?1
                "#,
                params![execution_id],
                |row| self.row_to_execution(row),
            )
            .optional()?;

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

    /// Search workflow executions with filters
    pub fn search_executions(
        &self,
        filters: &WorkflowSearchFilters,
    ) -> Result<Vec<WorkflowExecutionSummary>> {
        let mut query = r#"
            SELECT id, workflow_name, status, started_at, duration_ms, parameters, outputs, error_message
            FROM workflow_executions WHERE 1=1
        "#
        .to_string();

        let mut params = Vec::new();
        let mut param_index = 1;

        // Add filters
        if let Some(name) = &filters.workflow_name {
            query.push_str(&format!(" AND workflow_name LIKE ?{}", param_index));
            params.push(format!("%{}%", name));
            param_index += 1;
        }

        if let Some(status) = &filters.status {
            query.push_str(&format!(" AND status = ?{}", param_index));
            params.push(status.to_string());
            param_index += 1;
        }

        if let Some(date_from) = filters.date_from {
            query.push_str(&format!(" AND started_at >= ?{}", param_index));
            params.push(date_from.timestamp().to_string());
            param_index += 1;
        }

        if let Some(date_to) = filters.date_to {
            query.push_str(&format!(" AND started_at <= ?{}", param_index));
            params.push(date_to.timestamp().to_string());
            // Remove unused param_index increment since this is the last parameter
        }

        query.push_str(" ORDER BY started_at DESC");

        if let Some(limit) = filters.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = filters.offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        let mut stmt = self.conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

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
                started_at: DateTime::from_timestamp(row.get(3)?, 0)
                    .unwrap_or_else(|| Utc::now()),
                duration_ms: row.get(4)?,
                parameters_count: parameters.len(),
                has_outputs: !outputs.is_empty(),
                error_summary: row.get::<_, Option<String>>(7)?.map(|e| {
                    if e.len() > 100 {
                        format!("{}...", &e[..97])
                    } else {
                        e
                    }
                }),
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }

    /// Get recent workflow executions
    pub fn get_recent_executions(&self, limit: usize) -> Result<Vec<WorkflowExecutionSummary>> {
        let filters = WorkflowSearchFilters {
            limit: Some(limit),
            ..Default::default()
        };
        self.search_executions(&filters)
    }

    /// Get workflow execution statistics
    pub fn get_execution_stats(&self) -> Result<WorkflowExecutionStats> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM workflow_executions", [], |row| {
                row.get(0)
            })?;

        let successful: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM workflow_executions WHERE status = 'success'",
            [],
            |row| row.get(0),
        )?;

        let failed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM workflow_executions WHERE status = 'failed'",
            [],
            |row| row.get(0),
        )?;

        let running: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM workflow_executions WHERE status = 'running'",
            [],
            |row| row.get(0),
        )?;

        let avg_duration_ms: Option<f64> = self
            .conn
            .query_row(
                "SELECT AVG(duration_ms) FROM workflow_executions WHERE duration_ms IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten();

        Ok(WorkflowExecutionStats {
            total_executions: total as usize,
            successful_executions: successful as usize,
            failed_executions: failed as usize,
            running_executions: running as usize,
            average_duration_ms: avg_duration_ms.map(|d| d as i64),
        })
    }

    /// Delete old workflow executions (cleanup)
    pub fn cleanup_old_executions(&mut self, retention_days: u32) -> Result<usize> {
        let cutoff_timestamp = (Utc::now() - chrono::Duration::days(retention_days as i64)).timestamp();

        let deleted = self.conn.execute(
            "DELETE FROM workflow_executions WHERE started_at < ?1",
            params![cutoff_timestamp],
        )?;

        if deleted > 0 {
            info!("Cleaned up {} old workflow executions", deleted);
        }

        Ok(deleted)
    }

    /// Helper method to convert database row to WorkflowExecution
    fn row_to_execution(&self, row: &Row<'_>) -> rusqlite::Result<WorkflowExecution> {
        let parameters_json: String = row.get(4)?;
        let outputs_json: String = row.get(8)?;

        let parameters: HashMap<String, serde_json::Value> =
            serde_json::from_str(&parameters_json).unwrap_or_default();
        let outputs: HashMap<String, String> =
            serde_json::from_str(&outputs_json).unwrap_or_default();

        Ok(WorkflowExecution {
            id: row.get(0)?,
            workflow_id: row.get(1)?,
            workflow_name: row.get(2)?,
            status: WorkflowExecutionStatus::from_string(&row.get::<_, String>(3)?),
            parameters,
            started_at: DateTime::from_timestamp(row.get(5)?, 0).unwrap_or_else(|| Utc::now()),
            finished_at: row
                .get::<_, Option<i64>>(6)?
                .and_then(|ts| DateTime::from_timestamp(ts, 0)),
            duration_ms: row.get(7)?,
            outputs,
            logs: Vec::new(), // Loaded separately
            error_message: row.get(9)?,
            artifacts: Vec::new(), // Loaded separately
            created_by: row.get(10)?,
        })
    }

    /// Get execution logs
    fn get_execution_logs(&self, execution_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT message FROM workflow_execution_logs WHERE execution_id = ?1 ORDER BY timestamp ASC",
        )?;

        let logs = stmt
            .query_map(params![execution_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(logs)
    }

    /// Get execution artifacts
    fn get_execution_artifacts(&self, execution_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT artifact_path FROM workflow_execution_artifacts WHERE execution_id = ?1",
        )?;

        let artifacts = stmt
            .query_map(params![execution_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(artifacts)
    }
}

/// Workflow execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionStats {
    pub total_executions: usize,
    pub successful_executions: usize,
    pub failed_executions: usize,
    pub running_executions: usize,
    pub average_duration_ms: Option<i64>,
}

/// Create a new workflow execution record
pub fn create_workflow_execution(
    workflow_id: String,
    workflow_name: String,
    parameters: HashMap<String, serde_json::Value>,
    created_by: String,
) -> WorkflowExecution {
    WorkflowExecution {
        id: Uuid::new_v4().to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_workflow_persistence_creation() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_workflows.db");
        
        let _persistence = WorkflowPersistence::new(&db_path)?;
        
        // Verify database file was created
        assert!(db_path.exists());
        
        Ok(())
    }

    #[test]
    fn test_workflow_execution_lifecycle() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_workflows.db");
        let mut persistence = WorkflowPersistence::new(&db_path)?;

        // Create a test execution
        let execution = create_workflow_execution(
            "test-workflow-1".to_string(),
            "Test Workflow".to_string(),
            HashMap::new(),
            "test-user".to_string(),
        );

        // Save the execution
        persistence.save_execution(&execution)?;

        // Retrieve and verify
        let retrieved = persistence.get_execution(&execution.id)?;
        assert!(retrieved.is_some());

        let retrieved_execution = retrieved.unwrap();
        assert_eq!(retrieved_execution.workflow_name, "Test Workflow");
        assert_eq!(retrieved_execution.status, WorkflowExecutionStatus::Pending);

        // Update status
        persistence.update_execution_status(
            &execution.id,
            WorkflowExecutionStatus::Success,
            None,
        )?;

        // Verify status update
        let updated = persistence.get_execution(&execution.id)?.unwrap();
        assert_eq!(updated.status, WorkflowExecutionStatus::Success);
        assert!(updated.finished_at.is_some());
        assert!(updated.duration_ms.is_some());

        Ok(())
    }

    #[test]
    fn test_workflow_search() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_workflows.db");
        let mut persistence = WorkflowPersistence::new(&db_path)?;

        // Create multiple test executions
        for i in 0..5 {
            let execution = create_workflow_execution(
                format!("workflow-{}", i),
                format!("Test Workflow {}", i),
                HashMap::new(),
                "test-user".to_string(),
            );
            persistence.save_execution(&execution)?;
        }

        // Test search with no filters
        let all_results = persistence.search_executions(&WorkflowSearchFilters::default())?;
        assert_eq!(all_results.len(), 5);

        // Test search with limit
        let limited_results = persistence.search_executions(&WorkflowSearchFilters {
            limit: Some(3),
            ..Default::default()
        })?;
        assert_eq!(limited_results.len(), 3);

        // Test search by workflow name
        let name_results = persistence.search_executions(&WorkflowSearchFilters {
            workflow_name: Some("Test Workflow 1".to_string()),
            ..Default::default()
        })?;
        assert_eq!(name_results.len(), 1);
        assert_eq!(name_results[0].workflow_name, "Test Workflow 1");

        Ok(())
    }

    #[test]
    fn test_execution_stats() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_workflows.db");
        let mut persistence = WorkflowPersistence::new(&db_path)?;

        // Create and save test executions with different statuses
        let execution1 = create_workflow_execution(
            "workflow-1".to_string(),
            "Success Workflow".to_string(),
            HashMap::new(),
            "test-user".to_string(),
        );
        persistence.save_execution(&execution1)?;
        persistence.update_execution_status(&execution1.id, WorkflowExecutionStatus::Success, None)?;

        let execution2 = create_workflow_execution(
            "workflow-2".to_string(),
            "Failed Workflow".to_string(),
            HashMap::new(),
            "test-user".to_string(),
        );
        persistence.save_execution(&execution2)?;
        persistence.update_execution_status(&execution2.id, WorkflowExecutionStatus::Failed, Some("Test error"))?;

        // Get stats
        let stats = persistence.get_execution_stats()?;
        assert_eq!(stats.total_executions, 2);
        assert_eq!(stats.successful_executions, 1);
        assert_eq!(stats.failed_executions, 1);
        assert_eq!(stats.running_executions, 0);

        Ok(())
    }
}
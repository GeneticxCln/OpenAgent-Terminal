//! Migration functionality for OpenAgent Terminal
//!
//! This module provides migration tools for configuration and data.

use crate::{UtilsError, UtilsResult};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

/// Migration information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Migration {
    pub id: String,
    pub version: String,
    pub description: String,
    pub applied_at: Option<String>,
}

/// Migration manager
#[derive(Debug, Default)]
pub struct MigrateManager {
    migrations: Vec<Migration>,
}

impl MigrateManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initialize(&mut self) -> UtilsResult<()> {
        tracing::info!("Initializing migration manager");
        self.load_available_migrations()?;
        Ok(())
    }

    pub fn load_from_directory(&mut self, path: &Path) -> UtilsResult<()> {
        tracing::info!("Loading migrations from directory: {:?}", path);
        if !path.exists() {
            return Err(UtilsError::Migration(format!(
                "Migration directory does not exist: {:?}",
                path
            )));
        }
        if !path.is_dir() {
            return Err(UtilsError::Migration(format!(
                "Migration path is not a directory: {:?}",
                path
            )));
        }

        // Build a map to de-dupe by ID, allowing filesystem files to override built-ins
        let mut by_id: std::collections::HashMap<String, Migration> =
            self.migrations.iter().cloned().map(|m| (m.id.clone(), m)).collect();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();
            if !file_path.is_file() {
                continue;
            }
            let ext = file_path.extension().and_then(OsStr::to_str).unwrap_or("");
            let content = match fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("Failed to read migration file {:?}: {}", file_path, e);
                    continue;
                }
            };

            // Try multiple formats: single Migration or array of Migration
            let load_result: UtilsResult<Vec<Migration>> = match ext.to_ascii_lowercase().as_str() {
                "toml" => {
                    let parsed_single: Result<Migration, toml::de::Error> = toml::from_str(&content);
                    if let Ok(m) = parsed_single {
                        Ok(vec![m])
                    } else {
                        let parsed_many: Result<Vec<Migration>, toml::de::Error> = toml::from_str(&content);
                        parsed_many.map_err(UtilsError::from)
                    }
                }
                "yaml" | "yml" => {
                    let parsed_single: Result<Migration, serde_yaml::Error> = serde_yaml::from_str(&content);
                    if let Ok(m) = parsed_single {
                        Ok(vec![m])
                    } else {
                        let parsed_many: Result<Vec<Migration>, serde_yaml::Error> = serde_yaml::from_str(&content);
                        parsed_many.map_err(UtilsError::from)
                    }
                }
                "json" => {
                    let parsed_single: Result<Migration, serde_json::Error> = serde_json::from_str(&content);
                    if let Ok(m) = parsed_single {
                        Ok(vec![m])
                    } else {
                        let parsed_many: Result<Vec<Migration>, serde_json::Error> = serde_json::from_str(&content);
                        parsed_many.map_err(UtilsError::from)
                    }
                }
                _ => {
                    tracing::debug!("Skipping non-migration file {:?}", file_path);
                    continue;
                }
            };

            match load_result {
                Ok(migs) => {
                    for m in migs {
                        if m.id.trim().is_empty() {
                            tracing::warn!("Skipping migration with empty id in file {:?}", file_path);
                            continue;
                        }
                        by_id.insert(m.id.clone(), m);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to parse migrations in {:?}: {}", file_path, e);
                    continue;
                }
            }
        }

        // Replace internal list with de-duped, stable-sorted by id
        let mut merged: Vec<Migration> = by_id.into_values().collect();
        merged.sort_by(|a, b| a.id.cmp(&b.id));
        self.migrations = merged;
        Ok(())
    }

    pub fn list_migrations(&self) -> &[Migration] {
        &self.migrations
    }

    pub fn apply_migration(&mut self, migration_id: &str) -> UtilsResult<()> {
        tracing::info!("Applying migration: {}", migration_id);

        // Find the migration
        if let Some(migration) = self.migrations.iter_mut().find(|m| m.id == migration_id) {
            // Mark as applied (simplified - in real implementation would execute migration)
            migration.applied_at = Some(chrono::Utc::now().to_rfc3339());
            tracing::info!("Migration {} applied successfully", migration_id);
            Ok(())
        } else {
            Err(UtilsError::Migration(format!("Migration '{}' not found", migration_id)))
        }
    }

    pub fn rollback_migration(&mut self, migration_id: &str) -> UtilsResult<()> {
        tracing::info!("Rolling back migration: {}", migration_id);

        if let Some(migration) = self.migrations.iter_mut().find(|m| m.id == migration_id) {
            migration.applied_at = None;
            tracing::info!("Migration {} rolled back successfully", migration_id);
            Ok(())
        } else {
            Err(UtilsError::Migration(format!("Migration '{}' not found", migration_id)))
        }
    }

    pub fn get_pending_migrations(&self) -> Vec<&Migration> {
        self.migrations.iter().filter(|m| m.applied_at.is_none()).collect()
    }

    pub fn get_applied_migrations(&self) -> Vec<&Migration> {
        self.migrations.iter().filter(|m| m.applied_at.is_some()).collect()
    }

    fn load_available_migrations(&mut self) -> UtilsResult<()> {
        // Define built-in migrations
        let migrations = vec![
            Migration {
                id: "001_initial_setup".to_string(),
                version: "0.1.0".to_string(),
                description: "Initial configuration setup".to_string(),
                applied_at: None,
            },
            Migration {
                id: "002_theme_system".to_string(),
                version: "0.2.0".to_string(),
                description: "Add theme configuration support".to_string(),
                applied_at: None,
            },
            Migration {
                id: "003_ai_integration".to_string(),
                version: "0.3.0".to_string(),
                description: "Add AI provider configuration".to_string(),
                applied_at: None,
            },
        ];

        self.migrations = migrations;
        Ok(())
    }
}

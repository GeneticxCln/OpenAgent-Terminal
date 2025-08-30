//! Local filesystem sync provider implementation.
//! 
//! This provider stores sync state and data in the local filesystem,
//! typically under XDG_STATE_HOME or a configured directory.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{SyncConfig, SyncError, SyncProvider, SyncScope, SyncStatus};

/// Local filesystem sync provider.
/// 
/// Stores sync metadata and data in a local directory structure.
/// This is useful for testing, local backups, or syncing via external
/// tools like rsync, git, or cloud folder sync services.
#[derive(Debug)]
pub struct LocalFsProvider {
    /// Base directory for sync data
    base_dir: PathBuf,
}

impl LocalFsProvider {
    /// Create a new LocalFsProvider with the given configuration.
    pub fn new(config: &SyncConfig) -> Result<Self, SyncError> {
        let base_dir = if let Some(ref dir) = config.data_dir {
            dir.clone()
        } else {
            // Use XDG_STATE_HOME or fallback to ~/.local/state
            let state_dir = std::env::var("XDG_STATE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    let home = std::env::var("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(|_| PathBuf::from("."));
                    home.join(".local").join("state")
                });
            state_dir.join("openagent-terminal").join("sync")
        };

        // Ensure the base directory exists
        fs::create_dir_all(&base_dir)?;

        Ok(Self { base_dir })
    }

    /// Get the path for a specific sync scope.
    fn scope_dir(&self, scope: SyncScope) -> PathBuf {
        match scope {
            SyncScope::Settings => self.base_dir.join("settings"),
            SyncScope::History => self.base_dir.join("history"),
        }
    }

    /// Get the status file path.
    fn status_file(&self) -> PathBuf {
        self.base_dir.join("sync_status.json")
    }

    /// Read the current sync status from disk.
    fn read_status(&self) -> Result<SyncStatus, SyncError> {
        let status_path = self.status_file();
        
        if !status_path.exists() {
            return Ok(SyncStatus::default());
        }

        let content = fs::read_to_string(&status_path)?;
        
        // Simple JSON parsing
        let mut status = SyncStatus::default();
        
        if let Some(push_pos) = content.find("\"last_push\":") {
            if let Some(push_str) = content[push_pos + 12..].split(',').next() {
                if let Ok(timestamp) = push_str.trim().parse::<u64>() {
                    status.last_push = Some(timestamp);
                }
            }
        }
        
        if let Some(pull_pos) = content.find("\"last_pull\":") {
            if let Some(pull_str) = content[pull_pos + 12..].split(',').next() {
                if let Ok(timestamp) = pull_str.trim().parse::<u64>() {
                    status.last_pull = Some(timestamp);
                }
            }
        }
        
        if content.contains("\"pending\":true") {
            status.pending = true;
        }
        
        Ok(status)
    }

    /// Write the sync status to disk.
    fn write_status(&self, status: &SyncStatus) -> Result<(), SyncError> {
        let status_path = self.status_file();
        
        let json = format!(
            "{{\n  \"last_push\": {},\n  \"last_pull\": {},\n  \"pending\": {}\n}}\n",
            status.last_push.map_or("null".to_string(), |t| t.to_string()),
            status.last_pull.map_or("null".to_string(), |t| t.to_string()),
            status.pending
        );
        
        let mut file = fs::File::create(status_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        
        Ok(())
    }

    /// Get the current Unix timestamp in seconds.
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Get the source directory for a given scope (in user config).
    fn source_dir(&self, scope: SyncScope) -> PathBuf {
        match scope {
            SyncScope::Settings => {
                // Use XDG_CONFIG_HOME or fallback
                let config_dir = std::env::var("XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| {
                        let home = std::env::var("HOME")
                            .map(PathBuf::from)
                            .unwrap_or_else(|_| PathBuf::from("."));
                        home.join(".config")
                    });
                config_dir.join("openagent-terminal")
            }
            SyncScope::History => {
                // Use XDG_DATA_HOME or fallback
                let data_dir = std::env::var("XDG_DATA_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| {
                        let home = std::env::var("HOME")
                            .map(PathBuf::from)
                            .unwrap_or_else(|_| PathBuf::from("."));
                        home.join(".local").join("share")
                    });
                data_dir.join("openagent-terminal")
            }
        }
    }

    /// Copy files recursively from source to destination.
    fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), SyncError> {
        if !src.exists() {
            return Ok(());
        }

        fs::create_dir_all(dst)?;

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let file_name = entry.file_name();
            let dst_path = dst.join(&file_name);

            if src_path.is_dir() {
                Self::copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                // Skip temporary files and backups
                let name = file_name.to_string_lossy();
                if name.starts_with('.') && (name.ends_with(".swp") || name.ends_with("~")) {
                    continue;
                }
                
                fs::copy(&src_path, &dst_path)?;
            }
        }

        Ok(())
    }
}

impl SyncProvider for LocalFsProvider {
    fn name(&self) -> &'static str {
        "local_fs"
    }

    fn status(&self) -> Result<SyncStatus, SyncError> {
        self.read_status()
    }

    fn push(&self, scope: SyncScope) -> Result<(), SyncError> {
        let source = self.source_dir(scope);
        let destination = self.scope_dir(scope);

        // Create destination directory if it doesn't exist
        fs::create_dir_all(&destination)?;

        // Copy files from source to destination
        Self::copy_dir_recursive(&source, &destination)?;

        // Update status
        let mut status = self.read_status().unwrap_or_default();
        status.last_push = Some(Self::current_timestamp());
        status.pending = false;
        self.write_status(&status)?;

        Ok(())
    }

    fn pull(&self, scope: SyncScope) -> Result<(), SyncError> {
        let source = self.scope_dir(scope);
        let destination = self.source_dir(scope);

        if !source.exists() {
            return Err(SyncError::Other(format!(
                "No sync data found for scope {:?}",
                scope
            )));
        }

        // Create backup of current config
        if destination.exists() {
            let backup_path = destination.with_extension("backup");
            if backup_path.exists() {
                fs::remove_dir_all(&backup_path)?;
            }
            Self::copy_dir_recursive(&destination, &backup_path)?;
        }

        // Create destination directory if it doesn't exist
        fs::create_dir_all(&destination)?;

        // Copy files from sync storage to config location
        Self::copy_dir_recursive(&source, &destination)?;

        // Update status
        let mut status = self.read_status().unwrap_or_default();
        status.last_pull = Some(Self::current_timestamp());
        self.write_status(&status)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn test_local_fs_provider_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            provider: "local_fs".to_string(),
            data_dir: Some(temp_dir.path().to_path_buf()),
            endpoint_env: None,
            encryption_key_env: None,
        };

        let provider = LocalFsProvider::new(&config).unwrap();
        assert_eq!(provider.name(), "local_fs");
    }

    #[test]
    fn test_status_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let config = SyncConfig {
            provider: "local_fs".to_string(),
            data_dir: Some(temp_dir.path().to_path_buf()),
            endpoint_env: None,
            encryption_key_env: None,
        };

        let provider = LocalFsProvider::new(&config).unwrap();
        
        // Initial status should be default
        let status = provider.status().unwrap();
        assert_eq!(status.last_push, None);
        assert_eq!(status.last_pull, None);
        assert_eq!(status.pending, false);

        // Write a custom status
        let custom_status = SyncStatus {
            last_push: Some(1234567890),
            last_pull: Some(1234567891),
            pending: true,
        };
        provider.write_status(&custom_status).unwrap();

        // Read it back
        let read_status = provider.status().unwrap();
        assert_eq!(read_status.last_push, Some(1234567890));
        assert_eq!(read_status.last_pull, Some(1234567891));
        assert_eq!(read_status.pending, true);
    }
}

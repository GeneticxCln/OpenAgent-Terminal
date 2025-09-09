//! Command History Integration
//! Bridges the blocks_v2 system with the terminal for command/history tracking

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tracing::{debug, error, info};

#[cfg(feature = "blocks")]
use crate::blocks_v2::{
    BlockId, BlockManager, CreateBlockParams, ShellType, 
};

#[cfg(not(feature = "blocks"))]
use chrono::Utc;

/// Command history manager that integrates with the blocks system
pub struct CommandHistory {
    #[cfg(feature = "blocks")]
    block_manager: Option<Arc<Mutex<BlockManager>>>,
    
    // Fallback history when blocks feature is disabled
    #[cfg(not(feature = "blocks"))]
    simple_history: Vec<HistoryEntry>,
    
    // Current command tracking
    current_command: Option<ActiveCommand>,
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub command: String,
    pub exit_code: Option<i32>,
    pub output: String,
    pub working_dir: PathBuf,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct ActiveCommand {
    #[cfg(feature = "blocks")]
    pub block_id: Option<BlockId>,
    pub command: String,
    pub working_dir: PathBuf,
    pub start_time: std::time::Instant,
}

impl CommandHistory {
    /// Create new command history manager
    pub async fn new(data_dir: Option<PathBuf>) -> Self {
        #[cfg(feature = "blocks")]
        {
            let block_manager = if let Some(dir) = data_dir {
                match BlockManager::new(dir).await {
                    Ok(manager) => {
                        info!("Command history initialized with blocks storage");
                        Some(Arc::new(Mutex::new(manager)))
                    },
                    Err(e) => {
                        error!("Failed to initialize block manager: {}, using fallback", e);
                        None
                    }
                }
            } else {
                None
            };
            
            Self {
                block_manager,
                current_command: None,
            }
        }
        
        #[cfg(not(feature = "blocks"))]
        {
            info!("Command history initialized with simple fallback (blocks feature disabled)");
            Self {
                simple_history: Vec::new(),
                current_command: None,
            }
        }
    }

    /// Start tracking a new command
    pub async fn start_command(&mut self, command: String, working_dir: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
        let working_dir = working_dir.unwrap_or_else(|| 
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
        );

        debug!("Starting command: {} in {}", command, working_dir.display());

        #[cfg(feature = "blocks")]
        if let Some(ref block_manager) = self.block_manager {
            let mut manager = block_manager.lock().unwrap();
            let params = CreateBlockParams {
                command: command.clone(),
                directory: Some(working_dir.clone()),
                environment: None,
                shell: Some(self.detect_shell()),
                tags: None,
                parent_id: None,
                metadata: None,
            };
            
            match manager.create_block(params).await {
                Ok(block) => {
                    self.current_command = Some(ActiveCommand {
                        block_id: Some(block.id),
                        command: command.clone(),
                        working_dir,
                        start_time: std::time::Instant::now(),
                    });
                    info!("Created block {} for command", block.id);
                },
                Err(e) => {
                    error!("Failed to create block: {}", e);
                    // Fallback to simple tracking
                    self.current_command = Some(ActiveCommand {
                        block_id: None,
                        command: command.clone(),
                        working_dir,
                        start_time: std::time::Instant::now(),
                    });
                }
            }
        } else {
            self.current_command = Some(ActiveCommand {
                block_id: None,
                command: command.clone(),
                working_dir,
                start_time: std::time::Instant::now(),
            });
        }

        #[cfg(not(feature = "blocks"))]
        {
            self.current_command = Some(ActiveCommand {
                command: command.clone(),
                working_dir,
                start_time: std::time::Instant::now(),
            });
        }

        Ok(())
    }

    /// Complete the current command with output and exit code
    pub async fn complete_command(&mut self, exit_code: i32, output: String) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(active) = self.current_command.take() {
            let duration = active.start_time.elapsed();
            debug!("Completing command with exit_code={}, duration={:?}", exit_code, duration);

            #[cfg(feature = "blocks")]
            if let (Some(ref block_manager), Some(block_id)) = (&self.block_manager, active.block_id) {
                let mut manager = block_manager.lock().unwrap();
                match manager.update_block_output(block_id, output.clone(), exit_code, duration.as_millis() as u64).await {
                    Ok(_) => {
                        info!("Updated block {} with output", block_id);
                    },
                    Err(e) => {
                        error!("Failed to update block: {}", e);
                    }
                }
            }

            // Add to simple history as fallback
            #[cfg(not(feature = "blocks"))]
            {
                let entry = HistoryEntry {
                    command: active.command,
                    exit_code: Some(exit_code),
                    output,
                    working_dir: active.working_dir,
                    timestamp: Utc::now(),
                    duration: Some(duration),
                };
                self.simple_history.push(entry);
                
                // Keep only last 1000 entries
                if self.simple_history.len() > 1000 {
                    self.simple_history.remove(0);
                }
            }
        }

        Ok(())
    }

    /// Search command history
    pub async fn search(&self, query: &str, max_results: usize) -> Vec<HistoryEntry> {
        #[cfg(feature = "blocks")]
        if let Some(ref block_manager) = self.block_manager {
            let manager = block_manager.lock().unwrap();
            let search_query = crate::blocks_v2::SearchQuery {
                text: Some(query.to_string()),
                limit: Some(max_results),
                ..Default::default()
            };
            
            match manager.search(search_query).await {
                Ok(blocks) => {
                    return blocks.into_iter().map(|block| HistoryEntry {
                        command: block.command.clone(),
                        exit_code: block.exit_code,
                        output: block.output.clone(),
                        working_dir: block.directory.clone(),
                        timestamp: block.created_at,
                        duration: block.duration_ms.map(Duration::from_millis),
                    }).collect();
                },
                Err(e) => {
                    error!("Search failed: {}", e);
                }
            }
        }

        #[cfg(not(feature = "blocks"))]
        {
            let query_lower = query.to_lowercase();
            let mut results: Vec<_> = self.simple_history.iter()
                .filter(|entry| entry.command.to_lowercase().contains(&query_lower))
                .cloned()
                .collect();
            results.reverse(); // Most recent first
            results.truncate(max_results);
            return results;
        }
        
        Vec::new()
    }

    /// Get recent command history
    pub async fn get_recent(&self, limit: usize) -> Vec<HistoryEntry> {
        #[cfg(feature = "blocks")]
        if let Some(ref block_manager) = self.block_manager {
            let manager = block_manager.lock().unwrap();
            let search_query = crate::blocks_v2::SearchQuery {
                limit: Some(limit),
                sort_by: crate::blocks_v2::SortField::CreatedAt,
                sort_order: crate::blocks_v2::SortOrder::Descending,
                ..Default::default()
            };
            
            match manager.search(search_query).await {
                Ok(blocks) => {
                    return blocks.into_iter().map(|block| HistoryEntry {
                        command: block.command.clone(),
                        exit_code: block.exit_code,
                        output: block.output.clone(),
                        working_dir: block.directory.clone(),
                        timestamp: block.created_at,
                        duration: block.duration_ms.map(Duration::from_millis),
                    }).collect();
                },
                Err(e) => {
                    error!("Failed to get recent commands: {}", e);
                }
            }
        }

        #[cfg(not(feature = "blocks"))]
        {
            let mut recent: Vec<_> = self.simple_history.iter().cloned().collect();
            recent.reverse(); // Most recent first
            recent.truncate(limit);
            return recent;
        }
        
        Vec::new()
    }

    /// Detect current shell type
    #[cfg(feature = "blocks")]
    fn detect_shell(&self) -> ShellType {
        if let Ok(shell) = std::env::var("SHELL") {
            if shell.contains("zsh") {
                ShellType::Zsh
            } else if shell.contains("bash") {
                ShellType::Bash
            } else if shell.contains("fish") {
                ShellType::Fish
            } else {
                ShellType::Bash // Default fallback
            }
        } else {
            ShellType::Bash
        }
    }

    /// Get the current active command if any
    pub fn get_current_command(&self) -> Option<&ActiveCommand> {
        self.current_command.as_ref()
    }

    /// Cancel the current command (if running)
    pub fn cancel_current_command(&mut self) {
        if let Some(active) = self.current_command.take() {
            debug!("Cancelling command: {}", active.command);
            
            #[cfg(feature = "blocks")]
            if let (Some(ref block_manager), Some(block_id)) = (&self.block_manager, active.block_id) {
                let _manager = block_manager.lock().unwrap();
                // Note: In a full implementation, we'd update the block status to Cancelled
                // For now, just log it
                info!("Cancelled block {}", block_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_command_history_basic() {
        let temp_dir = TempDir::new().unwrap();
        let mut history = CommandHistory::new(Some(temp_dir.path().to_path_buf())).await;
        
        // Start a command
        history.start_command("echo hello".to_string(), None).await.unwrap();
        
        // Complete it
        history.complete_command(0, "hello\n".to_string()).await.unwrap();
        
        // Search for it
        let results = history.search("echo", 10).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, "echo hello");
        assert_eq!(results[0].exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_command_history_recent() {
        let temp_dir = TempDir::new().unwrap();
        let mut history = CommandHistory::new(Some(temp_dir.path().to_path_buf())).await;
        
        // Add a few commands
        for i in 0..5 {
            history.start_command(format!("command{}", i), None).await.unwrap();
            history.complete_command(0, format!("output{}\n", i)).await.unwrap();
        }
        
        // Get recent
        let recent = history.get_recent(3).await;
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].command, "command4"); // Most recent first
    }
}

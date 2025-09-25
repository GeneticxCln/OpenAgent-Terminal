//! Command History Integration
//! Simple command history tracking without blocks dependency

use std::path::PathBuf;
use std::time::Duration;

use tracing::{debug, info};

/// Timestamp type for history entries
pub type HistoryTimestamp = std::time::SystemTime;

/// Simple command history manager
pub struct CommandHistory {
    // Simple history storage
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
    pub timestamp: HistoryTimestamp,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct ActiveCommand {
    pub command: String,
    pub working_dir: PathBuf,
    pub start_time: std::time::Instant,
}

impl CommandHistory {
    /// Create new command history manager
    pub async fn new(_data_dir: Option<PathBuf>) -> Self {
        info!("Command history initialized with simple storage");
        Self { 
            simple_history: Vec::new(), 
            current_command: None 
        }
    }

    /// Start tracking a new command
    pub async fn start_command(
        &mut self,
        command: String,
        working_dir: Option<PathBuf>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let working_dir = working_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));

        debug!("Starting command: {} in {}", command, working_dir.display());

        self.current_command = Some(ActiveCommand {
            command,
            working_dir,
            start_time: std::time::Instant::now(),
        });

        Ok(())
    }

    /// Complete the current command with output and exit code
    pub async fn complete_command(
        &mut self,
        exit_code: i32,
        output: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(active) = self.current_command.take() {
            let duration = active.start_time.elapsed();
            debug!("Completing command with exit_code={}, duration={:?}", exit_code, duration);

            let entry = HistoryEntry {
                command: active.command,
                exit_code: Some(exit_code),
                output,
                working_dir: active.working_dir,
                timestamp: std::time::SystemTime::now(),
                duration: Some(duration),
            };
            self.simple_history.push(entry);

            // Keep only last 1000 entries
            if self.simple_history.len() > 1000 {
                self.simple_history.remove(0);
            }
        }

        Ok(())
    }

    /// Search command history
    pub async fn search(&self, query: &str, max_results: usize) -> Vec<HistoryEntry> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<_> = self
            .simple_history
            .iter()
            .filter(|entry| entry.command.to_lowercase().contains(&query_lower))
            .cloned()
            .collect();
        results.reverse(); // Most recent first
        results.truncate(max_results);
        results
    }

    /// Get recent command history
    pub async fn get_recent(&self, limit: usize) -> Vec<HistoryEntry> {
        let mut recent: Vec<_> = self.simple_history.to_vec();
        recent.reverse(); // Most recent first
        recent.truncate(limit);
        recent
    }


    /// Get the current active command if any
    pub fn get_current_command(&self) -> Option<&ActiveCommand> {
        self.current_command.as_ref()
    }

    /// Cancel the current command (if running)
    pub fn cancel_current_command(&mut self) {
        if let Some(active) = self.current_command.take() {
            debug!("Cancelling command: {}", active.command);
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

//! Native command execution pipeline for OpenAgent Terminal
//!
//! This module provides real-time integration between command execution and block creation,
//! ensuring immediate updates without lazy loading or deferred processing.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
#[cfg(feature = "never")]
use chrono::{DateTime, Utc};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::security_lens::{SecurityLens, SecurityPolicy};
use crate::ui_confirm;

use crate::blocks_v2::{BlockId, BlockManager, ShellType};
use crate::workspace::{TabId, TabManager};
use openagent_terminal_core::event::CommandBlockEvent;

type CommandPipelineEventCallback = Box<dyn Fn(&CommandPipelineEvent) + Send + Sync>;

/// Native command execution pipeline
pub struct CommandPipeline {
    /// Block manager for immediate block operations
    block_manager: Option<Arc<tokio::sync::Mutex<BlockManager>>>,

    /// Tab manager for context tracking
    tab_manager: Option<Arc<tokio::sync::Mutex<TabManager>>>,

    /// Active command executions
    active_commands: HashMap<BlockId, CommandExecution>,

    /// Event sender for terminal integration
    event_sender: Option<mpsc::UnboundedSender<CommandBlockEvent>>,

    /// Native event callbacks
    event_callbacks: Vec<CommandPipelineEventCallback>,

    /// Real-time output streaming
    output_streams: HashMap<BlockId, mpsc::UnboundedSender<OutputChunk>>,
}

/// Command pipeline events for real-time processing
#[derive(Debug, Clone)]
pub enum CommandPipelineEvent {
    CommandStarted { block_id: BlockId, command: String, working_dir: PathBuf },
    OutputReceived { block_id: BlockId, output: String, is_stderr: bool },
    /// Finalized outputs aggregated at the end of the run (for indexing/diff)
    BlockOutputFinalized { block_id: BlockId, stdout: String, stderr: String },
    CommandCompleted { block_id: BlockId, exit_code: i32, duration: std::time::Duration },
    CommandFailed { block_id: BlockId, error: String },
    BlockCreated { block_id: BlockId, tab_id: Option<TabId> },
}

/// Active command execution state
#[derive(Debug)]
pub struct CommandExecution {
    pub block_id: BlockId,
    pub tab_id: Option<TabId>,
    pub process: Option<Child>,
    pub command: String,
    pub working_dir: PathBuf,
    pub shell: ShellType,
    pub start_time: Instant,
    pub output_buffer: Arc<tokio::sync::Mutex<String>>,
    pub error_buffer: Arc<tokio::sync::Mutex<String>>,
}

/// Output chunk for streaming
#[derive(Debug, Clone)]
pub struct OutputChunk {
    pub block_id: BlockId,
    pub content: String,
    pub is_stderr: bool,
    pub timestamp: OutputTimestamp,
}

#[cfg(feature = "never")]
type OutputTimestamp = DateTime<Utc>;
#[cfg(not(feature = "never"))]
type OutputTimestamp = std::time::SystemTime;

#[inline]
fn now_ts() -> OutputTimestamp {
    #[cfg(feature = "never")]
    {
        Utc::now()
    }
    #[cfg(not(feature = "never"))]
    {
        std::time::SystemTime::now()
    }
}

impl CommandPipeline {
    /// Create new native command pipeline
    pub fn new() -> Self {
        Self {
            block_manager: None,
            tab_manager: None,
            active_commands: HashMap::new(),
            event_sender: None,
            event_callbacks: Vec::new(),
            output_streams: HashMap::new(),
        }
    }

    /// Set block manager for immediate block operations
    pub fn set_block_manager(&mut self, block_manager: Arc<tokio::sync::Mutex<BlockManager>>) {
        self.block_manager = Some(block_manager);
    }


    /// Set tab manager for context tracking
    pub fn set_tab_manager(&mut self, tab_manager: Arc<tokio::sync::Mutex<TabManager>>) {
        self.tab_manager = Some(tab_manager);
    }

    /// Set terminal event sender
    pub fn set_event_sender(&mut self, sender: mpsc::UnboundedSender<CommandBlockEvent>) {
        self.event_sender = Some(sender);
    }

    /// Register event callback for real-time updates
    pub fn register_event_callback<F>(&mut self, callback: F)
    where
        F: Fn(&CommandPipelineEvent) + Send + Sync + 'static,
    {
        self.event_callbacks.push(Box::new(callback));
    }

    /// Emit pipeline event immediately
    fn emit_event(&self, event: CommandPipelineEvent) {
        for callback in &self.event_callbacks {
            callback(&event);
        }
    }

    /// Execute command with immediate block creation and real-time updates
    pub async fn execute_command(
        &mut self,
        command: String,
        working_dir: Option<PathBuf>,
        tab_id: Option<TabId>,
        shell: Option<ShellType>,
    ) -> Result<BlockId> {
        let working_dir = working_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
        let shell = shell.unwrap_or(ShellType::Bash);

        // Security risk analysis and optional confirmation (Warp-like behavior)
        let mut lens = SecurityLens::new(SecurityPolicy::with_defaults());
        let risk = lens.analyze_command(&command);
        if lens.should_block(&risk) {
            let title = format!("Security confirmation — {:?}", risk.level);
            let mut body = format!("{}\n\nCommand:\n{}\n", risk.explanation, command);
            if !risk.factors.is_empty() {
                body.push_str("\nRisk factors:\n");
                for f in &risk.factors {
                    body.push_str(&format!("- [{}] {}\n", f.category, f.description));
                }
            }
            if !risk.mitigations.is_empty() {
                body.push_str("\nMitigations:\n");
                for m in &risk.mitigations { body.push_str(&format!("- {}\n", m)); }
            }
            // Block until user confirms or cancels; if no proxy configured, this returns Err
            match ui_confirm::request_confirm(title, body, Some("Run anyway".into()), Some("Cancel".into()), None) {
                Ok(true) => { /* proceed */ }
                Ok(false) => { return Err(anyhow::anyhow!("Command execution cancelled by user")); }
                Err(e) => {
                    // If we cannot show confirmation, fail safe by cancelling
                    return Err(anyhow::anyhow!(format!("Confirmation unavailable: {}; refusing to run high-risk command", e)));
                }
            }
        }

        // Create block immediately with database integration - no lazy loading
        let block_id = if let Some(ref block_manager) = self.block_manager {
            let mut manager = block_manager.lock().await;
            
            // Collect current environment variables
            let mut environment = HashMap::new();
            for (key, value) in std::env::vars() {
                environment.insert(key, value);
            }
            
            let params = crate::blocks_v2::CreateBlockParams {
                command: command.clone(),
                directory: Some(working_dir.clone()),
                environment: Some(environment),
                shell: Some(shell.clone()),
                tags: None,
                parent_id: None,
                metadata: None,
            };

            let block = manager.create_block(params).await?;
            debug!("Created database block {} for command: {}", block.id, command);
            block.id
        } else {
            return Err(anyhow::anyhow!("Block manager not set - database integration required"));
        };

        // Emit immediate block creation event
        self.emit_event(CommandPipelineEvent::BlockCreated { block_id, tab_id });

        // Send terminal event immediately
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(CommandBlockEvent::CommandStart { cmd: Some(command.clone()) });
        }

        // Start command execution immediately
        self.start_native_execution(block_id, command, working_dir, tab_id, shell).await?;

        Ok(block_id)
    }

    /// Start native command execution without lazy fallbacks
    async fn start_native_execution(
        &mut self,
        block_id: BlockId,
        command: String,
        working_dir: PathBuf,
        tab_id: Option<TabId>,
        shell: ShellType,
    ) -> Result<()> {
        let start_time = Instant::now();

        // Create output buffers
        let output_buffer = Arc::new(tokio::sync::Mutex::new(String::new()));
        let error_buffer = Arc::new(tokio::sync::Mutex::new(String::new()));

        // Emit command started event immediately
        self.emit_event(CommandPipelineEvent::CommandStarted {
            block_id,
            command: command.clone(),
            working_dir: working_dir.clone(),
        });

        // Prepare shell command
        let shell_command = match shell {
            ShellType::Bash => vec!["bash", "-c", &command],
            ShellType::Zsh => vec!["zsh", "-c", &command],
            ShellType::Fish => vec!["fish", "-c", &command],
            ShellType::PowerShell => vec!["pwsh", "-c", &command],
            ShellType::Nushell => vec!["nu", "-c", &command],
            ShellType::Custom(_) => vec!["sh", "-c", &command], // Fallback to sh
        };

        // Start process immediately
        let mut child = Command::new(shell_command[0])
            .args(&shell_command[1..])
            .current_dir(&working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        // Store execution state
        let execution = CommandExecution {
            block_id,
            tab_id,
            process: Some(child),
            command: command.clone(),
            working_dir,
            shell,
            start_time,
            output_buffer: output_buffer.clone(),
            error_buffer: error_buffer.clone(),
        };

        self.active_commands.insert(block_id, execution);

        // Set up real-time output streaming
        let (output_tx, mut output_rx) = mpsc::unbounded_channel();
        self.output_streams.insert(block_id, output_tx.clone());

        // Clone references for async tasks - full database integration
        let block_manager = self.block_manager.clone();
        let _event_sender = self.event_sender.clone();
        let _pipeline_callbacks = self.event_callbacks.len(); // We can't clone the callbacks easily

        // Spawn stdout reader
        let stdout_output_buffer = output_buffer.clone();
        let stdout_output_tx = output_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            while let Ok(bytes_read) = reader.read_line(&mut line).await {
                if bytes_read == 0 {
                    break; // EOF
                }

                // Update buffer immediately
                {
                    let mut buffer = stdout_output_buffer.lock().await;
                    buffer.push_str(&line);
                }

                // Send output chunk immediately
                let chunk = OutputChunk {
                    block_id,
                    content: line.clone(),
                    is_stderr: false,
timestamp: now_ts(),
                };

                let _ = stdout_output_tx.send(chunk);
                line.clear();
            }
        });

        // Spawn stderr reader
        let stderr_error_buffer = error_buffer.clone();
        let stderr_output_tx = output_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();

            while let Ok(bytes_read) = reader.read_line(&mut line).await {
                if bytes_read == 0 {
                    break; // EOF
                }

                // Update buffer immediately
                {
                    let mut buffer = stderr_error_buffer.lock().await;
                    buffer.push_str(&line);
                }

                // Send output chunk immediately
                let chunk = OutputChunk {
                    block_id,
                    content: line.clone(),
                    is_stderr: true,
                    timestamp: now_ts(),
                };

                let _ = stderr_output_tx.send(chunk);
                line.clear();
            }
        });

        // Spawn output processor
        tokio::spawn(async move {
            while let Some(chunk) = output_rx.recv().await {
                // Process output chunk immediately with database updates - no lazy processing
                debug!("Received output chunk for block {:?}: {} bytes", chunk.block_id, chunk.content.len());

                // Update block with output immediately via database
                if let Some(ref manager) = block_manager {
                    let mut mgr = manager.lock().await;
                    if let Err(e) = mgr.append_output(chunk.block_id, &chunk.content).await {
                        tracing::warn!("Failed to append output to database for block {:?}: {}", chunk.block_id, e);
                    }
                }
            }
        });

        // Wait for process completion here by taking ownership of the child from active_commands
        // and then finalizing the block and emitting terminal events.
        if let Some(mut child_proc) =
            self.active_commands.get_mut(&block_id).and_then(|exec| exec.process.take())
        {
            let status = child_proc.wait().await?;
            let exit_code = status.code().unwrap_or(0);
            let duration = start_time.elapsed();
            // Finalize the block via helper (updates storage, emits terminal event & pipeline event)
            self.process_completion(block_id, exit_code, duration).await?;
        } else {
            info!(
                "CommandPipeline: no child process found for block {:?} when awaiting completion",
                block_id
            );
        }

        Ok(())
    }

    /// Get active command count
    pub fn active_command_count(&self) -> usize {
        self.active_commands.len()
    }

    /// Get command execution state
    pub fn get_command_execution(&self, block_id: BlockId) -> Option<&CommandExecution> {
        self.active_commands.get(&block_id)
    }

    /// Cancel command execution
    pub async fn cancel_command(&mut self, block_id: BlockId) -> Result<()> {
        if let Some(mut execution) = self.active_commands.remove(&block_id) {
            // Attempt to kill the running process if still alive
            if let Some(child) = execution.process.as_mut() {
                let _ = child.start_kill().ok();
            }

            // Update block status to cancelled immediately via database
            if let Some(ref block_manager) = self.block_manager {
                let mut mgr = block_manager.lock().await;
                if let Err(e) = mgr.mark_block_cancelled(block_id).await {
                    tracing::warn!("Failed to mark block {:?} as cancelled in database: {}", block_id, e);
                }
            }

            // Send terminal event indicating command ended without an exit code
            if let Some(ref sender) = self.event_sender {
                let _ = sender.send(CommandBlockEvent::CommandEnd {
                    exit: None,
                    cwd: Some(execution.working_dir.to_string_lossy().to_string()),
                });
            }

            info!("Cancelled command execution for block {:?}", block_id);
        }

        Ok(())
    }

    /// Process completed command
    async fn process_completion(
        &mut self,
        block_id: BlockId,
        exit_code: i32,
        duration: std::time::Duration,
    ) -> Result<()> {
        if let Some(execution) = self.active_commands.remove(&block_id) {
            // Gather finalized outputs
            let stdout_final = {
                let buffer = execution.output_buffer.lock().await;
                buffer.clone()
            };
            let stderr_final = {
                let buffer = execution.error_buffer.lock().await;
                buffer.clone()
            };

            // Emit finalized outputs for downstream consumers
            self.emit_event(CommandPipelineEvent::BlockOutputFinalized {
                block_id,
                stdout: stdout_final.clone(),
                stderr: stderr_final.clone(),
            });

            // Update block immediately with complete output via database - no lazy updates
            if let Some(ref block_manager) = self.block_manager {
                let mut manager = block_manager.lock().await;
                
                // Use the comprehensive update method that handles both stdout and stderr
                if let Err(e) = manager
                    .update_block_output_with_error(
                        block_id,
                        stdout_final.clone(),
                        stderr_final.clone(),
                        exit_code,
                        duration.as_millis() as u64,
                    )
                    .await
                {
                    tracing::error!("Failed to update block {:?} output in database: {}", block_id, e);
                    return Err(e.into());
                }
            }

            // Send terminal event immediately
            if let Some(ref sender) = self.event_sender {
                let _ = sender.send(CommandBlockEvent::CommandEnd {
                    exit: Some(exit_code),
                    cwd: Some(execution.working_dir.to_string_lossy().to_string()),
                });
            }

            // Emit completion event immediately
            self.emit_event(CommandPipelineEvent::CommandCompleted {
                block_id,
                exit_code,
                duration,
            });

            info!("Command completed for block {:?} with exit code {}", block_id, exit_code);
        }

        Ok(())
    }
}

impl Default for CommandPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_command_pipeline_creation() {
        let pipeline = CommandPipeline::new();
        assert_eq!(pipeline.active_command_count(), 0);
    }

    #[tokio::test]
    async fn test_command_execution_setup() {
        let pipeline = CommandPipeline::new();

        // This test would require setting up a full block manager
        // For now, we'll just verify the pipeline can be created
        assert_eq!(pipeline.active_command_count(), 0);
    }
}

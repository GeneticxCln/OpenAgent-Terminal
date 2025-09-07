// Enhanced Workflow Executor - Handles parallel execution with error aggregation and timeouts

use super::*;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use tokio::time::timeout;
// logging disabled: use tracing::{debug, error, info, warn};

/// Enhanced parallel executor with comprehensive error handling
pub struct EnhancedWorkflowExecutor {
    /// Maximum concurrent tasks
    max_concurrent: usize,
    /// Default step timeout
    default_timeout: Duration,
    /// Per-step output limits
    output_limits: OutputLimits,
}

/// Output truncation configuration
#[derive(Debug, Clone)]
pub struct OutputLimits {
    /// Maximum bytes per step stdout
    pub max_stdout_bytes: usize,
    /// Maximum bytes per step stderr  
    pub max_stderr_bytes: usize,
    /// Maximum log message length
    pub max_log_message_bytes: usize,
    /// Whether to truncate at word boundaries
    pub truncate_at_word_boundary: bool,
}

impl Default for OutputLimits {
    fn default() -> Self {
        Self {
            max_stdout_bytes: 64 * 1024,     // 64KB
            max_stderr_bytes: 64 * 1024,     // 64KB
            max_log_message_bytes: 4 * 1024, // 4KB
            truncate_at_word_boundary: true,
        }
    }
}

/// Result of parallel step execution
#[derive(Debug)]
pub struct ParallelExecutionResult {
    pub success_count: usize,
    pub failure_count: usize,
    pub timeout_count: usize,
    pub step_results: Vec<StepExecutionResult>,
    pub aggregated_errors: Vec<StepError>,
    pub execution_time: Duration,
}

/// Individual step execution result
#[derive(Debug)]
pub struct StepExecutionResult {
    pub step_id: String,
    pub step_name: String,
    pub success: bool,
    pub execution_time: Duration,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub error: Option<StepError>,
    pub timed_out: bool,
}

/// Categorized step error
#[derive(Debug, Clone)]
pub enum StepError {
    /// Command execution failed
    CommandFailed { command: String, exit_code: i32, stderr: String },
    /// Step timed out
    Timeout { duration: Duration, partial_output: String },
    /// Resource limit exceeded
    ResourceLimit { limit_type: String, limit_value: usize, actual_value: usize },
    /// Internal error
    Internal { message: String, source: String },
    /// Permission denied
    PermissionDenied { operation: String, resource: String },
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepError::CommandFailed { command, exit_code, stderr } => {
                write!(f, "Command '{}' failed with exit code {}: {}", command, exit_code, stderr)
            },
            StepError::Timeout { duration, partial_output } => {
                write!(f, "Step timed out after {:?}. Partial output: {}", duration, partial_output)
            },
            StepError::ResourceLimit { limit_type, limit_value, actual_value } => {
                write!(
                    f,
                    "Resource limit exceeded: {} limit {} < actual {}",
                    limit_type, limit_value, actual_value
                )
            },
            StepError::Internal { message, source } => {
                write!(f, "Internal error: {} (source: {})", message, source)
            },
            StepError::PermissionDenied { operation, resource } => {
                write!(
                    f,
                    "Permission denied for operation '{}' on resource '{}'",
                    operation, resource
                )
            },
        }
    }
}

impl Default for EnhancedWorkflowExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl EnhancedWorkflowExecutor {
    pub fn new() -> Self {
        Self {
            max_concurrent: 10,
            default_timeout: Duration::from_secs(300), // 5 minutes
            output_limits: OutputLimits::default(),
        }
    }

    pub fn with_limits(mut self, output_limits: OutputLimits) -> Self {
        self.output_limits = output_limits;
        self
    }

    pub fn with_max_concurrent(mut self, max_concurrent: usize) -> Self {
        self.max_concurrent = max_concurrent;
        self
    }

    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Execute steps in parallel with comprehensive error handling
    pub async fn execute_parallel_steps(
        &self,
        steps: Vec<WorkflowStep>,
        execution_context: ExecutionContext,
    ) -> Result<ParallelExecutionResult> {
        let start_time = Instant::now();
        let mut tasks = JoinSet::new();

        // info!("Starting parallel execution of {} steps", steps.len());

        // Limit concurrent tasks to prevent resource exhaustion
        let batches = steps.chunks(self.max_concurrent);
        let mut all_results = Vec::new();

        for batch in batches {
            // Execute batch of steps concurrently
            for step in batch {
                let step_clone = step.clone();
                let context_clone = execution_context.clone();
                let limits = self.output_limits.clone();
                let step_timeout = self.parse_step_timeout(&step_clone);

                tasks.spawn(async move {
                    Self::execute_single_step(step_clone, context_clone, limits, step_timeout).await
                });
            }

            // Collect results from this batch
            let mut batch_results = Vec::new();
            while batch_results.len() < batch.len() {
                match tasks.join_next().await {
                    Some(Ok(result)) => batch_results.push(result),
                    Some(Err(e)) => {
                        // error!("Task join error: {}", e);
                        batch_results.push(StepExecutionResult {
                            step_id: "unknown".to_string(),
                            step_name: "unknown".to_string(),
                            success: false,
                            execution_time: Duration::ZERO,
                            stdout_bytes: 0,
                            stderr_bytes: 0,
                            error: Some(StepError::Internal {
                                message: e.to_string(),
                                source: "task_join".to_string(),
                            }),
                            timed_out: false,
                        });
                    },
                    None => break,
                }
            }

            all_results.extend(batch_results);
        }

        // Aggregate results
        let execution_time = start_time.elapsed();
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut timeout_count = 0;
        let mut aggregated_errors = Vec::new();

        for result in &all_results {
            if result.success {
                success_count += 1;
            } else {
                failure_count += 1;
                if result.timed_out {
                    timeout_count += 1;
                }
                if let Some(ref error) = result.error {
                    aggregated_errors.push(error.clone());
                }
            }
        }

        // info!(
        //     "Parallel execution completed: {} successful, {} failed, {} timed out in {:?}",
        //     success_count, failure_count, timeout_count, execution_time
        // );

        Ok(ParallelExecutionResult {
            success_count,
            failure_count,
            timeout_count,
            step_results: all_results,
            aggregated_errors,
            execution_time,
        })
    }

    /// Execute a single step with timeout and output limits
    async fn execute_single_step(
        step: WorkflowStep,
        context: ExecutionContext,
        limits: OutputLimits,
        step_timeout: Duration,
    ) -> StepExecutionResult {
        let start_time = Instant::now();

        // debug!("Executing step '{}' with timeout {:?}", step.name, step_timeout);

        // Apply per-step timeout
        let execution_result = timeout(step_timeout, async {
            Self::run_step_commands(&step, &context, &limits).await
        })
        .await;

        let execution_time = start_time.elapsed();

        match execution_result {
            Ok(Ok((stdout_bytes, stderr_bytes))) => {
                // debug!("Step '{}' completed successfully in {:?}", step.name, execution_time);
                StepExecutionResult {
                    step_id: step.id,
                    step_name: step.name,
                    success: true,
                    execution_time,
                    stdout_bytes,
                    stderr_bytes,
                    error: None,
                    timed_out: false,
                }
            },
            Ok(Err(error)) => {
                // warn!("Step '{}' failed in {:?}: {}", step.name, execution_time, error);
                StepExecutionResult {
                    step_id: step.id,
                    step_name: step.name,
                    success: false,
                    execution_time,
                    stdout_bytes: 0,
                    stderr_bytes: 0,
                    error: Some(error),
                    timed_out: false,
                }
            },
            Err(_timeout_err) => {
                // warn!("Step '{}' timed out after {:?}", step.name, step_timeout);
                StepExecutionResult {
                    step_id: step.id,
                    step_name: step.name,
                    success: false,
                    execution_time,
                    stdout_bytes: 0,
                    stderr_bytes: 0,
                    error: Some(StepError::Timeout {
                        duration: step_timeout,
                        partial_output: "[truncated due to timeout]".to_string(),
                    }),
                    timed_out: true,
                }
            },
        }
    }

    /// Run commands for a step with output limits
    async fn run_step_commands(
        step: &WorkflowStep,
        _context: &ExecutionContext,
        limits: &OutputLimits,
    ) -> Result<(usize, usize), StepError> {
        let mut total_stdout = 0;
        let mut total_stderr = 0;

        for command in &step.commands {
            // This would integrate with the actual command execution
            // For now, simulate command execution
            let simulated_stdout = format!("Executing: {}\n", command).into_bytes();
            let simulated_stderr: Vec<u8> = Vec::new();

            // Apply output limits
            let stdout_size = simulated_stdout.len().min(limits.max_stdout_bytes);
            let stderr_size = simulated_stderr.len().min(limits.max_stderr_bytes);

            total_stdout += stdout_size;
            total_stderr += stderr_size;

            // Check if we've exceeded total limits
            if total_stdout > limits.max_stdout_bytes {
                return Err(StepError::ResourceLimit {
                    limit_type: "stdout_bytes".to_string(),
                    limit_value: limits.max_stdout_bytes,
                    actual_value: total_stdout,
                });
            }

            if total_stderr > limits.max_stderr_bytes {
                return Err(StepError::ResourceLimit {
                    limit_type: "stderr_bytes".to_string(),
                    limit_value: limits.max_stderr_bytes,
                    actual_value: total_stderr,
                });
            }
        }

        Ok((total_stdout, total_stderr))
    }

    /// Parse step timeout from step definition
    fn parse_step_timeout(&self, step: &WorkflowStep) -> Duration {
        if let Some(timeout_str) = &step.timeout {
            parse_duration(timeout_str).unwrap_or(self.default_timeout)
        } else {
            self.default_timeout
        }
    }

    /// Truncate text with optional word boundary preservation
    pub fn truncate_text(&self, text: &str, max_bytes: usize) -> String {
        if text.len() <= max_bytes {
            return text.to_string();
        }

        if self.output_limits.truncate_at_word_boundary {
            // Find the last word boundary before the limit
            let truncate_point =
                text[..max_bytes].rfind(|c: char| c.is_whitespace()).unwrap_or(max_bytes);
            format!(
                "{}\n[truncated: {}/{} bytes]",
                &text[..truncate_point],
                truncate_point,
                text.len()
            )
        } else {
            format!("{}\n[truncated: {}/{} bytes]", &text[..max_bytes], max_bytes, text.len())
        }
    }
}

/// Execution context for steps
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub execution_id: String,
    pub environment: HashMap<String, String>,
    pub secrets: Option<Vec<Secret>>,
    pub working_directory: Option<String>,
}

/// Legacy workflow executor for backwards compatibility
pub struct WorkflowExecutor;

impl Default for WorkflowExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowExecutor {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute_parallel_steps(
        &self,
        steps: Vec<WorkflowStep>,
    ) -> Result<Vec<Result<()>>> {
        let enhanced = EnhancedWorkflowExecutor::new();
        let context = ExecutionContext {
            execution_id: "legacy".to_string(),
            environment: HashMap::new(),
            secrets: None,
            working_directory: None,
        };

        let result = enhanced.execute_parallel_steps(steps, context).await?;

        // Convert to legacy format
        Ok(result
            .step_results
            .into_iter()
            .map(|r| if r.success { Ok(()) } else { Err(anyhow!("Step failed: {:?}", r.error)) })
            .collect())
    }
}

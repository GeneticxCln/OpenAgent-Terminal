//! Workflow orchestration engine for AI agent coordination.
//! Provides baseline execution graph with sequential execution and hooks for parallel/dependency execution.

use crate::agents::types::{
    WorkflowExecutionGraph, WorkflowNode, WorkflowEdge, ExecutionStrategy, WorkflowStatus,
    NodeStatus, NodeType, JoinStrategy, ExecutionCondition, AgentExecutionContext, ConcurrencyState
};
use crate::agents::{AiAgent, AgentRequest, AgentResponse};

use anyhow::{Result, anyhow, Context};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use futures_util::future;
use tokio::time::{timeout, Duration};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::{info, warn, error, debug, instrument};

/// Main workflow orchestrator for managing agent execution graphs
pub struct WorkflowOrchestrator {
    /// Active workflows being executed
    active_workflows: Arc<RwLock<HashMap<Uuid, ActiveWorkflowExecution>>>,
    /// Registered agents available for execution
    agents: Arc<RwLock<HashMap<String, Arc<dyn AiAgent>>>>,
    /// Concurrency control
    concurrency_state: ConcurrencyState,
    /// Event channel for workflow notifications
    event_sender: broadcast::Sender<WorkflowEvent>,
    /// Configuration
    config: OrchestratorConfig,
}

/// Active workflow execution state
#[derive(Debug)]
struct ActiveWorkflowExecution {
    graph: WorkflowExecutionGraph,
    context: AgentExecutionContext,
    execution_state: ExecutionState,
    results: HashMap<String, NodeExecutionResult>,
    error_log: Vec<ExecutionError>,
    metrics: ExecutionMetrics,
}

/// Execution state tracking
#[derive(Debug, Clone)]
struct ExecutionState {
    current_phase: ExecutionPhase,
    ready_nodes: VecDeque<String>,
    running_nodes: HashSet<String>,
    completed_nodes: HashSet<String>,
    failed_nodes: HashSet<String>,
    dependency_graph: DependencyGraph,
}

/// Execution phases
#[derive(Debug, Clone, PartialEq)]
enum ExecutionPhase {
    Planning,
    Executing,
    Completing,
    Failed,
    Cancelled,
}

/// Dependency graph for topological sorting
#[derive(Debug, Clone)]
struct DependencyGraph {
    /// Node ID -> Set of nodes it depends on
    dependencies: HashMap<String, HashSet<String>>,
    /// Node ID -> Set of nodes that depend on it
    dependents: HashMap<String, HashSet<String>>,
    /// Topological ordering cache
    topo_order: Option<Vec<String>>,
}

/// Result of node execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionResult {
    node_id: String,
    status: NodeStatus,
    output: Option<serde_json::Value>,
    error: Option<String>,
    start_time: DateTime<Utc>,
    end_time: Option<DateTime<Utc>>,
    execution_time_ms: Option<u64>,
    agent_id: Option<String>,
    retry_count: u32,
}

/// Execution error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionError {
    node_id: String,
    error_type: ErrorType,
    message: String,
    timestamp: DateTime<Utc>,
    recoverable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum ErrorType {
    AgentNotFound,
    AgentFailure,
    Timeout,
    DependencyFailure,
    CycleDetected,
    ValidationError,
    ResourceExhausted,
}

/// Execution metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    total_nodes: usize,
    completed_nodes: usize,
    failed_nodes: usize,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    peak_concurrent_nodes: usize,
    total_retries: u32,
}

/// Workflow execution events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowEvent {
    WorkflowStarted {
        workflow_id: Uuid,
        name: String,
    },
    NodeStarted {
        workflow_id: Uuid,
        node_id: String,
        agent_id: Option<String>,
    },
    NodeCompleted {
        workflow_id: Uuid,
        node_id: String,
        status: NodeStatus,
        duration_ms: u64,
    },
    NodeFailed {
        workflow_id: Uuid,
        node_id: String,
        error: String,
        retry_count: u32,
    },
    WorkflowCompleted {
        workflow_id: Uuid,
        status: WorkflowStatus,
        duration_ms: u64,
        metrics: ExecutionMetrics,
    },
    CycleDetected {
        workflow_id: Uuid,
        nodes_in_cycle: Vec<String>,
    },
}

/// Orchestrator configuration
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub max_concurrent_workflows: usize,
    pub max_concurrent_nodes_per_workflow: usize,
    pub default_node_timeout_ms: u64,
    pub max_retries: u32,
    pub enable_cycle_detection: bool,
    pub enable_metrics: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_workflows: 10,
            max_concurrent_nodes_per_workflow: 5,
            default_node_timeout_ms: 30_000,
            max_retries: 3,
            enable_cycle_detection: true,
            enable_metrics: true,
        }
    }
}

impl WorkflowOrchestrator {
    /// Create a new workflow orchestrator
    pub fn new(config: OrchestratorConfig) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        
        Self {
            active_workflows: Arc::new(RwLock::new(HashMap::new())),
            agents: Arc::new(RwLock::new(HashMap::new())),
            concurrency_state: ConcurrencyState::default(),
            event_sender,
            config,
        }
    }

    /// Register an agent for workflow execution
    pub async fn register_agent(&self, agent: Arc<dyn AiAgent>) -> Result<()> {
        let agent_id = agent.name().to_string();
        let mut agents = self.agents.write().await;
        agents.insert(agent_id, agent);
        info!("Registered agent for workflow execution");
        Ok(())
    }

    /// Execute a workflow graph
    #[instrument(skip(self, graph, context))]
    pub async fn execute_workflow(
        &self,
        graph: WorkflowExecutionGraph,
        context: AgentExecutionContext,
    ) -> Result<WorkflowExecutionResult> {
        let workflow_id = graph.id;
        info!(%workflow_id, "Starting workflow execution");

        // Validate workflow graph
        self.validate_workflow(&graph).await?;

        // Create execution state
        let execution_state = self.create_execution_state(&graph).await?;
        
        let active_execution = ActiveWorkflowExecution {
            graph: graph.clone(),
            context,
            execution_state,
            results: HashMap::new(),
            error_log: Vec::new(),
            metrics: ExecutionMetrics {
                total_nodes: graph.nodes.len(),
                start_time: Some(Utc::now()),
                ..Default::default()
            },
        };

        // Add to active workflows
        {
            let mut workflows = self.active_workflows.write().await;
            workflows.insert(workflow_id, active_execution);
        }

        // Send start event
        let _ = self.event_sender.send(WorkflowEvent::WorkflowStarted {
            workflow_id,
            name: graph.name.clone(),
        });

        // Execute based on strategy
        let result = match graph.execution_strategy {
            ExecutionStrategy::Sequential => {
                self.execute_sequential(workflow_id).await
            }
            ExecutionStrategy::Parallel { max_concurrency } => {
                self.execute_parallel(workflow_id, max_concurrency).await
            }
            ExecutionStrategy::Hybrid => {
                self.execute_hybrid(workflow_id).await
            }
            ExecutionStrategy::Custom { executor_name } => {
                self.execute_custom(workflow_id, &executor_name).await
            }
        };

        // Cleanup and finalize
        let final_result = self.finalize_workflow(workflow_id, result).await?;
        
        info!(%workflow_id, status = ?final_result.status, "Workflow execution completed");
        Ok(final_result)
    }

    /// Validate workflow graph for cycles and consistency
    async fn validate_workflow(&self, graph: &WorkflowExecutionGraph) -> Result<()> {
        if graph.nodes.is_empty() {
            return Err(anyhow!("Workflow graph cannot be empty"));
        }

        // Check for cycle detection if enabled
        if self.config.enable_cycle_detection {
            let dep_graph = self.build_dependency_graph(&graph.nodes, &graph.edges)?;
            if let Some(cycle) = self.detect_cycles(&dep_graph) {
                let _ = self.event_sender.send(WorkflowEvent::CycleDetected {
                    workflow_id: graph.id,
                    nodes_in_cycle: cycle.clone(),
                });
                return Err(anyhow!("Cycle detected in workflow graph: {:?}", cycle));
            }
        }

        // Validate node references in edges
        for edge in &graph.edges {
            if !graph.nodes.contains_key(&edge.from) {
                return Err(anyhow!("Edge references unknown node: {}", edge.from));
            }
            if !graph.nodes.contains_key(&edge.to) {
                return Err(anyhow!("Edge references unknown node: {}", edge.to));
            }
        }

        // Validate agent availability for task nodes
        let agents = self.agents.read().await;
        for node in graph.nodes.values() {
            if let NodeType::Task { agent_capability, .. } = &node.node_type {
                // Check if any registered agent can handle this capability
                let has_capable_agent = agents.values().any(|agent| {
                    agent.capabilities().features.iter().any(|f| f == agent_capability)
                });
                
                if !has_capable_agent {
                    warn!("No agent available for capability: {}", agent_capability);
                }
            }
        }

        Ok(())
    }

    /// Build dependency graph for topological sorting
    fn build_dependency_graph(
        &self,
        nodes: &HashMap<String, WorkflowNode>,
        edges: &[WorkflowEdge],
    ) -> Result<DependencyGraph> {
        let mut dependencies = HashMap::new();
        let mut dependents = HashMap::new();

        // Initialize with all nodes
        for node_id in nodes.keys() {
            dependencies.insert(node_id.clone(), HashSet::new());
            dependents.insert(node_id.clone(), HashSet::new());
        }

        // Build dependency relationships from edges
        for edge in edges {
            dependencies.entry(edge.to.clone())
                .or_insert_with(HashSet::new)
                .insert(edge.from.clone());
            
            dependents.entry(edge.from.clone())
                .or_insert_with(HashSet::new)
                .insert(edge.to.clone());
        }

        // Add explicit dependencies from nodes
        for node in nodes.values() {
            for dep in &node.dependencies {
                dependencies.entry(node.id.clone())
                    .or_insert_with(HashSet::new)
                    .insert(dep.clone());
                
                dependents.entry(dep.clone())
                    .or_insert_with(HashSet::new)
                    .insert(node.id.clone());
            }
        }

        Ok(DependencyGraph {
            dependencies,
            dependents,
            topo_order: None,
        })
    }

    /// Detect cycles in dependency graph using DFS
    fn detect_cycles(&self, graph: &DependencyGraph) -> Option<Vec<String>> {
        let mut white = graph.dependencies.keys().cloned().collect::<HashSet<_>>();
        let mut gray = HashSet::new();
        let mut black = HashSet::new();

        for node in graph.dependencies.keys() {
            if white.contains(node) {
                if let Some(cycle) = self.dfs_cycle_detection(
                    node,
                    &graph.dependencies,
                    &mut white,
                    &mut gray,
                    &mut black,
                    &mut Vec::new(),
                ) {
                    return Some(cycle);
                }
            }
        }

        None
    }

    /// DFS helper for cycle detection
    fn dfs_cycle_detection(
        &self,
        node: &str,
        graph: &HashMap<String, HashSet<String>>,
        white: &mut HashSet<String>,
        gray: &mut HashSet<String>,
        black: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        white.remove(node);
        gray.insert(node.to_string());
        path.push(node.to_string());

        if let Some(deps) = graph.get(node) {
            for dep in deps {
                if black.contains(dep) {
                    continue;
                }
                
                if gray.contains(dep) {
                    // Found cycle
                    let cycle_start = path.iter().position(|n| n == dep).unwrap();
                    return Some(path[cycle_start..].to_vec());
                }

                if let Some(cycle) = self.dfs_cycle_detection(
                    dep, graph, white, gray, black, path
                ) {
                    return Some(cycle);
                }
            }
        }

        gray.remove(node);
        black.insert(node.to_string());
        path.pop();
        None
    }

    /// Create initial execution state
    async fn create_execution_state(
        &self,
        graph: &WorkflowExecutionGraph,
    ) -> Result<ExecutionState> {
        let dependency_graph = self.build_dependency_graph(&graph.nodes, &graph.edges)?;
        
        // Find nodes with no dependencies (ready to execute)
        let ready_nodes: VecDeque<String> = graph.nodes
            .iter()
            .filter(|(_, node)| {
                dependency_graph.dependencies
                    .get(&node.id)
                    .map(|deps| deps.is_empty())
                    .unwrap_or(true)
            })
            .map(|(id, _)| id.clone())
            .collect();

        Ok(ExecutionState {
            current_phase: ExecutionPhase::Planning,
            ready_nodes,
            running_nodes: HashSet::new(),
            completed_nodes: HashSet::new(),
            failed_nodes: HashSet::new(),
            dependency_graph,
        })
    }

    /// Execute workflow sequentially
    #[instrument(skip(self), fields(workflow_id = %workflow_id))]
    async fn execute_sequential(&self, workflow_id: Uuid) -> Result<WorkflowStatus> {
        debug!("Starting sequential execution");

        let mut execution_complete = false;
        
        while !execution_complete {
            let next_node = {
                let mut workflows = self.active_workflows.write().await;
                let workflow = workflows.get_mut(&workflow_id)
                    .ok_or_else(|| anyhow!("Workflow not found"))?;
                
                workflow.execution_state.current_phase = ExecutionPhase::Executing;
                workflow.execution_state.ready_nodes.pop_front()
            };

            match next_node {
                Some(node_id) => {
                    debug!(%node_id, "Executing node sequentially");
                    
                    match self.execute_node(workflow_id, &node_id).await {
                        Ok(_) => {
                            self.update_dependencies_on_completion(workflow_id, &node_id).await?;
                        }
                        Err(e) => {
                            error!(%node_id, error = %e, "Node execution failed");
                            return Ok(WorkflowStatus::Failed);
                        }
                    }
                }
                None => {
                    // Check if workflow is complete
                    let workflows = self.active_workflows.read().await;
                    let workflow = workflows.get(&workflow_id)
                        .ok_or_else(|| anyhow!("Workflow not found"))?;
                    
                    execution_complete = workflow.execution_state.ready_nodes.is_empty() 
                        && workflow.execution_state.running_nodes.is_empty();
                    
                    if !execution_complete && 
                       workflow.execution_state.failed_nodes.is_empty() {
                        // Possible deadlock or dependency issue
                        warn!("No ready nodes but workflow not complete");
                        return Ok(WorkflowStatus::Failed);
                    }
                }
            }
        }

        Ok(WorkflowStatus::Completed)
    }

    /// Execute workflow with parallel node execution
    #[instrument(skip(self), fields(workflow_id = %workflow_id, max_concurrency = max_concurrency))]
    async fn execute_parallel(&self, workflow_id: Uuid, max_concurrency: usize) -> Result<WorkflowStatus> {
        debug!("Starting parallel execution");

        let mut execution_complete = false;

        {
            let mut workflows = self.active_workflows.write().await;
            let workflow = workflows.get_mut(&workflow_id)
                .ok_or_else(|| anyhow!("Workflow not found"))?;
            workflow.execution_state.current_phase = ExecutionPhase::Executing;
        }

        while !execution_complete {
            // Start ready nodes up to concurrency limit
            let ready_nodes = {
                let mut workflows = self.active_workflows.write().await;
                let workflow = workflows.get_mut(&workflow_id)
                    .ok_or_else(|| anyhow!("Workflow not found"))?;
                
                let mut nodes = Vec::new();
                while let Some(node_id) = workflow.execution_state.ready_nodes.pop_front() {
                    if workflow.execution_state.running_nodes.len() < max_concurrency {
                        workflow.execution_state.running_nodes.insert(node_id.clone());
                        nodes.push(node_id);
                    } else {
                        workflow.execution_state.ready_nodes.push_front(node_id);
                        break;
                    }
                }
                nodes
            };

            // Execute ready nodes concurrently without spawning 'static futures
            if !ready_nodes.is_empty() {
                let futures = ready_nodes.into_iter().map(|node_id| async move {
                    debug!(%node_id, "Starting parallel node execution");
                    let res = self.execute_node(workflow_id, &node_id).await.map(|_| node_id.clone());
                    if res.is_ok() {
                        let _ = self.update_dependencies_on_completion(workflow_id, &node_id).await;
                    }
                    res
                });

                let results = future::join_all(futures).await;
                for result in results {
                    match result {
                        Ok(node_id) => {
                            debug!(%node_id, "Node completed successfully");
                            let mut workflows = self.active_workflows.write().await;
                            let workflow = workflows.get_mut(&workflow_id)
                                .ok_or_else(|| anyhow!("Workflow not found"))?;
                            workflow.execution_state.running_nodes.remove(&node_id);
                        }
                        Err(e) => {
                            error!("Node execution failed: {}", e);
                            let mut workflows = self.active_workflows.write().await;
                            let workflow = workflows.get_mut(&workflow_id)
                                .ok_or_else(|| anyhow!("Workflow not found"))?;
                            // We cannot know node_id here; leave running_nodes cleanup to dependency update
                            workflow.execution_state.failed_nodes.insert("unknown".to_string());
                            return Ok(WorkflowStatus::Failed);
                        }
                    }
                }
            }

            // Check completion
            let workflows = self.active_workflows.read().await;
            let workflow = workflows.get(&workflow_id)
                .ok_or_else(|| anyhow!("Workflow not found"))?;
            
            execution_complete = workflow.execution_state.ready_nodes.is_empty() 
                && workflow.execution_state.running_nodes.is_empty();
        }

        // No spawned tasks to wait for since we used join_all

        Ok(WorkflowStatus::Completed)
    }

    /// Execute workflow with hybrid strategy
    async fn execute_hybrid(&self, workflow_id: Uuid) -> Result<WorkflowStatus> {
        // For now, use parallel execution as hybrid implementation
        // TODO: Implement true hybrid logic with parallel groups
        self.execute_parallel(workflow_id, self.config.max_concurrent_nodes_per_workflow).await
    }

    /// Execute workflow with custom strategy
    async fn execute_custom(&self, _workflow_id: Uuid, executor_name: &str) -> Result<WorkflowStatus> {
        warn!("Custom executor not implemented: {}", executor_name);
        Err(anyhow!("Custom executors not yet supported"))
    }

    /// Execute a single node
    #[instrument(skip(self), fields(workflow_id = %workflow_id, node_id = %node_id))]
    async fn execute_node(&self, workflow_id: Uuid, node_id: &str) -> Result<()> {
        let start_time = Utc::now();
        
        // Get node information
        let (node, context, timeout_ms) = {
            let workflows = self.active_workflows.read().await;
            let workflow = workflows.get(&workflow_id)
                .ok_or_else(|| anyhow!("Workflow not found"))?;
            
            let node = workflow.graph.nodes.get(node_id)
                .ok_or_else(|| anyhow!("Node not found: {}", node_id))?;
            
            let timeout_ms = node.timeout_ms
                .unwrap_or(self.config.default_node_timeout_ms);
                
            (node.clone(), workflow.context.clone(), timeout_ms)
        };

        // Send start event
        let _ = self.event_sender.send(WorkflowEvent::NodeStarted {
            workflow_id,
            node_id: node_id.to_string(),
            agent_id: node.agent_id.clone(),
        });

        // Execute based on node type
        let execution_result = match &node.node_type {
            NodeType::Task { agent_capability, payload } => {
                self.execute_task_node(node_id, agent_capability, payload, &context, timeout_ms).await
            }
            NodeType::Decision { condition_expr, .. } => {
                self.execute_decision_node(node_id, condition_expr, &context).await
            }
            NodeType::ParallelGroup { nodes, join_strategy } => {
                self.execute_parallel_group_node(workflow_id, node_id, nodes, join_strategy).await
            }
            NodeType::Barrier { wait_for } => {
                self.execute_barrier_node(workflow_id, node_id, wait_for).await
            }
            NodeType::Start | NodeType::End => {
                Ok(serde_json::json!({"status": "completed"}))
            }
        };

        let end_time = Utc::now();
        let duration_ms = (end_time - start_time).num_milliseconds() as u64;

        // Record result
        let result = match execution_result {
            Ok(output) => {
                let _ = self.event_sender.send(WorkflowEvent::NodeCompleted {
                    workflow_id,
                    node_id: node_id.to_string(),
                    status: NodeStatus::Completed,
                    duration_ms,
                });

                NodeExecutionResult {
                    node_id: node_id.to_string(),
                    status: NodeStatus::Completed,
                    output: Some(output),
                    error: None,
                    start_time,
                    end_time: Some(end_time),
                    execution_time_ms: Some(duration_ms),
                    agent_id: node.agent_id.clone(),
                    retry_count: node.retry_count,
                }
            }
            Err(e) => {
                let _ = self.event_sender.send(WorkflowEvent::NodeFailed {
                    workflow_id,
                    node_id: node_id.to_string(),
                    error: e.to_string(),
                    retry_count: node.retry_count,
                });

                NodeExecutionResult {
                    node_id: node_id.to_string(),
                    status: NodeStatus::Failed,
                    output: None,
                    error: Some(e.to_string()),
                    start_time,
                    end_time: Some(end_time),
                    execution_time_ms: Some(duration_ms),
                    agent_id: node.agent_id.clone(),
                    retry_count: node.retry_count,
                }
            }
        };

        // Store result
        {
            let mut workflows = self.active_workflows.write().await;
            let workflow = workflows.get_mut(&workflow_id)
                .ok_or_else(|| anyhow!("Workflow not found"))?;
            workflow.results.insert(node_id.to_string(), result.clone());
        }

        if result.status == NodeStatus::Failed {
            return Err(anyhow!("Node execution failed: {}", result.error.unwrap_or_default()));
        }

        Ok(())
    }

    /// Execute a task node with an agent
    async fn execute_task_node(
        &self,
        node_id: &str,
        agent_capability: &str,
        payload: &serde_json::Value,
        _context: &AgentExecutionContext,
        timeout_ms: u64,
    ) -> Result<serde_json::Value> {
        // Find suitable agent
        let agent = {
            let agents = self.agents.read().await;
            agents.values()
                .find(|agent| agent.capabilities().features.iter().any(|f| f == agent_capability))
                .cloned()
        };

        let agent = agent.ok_or_else(|| {
            anyhow!("No agent available for capability: {}", agent_capability)
        })?;

        // Create agent request
        let request = AgentRequest::Command(crate::AiRequest {
            scratch_text: payload.to_string(),
            working_directory: None,
            shell_kind: None,
            context: Vec::new(),
        });

        // Execute with timeout
        let response = timeout(
            Duration::from_millis(timeout_ms),
            agent.process(request)
        ).await
        .with_context(|| format!("Node {} timed out after {}ms", node_id, timeout_ms))?
        .with_context(|| format!("Agent execution failed for node {}", node_id))?;

        // Convert response to JSON
        match response {
            AgentResponse::Commands(proposals) => {
                Ok(serde_json::json!({
                    "type": "commands",
                    "proposals": proposals
                }))
            }
            AgentResponse::Code { generated_code, language, explanation, suggestions } => {
                Ok(serde_json::json!({
                    "type": "code",
                    "generated_code": generated_code,
                    "language": language,
                    "explanation": explanation,
                    "suggestions": suggestions
                }))
            }
            AgentResponse::Context { project_info, suggestions } => {
                Ok(serde_json::json!({
                    "type": "context",
                    "project_info": project_info,
                    "suggestions": suggestions
                }))
            }
            AgentResponse::QualityReport { score, issues, suggestions, security_warnings } => {
                Ok(serde_json::json!({
                    "type": "quality_report",
                    "score": score,
                    "issues": issues,
                    "suggestions": suggestions,
                    "security_warnings": security_warnings
                }))
            }
            AgentResponse::CollaborationResult { participating_agents, result, confidence } => {
                Ok(serde_json::json!({
                    "type": "collaboration",
                    "participating_agents": participating_agents,
                    "result": result,
                    "confidence": confidence
                }))
            }
        }
    }

    /// Execute a decision node
    async fn execute_decision_node(
        &self,
        _node_id: &str,
        condition_expr: &str,
        _context: &AgentExecutionContext,
    ) -> Result<serde_json::Value> {
        // Simple expression evaluation (placeholder)
        // TODO: Implement proper expression evaluation
        let result = condition_expr == "true";
        Ok(serde_json::json!({"decision": result}))
    }

    /// Execute a parallel group node
    async fn execute_parallel_group_node(
        &self,
        _workflow_id: Uuid,
        _node_id: &str,
        _nodes: &[String],
        _join_strategy: &JoinStrategy,
    ) -> Result<serde_json::Value> {
        // TODO: Implement parallel group execution
        Ok(serde_json::json!({"status": "parallel_group_completed"}))
    }

    /// Execute a barrier node
    async fn execute_barrier_node(
        &self,
        workflow_id: Uuid,
        _node_id: &str,
        wait_for: &[String],
    ) -> Result<serde_json::Value> {
        // Check if all waited-for nodes are completed
        let workflows = self.active_workflows.read().await;
        let workflow = workflows.get(&workflow_id)
            .ok_or_else(|| anyhow!("Workflow not found"))?;

        for waited_node in wait_for {
            if !workflow.execution_state.completed_nodes.contains(waited_node) {
                return Err(anyhow!("Barrier waiting for incomplete node: {}", waited_node));
            }
        }

        Ok(serde_json::json!({"status": "barrier_passed"}))
    }

    /// Update dependencies when a node completes
    async fn update_dependencies_on_completion(
        &self,
        workflow_id: Uuid,
        completed_node_id: &str,
    ) -> Result<()> {
        let mut workflows = self.active_workflows.write().await;
        let workflow = workflows.get_mut(&workflow_id)
            .ok_or_else(|| anyhow!("Workflow not found"))?;

        // Mark node as completed
        workflow.execution_state.completed_nodes.insert(completed_node_id.to_string());
        workflow.metrics.completed_nodes += 1;

        // Find nodes that were waiting for this one
        if let Some(dependents) = workflow.execution_state.dependency_graph
            .dependents.get(completed_node_id) {
            
            for dependent in dependents {
                // Check if all dependencies of the dependent node are satisfied
                let dependencies = workflow.execution_state.dependency_graph
                    .dependencies.get(dependent)
                    .cloned()
                    .unwrap_or_default();
                
                let all_deps_satisfied = dependencies.iter()
                    .all(|dep| workflow.execution_state.completed_nodes.contains(dep));

                if all_deps_satisfied && !workflow.execution_state.completed_nodes.contains(dependent) {
                    // Node is ready to execute
                    workflow.execution_state.ready_nodes.push_back(dependent.clone());
                }
            }
        }

        Ok(())
    }

    /// Finalize workflow execution and return results
    async fn finalize_workflow(
        &self,
        workflow_id: Uuid,
        status: Result<WorkflowStatus>,
    ) -> Result<WorkflowExecutionResult> {
        let mut workflows = self.active_workflows.write().await;
        let mut workflow = workflows.remove(&workflow_id)
            .ok_or_else(|| anyhow!("Workflow not found"))?;

        // Update final status and metrics
        let final_status = status.unwrap_or(WorkflowStatus::Failed);
        workflow.graph.status = final_status.clone();
        workflow.graph.completed_at = Some(Utc::now());
        workflow.metrics.end_time = Some(Utc::now());

        let duration_ms = workflow.metrics.start_time
            .zip(workflow.metrics.end_time)
            .map(|(start, end)| (end - start).num_milliseconds() as u64)
            .unwrap_or_default();

        // Send completion event
        let _ = self.event_sender.send(WorkflowEvent::WorkflowCompleted {
            workflow_id,
            status: final_status.clone(),
            duration_ms,
            metrics: workflow.metrics.clone(),
        });

        Ok(WorkflowExecutionResult {
            workflow_id,
            status: final_status,
            results: workflow.results,
            metrics: workflow.metrics,
            errors: workflow.error_log,
            duration_ms,
        })
    }

    /// Get event receiver for workflow notifications
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<WorkflowEvent> {
        self.event_sender.subscribe()
    }

    /// Get active workflow status
    pub async fn get_workflow_status(&self, workflow_id: Uuid) -> Option<WorkflowStatus> {
        let workflows = self.active_workflows.read().await;
        workflows.get(&workflow_id).map(|w| w.graph.status.clone())
    }

    /// Cancel a running workflow
    pub async fn cancel_workflow(&self, workflow_id: Uuid) -> Result<()> {
        let mut workflows = self.active_workflows.write().await;
        if let Some(workflow) = workflows.get_mut(&workflow_id) {
            workflow.graph.status = WorkflowStatus::Cancelled;
            workflow.execution_state.current_phase = ExecutionPhase::Cancelled;
            
            // TODO: Implement proper cancellation of running nodes
            info!(%workflow_id, "Workflow cancelled");
        }
        Ok(())
    }
}

/// Final result of workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowExecutionResult {
    pub workflow_id: Uuid,
    pub status: WorkflowStatus,
    pub results: HashMap<String, NodeExecutionResult>,
    pub metrics: ExecutionMetrics,
    pub errors: Vec<ExecutionError>,
    pub duration_ms: u64,
}

impl WorkflowExecutionResult {
    /// Check if workflow completed successfully
    pub fn is_successful(&self) -> bool {
        matches!(self.status, WorkflowStatus::Completed)
    }

    /// Get result for a specific node
    pub fn get_node_result(&self, node_id: &str) -> Option<&NodeExecutionResult> {
        self.results.get(node_id)
    }

    /// Get all failed nodes
    pub fn failed_nodes(&self) -> Vec<&NodeExecutionResult> {
        self.results.values()
            .filter(|r| r.status == NodeStatus::Failed)
            .collect()
    }

    /// Get execution summary
    pub fn summary(&self) -> String {
        format!(
            "Workflow {} completed with status {:?} in {}ms. {} nodes executed, {} failed.",
            self.workflow_id,
            self.status,
            self.duration_ms,
            self.results.len(),
            self.failed_nodes().len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_workflow_orchestrator_creation() {
        let config = OrchestratorConfig::default();
        let orchestrator = WorkflowOrchestrator::new(config);
        
        // Basic smoke test
        assert_eq!(orchestrator.active_workflows.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_dependency_graph_building() {
        let orchestrator = WorkflowOrchestrator::new(OrchestratorConfig::default());
        
        let mut nodes = HashMap::new();
        nodes.insert("A".to_string(), WorkflowNode {
            id: "A".to_string(),
            name: "Node A".to_string(),
            node_type: NodeType::Start,
            agent_id: None,
            dependencies: vec![],
            status: NodeStatus::Pending,
            input_schema: None,
            output_schema: None,
            timeout_ms: None,
            retry_count: 0,
            max_retries: 0,
            parallel_group: None,
        });
        
        let edges = vec![
            WorkflowEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                condition: Some(ExecutionCondition::Always),
                weight: 1.0,
            }
        ];
        
        // This should not panic
        let result = orchestrator.build_dependency_graph(&nodes, &edges);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cycle_detection() {
        let orchestrator = WorkflowOrchestrator::new(OrchestratorConfig::default());
        
        // Create a simple cycle: A -> B -> A
        let mut dependencies = HashMap::new();
        dependencies.insert("A".to_string(), vec!["B".to_string()].into_iter().collect());
        dependencies.insert("B".to_string(), vec!["A".to_string()].into_iter().collect());
        
        let graph = DependencyGraph {
            dependencies,
            dependents: HashMap::new(),
            topo_order: None,
        };
        
        let cycle = orchestrator.detect_cycles(&graph);
        assert!(cycle.is_some());
        
        let cycle = cycle.unwrap();
        assert!(cycle.len() >= 2);
    }
}
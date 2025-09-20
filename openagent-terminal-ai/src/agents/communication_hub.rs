//! Communication hub for AI agent coordination with parallel execution and message routing.
//! Provides dependency-based scheduling, direct message routing, and comprehensive error handling.

use crate::agents::types::{AgentMessage, ConcurrencyState, MessagePriority, MessageType};
use crate::agents::{AgentError, AgentRequest, AgentResponse, AiAgent};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock, Semaphore};
use tokio::time::{timeout, Duration, Instant};
use tracing::{debug, info, instrument};
use uuid::Uuid;

/// Communication hub for coordinating agent interactions
pub struct CommunicationHub {
    /// Registered agents available for communication
    agents: Arc<RwLock<HashMap<String, Arc<dyn AiAgent>>>>,
    /// Message routing and delivery system
    router: Arc<RwLock<MessageRouter>>,
    /// Event broadcasting system
    event_bus: Arc<EventBus>,
    /// Dependency scheduler for coordinated execution
    scheduler: Arc<RwLock<DependencyScheduler>>,
    /// Concurrency management
    concurrency_state: ConcurrencyState,
    /// Configuration
    config: HubConfig,
    /// Message metrics and monitoring
    metrics: Arc<RwLock<HubMetrics>>,
}

/// Message routing system with priority queues and load balancing
#[derive(Debug)]
struct MessageRouter {
    /// Priority queues for messages by agent
    message_queues: HashMap<String, PriorityQueue>,
    /// Agent availability and load tracking
    agent_states: HashMap<String, AgentState>,
    /// Message delivery channels
    delivery_channels: HashMap<String, mpsc::UnboundedSender<AgentMessage>>,
    /// Message history for debugging
    message_history: VecDeque<DeliveredMessage>,
    /// Routing rules and preferences
    routing_rules: Vec<RoutingRule>,
}

/// Event broadcasting system for agent coordination
#[derive(Debug)]
pub struct EventBus {
    /// Channel for broadcasting events
    broadcast_tx: broadcast::Sender<HubEvent>,
    /// Event history
    event_history: Arc<RwLock<VecDeque<HubEvent>>>,
    /// Event subscriptions by agent
    subscriptions: Arc<RwLock<HashMap<String, HashSet<HubEventType>>>>,
}

/// Dependency-based scheduler for coordinated agent execution
#[derive(Debug)]
struct DependencyScheduler {
    /// Active execution tasks
    active_tasks: HashMap<Uuid, ScheduledTask>,
    /// Task dependency graph
    dependencies: HashMap<Uuid, Vec<Uuid>>,
    /// Task execution queue
    execution_queue: VecDeque<Uuid>,
    /// Completed tasks
    completed_tasks: HashSet<Uuid>,
    /// Failed tasks
    failed_tasks: HashSet<Uuid>,
}

/// Priority queue implementation for message ordering
#[derive(Debug)]
struct PriorityQueue {
    items: BTreeMap<MessagePriority, VecDeque<PendingMessage>>,
}

/// Agent state tracking
#[derive(Debug, Clone)]
struct AgentState {
    id: String,
    status: AgentStatus,
    load: f64,
    last_activity: DateTime<Utc>,
    capabilities: Vec<String>,
    max_concurrent: usize,
    current_tasks: usize,
}

/// Message pending delivery
#[derive(Debug, Clone)]
struct PendingMessage {
    message: AgentMessage,
    attempts: u32,
    next_retry: DateTime<Utc>,
    timeout: DateTime<Utc>,
}

/// Delivered message record
#[derive(Debug, Clone)]
struct DeliveredMessage {
    message: AgentMessage,
    delivered_at: DateTime<Utc>,
    delivery_time_ms: u64,
    success: bool,
    error: Option<String>,
}

/// Routing rule for message delivery
#[derive(Debug, Clone)]
struct RoutingRule {
    id: String,
    condition: RoutingCondition,
    action: RoutingAction,
    priority: i32,
}

/// Conditions for routing rules
#[derive(Debug, Clone)]
enum RoutingCondition {
    MessageType(MessageType),
    FromAgent(String),
    ToAgent(String),
    Priority(MessagePriority),
    Custom(String), // Custom condition expression
}

/// Actions for routing rules
#[derive(Debug, Clone)]
enum RoutingAction {
    Route,
    Block,
    Transform(String),
    Delay(Duration),
    Broadcast,
}

/// Scheduled task for dependency-based execution
#[derive(Debug, Clone)]
struct ScheduledTask {
    id: Uuid,
    agent_id: String,
    request: AgentRequest,
    dependencies: Vec<Uuid>,
    priority: TaskPriority,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    result: Option<AgentResponse>,
    error: Option<String>,
    timeout: Duration,
    retry_count: u32,
    max_retries: u32,
}

/// Task priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
    Emergency = 5,
}

/// Agent status enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
enum AgentStatus {
    Available,
    Busy,
    Offline,
    Error,
}

/// Hub events for coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HubEvent {
    AgentRegistered { agent_id: String, capabilities: Vec<String> },
    AgentUnregistered { agent_id: String },
    MessageSent { message_id: Uuid, from: String, to: String, message_type: MessageType },
    MessageDelivered { message_id: Uuid, delivery_time_ms: u64 },
    MessageFailed { message_id: Uuid, error: String, retry_count: u32 },
    TaskScheduled { task_id: Uuid, agent_id: String, dependencies: Vec<Uuid> },
    TaskStarted { task_id: Uuid, agent_id: String },
    TaskCompleted { task_id: Uuid, agent_id: String, duration_ms: u64 },
    TaskFailed { task_id: Uuid, agent_id: String, error: String },
    LoadBalanced { agent_id: String, new_load: f64 },
}

/// Hub event types for subscriptions
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum HubEventType {
    Agent,
    Message,
    Task,
    LoadBalance,
    All,
}

/// Hub configuration
#[derive(Debug, Clone)]
pub struct HubConfig {
    pub max_concurrent_tasks: usize,
    pub max_message_retries: u32,
    pub message_timeout_seconds: u64,
    pub task_timeout_seconds: u64,
    pub enable_load_balancing: bool,
    pub enable_message_history: bool,
    pub max_history_size: usize,
    pub heartbeat_interval_seconds: u64,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 10,
            max_message_retries: 3,
            message_timeout_seconds: 30,
            task_timeout_seconds: 300,
            enable_load_balancing: true,
            enable_message_history: true,
            max_history_size: 1000,
            heartbeat_interval_seconds: 30,
        }
    }
}

/// Hub performance metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HubMetrics {
    messages_sent: u64,
    messages_delivered: u64,
    messages_failed: u64,
    tasks_scheduled: u64,
    tasks_completed: u64,
    tasks_failed: u64,
    average_message_delivery_time_ms: f64,
    average_task_execution_time_ms: f64,
    agent_load_distribution: HashMap<String, f64>,
}

impl CommunicationHub {
    /// Create a new communication hub
    pub fn new(config: HubConfig) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);

        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            router: Arc::new(RwLock::new(MessageRouter::new())),
            event_bus: Arc::new(EventBus::new(broadcast_tx)),
            scheduler: Arc::new(RwLock::new(DependencyScheduler::new())),
            concurrency_state: ConcurrencyState::default(),
            metrics: Arc::new(RwLock::new(HubMetrics::default())),
            config,
        }
    }

    /// Register an agent with the communication hub
    #[instrument(skip(self, agent))]
    pub async fn register_agent(&self, agent: Arc<dyn AiAgent>) -> Result<()> {
        let agent_id = agent.name().to_string();
        let capabilities = agent.capabilities().features;

        info!(agent_id = %agent_id, "Registering agent");

        // Add to agent registry
        {
            let mut agents = self.agents.write().await;
            agents.insert(agent_id.clone(), agent);
        }

        // Initialize routing state
        {
            let mut router = self.router.write().await;
            router.register_agent(&agent_id, capabilities.clone()).await?;
        }

        // Broadcast registration event
        let event = HubEvent::AgentRegistered {
            agent_id: agent_id.clone(),
            capabilities: capabilities.clone(),
        };
        self.event_bus.broadcast(event).await;

        info!(agent_id = %agent_id, capabilities = ?capabilities, "Agent registered successfully");
        Ok(())
    }

    /// Unregister an agent
    #[instrument(skip(self))]
    pub async fn unregister_agent(&self, agent_id: &str) -> Result<()> {
        info!(agent_id = %agent_id, "Unregistering agent");

        // Remove from agent registry
        {
            let mut agents = self.agents.write().await;
            agents.remove(agent_id);
        }

        // Clean up routing state
        {
            let mut router = self.router.write().await;
            router.unregister_agent(agent_id).await?;
        }

        // Broadcast unregistration event
        let event = HubEvent::AgentUnregistered { agent_id: agent_id.to_string() };
        self.event_bus.broadcast(event).await;

        info!(agent_id = %agent_id, "Agent unregistered successfully");
        Ok(())
    }

    /// Send a message between agents
    #[instrument(skip(self, message))]
    pub async fn send_message(&self, message: AgentMessage) -> Result<()> {
        debug!(
            message_id = %message.id,
            from = %message.from_agent,
            to = %message.to_agent,
            message_type = ?message.message_type,
            "Sending message"
        );

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.messages_sent += 1;
        }

        // Queue message for delivery
        {
            let mut router = self.router.write().await;
            router.queue_message(message.clone()).await?;
        }

        // Broadcast message sent event
        let event = HubEvent::MessageSent {
            message_id: message.id,
            from: message.from_agent.clone(),
            to: message.to_agent.clone(),
            message_type: message.message_type.clone(),
        };
        self.event_bus.broadcast(event).await;

        // Start delivery process
        self.process_message_queue().await?;

        Ok(())
    }

    /// Schedule a task with dependencies
    #[instrument(skip(self, request))]
    pub async fn schedule_task(
        &self,
        agent_id: String,
        request: AgentRequest,
        dependencies: Vec<Uuid>,
        priority: TaskPriority,
    ) -> Result<Uuid> {
        let task_id = Uuid::new_v4();

        info!(
            task_id = %task_id,
            agent_id = %agent_id,
            dependencies = ?dependencies,
            priority = ?priority,
            "Scheduling task"
        );

        let task = ScheduledTask {
            id: task_id,
            agent_id: agent_id.clone(),
            request,
            dependencies: dependencies.clone(),
            priority,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
            timeout: Duration::from_secs(self.config.task_timeout_seconds),
            retry_count: 0,
            max_retries: 3,
        };

        // Add to scheduler
        {
            let mut scheduler = self.scheduler.write().await;
            scheduler.schedule_task(task).await?;
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.tasks_scheduled += 1;
        }

        // Broadcast task scheduled event
        let event = HubEvent::TaskScheduled { task_id, agent_id, dependencies };
        self.event_bus.broadcast(event).await;

        // Process scheduling queue
        self.process_scheduling_queue().await?;

        Ok(task_id)
    }

    /// Execute parallel tasks with dependency resolution
    #[instrument(skip(self, tasks))]
    pub async fn execute_parallel_tasks(
        &self,
        tasks: Vec<(String, AgentRequest, Vec<Uuid>)>,
    ) -> Result<Vec<(Uuid, Result<AgentResponse, AgentError>)>> {
        info!(task_count = tasks.len(), "Executing parallel tasks");

        // Schedule all tasks
        let mut task_ids = Vec::new();
        for (agent_id, request, dependencies) in tasks {
            let task_id =
                self.schedule_task(agent_id, request, dependencies, TaskPriority::Normal).await?;
            task_ids.push(task_id);
        }

        // Wait for all tasks to complete
        let mut results = Vec::new();
        for task_id in task_ids {
            let result = self.wait_for_task_completion(task_id).await;
            results.push((task_id, result));
        }

        Ok(results)
    }

    /// Wait for a specific task to complete
    async fn wait_for_task_completion(&self, task_id: Uuid) -> Result<AgentResponse, AgentError> {
        let timeout_duration = Duration::from_secs(self.config.task_timeout_seconds);
        let start_time = Instant::now();

        loop {
            {
                let scheduler = self.scheduler.read().await;
                if let Some(task) = scheduler.active_tasks.get(&task_id) {
                    if let Some(result) = &task.result {
                        return Ok(result.clone());
                    }
                    if let Some(error) = &task.error {
                        return Err(AgentError::ProcessingError(error.clone()));
                    }
                }

                if scheduler.completed_tasks.contains(&task_id) {
                    return Err(AgentError::ProcessingError(
                        "Task completed without result".to_string(),
                    ));
                }

                if scheduler.failed_tasks.contains(&task_id) {
                    return Err(AgentError::ProcessingError("Task failed".to_string()));
                }
            }

            if start_time.elapsed() > timeout_duration {
                return Err(AgentError::ProcessingError("Task timeout".to_string()));
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Process message delivery queue
    async fn process_message_queue(&self) -> Result<()> {
        let mut router = self.router.write().await;
        let agents = self.agents.read().await;

        // Snapshot keys and agent states to avoid mutable/immutable borrow conflicts
        let keys: Vec<String> = router.message_queues.keys().cloned().collect();
        let agent_states_snapshot = router.agent_states.clone();

        for agent_id in keys {
            let queue_opt = router.message_queues.get_mut(&agent_id);
            if queue_opt.is_none() {
                continue;
            }
            let queue = queue_opt.unwrap();

            if let Some(agent) = agents.get(&agent_id) {
                if let Some(agent_state) = agent_states_snapshot.get(&agent_id) {
                    if agent_state.status == AgentStatus::Available
                        && agent_state.current_tasks < agent_state.max_concurrent
                    {
                        if let Some(pending) = queue.pop() {
                            let agent_clone = agent.clone();
                            let message = pending.message.clone();
                            let hub_metrics = self.metrics.clone();
                            let event_bus = self.event_bus.clone();

                            // Spawn delivery task
                            tokio::spawn(async move {
                                let start_time = Instant::now();
                                let delivery_result =
                                    Self::deliver_message_to_agent(agent_clone, message.clone())
                                        .await;

                                let delivery_time = start_time.elapsed().as_millis() as u64;

                                // Update metrics
                                {
                                    let mut metrics = hub_metrics.write().await;
                                    if delivery_result.is_ok() {
                                        metrics.messages_delivered += 1;
                                        metrics.average_message_delivery_time_ms = (metrics
                                            .average_message_delivery_time_ms
                                            + delivery_time as f64)
                                            / 2.0;
                                    } else {
                                        metrics.messages_failed += 1;
                                    }
                                }

                                // Broadcast delivery event
                                let event = match delivery_result {
                                    Ok(_) => HubEvent::MessageDelivered {
                                        message_id: message.id,
                                        delivery_time_ms: delivery_time,
                                    },
                                    Err(e) => HubEvent::MessageFailed {
                                        message_id: message.id,
                                        error: e.to_string(),
                                        retry_count: pending.attempts,
                                    },
                                };
                                let _ = event_bus.broadcast(event).await;
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Deliver a message to a specific agent
    async fn deliver_message_to_agent(
        agent: Arc<dyn AiAgent>,
        message: AgentMessage,
    ) -> Result<()> {
        // Convert hub message to agent request if needed
        let request = match message.message_type {
            MessageType::Request => {
                // Extract request from message payload
                if let Ok(ai_request) = serde_json::from_value::<crate::AiRequest>(message.payload)
                {
                    AgentRequest::Command(ai_request)
                } else {
                    return Err(anyhow!("Invalid request payload"));
                }
            }
            _ => {
                // For other message types, we might need different handling
                return Ok(());
            }
        };

        // Execute the request
        let _response = agent
            .process(request)
            .await
            .with_context(|| format!("Failed to process message {}", message.id))?;

        Ok(())
    }

    /// Process task scheduling queue
    async fn process_scheduling_queue(&self) -> Result<()> {
        let mut scheduler = self.scheduler.write().await;
        let agents = self.agents.read().await;
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_tasks));

        // Check for ready tasks (dependencies satisfied)
        let ready_tasks: Vec<Uuid> = scheduler
            .active_tasks
            .values()
            .filter(|task| {
                task.started_at.is_none()
                    && task.dependencies.iter().all(|dep| scheduler.completed_tasks.contains(dep))
            })
            .map(|task| task.id)
            .collect();

        for task_id in ready_tasks {
            if let Some(task) = scheduler.active_tasks.get(&task_id).cloned() {
                if let Some(agent) = agents.get(&task.agent_id) {
                    let agent_clone = agent.clone();
                    let task_clone = task.clone();
                    let scheduler_ref = self.scheduler.clone();
                    let metrics_ref = self.metrics.clone();
                    let event_bus_ref = self.event_bus.clone();
                    let permit = semaphore.clone().acquire_owned().await?;

                    // Mark task as started
                    scheduler.active_tasks.get_mut(&task_id).unwrap().started_at = Some(Utc::now());

                    // Spawn task execution
                    tokio::spawn(async move {
                        let _permit = permit;
                        let start_time = Instant::now();

                        // Broadcast task started event
                        let start_event = HubEvent::TaskStarted {
                            task_id: task_clone.id,
                            agent_id: task_clone.agent_id.clone(),
                        };
                        let _ = event_bus_ref.broadcast(start_event).await;

                        // Execute task with timeout
                        let execution_result = timeout(
                            task_clone.timeout,
                            agent_clone.process(task_clone.request.clone()),
                        )
                        .await;

                        let execution_time = start_time.elapsed().as_millis() as u64;

                        // Update task status
                        {
                            let mut scheduler = scheduler_ref.write().await;
                            if let Some(task) = scheduler.active_tasks.get_mut(&task_clone.id) {
                                task.completed_at = Some(Utc::now());

                                match execution_result {
                                    Ok(Ok(ref response)) => {
                                        task.result = Some(response.clone());
                                        scheduler.completed_tasks.insert(task_clone.id);
                                    }
                                    Ok(Err(ref e)) => {
                                        task.error = Some(e.to_string());
                                        scheduler.failed_tasks.insert(task_clone.id);
                                    }
                                    Err(_) => {
                                        task.error = Some("Task timeout".to_string());
                                        scheduler.failed_tasks.insert(task_clone.id);
                                    }
                                }
                            }
                        }

                        // Update metrics
                        {
                            let mut metrics = metrics_ref.write().await;
                            if execution_result.is_ok() {
                                metrics.tasks_completed += 1;
                                metrics.average_task_execution_time_ms = (metrics
                                    .average_task_execution_time_ms
                                    + execution_time as f64)
                                    / 2.0;
                            } else {
                                metrics.tasks_failed += 1;
                            }
                        }

                        // Broadcast completion event
                        let completion_event = match execution_result {
                            Ok(Ok(_)) => HubEvent::TaskCompleted {
                                task_id: task_clone.id,
                                agent_id: task_clone.agent_id.clone(),
                                duration_ms: execution_time,
                            },
                            _ => HubEvent::TaskFailed {
                                task_id: task_clone.id,
                                agent_id: task_clone.agent_id.clone(),
                                error: "Task execution failed or timed out".to_string(),
                            },
                        };
                        let _ = event_bus_ref.broadcast(completion_event).await;
                    });
                }
            }
        }

        Ok(())
    }

    /// Subscribe to hub events
    pub async fn subscribe_to_events(
        &self,
        agent_id: String,
        event_types: Vec<HubEventType>,
    ) -> broadcast::Receiver<HubEvent> {
        self.event_bus.subscribe(agent_id, event_types).await
    }

    /// Get hub performance metrics
    pub async fn get_metrics(&self) -> HubMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Perform load balancing across agents
    pub async fn balance_load(&self) -> Result<()> {
        if !self.config.enable_load_balancing {
            return Ok(());
        }

        let mut router = self.router.write().await;
        router.balance_agent_loads().await;

        Ok(())
    }

    /// Get current system status
    pub async fn get_status(&self) -> HubStatus {
        let agents = self.agents.read().await;
        let router = self.router.read().await;
        let scheduler = self.scheduler.read().await;
        let metrics = self.metrics.read().await;

        HubStatus {
            registered_agents: agents.len(),
            active_tasks: scheduler.active_tasks.len(),
            pending_messages: router.message_queues.values().map(|queue| queue.len()).sum(),
            completed_tasks: scheduler.completed_tasks.len(),
            failed_tasks: scheduler.failed_tasks.len(),
            average_load: router.agent_states.values().map(|state| state.load).sum::<f64>()
                / router.agent_states.len().max(1) as f64,
            uptime_seconds: 0, // Would need to track start time
            metrics: metrics.clone(),
        }
    }
}

/// Hub status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubStatus {
    pub registered_agents: usize,
    pub active_tasks: usize,
    pub pending_messages: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub average_load: f64,
    pub uptime_seconds: u64,
    pub metrics: HubMetrics,
}

// Implementation of supporting structures
impl MessageRouter {
    fn new() -> Self {
        Self {
            message_queues: HashMap::new(),
            agent_states: HashMap::new(),
            delivery_channels: HashMap::new(),
            message_history: VecDeque::new(),
            routing_rules: Vec::new(),
        }
    }

    async fn register_agent(&mut self, agent_id: &str, capabilities: Vec<String>) -> Result<()> {
        self.message_queues.insert(agent_id.to_string(), PriorityQueue::new());
        self.agent_states.insert(
            agent_id.to_string(),
            AgentState {
                id: agent_id.to_string(),
                status: AgentStatus::Available,
                load: 0.0,
                last_activity: Utc::now(),
                capabilities,
                max_concurrent: 5, // Default
                current_tasks: 0,
            },
        );

        Ok(())
    }

    async fn unregister_agent(&mut self, agent_id: &str) -> Result<()> {
        self.message_queues.remove(agent_id);
        self.agent_states.remove(agent_id);
        self.delivery_channels.remove(agent_id);
        Ok(())
    }

    async fn queue_message(&mut self, message: AgentMessage) -> Result<()> {
        let pending = PendingMessage {
            timeout: Utc::now() + chrono::Duration::seconds(30),
            next_retry: Utc::now(),
            attempts: 0,
            message,
        };

        if let Some(queue) = self.message_queues.get_mut(&pending.message.to_agent) {
            queue.push(pending);
        } else {
            return Err(anyhow!("Agent {} not found", pending.message.to_agent));
        }

        Ok(())
    }

    async fn balance_agent_loads(&mut self) {
        // Simple load balancing - redistribute messages from overloaded agents
        let mut overloaded: Vec<String> = Vec::new();
        let mut underloaded: Vec<String> = Vec::new();

        for (agent_id, state) in &self.agent_states {
            if state.load > 0.8 {
                overloaded.push(agent_id.clone());
            } else if state.load < 0.3 {
                underloaded.push(agent_id.clone());
            }
        }

        // Redistribute messages (simplified implementation)
        for _overloaded_agent in overloaded {
            if let Some(_underloaded_agent) = underloaded.first() {
                // Move some messages from overloaded to underloaded agent
                // This would require more sophisticated logic in practice
            }
        }
    }
}

impl PriorityQueue {
    fn new() -> Self {
        Self { items: BTreeMap::new() }
    }

    fn push(&mut self, item: PendingMessage) {
        let priority = item.message.priority.clone();
        self.items.entry(priority).or_default().push_back(item);
    }

    fn pop(&mut self) -> Option<PendingMessage> {
        // Pop from highest priority queue first
        for (_, queue) in self.items.iter_mut().rev() {
            if let Some(item) = queue.pop_front() {
                return Some(item);
            }
        }
        None
    }

    fn len(&self) -> usize {
        self.items.values().map(|queue| queue.len()).sum()
    }
}

impl EventBus {
    fn new(broadcast_tx: broadcast::Sender<HubEvent>) -> Self {
        Self {
            broadcast_tx,
            event_history: Arc::new(RwLock::new(VecDeque::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn broadcast(&self, event: HubEvent) {
        // Add to history
        {
            let mut history = self.event_history.write().await;
            history.push_back(event.clone());
            if history.len() > 1000 {
                history.pop_front();
            }
        }

        // Broadcast event
        let _ = self.broadcast_tx.send(event);
    }

    async fn subscribe(
        &self,
        agent_id: String,
        event_types: Vec<HubEventType>,
    ) -> broadcast::Receiver<HubEvent> {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(agent_id, event_types.into_iter().collect());

        self.broadcast_tx.subscribe()
    }
}

impl DependencyScheduler {
    fn new() -> Self {
        Self {
            active_tasks: HashMap::new(),
            dependencies: HashMap::new(),
            execution_queue: VecDeque::new(),
            completed_tasks: HashSet::new(),
            failed_tasks: HashSet::new(),
        }
    }

    async fn schedule_task(&mut self, task: ScheduledTask) -> Result<()> {
        let task_id = task.id;
        self.dependencies.insert(task_id, task.dependencies.clone());
        self.active_tasks.insert(task_id, task);
        self.execution_queue.push_back(task_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_communication_hub_creation() {
        let config = HubConfig::default();
        let hub = CommunicationHub::new(config);

        let status = hub.get_status().await;
        assert_eq!(status.registered_agents, 0);
        assert_eq!(status.active_tasks, 0);
    }

    #[tokio::test]
    async fn test_priority_queue() {
        let mut queue: PriorityQueue = PriorityQueue::new();

        // Test basic operations
        assert_eq!(queue.len(), 0);
        assert!(queue.pop().is_none());
    }

    #[tokio::test]
    async fn test_event_bus() {
        let (tx, _rx) = broadcast::channel(100);
        let event_bus = EventBus::new(tx);

        let test_event = HubEvent::AgentRegistered {
            agent_id: "test-agent".to_string(),
            capabilities: vec!["test".to_string()],
        };

        event_bus.broadcast(test_event).await;

        let history = event_bus.event_history.read().await;
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_hub_config_defaults() {
        let config = HubConfig::default();
        assert_eq!(config.max_concurrent_tasks, 10);
        assert_eq!(config.max_message_retries, 3);
        assert_eq!(config.message_timeout_seconds, 30);
    }
}

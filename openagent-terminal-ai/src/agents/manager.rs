use super::{
    AgentCapabilities, AgentError, AgentRequest, AgentResponse, AiAgent, CollaborationContext,
    PrivacyLevel, ProjectInfo,
};
use std::collections::HashMap;
use futures_util::future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use serde::{Deserialize, Serialize};

/// Agent Manager coordinates multiple AI agents and handles request routing
pub struct AgentManager {
    /// Registered agents by name
    agents: Arc<RwLock<HashMap<String, Box<dyn AiAgent>>>>,

    /// Agent configuration and settings
    config: AgentManagerConfig,

    /// Cached project context for performance
    project_context: Arc<RwLock<Option<ProjectInfo>>>,

    /// Request routing rules
    routing_rules: Vec<RoutingRule>,

    /// Agent performance metrics
    metrics: Arc<RwLock<AgentMetrics>>,
}

/// Performance metrics for agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    /// Per-agent performance statistics
    pub agent_stats: HashMap<String, AgentPerformanceStats>,
    
    /// Global statistics
    pub total_requests: u64,
    pub total_successful_requests: u64,
    pub average_response_time: Duration,
    pub last_reset: std::time::SystemTime,
}

/// Performance statistics for individual agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPerformanceStats {
    /// Total number of requests handled
    pub request_count: u64,
    
    /// Number of successful requests
    pub success_count: u64,
    
    /// Number of failed requests
    pub error_count: u64,
    
    /// Average response time
    pub avg_response_time: Duration,
    
    /// Min/Max response times
    pub min_response_time: Duration,
    pub max_response_time: Duration,
    
    /// Last request timestamp
    pub last_request_time: std::time::SystemTime,
    
    /// User satisfaction scores (if available)
    pub satisfaction_scores: Vec<f32>,
    
    /// Average satisfaction score
    pub avg_satisfaction: f32,
    
    /// Utilization metrics
    pub total_processing_time: Duration,
    pub peak_concurrent_requests: u32,
    pub current_load: u32,
}

impl Default for AgentMetrics {
    fn default() -> Self {
        Self {
            agent_stats: HashMap::new(),
            total_requests: 0,
            total_successful_requests: 0,
            average_response_time: Duration::ZERO,
            last_reset: std::time::SystemTime::now(),
        }
    }
}

impl Default for AgentPerformanceStats {
    fn default() -> Self {
        Self {
            request_count: 0,
            success_count: 0,
            error_count: 0,
            avg_response_time: Duration::ZERO,
            min_response_time: Duration::MAX,
            max_response_time: Duration::ZERO,
            last_request_time: std::time::SystemTime::UNIX_EPOCH,
            satisfaction_scores: Vec::new(),
            avg_satisfaction: 0.0,
            total_processing_time: Duration::ZERO,
            peak_concurrent_requests: 0,
            current_load: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentManagerConfig {
    pub default_agent: String,
    pub enable_collaboration: bool,
    pub privacy_level: PrivacyLevel,
    pub max_concurrent_agents: usize,
    pub request_timeout_ms: u64,
    pub cache_project_context: bool,
}

impl Default for AgentManagerConfig {
    fn default() -> Self {
        Self {
            default_agent: "command".to_string(),
            enable_collaboration: true,
            privacy_level: PrivacyLevel::Local,
            max_concurrent_agents: 3,
            request_timeout_ms: 30000,
            cache_project_context: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoutingRule {
    pub pattern: RequestPattern,
    pub target_agent: String,
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub enum RequestPattern {
    /// Route based on request type
    RequestType(String),
    /// Route based on keywords in the request
    Keywords(Vec<String>),
    /// Route based on file extension or language
    Language(String),
    /// Route based on project context
    ProjectType(String),
    /// Custom routing logic
    Custom(fn(&AgentRequest) -> bool),
}

impl AgentManager {
    /// Create a new AgentManager with default configuration
    pub fn new() -> Self {
        Self::with_config(AgentManagerConfig::default())
    }

    /// Create a new AgentManager with specific configuration
    pub fn with_config(config: AgentManagerConfig) -> Self {
        let routing_rules = Self::default_routing_rules();

        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            config,
            project_context: Arc::new(RwLock::new(None)),
            routing_rules,
            metrics: Arc::new(RwLock::new(AgentMetrics::default())),
        }
    }

    /// Register a new agent
    pub async fn register_agent(&self, agent: Box<dyn AiAgent>) -> Result<(), AgentError> {
        let agent_name = agent.name().to_string();

        info!("Registering agent: {} v{}", agent_name, agent.version());

        let mut agents = self.agents.write().await;

        if agents.contains_key(&agent_name) {
            warn!("Agent {} already registered, replacing", agent_name);
        }

        agents.insert(agent_name.clone(), agent);

        info!("Agent {} registered successfully", agent_name);
        Ok(())
    }

    /// Unregister an agent
    pub async fn unregister_agent(&self, name: &str) -> Result<(), AgentError> {
        let mut agents = self.agents.write().await;

        if agents.remove(name).is_some() {
            info!("Agent {} unregistered", name);
            Ok(())
        } else {
            Err(AgentError::AgentNotFound(name.to_string()))
        }
    }

    /// Get list of registered agents
    pub async fn list_agents(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        agents.keys().cloned().collect()
    }

    /// Get agent capabilities
    pub async fn get_agent_capabilities(
        &self,
        name: &str,
    ) -> Result<AgentCapabilities, AgentError> {
        let agents = self.agents.read().await;

        agents
            .get(name)
            .map(|agent| agent.capabilities())
            .ok_or_else(|| AgentError::AgentNotFound(name.to_string()))
    }

    /// Process a request by routing it to the appropriate agent(s)
    pub async fn process_request(
        &self,
        request: AgentRequest,
    ) -> Result<AgentResponse, AgentError> {
        debug!("Processing agent request: {:?}", request);
        let start_time = Instant::now();

        // Determine the best agent for this request
        let agent_name = self.route_request(&request).await?;

        // Update current load
        self.increment_agent_load(&agent_name).await;

        // Get the agent
        let agents = self.agents.read().await;
        let agent = agents
            .get(&agent_name)
            .ok_or_else(|| AgentError::AgentNotFound(agent_name.clone()))?;

        // Check if agent can handle the request
        if !agent.can_handle(&request) {
            self.decrement_agent_load(&agent_name).await;
            return Err(AgentError::NotSupported(format!(
                "Agent {} cannot handle this request type",
                agent_name
            )));
        }

        // Process the request with timeout and metrics collection
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(self.config.request_timeout_ms),
            agent.process(request),
        )
        .await
        .map_err(|_| AgentError::ProcessingError("Request timeout".to_string()));

        let processing_time = start_time.elapsed();
        
        // Update metrics
        match &result {
            Ok(Ok(_)) => {
                self.record_success(&agent_name, processing_time).await;
                info!("Request processed successfully by agent: {} ({}ms)", agent_name, processing_time.as_millis());
            }
            Ok(Err(e)) => {
                self.record_error(&agent_name, processing_time).await;
                warn!("Agent {} returned error: {}", agent_name, e);
            }
            Err(e) => {
                self.record_error(&agent_name, processing_time).await;
                error!("Agent {} request failed: {}", agent_name, e);
            }
        }

        // Decrement load
        self.decrement_agent_load(&agent_name).await;

        result?
    }

    /// Process a collaboration request involving multiple agents
    pub async fn collaborate(
        &self,
        agent_names: Vec<String>,
        context: CollaborationContext,
        goal: String,
    ) -> Result<AgentResponse, AgentError> {
        if !self.config.enable_collaboration {
            return Err(AgentError::ConfigurationError(
                "Collaboration is disabled".to_string(),
            ));
        }

        if agent_names.len() > self.config.max_concurrent_agents {
            return Err(AgentError::ConfigurationError(format!(
                "Too many agents requested: {} > {}",
                agent_names.len(),
                self.config.max_concurrent_agents
            )));
        }

        info!("Starting collaboration with agents: {:?}", agent_names);

        let agents = self.agents.read().await;

        // Validate all requested agents exist
        for name in &agent_names {
            if !agents.contains_key(name) {
                return Err(AgentError::AgentNotFound(name.clone()));
            }
        }

        // Create collaboration requests for each agent
        let mut tasks = Vec::new();

        for name in &agent_names {
            let agent = agents.get(name).unwrap();
            let collaboration_request = AgentRequest::Collaboration {
                agents: agent_names.clone(),
                context: context.clone(),
                goal: goal.clone(),
            };

            let task = agent.process(collaboration_request);
            tasks.push(task);
        }

        // Wait for all agents to respond
        let results: Vec<Result<AgentResponse, AgentError>> =
            future::join_all(tasks).await;

        // Combine results from all participating agents
        let mut successful_results = Vec::new();
        let mut errors = Vec::new();

        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(response) => successful_results.push((agent_names[i].clone(), response)),
                Err(e) => errors.push((agent_names[i].clone(), e)),
            }
        }

        if successful_results.is_empty() {
            return Err(AgentError::CollaborationFailed(format!(
                "All agents failed: {:?}",
                errors
            )));
        }

        // Synthesize collaboration result
        self.synthesize_collaboration_result(successful_results, goal)
            .await
    }

    /// Route a request to the most appropriate agent
    async fn route_request(&self, request: &AgentRequest) -> Result<String, AgentError> {
        // Apply routing rules in priority order
        for rule in &self.routing_rules {
            if self.matches_pattern(&rule.pattern, request) {
                debug!(
                    "Request routed to {} via rule {:?}",
                    rule.target_agent, rule.pattern
                );
                return Ok(rule.target_agent.clone());
            }
        }

        // Fallback to default agent
        debug!(
            "Request routed to default agent: {}",
            self.config.default_agent
        );
        Ok(self.config.default_agent.clone())
    }

    /// Check if a request matches a routing pattern
    fn matches_pattern(&self, pattern: &RequestPattern, request: &AgentRequest) -> bool {
        match pattern {
            RequestPattern::RequestType(req_type) => match (req_type.as_str(), request) {
                ("command", AgentRequest::Command(_)) => true,
                ("code", AgentRequest::CodeGeneration { .. }) => true,
                ("context", AgentRequest::ProjectContext { .. }) => true,
                ("quality", AgentRequest::Quality { .. }) => true,
                ("collaboration", AgentRequest::Collaboration { .. }) => true,
                _ => false,
            },
            RequestPattern::Keywords(keywords) => {
                let request_text = self.extract_text_from_request(request);
                keywords.iter().any(|keyword| {
                    request_text
                        .to_lowercase()
                        .contains(&keyword.to_lowercase())
                })
            }
            RequestPattern::Language(lang) => match request {
                AgentRequest::CodeGeneration { language, .. } => {
                    language.as_ref().map_or(false, |l| l == lang)
                }
                AgentRequest::Quality { language, .. } => {
                    language.as_ref().map_or(false, |l| l == lang)
                }
                _ => false,
            },
            RequestPattern::ProjectType(_proj_type) => {
                // This would require project context analysis
                // For now, return false
                false
            }
            RequestPattern::Custom(matcher) => matcher(request),
        }
    }

    /// Extract searchable text from a request
    fn extract_text_from_request(&self, request: &AgentRequest) -> String {
        match request {
            AgentRequest::Command(ai_req) => ai_req.scratch_text.clone(),
            AgentRequest::CodeGeneration { prompt, .. } => prompt.clone(),
            AgentRequest::ProjectContext { project_path, .. } => project_path.clone(),
            AgentRequest::Quality { code, .. } => code.clone(),
            AgentRequest::Collaboration { goal, .. } => goal.clone(),
        }
    }

    /// Synthesize results from multiple agents into a single response
    async fn synthesize_collaboration_result(
        &self,
        results: Vec<(String, AgentResponse)>,
        _goal: String,
    ) -> Result<AgentResponse, AgentError> {
        let participating_agents: Vec<String> =
            results.iter().map(|(name, _)| name.clone()).collect();

        // Simple synthesis - in a real implementation, this would be more sophisticated
        let combined_result = results
            .iter()
            .map(|(agent_name, response)| format!("{}: {:?}", agent_name, response))
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(AgentResponse::CollaborationResult {
            participating_agents,
            result: combined_result,
            confidence: 0.8, // Simple confidence calculation
        })
    }

    /// Create default routing rules
    fn default_routing_rules() -> Vec<RoutingRule> {
        vec![
            RoutingRule {
                pattern: RequestPattern::RequestType("code".to_string()),
                target_agent: "code_generation".to_string(),
                priority: 100,
            },
            RoutingRule {
                pattern: RequestPattern::RequestType("context".to_string()),
                target_agent: "project_context".to_string(),
                priority: 90,
            },
            RoutingRule {
                pattern: RequestPattern::RequestType("quality".to_string()),
                target_agent: "quality".to_string(),
                priority: 90,
            },
            RoutingRule {
                pattern: RequestPattern::Keywords(vec![
                    "generate code".to_string(),
                    "write function".to_string(),
                    "create class".to_string(),
                ]),
                target_agent: "code_generation".to_string(),
                priority: 80,
            },
            RoutingRule {
                pattern: RequestPattern::Keywords(vec![
                    "analyze project".to_string(),
                    "project structure".to_string(),
                    "dependencies".to_string(),
                ]),
                target_agent: "project_context".to_string(),
                priority: 75,
            },
            RoutingRule {
                pattern: RequestPattern::Keywords(vec![
                    "code quality".to_string(),
                    "security scan".to_string(),
                    "lint".to_string(),
                    "vulnerability".to_string(),
                ]),
                target_agent: "quality".to_string(),
                priority: 75,
            },
        ]
    }

    /// Update project context cache
    pub async fn update_project_context(&self, context: ProjectInfo) {
        if self.config.cache_project_context {
            let mut cached_context = self.project_context.write().await;
            *cached_context = Some(context);
            debug!("Project context cache updated");
        }
    }

    /// Get cached project context
    pub async fn get_project_context(&self) -> Option<ProjectInfo> {
        if self.config.cache_project_context {
            let cached_context = self.project_context.read().await;
            cached_context.clone()
        } else {
            None
        }
    }

    /// Increment the current load for an agent
    async fn increment_agent_load(&self, agent_name: &str) {
        let mut metrics = self.metrics.write().await;
        let stats = metrics.agent_stats.entry(agent_name.to_string())
            .or_insert_with(AgentPerformanceStats::default);
        
        stats.current_load += 1;
        if stats.current_load > stats.peak_concurrent_requests {
            stats.peak_concurrent_requests = stats.current_load;
        }
    }

    /// Decrement the current load for an agent
    async fn decrement_agent_load(&self, agent_name: &str) {
        let mut metrics = self.metrics.write().await;
        if let Some(stats) = metrics.agent_stats.get_mut(agent_name) {
            if stats.current_load > 0 {
                stats.current_load -= 1;
            }
        }
    }

    /// Record a successful request
    async fn record_success(&self, agent_name: &str, processing_time: Duration) {
        let mut metrics = self.metrics.write().await;
        
        // Update global metrics
        metrics.total_requests += 1;
        metrics.total_successful_requests += 1;
        
        // Update agent-specific metrics
        let stats = metrics.agent_stats.entry(agent_name.to_string())
            .or_insert_with(AgentPerformanceStats::default);
        
        stats.request_count += 1;
        stats.success_count += 1;
        stats.last_request_time = std::time::SystemTime::now();
        stats.total_processing_time += processing_time;
        
        // Update response time statistics
        if processing_time < stats.min_response_time {
            stats.min_response_time = processing_time;
        }
        if processing_time > stats.max_response_time {
            stats.max_response_time = processing_time;
        }
        
        // Calculate moving average for response time
        let total_time = stats.avg_response_time * (stats.request_count - 1) as u32 + processing_time;
        stats.avg_response_time = total_time / stats.request_count as u32;
    }

    /// Record a failed request
    async fn record_error(&self, agent_name: &str, processing_time: Duration) {
        let mut metrics = self.metrics.write().await;
        
        // Update global metrics
        metrics.total_requests += 1;
        
        // Update agent-specific metrics
        let stats = metrics.agent_stats.entry(agent_name.to_string())
            .or_insert_with(AgentPerformanceStats::default);
        
        stats.request_count += 1;
        stats.error_count += 1;
        stats.last_request_time = std::time::SystemTime::now();
        
        // Still track processing time for failed requests
        if processing_time < stats.min_response_time {
            stats.min_response_time = processing_time;
        }
        if processing_time > stats.max_response_time {
            stats.max_response_time = processing_time;
        }
    }

    /// Get performance metrics for all agents
    pub async fn get_metrics(&self) -> AgentMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Get performance metrics for a specific agent
    pub async fn get_agent_metrics(&self, agent_name: &str) -> Option<AgentPerformanceStats> {
        let metrics = self.metrics.read().await;
        metrics.agent_stats.get(agent_name).cloned()
    }

    /// Reset performance metrics
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = AgentMetrics::default();
        info!("Agent performance metrics reset");
    }

    /// Record user satisfaction score for an agent's response
    pub async fn record_satisfaction(&self, agent_name: &str, score: f32) {
        if score < 0.0 || score > 1.0 {
            warn!("Invalid satisfaction score: {}. Score should be between 0.0 and 1.0", score);
            return;
        }
        
        let mut metrics = self.metrics.write().await;
        if let Some(stats) = metrics.agent_stats.get_mut(agent_name) {
            stats.satisfaction_scores.push(score);
            
            // Calculate new average satisfaction
            let sum: f32 = stats.satisfaction_scores.iter().sum();
            stats.avg_satisfaction = sum / stats.satisfaction_scores.len() as f32;
            
            // Keep only last 100 satisfaction scores to prevent unbounded growth
            if stats.satisfaction_scores.len() > 100 {
                stats.satisfaction_scores.remove(0);
                let sum: f32 = stats.satisfaction_scores.iter().sum();
                stats.avg_satisfaction = sum / stats.satisfaction_scores.len() as f32;
            }
        }
    }

    /// Get agent performance summary for routing decisions
    pub async fn get_routing_recommendations(&self) -> HashMap<String, f32> {
        let metrics = self.metrics.read().await;
        let mut recommendations = HashMap::new();
        
        for (agent_name, stats) in &metrics.agent_stats {
            // Calculate a composite score based on:
            // - Success rate (40%)
            // - Average response time (30%)
            // - User satisfaction (20%)
            // - Current load (10%)
            
            let success_rate = if stats.request_count > 0 {
                stats.success_count as f32 / stats.request_count as f32
            } else {
                1.0 // No data yet, assume perfect
            };
            
            // Normalize response time (lower is better, scale to 0-1)
            let response_score = if stats.avg_response_time.is_zero() {
                1.0
            } else {
                let millis = stats.avg_response_time.as_millis() as f32;
                (1000.0 - millis.min(1000.0)) / 1000.0 // Cap at 1 second
            };
            
            // Current load penalty (lower load is better)
            let load_score = if stats.current_load == 0 {
                1.0
            } else {
                (10.0 - stats.current_load as f32).max(0.0) / 10.0
            };
            
            let composite_score = 
                success_rate * 0.4 +
                response_score * 0.3 +
                stats.avg_satisfaction * 0.2 +
                load_score * 0.1;
            
            recommendations.insert(agent_name.clone(), composite_score);
        }
        
        recommendations
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}

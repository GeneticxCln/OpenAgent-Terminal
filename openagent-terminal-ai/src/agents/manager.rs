use super::{
    AiAgent, AgentRequest, AgentResponse, AgentError, AgentCapabilities,
    CollaborationContext, PrivacyLevel, ProjectInfo
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

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
    pub async fn get_agent_capabilities(&self, name: &str) -> Result<AgentCapabilities, AgentError> {
        let agents = self.agents.read().await;
        
        agents.get(name)
            .map(|agent| agent.capabilities())
            .ok_or_else(|| AgentError::AgentNotFound(name.to_string()))
    }
    
    /// Process a request by routing it to the appropriate agent(s)
    pub async fn process_request(&self, request: AgentRequest) -> Result<AgentResponse, AgentError> {
        debug!("Processing agent request: {:?}", request);
        
        // Determine the best agent for this request
        let agent_name = self.route_request(&request).await?;
        
        // Get the agent
        let agents = self.agents.read().await;
        let agent = agents.get(&agent_name)
            .ok_or_else(|| AgentError::AgentNotFound(agent_name.clone()))?;
        
        // Check if agent can handle the request
        if !agent.can_handle(&request) {
            return Err(AgentError::NotSupported(
                format!("Agent {} cannot handle this request type", agent_name)
            ));
        }
        
        // Process the request with timeout
        let response = tokio::time::timeout(
            std::time::Duration::from_millis(self.config.request_timeout_ms),
            agent.process(request)
        ).await
        .map_err(|_| AgentError::ProcessingError("Request timeout".to_string()))??;
        
        info!("Request processed successfully by agent: {}", agent_name);
        Ok(response)
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
                "Collaboration is disabled".to_string()
            ));
        }
        
        if agent_names.len() > self.config.max_concurrent_agents {
            return Err(AgentError::ConfigurationError(
                format!("Too many agents requested: {} > {}", 
                    agent_names.len(), 
                    self.config.max_concurrent_agents)
            ));
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
            futures::future::join_all(tasks).await;
        
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
            return Err(AgentError::CollaborationFailed(
                format!("All agents failed: {:?}", errors)
            ));
        }
        
        // Synthesize collaboration result
        self.synthesize_collaboration_result(successful_results, goal).await
    }
    
    /// Route a request to the most appropriate agent
    async fn route_request(&self, request: &AgentRequest) -> Result<String, AgentError> {
        // Apply routing rules in priority order
        for rule in &self.routing_rules {
            if self.matches_pattern(&rule.pattern, request) {
                debug!("Request routed to {} via rule {:?}", rule.target_agent, rule.pattern);
                return Ok(rule.target_agent.clone());
            }
        }
        
        // Fallback to default agent
        debug!("Request routed to default agent: {}", self.config.default_agent);
        Ok(self.config.default_agent.clone())
    }
    
    /// Check if a request matches a routing pattern
    fn matches_pattern(&self, pattern: &RequestPattern, request: &AgentRequest) -> bool {
        match pattern {
            RequestPattern::RequestType(req_type) => {
                match (req_type.as_str(), request) {
                    ("command", AgentRequest::Command(_)) => true,
                    ("code", AgentRequest::CodeGeneration { .. }) => true,
                    ("context", AgentRequest::ProjectContext { .. }) => true,
                    ("quality", AgentRequest::Quality { .. }) => true,
                    ("collaboration", AgentRequest::Collaboration { .. }) => true,
                    _ => false,
                }
            }
            RequestPattern::Keywords(keywords) => {
                let request_text = self.extract_text_from_request(request);
                keywords.iter().any(|keyword| {
                    request_text.to_lowercase().contains(&keyword.to_lowercase())
                })
            }
            RequestPattern::Language(lang) => {
                match request {
                    AgentRequest::CodeGeneration { language, .. } => {
                        language.as_ref().map_or(false, |l| l == lang)
                    }
                    AgentRequest::Quality { language, .. } => {
                        language.as_ref().map_or(false, |l| l == lang)
                    }
                    _ => false,
                }
            }
            RequestPattern::ProjectType(proj_type) => {
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
        goal: String,
    ) -> Result<AgentResponse, AgentError> {
        let participating_agents: Vec<String> = results.iter()
            .map(|(name, _)| name.clone())
            .collect();
        
        // Simple synthesis - in a real implementation, this would be more sophisticated
        let combined_result = results.iter()
            .map(|(agent_name, response)| {
                format!("{}: {:?}", agent_name, response)
            })
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
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}
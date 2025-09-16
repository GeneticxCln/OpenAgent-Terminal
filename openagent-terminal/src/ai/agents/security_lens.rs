// Security Lens Agent
// Specialized AI agent for analyzing security risks in commands and code

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::*;
use openagent_terminal_ai::AiProvider;

/// Specialized agent for security analysis
pub struct SecurityLensAgent {
    id: String,
    name: String,
    ai_provider: Option<Box<dyn AiProvider>>,
    config: SecurityLensConfig,
    is_initialized: bool,
    last_activity: chrono::DateTime<chrono::Utc>,
    risk_patterns: Vec<SecurityPattern>,
}

/// Configuration for security analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityLensConfig {
    pub risk_tolerance: RiskTolerance,
    pub enable_ai_analysis: bool,
    pub enable_pattern_matching: bool,
    pub block_critical_risks: bool,
    pub warn_on_medium_risks: bool,
    pub custom_patterns: Vec<SecurityPattern>,
}

/// Risk tolerance levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskTolerance {
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}

/// Security risk levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SecurityRiskLevel {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// Security patterns for pattern-based detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPattern {
    pub name: String,
    pub pattern: String, // Regex pattern
    pub risk_level: SecurityRiskLevel,
    pub description: String,
    pub mitigation: Option<String>,
}

/// Security analysis request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAnalysisRequest {
    pub content: String,
    pub content_type: SecurityContentType,
    pub context: Option<String>,
    pub skip_patterns: Vec<String>,
}

/// Types of content that can be analyzed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityContentType {
    Command,
    Script,
    Code,
    Configuration,
    Environment,
    Network,
}

/// Security analysis response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAnalysisResponse {
    pub overall_risk: SecurityRiskLevel,
    pub risks: Vec<SecurityRisk>,
    pub safe_alternatives: Vec<String>,
    pub recommendations: Vec<String>,
    pub confidence_score: f32,
}

/// Individual security risk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRisk {
    pub id: String,
    pub risk_level: SecurityRiskLevel,
    pub category: String,
    pub description: String,
    pub location: Option<String>,
    pub mitigation: Option<String>,
    pub pattern_match: bool,
}

impl Default for SecurityLensConfig {
    fn default() -> Self {
        Self {
            risk_tolerance: RiskTolerance::Medium,
            enable_ai_analysis: true,
            enable_pattern_matching: true,
            block_critical_risks: true,
            warn_on_medium_risks: true,
            custom_patterns: vec![],
        }
    }
}

impl SecurityLensAgent {
    pub fn new() -> Self {
        Self {
            id: "security-lens".to_string(),
            name: "Security Lens Agent".to_string(),
            ai_provider: None,
            config: SecurityLensConfig::default(),
            is_initialized: false,
            last_activity: chrono::Utc::now(),
            risk_patterns: Self::default_security_patterns(),
        }
    }

    pub fn with_ai_provider(mut self, ai_provider: Box<dyn AiProvider>) -> Self {
        self.ai_provider = Some(ai_provider);
        self
    }

    pub fn with_config(mut self, config: SecurityLensConfig) -> Self {
        self.config = config;
        self
    }

    /// Default security patterns for common risks
    fn default_security_patterns() -> Vec<SecurityPattern> {
        vec![
            SecurityPattern {
                name: "sudo_command".to_string(),
                pattern: r"(?i)^sudo\s+".to_string(),
                risk_level: SecurityRiskLevel::High,
                description: "Command requires elevated privileges".to_string(),
                mitigation: Some("Review the command carefully before execution".to_string()),
            },
            SecurityPattern {
                name: "rm_recursive".to_string(),
                pattern: r"(?i)rm\s+(-[^\\s]*r|--recursive)".to_string(),
                risk_level: SecurityRiskLevel::Critical,
                description: "Recursive file deletion command".to_string(),
                mitigation: Some("Use specific paths and consider --interactive flag".to_string()),
            },
            SecurityPattern {
                name: "chmod_777".to_string(),
                pattern: r"chmod\s+777".to_string(),
                risk_level: SecurityRiskLevel::High,
                description: "Setting overly permissive file permissions".to_string(),
                mitigation: Some("Use more restrictive permissions like 755 or 644".to_string()),
            },
            SecurityPattern {
                name: "curl_pipe_shell".to_string(),
                pattern: r"(?i)(curl|wget).*\|\s*(bash|sh|zsh|fish)".to_string(),
                risk_level: SecurityRiskLevel::Critical,
                description: "Downloading and executing remote scripts".to_string(),
                mitigation: Some(
                    "Download first, review content, then execute manually".to_string(),
                ),
            },
            SecurityPattern {
                name: "exposed_api_key".to_string(),
                pattern:
                    r"(?i)(api[_-]?key|secret|token|password)\s*[=:]\s*['\x22][^'\x22]+['\x22]"
                        .to_string(),
                risk_level: SecurityRiskLevel::Critical,
                description: "Exposed API key or secret in plain text".to_string(),
                mitigation: Some("Use environment variables or secure storage".to_string()),
            },
            SecurityPattern {
                name: "docker_privileged".to_string(),
                pattern: r"(?i)docker\s+run.*--privileged".to_string(),
                risk_level: SecurityRiskLevel::High,
                description: "Running Docker container with privileged access".to_string(),
                mitigation: Some("Avoid privileged mode unless absolutely necessary".to_string()),
            },
            SecurityPattern {
                name: "system_directories".to_string(),
                pattern: r"(?i)(rm|mv|cp).*(/bin|/sbin|/usr|/etc|/boot|/sys|/proc)".to_string(),
                risk_level: SecurityRiskLevel::Critical,
                description: "Operations on critical system directories".to_string(),
                mitigation: Some("Extreme caution required - could break the system".to_string()),
            },
            SecurityPattern {
                name: "network_exposure".to_string(),
                pattern: r"(?i)(nc|netcat).*-l.*0\.0\.0\.0".to_string(),
                risk_level: SecurityRiskLevel::Medium,
                description: "Opening network listener on all interfaces".to_string(),
                mitigation: Some("Bind to specific interfaces like 127.0.0.1".to_string()),
            },
        ]
    }

    /// Analyze content using pattern matching
    fn analyze_with_patterns(&self, content: &str, skip_patterns: &[String]) -> Vec<SecurityRisk> {
        let mut risks = Vec::new();

        for pattern in &self.risk_patterns {
            if skip_patterns.contains(&pattern.name) {
                continue;
            }

            if let Ok(regex) = Regex::new(&pattern.pattern) {
                if let Some(match_result) = regex.find(content) {
                    risks.push(SecurityRisk {
                        id: Uuid::new_v4().to_string(),
                        risk_level: pattern.risk_level.clone(),
                        category: pattern.name.clone(),
                        description: pattern.description.clone(),
                        location: Some(format!(
                            "Position {}-{}",
                            match_result.start(),
                            match_result.end()
                        )),
                        mitigation: pattern.mitigation.clone(),
                        pattern_match: true,
                    });
                }
            }
        }

        // Add custom patterns from config
        for custom_pattern in &self.config.custom_patterns {
            if skip_patterns.contains(&custom_pattern.name) {
                continue;
            }

            if let Ok(regex) = Regex::new(&custom_pattern.pattern) {
                if let Some(match_result) = regex.find(content) {
                    risks.push(SecurityRisk {
                        id: Uuid::new_v4().to_string(),
                        risk_level: custom_pattern.risk_level.clone(),
                        category: custom_pattern.name.clone(),
                        description: custom_pattern.description.clone(),
                        location: Some(format!(
                            "Position {}-{}",
                            match_result.start(),
                            match_result.end()
                        )),
                        mitigation: custom_pattern.mitigation.clone(),
                        pattern_match: true,
                    });
                }
            }
        }

        risks
    }

    /// Create system prompt for AI-based security analysis
    fn create_security_analysis_prompt(&self, request: &SecurityAnalysisRequest) -> String {
        let mut prompt = String::new();

        prompt.push_str("You are a cybersecurity expert analyzing ");
        prompt.push_str(&format!("{:?}", request.content_type).to_lowercase());
        prompt.push_str(" for security risks and vulnerabilities.\n\n");

        prompt.push_str("Analyze the following content and identify:\n");
        prompt.push_str("1. Security risks and vulnerabilities\n");
        prompt.push_str("2. Potential attack vectors\n");
        prompt.push_str("3. Best practices violations\n");
        prompt.push_str("4. Privacy concerns\n");
        prompt.push_str("5. Safe alternatives where applicable\n\n");

        prompt.push_str("Consider these risk categories:\n");
        prompt.push_str("- Privilege escalation\n");
        prompt.push_str("- Data exposure\n");
        prompt.push_str("- System damage\n");
        prompt.push_str("- Network security\n");
        prompt.push_str("- Code injection\n");
        prompt.push_str("- Authentication bypasses\n\n");

        if let Some(context) = &request.context {
            prompt.push_str(&format!("Additional context: {}\n\n", context));
        }

        prompt.push_str("Return your analysis in this JSON format:\n");
        prompt.push_str("{\n");
        prompt.push_str("  \"overall_risk\": \"Critical|High|Medium|Low|Info\",\n");
        prompt.push_str("  \"risks\": [\n");
        prompt.push_str("    {\n");
        prompt.push_str("      \"id\": \"unique_id\",\n");
        prompt.push_str("      \"risk_level\": \"Critical|High|Medium|Low|Info\",\n");
        prompt.push_str("      \"category\": \"risk_category\",\n");
        prompt.push_str("      \"description\": \"detailed_description\",\n");
        prompt.push_str("      \"location\": \"where_in_content\",\n");
        prompt.push_str("      \"mitigation\": \"how_to_mitigate\",\n");
        prompt.push_str("      \"pattern_match\": false\n");
        prompt.push_str("    }\n");
        prompt.push_str("  ],\n");
        prompt.push_str("  \"safe_alternatives\": [\"alternative_1\", \"alternative_2\"],\n");
        prompt.push_str("  \"recommendations\": [\"recommendation_1\", \"recommendation_2\"],\n");
        prompt.push_str("  \"confidence_score\": 0.85\n");
        prompt.push_str("}\n");

        prompt
    }

    /// Perform AI-based security analysis
    async fn analyze_with_ai(
        &self,
        request: &SecurityAnalysisRequest,
    ) -> Result<SecurityAnalysisResponse> {
        let system_prompt = self.create_security_analysis_prompt(request);
        let user_prompt = format!("Content to analyze:\n```\n{}\n```", request.content);

        let response = if let Some(provider) = &self.ai_provider {
            let ai_request = openagent_terminal_ai::AiRequest {
                scratch_text: format!("{}\n\n{}", system_prompt, user_prompt),
                working_directory: None,
                shell_kind: None,
                context: vec![
                    ("mode".to_string(), "security_analysis".to_string()),
                    (
                        "content_type".to_string(),
                        format!("{:?}", request.content_type),
                    ),
                ],
            };

            let proposals = provider
                .propose(ai_request)
                .map_err(|e| anyhow!("AI provider error: {}", e))?;
            proposals
                .first()
                .ok_or_else(|| anyhow!("No response from AI provider"))?
                .proposed_commands
                .join("\n")
        } else {
            // Return mock response when no AI provider is available
            serde_json::json!({
                "overall_risk": "Medium",
                "risks": [],
                "safe_alternatives": [],
                "recommendations": ["Consider reviewing the content manually for security issues"],
                "confidence_score": 0.5
            })
            .to_string()
        };

        // Parse the JSON response
        let parsed: SecurityAnalysisResponse = serde_json::from_str(&response)
            .map_err(|e| anyhow!("Failed to parse security analysis response: {}", e))?;

        // Convert string risk levels to enum
        // (This would be handled by custom deserialization in a full implementation)

        Ok(parsed)
    }

    /// Combine pattern and AI analysis results
    fn combine_analysis_results(
        &self,
        pattern_risks: Vec<SecurityRisk>,
        ai_response: Option<SecurityAnalysisResponse>,
    ) -> SecurityAnalysisResponse {
        let mut all_risks = pattern_risks;
        let mut safe_alternatives = Vec::new();
        let mut recommendations = Vec::new();
        let mut confidence_score = 0.9; // High confidence for pattern matching

        // Merge AI analysis if available
        if let Some(ai_result) = ai_response {
            all_risks.extend(ai_result.risks);
            safe_alternatives.extend(ai_result.safe_alternatives);
            recommendations.extend(ai_result.recommendations);
            confidence_score = (confidence_score + ai_result.confidence_score) / 2.0;
        }

        // Determine overall risk level
        let overall_risk = if all_risks
            .iter()
            .any(|r| r.risk_level == SecurityRiskLevel::Critical)
        {
            SecurityRiskLevel::Critical
        } else if all_risks
            .iter()
            .any(|r| r.risk_level == SecurityRiskLevel::High)
        {
            SecurityRiskLevel::High
        } else if all_risks
            .iter()
            .any(|r| r.risk_level == SecurityRiskLevel::Medium)
        {
            SecurityRiskLevel::Medium
        } else if all_risks
            .iter()
            .any(|r| r.risk_level == SecurityRiskLevel::Low)
        {
            SecurityRiskLevel::Low
        } else {
            SecurityRiskLevel::Info
        };

        SecurityAnalysisResponse {
            overall_risk,
            risks: all_risks,
            safe_alternatives,
            recommendations,
            confidence_score,
        }
    }

    /// Check if execution should be blocked based on risk level and config
    fn should_block_execution(&self, risk_level: &SecurityRiskLevel) -> bool {
        match risk_level {
            SecurityRiskLevel::Critical => self.config.block_critical_risks,
            SecurityRiskLevel::High => match self.config.risk_tolerance {
                RiskTolerance::VeryLow | RiskTolerance::Low => true,
                _ => false,
            },
            SecurityRiskLevel::Medium => match self.config.risk_tolerance {
                RiskTolerance::VeryLow => true,
                _ => false,
            },
            _ => false,
        }
    }

    /// Create suggested actions based on security analysis
    fn create_security_actions(&self, analysis: &SecurityAnalysisResponse) -> Vec<SuggestedAction> {
        let mut actions = Vec::new();

        // Suggest blocking execution if needed
        if self.should_block_execution(&analysis.overall_risk) {
            actions.push(SuggestedAction {
                action_type: ActionType::Custom("block_execution".to_string()),
                description: format!(
                    "Block execution due to {} risk",
                    format!("{:?}", analysis.overall_risk).to_lowercase()
                ),
                command: None,
                priority: ActionPriority::Critical,
                safe_to_auto_execute: true, // Auto-block for safety
            });
        }

        // Suggest safe alternatives
        for alternative in &analysis.safe_alternatives {
            actions.push(SuggestedAction {
                action_type: ActionType::Custom("safe_alternative".to_string()),
                description: format!("Use safer alternative: {}", alternative),
                command: Some(alternative.clone()),
                priority: ActionPriority::High,
                safe_to_auto_execute: false,
            });
        }

        actions
    }
}

#[async_trait]
impl Agent for SecurityLensAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Analyzes commands and code for security risks and vulnerabilities"
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::SecurityAnalysis,
            AgentCapability::CodeAnalysis,
        ]
    }

    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        if !self.is_initialized {
            return Err(anyhow!("Agent not initialized"));
        }

        match request.request_type {
            AgentRequestType::CheckSecurity => {
                let security_request: SecurityAnalysisRequest =
                    serde_json::from_value(request.payload)
                        .map_err(|e| anyhow!("Invalid security analysis request: {}", e))?;

                // Pattern-based analysis (always performed)
                let pattern_risks = if self.config.enable_pattern_matching {
                    self.analyze_with_patterns(
                        &security_request.content,
                        &security_request.skip_patterns,
                    )
                } else {
                    vec![]
                };

                // AI-based analysis (optional)
                let ai_analysis = if self.config.enable_ai_analysis {
                    match self.analyze_with_ai(&security_request).await {
                        Ok(analysis) => Some(analysis),
                        Err(e) => {
                            tracing::warn!("AI security analysis failed: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };

                // Combine results
                let analysis = self.combine_analysis_results(pattern_risks, ai_analysis);
                let actions = self.create_security_actions(&analysis);

                // Create artifacts for detailed reporting
                let artifacts = vec![AgentArtifact {
                    id: Uuid::new_v4(),
                    artifact_type: ArtifactType::Report,
                    content: serde_json::to_string_pretty(&analysis)?,
                    metadata: {
                        let mut meta = HashMap::new();
                        meta.insert("type".to_string(), "security_analysis".to_string());
                        meta.insert(
                            "risk_level".to_string(),
                            format!("{:?}", analysis.overall_risk),
                        );
                        meta.insert("risk_count".to_string(), analysis.risks.len().to_string());
                        meta
                    },
                }];

                Ok(AgentResponse {
                    request_id: request.id,
                    agent_id: self.id.clone(),
                    success: true,
                    payload: serde_json::to_value(&analysis)?,
                    artifacts,
                    next_actions: actions,
                    metadata: HashMap::new(),
                })
            }
            _ => Err(anyhow!(
                "Unsupported request type: {:?}",
                request.request_type
            )),
        }
    }

    fn can_handle(&self, request_type: &AgentRequestType) -> bool {
        matches!(request_type, AgentRequestType::CheckSecurity)
    }

    async fn status(&self) -> AgentStatus {
        AgentStatus {
            is_healthy: self.is_initialized,
            is_busy: false,
            last_activity: self.last_activity,
            current_task: None,
            error_message: None,
        }
    }

    async fn initialize(&mut self, config: AgentConfig) -> Result<()> {
        // Load custom security configuration
        if let Some(security_config) = config.custom_settings.get("security_lens") {
            if let Ok(parsed_config) =
                serde_json::from_value::<SecurityLensConfig>(security_config.clone())
            {
                self.config = parsed_config;
            }
        }

        // Load custom patterns
        self.risk_patterns
            .extend(self.config.custom_patterns.clone());

        self.is_initialized = true;
        self.last_activity = chrono::Utc::now();

        tracing::info!(
            "Security Lens Agent initialized with {} patterns",
            self.risk_patterns.len()
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.is_initialized = false;
        tracing::info!("Security Lens Agent shut down");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_security_lens_agent_creation() {
        let agent = SecurityLensAgent::new();

        assert_eq!(agent.id(), "security-lens");
        assert_eq!(agent.name(), "Security Lens Agent");
        assert!(agent
            .capabilities()
            .contains(&AgentCapability::SecurityAnalysis));
        assert!(agent.can_handle(&AgentRequestType::CheckSecurity));
        assert!(!agent.can_handle(&AgentRequestType::GenerateCode));
    }

    #[test]
    fn test_pattern_analysis() {
        let agent = SecurityLensAgent::new();

        // Test dangerous command detection
        let dangerous_command = "sudo rm -rf /";
        let risks = agent.analyze_with_patterns(dangerous_command, &[]);

        assert!(!risks.is_empty());
        assert!(risks.iter().any(|r| r.category == "sudo_command"));
        assert!(risks.iter().any(|r| r.category == "rm_recursive"));
        assert!(risks
            .iter()
            .any(|r| r.risk_level == SecurityRiskLevel::Critical));
    }

    #[test]
    fn test_api_key_detection() {
        let agent = SecurityLensAgent::new();

        let code_with_secret = r#"API_KEY="sk-1234567890abcdef""#;
        let risks = agent.analyze_with_patterns(code_with_secret, &[]);

        assert!(!risks.is_empty());
        assert!(risks.iter().any(|r| r.category == "exposed_api_key"));
        assert!(risks
            .iter()
            .any(|r| r.risk_level == SecurityRiskLevel::Critical));
    }

    #[test]
    fn test_risk_level_determination() {
        let agent = SecurityLensAgent::new();

        let pattern_risks = vec![SecurityRisk {
            id: "test1".to_string(),
            risk_level: SecurityRiskLevel::Medium,
            category: "test".to_string(),
            description: "Test risk".to_string(),
            location: None,
            mitigation: None,
            pattern_match: true,
        }];

        let result = agent.combine_analysis_results(pattern_risks, None);
        assert_eq!(result.overall_risk, SecurityRiskLevel::Medium);
    }
}

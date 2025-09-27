//! Production-ready Security Lens system for command analysis and risk assessment
//! 
//! This module provides comprehensive security analysis for terminal commands,
//! including pattern matching, risk classification, and policy enforcement.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use regex::Regex;
use anyhow::{Result, Context};

/// Risk levels for security assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord, Default)]
pub enum RiskLevel {
    #[default]
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Safe => write!(f, "Safe"),
            RiskLevel::Low => write!(f, "Low"),
            RiskLevel::Medium => write!(f, "Medium"),
            RiskLevel::High => write!(f, "High"),
            RiskLevel::Critical => write!(f, "Critical"),
        }
    }
}


/// Risk factors contributing to the overall assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    pub category: String,
    pub description: String,
    pub severity: RiskLevel,
    pub pattern: String,
}

// For backward compatibility
pub type CommandRiskFactor = RiskFactor;

/// Complete risk assessment for a command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRisk {
    pub level: RiskLevel,
    pub explanation: String,
    pub factors: Vec<RiskFactor>,
    pub mitigations: Vec<String>,
    pub confidence: f32, // 0.0 - 1.0
}

impl Default for CommandRisk {
    fn default() -> Self {
        Self {
            level: RiskLevel::Safe,
            explanation: "Command appears safe".to_string(),
            factors: Vec::new(),
            mitigations: Vec::new(),
            confidence: 1.0,
        }
    }
}

/// Security policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub enabled: bool,
    pub block_critical: bool,
    pub require_confirmation: HashMap<RiskLevel, bool>,
    pub custom_patterns: Vec<CustomPattern>,
    pub whitelisted_commands: Vec<String>,
    pub blacklisted_patterns: Vec<String>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        let mut require_confirmation = HashMap::new();
        require_confirmation.insert(RiskLevel::Safe, false);
        require_confirmation.insert(RiskLevel::Low, false);
        require_confirmation.insert(RiskLevel::Medium, true);
        require_confirmation.insert(RiskLevel::High, true);
        require_confirmation.insert(RiskLevel::Critical, true);

        Self {
            enabled: true,
            block_critical: false,
            require_confirmation,
            custom_patterns: Vec::new(),
            whitelisted_commands: vec![
                "ls".to_string(),
                "pwd".to_string(),
                "whoami".to_string(),
                "date".to_string(),
                "cat".to_string(),
                "echo".to_string(),
                "which".to_string(),
                "type".to_string(),
            ],
            blacklisted_patterns: Vec::new(),
        }
    }
}

impl SecurityPolicy {
    /// Create a conservative security policy
    pub fn with_defaults() -> Self {
        Self::default()
    }

    /// Create a conservative policy that blocks more operations
    pub fn preset_conservative() -> Self {
        let mut policy = Self { block_critical: true, ..Self::default() };
        policy.require_confirmation.insert(RiskLevel::Low, true);
        policy
    }

    /// Create a permissive policy for advanced users
    pub fn preset_permissive() -> Self {
        let mut policy = Self::default();
        policy.require_confirmation.insert(RiskLevel::Medium, false);
        policy.require_confirmation.insert(RiskLevel::High, false);
        policy
    }

    /// Create a disabled policy (all commands pass through)
    pub fn preset_disabled() -> Self {
        Self { enabled: false, ..Self::default() }
    }
}

/// Custom security pattern for domain-specific rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPattern {
    pub name: String,
    pub pattern: String,
    pub risk_level: RiskLevel,
    pub description: String,
    #[serde(skip)]
    pub regex: Option<Regex>,
}

/// Main security lens analyzer
pub struct SecurityLens {
    policy: SecurityPolicy,
    critical_patterns: Vec<Regex>,
    high_risk_patterns: Vec<Regex>,
    medium_risk_patterns: Vec<Regex>,
    low_risk_patterns: Vec<Regex>,
    custom_patterns: Vec<CustomPattern>,
}

impl SecurityLens {
    /// Create a new security lens with the given policy
    pub fn new(policy: SecurityPolicy) -> Self {
        let mut lens = Self {
            policy,
            critical_patterns: Vec::new(),
            high_risk_patterns: Vec::new(),
            medium_risk_patterns: Vec::new(),
            low_risk_patterns: Vec::new(),
            custom_patterns: Vec::new(),
        };
        lens.initialize_patterns();
        lens
    }

    /// Initialize built-in security patterns
    fn initialize_patterns(&mut self) {
        // Critical patterns - extremely dangerous operations
        let critical_patterns = vec![
            r"rm\s+-rf\s+/",
            r"rm\s+-rf\s+\*",
            r":\(\)\{\s*:\|\:&\s*\}",  // Fork bomb
            r"dd\s+if=/dev/zero\s+of=/",
            r"format\s+c:",
            r"mkfs\.",
            r"fdisk\s+/dev/",
            r"parted\s+/dev/",
        ];

        // High risk patterns - operations requiring elevated privileges or system changes
        let high_risk_patterns = vec![
            r"sudo\s+",
            r"chmod\s+777",
            r"chown\s+root",
            r"systemctl\s+(stop|disable|mask)",
            r"service\s+\w+\s+stop",
            r"iptables\s+-F",
            r"ufw\s+disable",
            r"setenforce\s+0",
            r"mount\s+",
            r"umount\s+",
            r"crontab\s+-r",
            r"passwd\s+",
        ];

        // Medium risk patterns - network operations, package management
        let medium_risk_patterns = vec![
            r"curl\s+.*\|\s*bash",
            r"wget\s+.*\|\s*bash",
            r"apt\s+(remove|purge)",
            r"yum\s+(remove|erase)",
            r"pip\s+uninstall",
            r"npm\s+uninstall\s+-g",
            r"docker\s+(rm|rmi)\s+",
            r"git\s+reset\s+--hard",
            r"git\s+clean\s+-fd",
        ];

        // Low risk patterns - development operations that could affect work
        let low_risk_patterns = vec![
            r"git\s+push\s+--force",
            r"rm\s+-rf\s+\w+",
            r"truncate\s+-s\s*0",
            r">\s*/dev/null",
        ];

        // Compile patterns
        for pattern in critical_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                self.critical_patterns.push(regex);
            }
        }
        for pattern in high_risk_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                self.high_risk_patterns.push(regex);
            }
        }
        for pattern in medium_risk_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                self.medium_risk_patterns.push(regex);
            }
        }
        for pattern in low_risk_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                self.low_risk_patterns.push(regex);
            }
        }

        // Compile custom patterns
        for pattern in &mut self.policy.custom_patterns {
            if let Ok(regex) = Regex::new(&pattern.pattern) {
                pattern.regex = Some(regex);
                self.custom_patterns.push(pattern.clone());
            }
        }
    }

    /// Analyze a command for security risks
    pub fn analyze_command(&mut self, command: &str) -> CommandRisk {
        if !self.policy.enabled {
            return CommandRisk::default();
        }

        // Check if command is whitelisted
        let cmd_parts: Vec<&str> = command.split_whitespace().collect();
        if let Some(first_cmd) = cmd_parts.first() {
            if self.policy.whitelisted_commands.contains(&first_cmd.to_string()) {
                return CommandRisk::default();
            }
        }

        // Check blacklisted patterns first
        for pattern in &self.policy.blacklisted_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                if regex.is_match(command) {
                    return CommandRisk {
                        level: RiskLevel::Critical,
                        explanation: "Command matches blacklisted pattern".to_string(),
                        factors: vec![RiskFactor {
                            category: "Blacklist".to_string(),
                            description: format!("Matches pattern: {}", pattern),
                            severity: RiskLevel::Critical,
                            pattern: pattern.clone(),
                        }],
                        mitigations: vec!["Remove from blacklist if command is safe".to_string()],
                        confidence: 1.0,
                    };
                }
            }
        }

        let mut risk = CommandRisk::default();
        let mut factors = Vec::new();

        // Check custom patterns first
        for pattern in &self.custom_patterns {
            if let Some(regex) = &pattern.regex {
                if regex.is_match(command) {
                    factors.push(RiskFactor {
                        category: "Custom".to_string(),
                        description: pattern.description.clone(),
                        severity: pattern.risk_level,
                        pattern: pattern.pattern.clone(),
                    });
                    if pattern.risk_level as u8 > risk.level as u8 {
                        risk.level = pattern.risk_level;
                    }
                }
            }
        }

        // Check built-in patterns
        for regex in &self.critical_patterns {
            if regex.is_match(command) {
                factors.push(RiskFactor {
                    category: "System Destruction".to_string(),
                    description: "Command could cause irreversible system damage".to_string(),
                    severity: RiskLevel::Critical,
                    pattern: regex.as_str().to_string(),
                });
                risk.level = RiskLevel::Critical;
            }
        }

        for regex in &self.high_risk_patterns {
            if regex.is_match(command) {
                factors.push(RiskFactor {
                    category: "Privileged Operation".to_string(),
                    description: "Command requires elevated privileges or modifies system".to_string(),
                    severity: RiskLevel::High,
                    pattern: regex.as_str().to_string(),
                });
                if (risk.level as u8) < (RiskLevel::High as u8) {
                    risk.level = RiskLevel::High;
                }
            }
        }

        for regex in &self.medium_risk_patterns {
            if regex.is_match(command) {
                factors.push(RiskFactor {
                    category: "Network/Package Operation".to_string(),
                    description: "Command performs network operations or package management".to_string(),
                    severity: RiskLevel::Medium,
                    pattern: regex.as_str().to_string(),
                });
                if (risk.level as u8) < (RiskLevel::Medium as u8) {
                    risk.level = RiskLevel::Medium;
                }
            }
        }

        for regex in &self.low_risk_patterns {
            if regex.is_match(command) {
                factors.push(RiskFactor {
                    category: "Development Operation".to_string(),
                    description: "Command could affect development work".to_string(),
                    severity: RiskLevel::Low,
                    pattern: regex.as_str().to_string(),
                });
                if (risk.level as u8) < (RiskLevel::Low as u8) {
                    risk.level = RiskLevel::Low;
                }
            }
        }

        risk.factors = factors;
        risk.explanation = self.generate_explanation(&risk);
        risk.mitigations = self.generate_mitigations(&risk);
        risk.confidence = self.calculate_confidence(&risk);

        risk
    }

    /// Check if a command should be blocked based on policy
    pub fn should_block(&self, risk: &CommandRisk) -> bool {
        if !self.policy.enabled {
            return false;
        }

        if self.policy.block_critical && risk.level == RiskLevel::Critical {
            return true;
        }

        self.policy.require_confirmation.get(&risk.level).copied().unwrap_or(false)
    }

    /// Generate human-readable explanation for the risk assessment
    fn generate_explanation(&self, risk: &CommandRisk) -> String {
        match risk.level {
            RiskLevel::Safe => "Command appears safe to execute".to_string(),
            RiskLevel::Low => "Command has low risk but may affect your work".to_string(),
            RiskLevel::Medium => "Command has moderate risk and requires attention".to_string(),
            RiskLevel::High => "Command is high-risk and could affect your system".to_string(),
            RiskLevel::Critical => "Command is extremely dangerous and could cause irreversible damage".to_string(),
        }
    }

    /// Generate mitigation suggestions
    fn generate_mitigations(&self, risk: &CommandRisk) -> Vec<String> {
        let mut mitigations = Vec::new();

        match risk.level {
            RiskLevel::Safe => {},
            RiskLevel::Low => {
                mitigations.push("Review the command carefully before execution".to_string());
                mitigations.push("Consider running in a test environment first".to_string());
            },
            RiskLevel::Medium => {
                mitigations.push("Backup important data before execution".to_string());
                mitigations.push("Run with limited permissions if possible".to_string());
                mitigations.push("Test in a non-production environment".to_string());
            },
            RiskLevel::High => {
                mitigations.push("Create a full system backup before execution".to_string());
                mitigations.push("Verify the command source and intent".to_string());
                mitigations.push("Run in an isolated environment".to_string());
                mitigations.push("Have a rollback plan ready".to_string());
            },
            RiskLevel::Critical => {
                mitigations.push("DO NOT EXECUTE without expert review".to_string());
                mitigations.push("Verify this is absolutely necessary".to_string());
                mitigations.push("Use a disposable test environment".to_string());
                mitigations.push("Have complete system restoration capability".to_string());
            },
        }

        mitigations
    }

    /// Calculate confidence in the risk assessment
    fn calculate_confidence(&self, risk: &CommandRisk) -> f32 {
        if risk.factors.is_empty() {
            0.8 // Default confidence for safe commands
        } else {
            let pattern_confidence = risk.factors.len() as f32 * 0.2;
            (0.6 + pattern_confidence).min(1.0)
        }
    }

    /// Update the security policy
    pub fn update_policy(&mut self, policy: SecurityPolicy) {
        self.policy = policy;
        self.initialize_patterns();
    }

    /// Add a custom pattern to the analyzer
    pub fn add_custom_pattern(&mut self, pattern: CustomPattern) -> Result<()> {
        let mut pattern = pattern;
        let regex = Regex::new(&pattern.pattern)
            .with_context(|| format!("Invalid regex pattern: {}", pattern.pattern))?;
        pattern.regex = Some(regex);
        
        self.custom_patterns.push(pattern.clone());
        self.policy.custom_patterns.push(pattern);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_commands() {
        let _lens = SecurityLens::new(SecurityPolicy::default());
        
        let safe_commands = vec!["ls", "pwd", "whoami", "date", "echo hello"];
        for cmd in safe_commands {
            let mut lens_mut = SecurityLens::new(SecurityPolicy::default());
            let risk = lens_mut.analyze_command(cmd);
            assert_eq!(risk.level, RiskLevel::Safe);
        }
    }

    #[test]
    fn test_critical_commands() {
        let safe_commands = vec!["rm -rf /", "dd if=/dev/zero of=/dev/sda"];
        for cmd in safe_commands {
            let mut lens = SecurityLens::new(SecurityPolicy::default());
            let risk = lens.analyze_command(cmd);
            assert_eq!(risk.level, RiskLevel::Critical);
        }
    }

    #[test]
    fn test_policy_blocking() {
        let policy = SecurityPolicy::preset_conservative();
        let lens = SecurityLens::new(policy);
        
        let mut lens_mut = SecurityLens::new(SecurityPolicy::preset_conservative());
        let risk = lens_mut.analyze_command("rm -rf /");
        assert_eq!(risk.level, RiskLevel::Critical);
        assert!(lens.should_block(&risk));
    }

    #[test]
    fn test_custom_patterns() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());
        
        let custom_pattern = CustomPattern {
            name: "Deploy to production".to_string(),
            pattern: r"deploy.*prod".to_string(),
            risk_level: RiskLevel::High,
            description: "Production deployment detected".to_string(),
            regex: None,
        };
        
        lens.add_custom_pattern(custom_pattern).unwrap();
        
        let risk = lens.analyze_command("deploy myapp prod");
        assert_eq!(risk.level, RiskLevel::High);
    }

    #[test]
    fn test_disabled_policy() {
        let policy = SecurityPolicy::preset_disabled();
        let mut lens = SecurityLens::new(policy);
        
        let risk = lens.analyze_command("rm -rf /");
        assert_eq!(risk.level, RiskLevel::Safe);
        assert!(!lens.should_block(&risk));
    }
}

// Security Lens - Command Analysis System
// Provides real-time risk assessment for terminal commands

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RiskLevel {
    Safe,
    Caution,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    pub category: String,
    pub description: String,
    pub pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRisk {
    pub level: RiskLevel,
    pub factors: Vec<RiskFactor>,
    pub explanation: String,
    pub mitigations: Vec<String>,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub enabled: bool,
    pub block_critical: bool,
    pub require_confirmation: HashMap<RiskLevel, bool>,
    pub require_reason: HashMap<RiskLevel, bool>,
    pub custom_patterns: Vec<CustomPattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPattern {
    pub pattern: String,
    pub risk_level: RiskLevel,
    pub message: String,
}

pub struct SecurityLens {
    policy: SecurityPolicy,
    dangerous_patterns: Vec<(Regex, RiskFactor, RiskLevel)>,
    sensitive_patterns: Vec<Regex>,
}

impl SecurityLens {
    pub fn new(policy: SecurityPolicy) -> Self {
        let dangerous_patterns = Self::init_dangerous_patterns();
        let sensitive_patterns = Self::init_sensitive_patterns();
        
        SecurityLens {
            policy,
            dangerous_patterns,
            sensitive_patterns,
        }
    }

    fn init_dangerous_patterns() -> Vec<(Regex, RiskFactor, RiskLevel)> {
        vec![
            // Critical: System destruction
            (
                Regex::new(r"rm\s+-rf\s+/\s*$|rm\s+-rf\s+/\*").unwrap(),
                RiskFactor {
                    category: "system_destruction".to_string(),
                    description: "Attempts to delete entire filesystem".to_string(),
                    pattern: "rm -rf /".to_string(),
                },
                RiskLevel::Critical,
            ),
            // Critical: Disk overwrite
            (
                Regex::new(r"dd\s+if=/dev/(zero|random|urandom)\s+of=/dev/[sh]d[a-z]").unwrap(),
                RiskFactor {
                    category: "disk_overwrite".to_string(),
                    description: "Direct disk overwrite operation".to_string(),
                    pattern: "dd to disk device".to_string(),
                },
                RiskLevel::Critical,
            ),
            // Critical: Fork bomb
            (
                Regex::new(r":\(\)\s*\{\s*:\|:&\s*\};:").unwrap(),
                RiskFactor {
                    category: "fork_bomb".to_string(),
                    description: "Fork bomb that can crash the system".to_string(),
                    pattern: ":(){ :|:& };:".to_string(),
                },
                RiskLevel::Critical,
            ),
            // Warning: Curl pipe to shell
            (
                Regex::new(r"curl.*\|.*sh|wget.*\|.*bash").unwrap(),
                RiskFactor {
                    category: "remote_execution".to_string(),
                    description: "Downloading and executing remote code".to_string(),
                    pattern: "curl | sh".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: Chmod 777
            (
                Regex::new(r"chmod\s+777").unwrap(),
                RiskFactor {
                    category: "permission_exposure".to_string(),
                    description: "Setting world-writable permissions".to_string(),
                    pattern: "chmod 777".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Caution: Recursive operations
            (
                Regex::new(r"(rm|chmod|chown)\s+.*-[rR]").unwrap(),
                RiskFactor {
                    category: "recursive_operation".to_string(),
                    description: "Recursive file operation".to_string(),
                    pattern: "recursive command".to_string(),
                },
                RiskLevel::Caution,
            ),
            // Warning: AWS deletion
            (
                Regex::new(r"aws\s+.*delete|aws\s+.*terminate").unwrap(),
                RiskFactor {
                    category: "cloud_deletion".to_string(),
                    description: "AWS resource deletion operation".to_string(),
                    pattern: "aws delete/terminate".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: Database drops
            (
                Regex::new(r"DROP\s+(DATABASE|TABLE|SCHEMA)").unwrap(),
                RiskFactor {
                    category: "database_deletion".to_string(),
                    description: "Database deletion operation".to_string(),
                    pattern: "DROP DATABASE/TABLE".to_string(),
                },
                RiskLevel::Warning,
            ),
        ]
    }

    fn init_sensitive_patterns() -> Vec<Regex> {
        vec![
            // API Keys
            Regex::new(r#"(?i)(api[_-]?key|apikey|api_secret)[=:\s]+['\"]?[a-zA-Z0-9_-]{8,}"#).unwrap(),
            // AWS Keys
            Regex::new(r#"AKIA[0-9A-Z]{16}"#).unwrap(),
            // Generic secrets
            Regex::new(r#"(password|passwd|pwd|secret|token)[=:\s]+['"]?[^\s'"]+"#).unwrap(),
            // SSH private keys
            Regex::new(r#"-----BEGIN (RSA|DSA|EC|OPENSSH) PRIVATE KEY-----"#).unwrap(),
            // JWT tokens
            Regex::new(r#"eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+"#).unwrap(),
        ]
    }

    pub fn analyze_command(&self, command: &str) -> CommandRisk {
        if !self.policy.enabled {
            return CommandRisk {
                level: RiskLevel::Safe,
                factors: vec![],
                explanation: "Security lens is disabled".to_string(),
                mitigations: vec![],
                requires_confirmation: false,
            };
        }

        let mut risk_factors = vec![];
        let mut highest_risk = RiskLevel::Safe;

        // Check dangerous patterns
        for (pattern, factor, risk_level) in &self.dangerous_patterns {
            if pattern.is_match(command) {
                risk_factors.push(factor.clone());
                if self.risk_level_value(risk_level) > self.risk_level_value(&highest_risk) {
                    highest_risk = *risk_level;
                }
            }
        }

        // Check for sensitive data exposure
        for pattern in &self.sensitive_patterns {
            if pattern.is_match(command) {
                risk_factors.push(RiskFactor {
                    category: "sensitive_data".to_string(),
                    description: "Command contains potential sensitive data".to_string(),
                    pattern: "sensitive data pattern".to_string(),
                });
                if self.risk_level_value(&RiskLevel::Warning) > self.risk_level_value(&highest_risk) {
                    highest_risk = RiskLevel::Warning;
                }
            }
        }

        // Check custom patterns
        for custom in &self.policy.custom_patterns {
            if let Ok(pattern) = Regex::new(&custom.pattern) {
                if pattern.is_match(command) {
                    risk_factors.push(RiskFactor {
                        category: "custom".to_string(),
                        description: custom.message.clone(),
                        pattern: custom.pattern.clone(),
                    });
                    if self.risk_level_value(&custom.risk_level) > self.risk_level_value(&highest_risk) {
                        highest_risk = custom.risk_level;
                    }
                }
            }
        }

        let explanation = self.generate_explanation(&risk_factors, &highest_risk);
        let mitigations = self.generate_mitigations(&risk_factors);
        let requires_confirmation = *self.policy.require_confirmation
            .get(&highest_risk)
            .unwrap_or(&false);

        CommandRisk {
            level: highest_risk,
            factors: risk_factors,
            explanation,
            mitigations,
            requires_confirmation,
        }
    }

    fn risk_level_value(&self, level: &RiskLevel) -> u8 {
        match level {
            RiskLevel::Safe => 0,
            RiskLevel::Caution => 1,
            RiskLevel::Warning => 2,
            RiskLevel::Critical => 3,
        }
    }

    fn generate_explanation(&self, factors: &[RiskFactor], level: &RiskLevel) -> String {
        if factors.is_empty() {
            return "Command appears safe to execute.".to_string();
        }

        let prefix = match level {
            RiskLevel::Safe => "Command is safe.",
            RiskLevel::Caution => "Command requires caution.",
            RiskLevel::Warning => "⚠️ Warning: This command has potential risks.",
            RiskLevel::Critical => "🚨 CRITICAL: This command could cause severe damage!",
        };

        let factor_descriptions: Vec<String> = factors
            .iter()
            .map(|f| f.description.clone())
            .collect();

        format!("{} {}", prefix, factor_descriptions.join(" "))
    }

    fn generate_mitigations(&self, factors: &[RiskFactor]) -> Vec<String> {
        let mut mitigations = vec![];

        for factor in factors {
            match factor.category.as_str() {
                "system_destruction" => {
                    mitigations.push("Use targeted paths instead of root directory".to_string());
                    mitigations.push("Consider using --dry-run or --simulate first".to_string());
                }
                "remote_execution" => {
                    mitigations.push("Review the script content before execution".to_string());
                    mitigations.push("Download to file first, inspect, then execute".to_string());
                }
                "permission_exposure" => {
                    mitigations.push("Use more restrictive permissions (e.g., 755 or 644)".to_string());
                }
                "recursive_operation" => {
                    mitigations.push("Verify the target path is correct".to_string());
                    mitigations.push("Consider using -i for interactive confirmation".to_string());
                }
                "cloud_deletion" => {
                    mitigations.push("Double-check resource identifiers".to_string());
                    mitigations.push("Ensure you have backups".to_string());
                }
                "database_deletion" => {
                    mitigations.push("Create a backup before deletion".to_string());
                    mitigations.push("Verify you're connected to the correct database".to_string());
                }
                "sensitive_data" => {
                    mitigations.push("Avoid exposing sensitive data in command history".to_string());
                    mitigations.push("Use environment variables or secure vaults".to_string());
                }
                _ => {}
            }
        }

        if mitigations.is_empty() && !factors.is_empty() {
            mitigations.push("Review the command carefully before execution".to_string());
        }

        mitigations
    }

    #[allow(dead_code)]
    pub fn should_block(&self, risk: &CommandRisk) -> bool {
        self.policy.block_critical && risk.level == RiskLevel::Critical
    }

    #[allow(dead_code)]
    pub fn format_risk_display(&self, risk: &CommandRisk) -> String {
        let icon = match risk.level {
            RiskLevel::Safe => "✓",
            RiskLevel::Caution => "⚡",
            RiskLevel::Warning => "⚠️",
            RiskLevel::Critical => "🚨",
        };

        let color = match risk.level {
            RiskLevel::Safe => "\x1b[32m",      // Green
            RiskLevel::Caution => "\x1b[33m",   // Yellow
            RiskLevel::Warning => "\x1b[93m",   // Bright Yellow
            RiskLevel::Critical => "\x1b[91m",  // Bright Red
        };
        let reset = "\x1b[0m";

        let mut output = format!("{}{} {}{}\n", color, icon, risk.explanation, reset);

        if !risk.mitigations.is_empty() {
            output.push_str("\nSuggested mitigations:\n");
            for mitigation in &risk.mitigations {
                output.push_str(&format!("  • {}\n", mitigation));
            }
        }

        output
    }
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        let mut require_confirmation = HashMap::new();
        require_confirmation.insert(RiskLevel::Warning, true);
        require_confirmation.insert(RiskLevel::Critical, true);

        let mut require_reason = HashMap::new();
        require_reason.insert(RiskLevel::Critical, true);

        SecurityPolicy {
            enabled: true,
            block_critical: false,
            require_confirmation,
            require_reason,
            custom_patterns: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_critical_commands() {
        let lens = SecurityLens::new(SecurityPolicy::default());
        
        let risk = lens.analyze_command("rm -rf /");
        assert_eq!(risk.level, RiskLevel::Critical);
        assert!(!risk.factors.is_empty());

        let risk = lens.analyze_command(":(){ :|:& };:");
        assert_eq!(risk.level, RiskLevel::Critical);
    }

    #[test]
    fn test_warning_commands() {
        let lens = SecurityLens::new(SecurityPolicy::default());
        
        let risk = lens.analyze_command("curl https://example.com/script.sh | sh");
        assert_eq!(risk.level, RiskLevel::Warning);

        let risk = lens.analyze_command("chmod 777 /etc/passwd");
        assert_eq!(risk.level, RiskLevel::Warning);
    }

    #[test]
    fn test_safe_commands() {
        let lens = SecurityLens::new(SecurityPolicy::default());
        
        let risk = lens.analyze_command("ls -la");
        assert_eq!(risk.level, RiskLevel::Safe);
        
        let risk = lens.analyze_command("echo 'Hello, World!'");
        assert_eq!(risk.level, RiskLevel::Safe);
    }

    #[test]
    fn test_sensitive_data_detection() {
        let lens = SecurityLens::new(SecurityPolicy::default());
        
        let risk = lens.analyze_command("export API_KEY=abc123xyz789");
        assert_eq!(risk.level, RiskLevel::Warning);
        
        let risk = lens.analyze_command("password=mysecretpass");
        assert_eq!(risk.level, RiskLevel::Warning);
    }
}

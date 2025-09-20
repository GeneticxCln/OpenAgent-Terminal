//! Security configuration for OpenAgent Terminal
//! Provides easy configuration access for security features

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "security-lens")]
use crate::security::{RiskLevel, SecurityPolicy as InternalSecurityPolicy};

#[cfg(not(feature = "security-lens"))]
use crate::security::{RiskLevel, SecurityPolicy as InternalSecurityPolicy};

/// User-facing security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable security analysis
    pub enabled: bool,
    /// Block critical commands automatically
    pub block_critical: bool,
    /// Require confirmation for specific risk levels
    pub require_confirmation: HashMap<String, bool>,
    /// Enable paste event security checking
    pub gate_paste_events: bool,
    /// Custom security patterns
    pub custom_patterns: Vec<CustomSecurityPattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSecurityPattern {
    /// Regex pattern to match
    pub pattern: String,
    /// Risk level to assign
    pub risk_level: String,
    /// Description of the risk
    pub message: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        let mut require_confirmation = HashMap::new();
        require_confirmation.insert("Safe".to_string(), false);
        require_confirmation.insert("Caution".to_string(), true);
        require_confirmation.insert("Warning".to_string(), true);
        require_confirmation.insert("Critical".to_string(), true);

        Self {
            enabled: true,
            block_critical: false, // Conservative default - warn but don't block
            require_confirmation,
            gate_paste_events: true,
            custom_patterns: vec![
                // Example custom patterns for organizations
                CustomSecurityPattern {
                    pattern: r"(?i)kubectl\s+delete\s+.*prod".to_string(),
                    risk_level: "Critical".to_string(),
                    message: "Deleting production Kubernetes resources".to_string(),
                },
                CustomSecurityPattern {
                    pattern: r"(?i)aws\s+s3\s+rm\s+.*--recursive".to_string(),
                    risk_level: "Warning".to_string(),
                    message: "Recursive S3 deletion can affect many files".to_string(),
                },
            ],
        }
    }
}

impl SecurityConfig {
    /// Convert to internal security policy format
    pub fn to_internal_policy(&self) -> InternalSecurityPolicy {
        let mut require_confirmation = HashMap::new();

        for (level_str, required) in &self.require_confirmation {
            if let Ok(level) = self.parse_risk_level(level_str) {
                require_confirmation.insert(level, *required);
            }
        }

        #[cfg(feature = "security-lens")]
        {
            let custom_patterns = self
                .custom_patterns
                .iter()
                .filter_map(|pattern| {
                    self.parse_risk_level(&pattern.risk_level).ok().map(|level| {
                        crate::security::CustomPattern {
                            pattern: pattern.pattern.clone(),
                            risk_level: level,
                            message: pattern.message.clone(),
                        }
                    })
                })
                .collect();

            InternalSecurityPolicy {
                enabled: self.enabled,
                block_critical: self.block_critical,
                require_confirmation,
                require_reason: HashMap::new(), // Can be extended later
                custom_patterns,
                platform_groups: Vec::new(), // Can be extended later
                gate_paste_events: self.gate_paste_events,
                rate_limit: crate::security::RateLimitConfig::default(),
                docs_base_url: "https://docs.openagent.dev/security".to_string(),
            }
        }

        #[cfg(not(feature = "security-lens"))]
        {
            InternalSecurityPolicy {
                enabled: self.enabled,
                block_critical: self.block_critical,
                require_confirmation,
                gate_paste_events: self.gate_paste_events,
            }
        }
    }

    /// Parse risk level from string
    fn parse_risk_level(&self, level_str: &str) -> Result<RiskLevel, &'static str> {
        match level_str.to_lowercase().as_str() {
            "safe" => Ok(RiskLevel::Safe),
            "caution" => Ok(RiskLevel::Caution),
            "warning" => Ok(RiskLevel::Warning),
            "critical" => Ok(RiskLevel::Critical),
            _ => Err("Invalid risk level"),
        }
    }

    /// Get a preset configuration for different security levels
    pub fn preset_conservative() -> Self {
        SecurityConfig { block_critical: true, ..Default::default() }
    }

    pub fn preset_permissive() -> Self {
        let mut require_confirmation = SecurityConfig::default().require_confirmation;
        require_confirmation.insert("Caution".to_string(), false);
        SecurityConfig { block_critical: false, require_confirmation, ..Default::default() }
    }

    pub fn preset_disabled() -> Self {
        Self {
            enabled: false,
            block_critical: false,
            require_confirmation: HashMap::new(),
            gate_paste_events: false,
            custom_patterns: Vec::new(),
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate custom patterns
        for pattern in &self.custom_patterns {
            if regex::Regex::new(&pattern.pattern).is_err() {
                return Err(format!("Invalid regex pattern: {}", pattern.pattern));
            }

            if self.parse_risk_level(&pattern.risk_level).is_err() {
                return Err(format!("Invalid risk level: {}", pattern.risk_level));
            }
        }

        // Validate risk level mappings
        for level_str in self.require_confirmation.keys() {
            if self.parse_risk_level(level_str).is_err() {
                return Err(format!("Invalid risk level in confirmation settings: {}", level_str));
            }
        }

        Ok(())
    }

    /// Add a custom security pattern
    pub fn add_custom_pattern(&mut self, pattern: CustomSecurityPattern) -> Result<(), String> {
        // Validate the pattern
        if regex::Regex::new(&pattern.pattern).is_err() {
            return Err(format!("Invalid regex pattern: {}", pattern.pattern));
        }

        if self.parse_risk_level(&pattern.risk_level).is_err() {
            return Err(format!("Invalid risk level: {}", pattern.risk_level));
        }

        self.custom_patterns.push(pattern);
        Ok(())
    }

    /// Remove a custom security pattern by index
    pub fn remove_custom_pattern(&mut self, index: usize) -> Result<(), String> {
        if index >= self.custom_patterns.len() {
            return Err("Pattern index out of bounds".to_string());
        }

        self.custom_patterns.remove(index);
        Ok(())
    }

    /// Get security level summary
    pub fn get_security_summary(&self) -> String {
        if !self.enabled {
            return "Disabled".to_string();
        }

        let confirmation_count = self.require_confirmation.values().filter(|&&v| v).count();
        let custom_patterns_count = self.custom_patterns.len();

        if self.block_critical {
            format!(
                "High Security: Critical blocking enabled, {} confirmation levels, {} custom patterns", 
                confirmation_count, custom_patterns_count
            )
        } else if confirmation_count >= 3 {
            format!(
                "Medium Security: {} confirmation levels, {} custom patterns",
                confirmation_count, custom_patterns_count
            )
        } else {
            format!(
                "Basic Security: {} confirmation levels, {} custom patterns",
                confirmation_count, custom_patterns_count
            )
        }
    }
}

/// Security lens factory for creating configured instances
pub struct SecurityLensFactory;

impl SecurityLensFactory {
    /// Create a security lens with the given configuration
    #[cfg(feature = "security-lens")]
    pub fn create(config: &SecurityConfig) -> crate::security::SecurityLens {
        let policy = config.to_internal_policy();
        crate::security::SecurityLens::new(policy)
    }

    /// Create a security lens with the given configuration (stub version)
    #[cfg(not(feature = "security-lens"))]
    pub fn create(config: &SecurityConfig) -> crate::security::SecurityLens {
        let policy = config.to_internal_policy();
        crate::security::SecurityLens::new(policy)
    }

    /// Test a command against the security configuration
    pub fn test_command(
        config: &SecurityConfig,
        command: &str,
    ) -> Result<SecurityTestResult, String> {
        let mut lens = Self::create(config);
        let risk = lens.analyze_command(command);

        Ok(SecurityTestResult {
            risk_level: format!("{:?}", risk.level),
            explanation: risk.explanation.clone(),
            requires_confirmation: risk.requires_confirmation,
            mitigations: risk.mitigations.clone(),
            would_block: config.block_critical && matches!(risk.level, RiskLevel::Critical),
        })
    }
}

#[derive(Debug, Clone)]
pub struct SecurityTestResult {
    pub risk_level: String,
    pub explanation: String,
    pub requires_confirmation: bool,
    pub mitigations: Vec<String>,
    pub would_block: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert!(config.enabled);
        assert!(!config.block_critical);
        assert!(config.gate_paste_events);
        assert!(config.require_confirmation.get("Critical").unwrap_or(&false));
    }

    #[test]
    fn test_security_config_presets() {
        let conservative = SecurityConfig::preset_conservative();
        assert!(conservative.block_critical);

        let permissive = SecurityConfig::preset_permissive();
        assert!(!permissive.block_critical);

        let disabled = SecurityConfig::preset_disabled();
        assert!(!disabled.enabled);
    }

    #[test]
    fn test_custom_pattern_validation() {
        let mut config = SecurityConfig::default();

        // Valid pattern
        let valid_pattern = CustomSecurityPattern {
            pattern: r"rm\s+-rf".to_string(),
            risk_level: "Warning".to_string(),
            message: "Recursive deletion".to_string(),
        };
        assert!(config.add_custom_pattern(valid_pattern).is_ok());

        // Invalid regex
        let invalid_regex = CustomSecurityPattern {
            pattern: r"[invalid(".to_string(),
            risk_level: "Warning".to_string(),
            message: "Invalid regex".to_string(),
        };
        assert!(config.add_custom_pattern(invalid_regex).is_err());

        // Invalid risk level
        let invalid_level = CustomSecurityPattern {
            pattern: r"test".to_string(),
            risk_level: "Invalid".to_string(),
            message: "Invalid level".to_string(),
        };
        assert!(config.add_custom_pattern(invalid_level).is_err());
    }

    #[cfg(feature = "security-lens")]
    #[test]
    fn test_security_lens_factory() {
        let config = SecurityConfig::default();

        // Test a dangerous command
        let result = SecurityLensFactory::test_command(&config, "rm -rf /").unwrap();
        #[cfg(feature = "security-lens")]
        {
            assert_eq!(result.risk_level, "Critical");
            assert!(result.requires_confirmation);
            assert!(!result.mitigations.is_empty());
        }
        #[cfg(not(feature = "security-lens"))]
        {
            // Stub security lens marks all commands as Safe
            assert_eq!(result.risk_level, "Safe");
            assert!(!result.requires_confirmation);
        }

        // Test with conservative config
        let conservative_config = SecurityConfig::preset_conservative();
        let result = SecurityLensFactory::test_command(&conservative_config, "rm -rf /").unwrap();
        #[cfg(feature = "security-lens")]
        assert!(result.would_block);
        #[cfg(not(feature = "security-lens"))]
        assert!(!result.would_block);
    }
}

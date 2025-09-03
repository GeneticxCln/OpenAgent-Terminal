// Security Lens - Command Analysis System
// Provides real-time risk assessment for terminal commands

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use log::{warn, info, debug};

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
    pub mitigation_links: Vec<MitigationLink>,
    pub requires_confirmation: bool,
    pub platform_specific: bool,
    pub detection_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MitigationLink {
    pub title: String,
    pub url: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOS,
    Windows,
    Unknown,
}

impl Platform {
    pub fn current() -> Self {
        #[cfg(target_os = "linux")]
        return Platform::Linux;
        #[cfg(target_os = "macos")]
        return Platform::MacOS;
        #[cfg(target_os = "windows")]
        return Platform::Windows;
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        return Platform::Unknown;
    }
    
    pub fn to_str(&self) -> &'static str {
        match self {
            Platform::Linux => "linux",
            Platform::MacOS => "macos",
            Platform::Windows => "windows",
            Platform::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformPatternGroup {
    pub enabled: bool,
    pub platform: Platform,
    pub patterns: Vec<CustomPattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPolicy {
    pub enabled: bool,
    pub block_critical: bool,
    pub require_confirmation: HashMap<RiskLevel, bool>,
    pub require_reason: HashMap<RiskLevel, bool>,
    pub custom_patterns: Vec<CustomPattern>,
    /// Platform-specific pattern groups
    #[serde(default)]
    pub platform_groups: Vec<PlatformPatternGroup>,
    /// Enable paste event gating
    #[serde(default)]
    pub gate_paste_events: bool,
    /// Rate limiting settings
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    /// Documentation base URL for mitigation links
    #[serde(default)]
    pub docs_base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimitConfig {
    /// Maximum detections per time window
    pub max_detections: u32,
    /// Time window in seconds
    pub window_seconds: u64,
    /// Enable rate limiting
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_detections: 10,
            window_seconds: 300, // 5 minutes
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomPattern {
    pub pattern: String,
    pub risk_level: RiskLevel,
    pub message: String,
}

pub struct SecurityLens {
    policy: SecurityPolicy,
    dangerous_patterns: Vec<(Regex, RiskFactor, RiskLevel)>,
    sensitive_patterns: Vec<Regex>,
    platform_patterns: Vec<(Regex, RiskFactor, RiskLevel)>,
    rate_limit_tracker: RateLimitTracker,
}

#[derive(Debug)]
struct RateLimitTracker {
    detections: Vec<DetectionRecord>,
}

#[derive(Debug, Clone)]
struct DetectionRecord {
    timestamp: u64,
    risk_level: RiskLevel,
    pattern_category: String,
    command_hash: u64,
}

#[allow(dead_code)]
impl RateLimitTracker {
    fn new() -> Self {
        Self {
            detections: Vec::new(),
        }
    }
    
    fn record_detection(&mut self, risk_level: RiskLevel, category: &str, command: &str) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        let command_hash = self.hash_command(command);
        
        self.detections.push(DetectionRecord {
            timestamp,
            risk_level,
            pattern_category: category.to_string(),
            command_hash,
        });
    }
    
    fn is_rate_limited(&mut self, config: &RateLimitConfig) -> bool {
        if !config.enabled {
            return false;
        }
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        // Clean up old records
        self.detections.retain(|record| {
            now - record.timestamp < config.window_seconds
        });
        
        // Check if we've exceeded the limit
        self.detections.len() >= config.max_detections as usize
    }
    
    fn hash_command(&self, command: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        command.hash(&mut hasher);
        hasher.finish()
    }
}

impl SecurityLens {
    pub fn new(policy: SecurityPolicy) -> Self {
        let dangerous_patterns = Self::init_dangerous_patterns();
        let sensitive_patterns = Self::init_sensitive_patterns();
        let platform_patterns = Self::init_platform_patterns(&policy);
        
        SecurityLens {
            policy,
            dangerous_patterns,
            sensitive_patterns,
            platform_patterns,
            rate_limit_tracker: RateLimitTracker::new(),
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
            // Warning: Kubernetes operations (general)
            (
                Regex::new(r"(?i)kubectl\s+(apply|scale|rollout)\b").unwrap(),
                RiskFactor {
                    category: "kubernetes_change".to_string(),
                    description: "Kubernetes apply/scale/rollout operation".to_string(),
                    pattern: "kubectl apply/scale/rollout".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Critical: kubectl delete in production namespace
            (
                Regex::new(r"(?i)kubectl\s+delete\s+.*(-n|--namespace)\s+(prod|production)\b").unwrap(),
                RiskFactor {
                    category: "kubernetes_delete".to_string(),
                    description: "Deleting resources in production namespace".to_string(),
                    pattern: "kubectl delete -n prod".to_string(),
                },
                RiskLevel::Critical,
            ),
            // Warning: terraform destroy
            (
                Regex::new(r"(?i)terraform\s+destroy(\s|$)").unwrap(),
                RiskFactor {
                    category: "iac_destroy".to_string(),
                    description: "Terraform destroy will remove infrastructure".to_string(),
                    pattern: "terraform destroy".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: docker system prune -a
            (
                Regex::new(r"(?i)docker\s+system\s+prune\s+-a(\s|$)").unwrap(),
                RiskFactor {
                    category: "container_cleanup".to_string(),
                    description: "Pruning all unused Docker data".to_string(),
                    pattern: "docker system prune -a".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Caution: git reset --hard
            (
                Regex::new(r"(?i)git\s+reset\s+--hard(\s|$)").unwrap(),
                RiskFactor {
                    category: "vcs_rewrite".to_string(),
                    description: "Hard reset will discard local changes".to_string(),
                    pattern: "git reset --hard".to_string(),
                },
                RiskLevel::Caution,
            ),
            // Warning: remove .git directory
            (
                Regex::new(r"rm\s+-rf\s+\.git(\s|$)").unwrap(),
                RiskFactor {
                    category: "vcs_removal".to_string(),
                    description: "Removing VCS metadata (.git)".to_string(),
                    pattern: "rm -rf .git".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: systemctl stop/disable services
            (
                Regex::new(r"(?i)systemctl\s+(stop|disable)\s+\S+").unwrap(),
                RiskFactor {
                    category: "service_control".to_string(),
                    description: "Stopping or disabling a system service".to_string(),
                    pattern: "systemctl stop/disable".to_string(),
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
    
    fn init_platform_patterns(policy: &SecurityPolicy) -> Vec<(Regex, RiskFactor, RiskLevel)> {
        let mut patterns = Vec::new();
        let current_platform = Platform::current();
        
        // Add patterns for current platform from enabled groups
        for group in &policy.platform_groups {
            if group.enabled && group.platform == current_platform {
                for custom_pattern in &group.patterns {
                    if let Ok(regex) = Regex::new(&custom_pattern.pattern) {
                        patterns.push((
                            regex,
                            RiskFactor {
                                category: format!("platform_{}", current_platform.to_str()),
                                description: custom_pattern.message.clone(),
                                pattern: custom_pattern.pattern.clone(),
                            },
                            custom_pattern.risk_level,
                        ));
                    }
                }
            }
        }
        
        // Add built-in platform-specific patterns
        match current_platform {
            Platform::Linux => {
                patterns.extend(vec![
                    // systemd specific commands
                    (
                        Regex::new(r"(?i)systemctl\s+mask\s+\S+").unwrap(),
                        RiskFactor {
                            category: "systemd_mask".to_string(),
                            description: "Masking systemd service (prevents start)".to_string(),
                            pattern: "systemctl mask".to_string(),
                        },
                        RiskLevel::Warning,
                    ),
                    // iptables flush
                    (
                        Regex::new(r"(?i)iptables\s+-F").unwrap(),
                        RiskFactor {
                            category: "firewall_flush".to_string(),
                            description: "Flushing iptables rules (removes firewall protection)".to_string(),
                            pattern: "iptables -F".to_string(),
                        },
                        RiskLevel::Warning,
                    ),
                    // modprobe risky modules
                    (
                        Regex::new(r"(?i)modprobe\s+.*?(pcspkr|nouveau)").unwrap(),
                        RiskFactor {
                            category: "kernel_module".to_string(),
                            description: "Loading potentially problematic kernel module".to_string(),
                            pattern: "modprobe risky module".to_string(),
                        },
                        RiskLevel::Caution,
                    ),
                ]);
            },
            Platform::MacOS => {
                patterns.extend(vec![
                    // System Integrity Protection disable
                    (
                        Regex::new(r"(?i)csrutil\s+disable").unwrap(),
                        RiskFactor {
                            category: "sip_disable".to_string(),
                            description: "Disabling System Integrity Protection".to_string(),
                            pattern: "csrutil disable".to_string(),
                        },
                        RiskLevel::Critical,
                    ),
                    // Gatekeeper disable
                    (
                        Regex::new(r"(?i)spctl\s+--master-disable").unwrap(),
                        RiskFactor {
                            category: "gatekeeper_disable".to_string(),
                            description: "Disabling Gatekeeper security".to_string(),
                            pattern: "spctl --master-disable".to_string(),
                        },
                        RiskLevel::Warning,
                    ),
                    // Keychain manipulation
                    (
                        Regex::new(r"(?i)security\s+(delete-certificate|delete-identity)").unwrap(),
                        RiskFactor {
                            category: "keychain_delete".to_string(),
                            description: "Deleting certificates or identities from keychain".to_string(),
                            pattern: "security delete operations".to_string(),
                        },
                        RiskLevel::Warning,
                    ),
                ]);
            },
            Platform::Windows => {
                patterns.extend(vec![
                    // PowerShell execution policy
                    (
                        Regex::new(r"(?i)Set-ExecutionPolicy\s+Unrestricted").unwrap(),
                        RiskFactor {
                            category: "execution_policy".to_string(),
                            description: "Setting unrestricted PowerShell execution policy".to_string(),
                            pattern: "Set-ExecutionPolicy Unrestricted".to_string(),
                        },
                        RiskLevel::Warning,
                    ),
                    // Windows Defender disable
                    (
                        Regex::new(r"(?i)(Set-MpPreference\s+-DisableRealtimeMonitoring|Add-MpPreference\s+-ExclusionPath)").unwrap(),
                        RiskFactor {
                            category: "defender_disable".to_string(),
                            description: "Disabling or bypassing Windows Defender".to_string(),
                            pattern: "Defender disable/exclude".to_string(),
                        },
                        RiskLevel::Warning,
                    ),
                    // Registry manipulation
                    (
                        Regex::new(r"(?i)reg\s+(delete|add).*HKLM").unwrap(),
                        RiskFactor {
                            category: "registry_hklm".to_string(),
                            description: "Modifying system registry (HKLM)".to_string(),
                            pattern: "reg delete/add HKLM".to_string(),
                        },
                        RiskLevel::Warning,
                    ),
                ]);
            },
            Platform::Unknown => {},
        }
        
        patterns
    }

    pub fn analyze_command(&mut self, command: &str) -> CommandRisk {
        if !self.policy.enabled {
            return CommandRisk {
                level: RiskLevel::Safe,
                factors: vec![],
                explanation: "Security lens is disabled".to_string(),
                mitigations: vec![],
                mitigation_links: vec![],
                requires_confirmation: false,
                platform_specific: false,
                detection_id: self.generate_detection_id(),
            };
        }
        
        // Check rate limiting first
        if self.rate_limit_tracker.is_rate_limited(&self.policy.rate_limit) {
            warn!("Security Lens rate limit exceeded for command analysis");
            return CommandRisk {
                level: RiskLevel::Warning,
                factors: vec![RiskFactor {
                    category: "rate_limit".to_string(),
                    description: "Too many security detections in a short time".to_string(),
                    pattern: "rate limiting".to_string(),
                }],
                explanation: "Rate limit exceeded. Please wait before analyzing more commands.".to_string(),
                mitigations: vec!["Wait a few minutes before continuing".to_string()],
                mitigation_links: vec![],
                requires_confirmation: true,
                platform_specific: false,
                detection_id: self.generate_detection_id(),
            };
        }

        let mut risk_factors = vec![];
        let mut highest_risk = RiskLevel::Safe;
        let mut platform_specific = false;

        // Check dangerous patterns
        for (pattern, factor, risk_level) in &self.dangerous_patterns {
            if pattern.is_match(command) {
                risk_factors.push(factor.clone());
                if self.risk_level_value(risk_level) > self.risk_level_value(&highest_risk) {
                    highest_risk = *risk_level;
                }
                // Record detection for rate limiting
                self.rate_limit_tracker.record_detection(*risk_level, &factor.category, command);
            }
        }
        
        // Check platform-specific patterns
        for (pattern, factor, risk_level) in &self.platform_patterns {
            if pattern.is_match(command) {
                risk_factors.push(factor.clone());
                platform_specific = true;
                if self.risk_level_value(risk_level) > self.risk_level_value(&highest_risk) {
                    highest_risk = *risk_level;
                }
                // Record detection for rate limiting
                self.rate_limit_tracker.record_detection(*risk_level, &factor.category, command);
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
        let mitigation_links = self.generate_mitigation_links(&risk_factors);
        let requires_confirmation = *self.policy.require_confirmation
            .get(&highest_risk)
            .unwrap_or(&false);
            
        // Log detection if significant
        if highest_risk != RiskLevel::Safe {
            info!(
                "Security Lens detection: level={:?}, platform_specific={}, factors={}",
                highest_risk, platform_specific, risk_factors.len()
            );
            debug!(
                "Command analyzed: '{}' -> {:?} (factors: {})",
                command.chars().take(50).collect::<String>(),
                highest_risk,
                risk_factors.iter().map(|f| f.category.as_str()).collect::<Vec<&str>>().join(", ")
            );
        }

        CommandRisk {
            level: highest_risk,
            factors: risk_factors,
            explanation,
            mitigations,
            mitigation_links,
            requires_confirmation,
            platform_specific,
            detection_id: self.generate_detection_id(),
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
    
    fn generate_mitigation_links(&self, factors: &[RiskFactor]) -> Vec<MitigationLink> {
        let mut links = vec![];
        let base_url = if self.policy.docs_base_url.is_empty() {
            "https://docs.openagent.dev/security"
        } else {
            &self.policy.docs_base_url
        };
        
        for factor in factors {
            match factor.category.as_str() {
                "system_destruction" => {
                    links.push(MitigationLink {
                        title: "Safe File Operations Guide".to_string(),
                        url: format!("{}/safe-file-operations", base_url),
                        description: "Learn about safe file deletion practices".to_string(),
                    });
                },
                "remote_execution" => {
                    links.push(MitigationLink {
                        title: "Remote Script Security".to_string(),
                        url: format!("{}/remote-scripts", base_url),
                        description: "Best practices for executing remote scripts".to_string(),
                    });
                },
                "cloud_deletion" => {
                    links.push(MitigationLink {
                        title: "Cloud Resource Management".to_string(),
                        url: format!("{}/cloud-safety", base_url),
                        description: "Safe cloud resource deletion practices".to_string(),
                    });
                },
                "database_deletion" => {
                    links.push(MitigationLink {
                        title: "Database Safety Guide".to_string(),
                        url: format!("{}/database-safety", base_url),
                        description: "Database backup and recovery strategies".to_string(),
                    });
                },
                "sensitive_data" => {
                    links.push(MitigationLink {
                        title: "Secrets Management".to_string(),
                        url: format!("{}/secrets-management", base_url),
                        description: "How to handle secrets securely".to_string(),
                    });
                },
                "systemd_mask" | "firewall_flush" | "kernel_module" => {
                    links.push(MitigationLink {
                        title: "Linux System Administration".to_string(),
                        url: format!("{}/linux-admin", base_url),
                        description: "Safe Linux system administration practices".to_string(),
                    });
                },
                "sip_disable" | "gatekeeper_disable" | "keychain_delete" => {
                    links.push(MitigationLink {
                        title: "macOS Security Features".to_string(),
                        url: format!("{}/macos-security", base_url),
                        description: "Understanding macOS security mechanisms".to_string(),
                    });
                },
                "execution_policy" | "defender_disable" | "registry_hklm" => {
                    links.push(MitigationLink {
                        title: "Windows Security Configuration".to_string(),
                        url: format!("{}/windows-security", base_url),
                        description: "Windows security best practices".to_string(),
                    });
                },
                _ => {}
            }
        }
        
        links
    }
    
    fn generate_detection_id(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
            
        let mut hasher = DefaultHasher::new();
        timestamp.hash(&mut hasher);
        let hash = hasher.finish();
        
        format!("sec_{:x}", hash)
    }
    
    /// Analyze paste content for security risks
    pub fn analyze_paste_content(&mut self, content: &str) -> Option<CommandRisk> {
        if !self.policy.gate_paste_events {
            return None;
        }
        
        // Split content into potential command lines
        let lines: Vec<&str> = content.lines().collect();
        let mut highest_risk = RiskLevel::Safe;
        let mut all_factors = Vec::new();
        let mut any_platform_specific = false;
        
        for line in &lines {
            let line_trimmed = line.trim();
            if !line_trimmed.is_empty() && !line_trimmed.starts_with('#') {
                let risk = self.analyze_command(line_trimmed);
                if self.risk_level_value(&risk.level) > self.risk_level_value(&highest_risk) {
                    highest_risk = risk.level;
                }
                all_factors.extend(risk.factors);
                any_platform_specific |= risk.platform_specific;
            }
        }
        
        // Only return risk if it's above Safe and requires confirmation
        if highest_risk != RiskLevel::Safe {
            let requires_confirmation = *self.policy.require_confirmation
                .get(&highest_risk)
                .unwrap_or(&false);
                
            if requires_confirmation {
                let mitigations = self.generate_mitigations(&all_factors);
                let mitigation_links = self.generate_mitigation_links(&all_factors);
                return Some(CommandRisk {
                    level: highest_risk,
                    factors: all_factors,
                    explanation: format!(
                        "Pasted content contains {} potentially risky command(s)",
                        lines.len()
                    ),
                    mitigations,
                    mitigation_links,
                    requires_confirmation: true,
                    platform_specific: any_platform_specific,
                    detection_id: self.generate_detection_id(),
                });
            }
        }
        
        None
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
            platform_groups: vec![],
            gate_paste_events: true,
            docs_base_url: String::new(),
            rate_limit: RateLimitConfig {
                max_detections: 5,
                window_seconds: 60,
                enabled: true,
            },
        }
    }
}

impl openagent_terminal_config::SerdeReplace for SecurityPolicy {
    fn replace(&mut self, value: toml::Value) -> Result<(), Box<dyn std::error::Error>> {
        *self = SecurityPolicy::deserialize(value)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_critical_commands() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());
        
        let risk = lens.analyze_command("rm -rf /");
        assert_eq!(risk.level, RiskLevel::Critical);
        assert!(!risk.factors.is_empty());

        let risk = lens.analyze_command(":(){ :|:& };:");
        assert_eq!(risk.level, RiskLevel::Critical);
    }

    #[test]
    fn test_warning_commands() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());
        
        let risk = lens.analyze_command("curl https://example.com/script.sh | sh");
        assert_eq!(risk.level, RiskLevel::Warning);

        let risk = lens.analyze_command("chmod 777 /etc/passwd");
        assert_eq!(risk.level, RiskLevel::Warning);
    }

    #[test]
    fn test_safe_commands() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());
        
        let risk = lens.analyze_command("ls -la");
        assert_eq!(risk.level, RiskLevel::Safe);
        
        let risk = lens.analyze_command("echo 'Hello, World!'");
        assert_eq!(risk.level, RiskLevel::Safe);
    }

    #[test]
    fn test_sensitive_data_detection() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());
        
        let risk = lens.analyze_command("export API_KEY=abc123xyz789");
        assert_eq!(risk.level, RiskLevel::Warning);
        
        let risk = lens.analyze_command("password=mysecretpass");
        assert_eq!(risk.level, RiskLevel::Warning);
    }

    #[test]
    fn test_policy_require_confirmation_and_blocking() {
        // Policy: block critical, require confirmation for Caution and Warning; Safe=false
        let mut require_confirmation = std::collections::HashMap::new();
        require_confirmation.insert(RiskLevel::Safe, false);
        require_confirmation.insert(RiskLevel::Caution, true);
        require_confirmation.insert(RiskLevel::Warning, true);
        require_confirmation.insert(RiskLevel::Critical, true);

        let policy = SecurityPolicy {
            enabled: true,
            block_critical: true,
            require_confirmation: require_confirmation.clone(),
            require_reason: Default::default(),
            custom_patterns: vec![],
            platform_groups: vec![],
            gate_paste_events: false,
            docs_base_url: String::new(),
            rate_limit: RateLimitConfig::default(),
        };
        let mut lens = SecurityLens::new(policy);

        // Critical command should be blocked
        let risk = lens.analyze_command("rm -rf /");
        assert_eq!(risk.level, RiskLevel::Critical);
        assert!(lens.should_block(&risk));
        assert_eq!(*require_confirmation.get(&risk.level).unwrap(), true);

        // Warning requires confirmation
        let risk = lens.analyze_command("curl https://x | sh");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(!lens.should_block(&risk));

        // Safe should not require confirmation
        let risk = lens.analyze_command("echo ok");
        assert_eq!(risk.level, RiskLevel::Safe);
        assert_eq!(risk.requires_confirmation, false);
    }

    #[test]
    fn test_custom_patterns_and_disabled_policy() {
        // Custom pattern should classify to requested risk level
        let policy = SecurityPolicy {
            enabled: true,
            block_critical: false,
            require_confirmation: HashMap::new(),
            require_reason: HashMap::new(),
            custom_patterns: vec![CustomPattern {
                pattern: r"(?i)kubectl\s+delete\s+ns\s+prod".into(),
                risk_level: RiskLevel::Critical,
                message: "Deleting the production namespace".into(),
            }],
            platform_groups: vec![],
            gate_paste_events: false,
            docs_base_url: String::new(),
            rate_limit: RateLimitConfig::default(),
        };
        let mut lens = SecurityLens::new(policy);
        let risk = lens.analyze_command("kubectl delete ns prod");
        assert_eq!(risk.level, RiskLevel::Critical);
        assert!(risk.factors.iter().any(|f| f.category == "custom"));

        // Disabled policy should always return Safe
        let disabled = SecurityPolicy { enabled: false, ..SecurityPolicy::default() };
        let mut lens_disabled = SecurityLens::new(disabled);
        let risk = lens_disabled.analyze_command("rm -rf /");
        assert_eq!(risk.level, RiskLevel::Safe);
        assert!(risk.explanation.contains("disabled"));
    }

    #[test]
    fn test_format_risk_display_output() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());
        let risk = lens.analyze_command("chmod 777 somefile");
        let rendered = lens.format_risk_display(&risk);
        // Should include icon and explanation
        assert!(rendered.contains("⚠️") || rendered.contains("\u{26a0}"));
        assert!(rendered.contains("Warning") || rendered.contains("warning"));
    }
    
    #[test]
    fn test_platform_awareness() {
        // Test that platform-specific patterns are detected correctly
        let mut lens = SecurityLens::new(SecurityPolicy::default());
        
        // These are built-in platform patterns that should be detected on the current platform
        #[cfg(target_os = "linux")]
        {
            let risk = lens.analyze_command("systemctl mask firewalld");
            assert_eq!(risk.level, RiskLevel::Warning);
            assert!(risk.platform_specific);
        }
        
        #[cfg(target_os = "macos")]
        {
            let risk = lens.analyze_command("csrutil disable");
            assert_eq!(risk.level, RiskLevel::Critical);
            assert!(risk.platform_specific);
        }
        
        #[cfg(target_os = "windows")]
        {
            let risk = lens.analyze_command("Set-ExecutionPolicy Unrestricted");
            assert_eq!(risk.level, RiskLevel::Warning);
            assert!(risk.platform_specific);
        }
    }
    
    #[test]
    fn test_paste_content_analysis() {
        let policy = SecurityPolicy {
            gate_paste_events: true,
            ..SecurityPolicy::default()
        };
        let mut lens = SecurityLens::new(policy);
        
        // Test safe paste content
        let safe_content = "ls -la\ncd /tmp\necho hello";
        let result = lens.analyze_paste_content(safe_content);
        assert!(result.is_none());
        
        // Test risky paste content
        let risky_content = "rm -rf /\necho $API_KEY";
        let result = lens.analyze_paste_content(risky_content);
        assert!(result.is_some());
        let risk = result.unwrap();
        assert_eq!(risk.level, RiskLevel::Critical);
        assert!(risk.requires_confirmation);
        assert!(!risk.detection_id.is_empty());
    }
    
    #[test]
    fn test_mitigation_link_generation() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());
        
        let risk = lens.analyze_command("rm -rf /");
        assert!(!risk.mitigation_links.is_empty());
        
        let link = &risk.mitigation_links[0];
        assert!(link.url.contains("safe-file-operations"));
        assert!(!link.title.is_empty());
        assert!(!link.description.is_empty());
    }
}

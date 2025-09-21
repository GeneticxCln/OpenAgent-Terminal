use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

/// Security audit events for plugin activities
#[allow(dead_code)] // Public API for future integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityEvent {
    PluginLoaded {
        plugin_name: String,
        capabilities: Vec<String>,
        timestamp: SystemTime,
    },
    ResourceAccess {
        plugin_name: String,
        resource_type: String,
        resource_path: String,
        access_type: AccessType,
        timestamp: SystemTime,
    },
    SuspiciousActivity {
        plugin_name: String,
        activity_type: String,
        details: String,
        severity: SeverityLevel,
        timestamp: SystemTime,
    },
    ResourceLimitExceeded {
        plugin_name: String,
        limit_type: String,
        current_value: u64,
        limit_value: u64,
        timestamp: SystemTime,
    },
}

#[allow(dead_code)] // Public API for future integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessType {
    Read,
    Write,
    Execute,
    Network,
}

#[allow(dead_code)] // Public API for future integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SeverityLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Security audit configuration
#[allow(dead_code)] // Public API for future integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable real-time monitoring
    pub enable_monitoring: bool,
    /// Maximum memory usage per plugin (bytes)
    pub max_memory_per_plugin: u64,
    /// Maximum file system operations per second per plugin
    pub max_fs_ops_per_second: u32,
    /// Maximum network requests per minute per plugin
    pub max_network_requests_per_minute: u32,
    /// Alert threshold for suspicious activity
    pub alert_threshold: u32,
    /// Log security events to file
    pub log_security_events: bool,
    /// Block plugins on security violations
    pub block_on_violations: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_monitoring: true,
            max_memory_per_plugin: 64 * 1024 * 1024, // 64MB
            max_fs_ops_per_second: 100,
            max_network_requests_per_minute: 60,
            alert_threshold: 5,
            log_security_events: true,
            block_on_violations: false,
        }
    }
}

/// Runtime security auditor for plugins
#[allow(dead_code)] // Public API for future integration
#[derive(Debug)]
pub struct SecurityAuditor {
    config: SecurityConfig,
    events: Vec<SecurityEvent>,
    plugin_stats: HashMap<String, PluginStats>,
    start_time: Instant,
}

#[allow(dead_code)] // Internal API for security tracking
#[derive(Debug, Clone)]
struct PluginStats {
    memory_usage: u64,
    fs_operations: u32,
    network_requests: u32,
    last_fs_op_time: Instant,
    last_network_time: Instant,
    violation_count: u32,
}

#[allow(dead_code)] // Public API for future integration
impl SecurityAuditor {
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            config,
            events: Vec::new(),
            plugin_stats: HashMap::new(),
            start_time: Instant::now(),
        }
    }

    /// Record a plugin being loaded
    pub fn record_plugin_load(&mut self, plugin_name: String, capabilities: Vec<String>) {
        let event = SecurityEvent::PluginLoaded {
            plugin_name: plugin_name.clone(),
            capabilities: capabilities.clone(),
            timestamp: SystemTime::now(),
        };

        self.events.push(event);
        self.plugin_stats.insert(
            plugin_name.clone(),
            PluginStats {
                memory_usage: 0,
                fs_operations: 0,
                network_requests: 0,
                last_fs_op_time: Instant::now(),
                last_network_time: Instant::now(),
                violation_count: 0,
            },
        );

        info!("Plugin '{}' loaded with capabilities: {:?}", plugin_name, capabilities);
    }

    /// Record resource access by a plugin
    pub fn record_resource_access(
        &mut self,
        plugin_name: &str,
        resource_type: &str,
        resource_path: &str,
        access_type: AccessType,
    ) -> Result<(), SecurityViolation> {
        let now = Instant::now();
        let event = SecurityEvent::ResourceAccess {
            plugin_name: plugin_name.to_string(),
            resource_type: resource_type.to_string(),
            resource_path: resource_path.to_string(),
            access_type: access_type.clone(),
            timestamp: SystemTime::now(),
        };

        // Check rate limits
        if let Some(stats) = self.plugin_stats.get_mut(plugin_name) {
            match access_type {
                AccessType::Read | AccessType::Write => {
                    let time_diff = now.duration_since(stats.last_fs_op_time);
                    if time_diff < Duration::from_secs(1) {
                        stats.fs_operations += 1;
                    } else {
                        stats.fs_operations = 1;
                        stats.last_fs_op_time = now;
                    }

                    if stats.fs_operations > self.config.max_fs_ops_per_second {
                        return Err(SecurityViolation::RateLimitExceeded {
                            plugin: plugin_name.to_string(),
                            limit_type: "filesystem_operations".to_string(),
                        });
                    }
                }
                AccessType::Network => {
                    let time_diff = now.duration_since(stats.last_network_time);
                    if time_diff < Duration::from_secs(60) {
                        stats.network_requests += 1;
                    } else {
                        stats.network_requests = 1;
                        stats.last_network_time = now;
                    }

                    if stats.network_requests > self.config.max_network_requests_per_minute {
                        return Err(SecurityViolation::RateLimitExceeded {
                            plugin: plugin_name.to_string(),
                            limit_type: "network_requests".to_string(),
                        });
                    }
                }
                _ => {}
            }
        }

        self.events.push(event);
        Ok(())
    }

    /// Record memory usage update for a plugin
    pub fn update_memory_usage(&mut self, plugin_name: &str, memory_bytes: u64) -> Result<(), SecurityViolation> {
        if memory_bytes > self.config.max_memory_per_plugin {
            let event = SecurityEvent::ResourceLimitExceeded {
                plugin_name: plugin_name.to_string(),
                limit_type: "memory".to_string(),
                current_value: memory_bytes,
                limit_value: self.config.max_memory_per_plugin,
                timestamp: SystemTime::now(),
            };
            self.events.push(event);

            return Err(SecurityViolation::MemoryLimitExceeded {
                plugin: plugin_name.to_string(),
                current: memory_bytes,
                limit: self.config.max_memory_per_plugin,
            });
        }

        if let Some(stats) = self.plugin_stats.get_mut(plugin_name) {
            stats.memory_usage = memory_bytes;
        }

        Ok(())
    }

    /// Record suspicious activity
    pub fn record_suspicious_activity(
        &mut self,
        plugin_name: &str,
        activity_type: &str,
        details: &str,
        severity: SeverityLevel,
    ) {
        let event = SecurityEvent::SuspiciousActivity {
            plugin_name: plugin_name.to_string(),
            activity_type: activity_type.to_string(),
            details: details.to_string(),
            severity: severity.clone(),
            timestamp: SystemTime::now(),
        };

        self.events.push(event);

        match severity {
            SeverityLevel::Low => info!("Low severity activity in plugin '{}': {}", plugin_name, details),
            SeverityLevel::Medium => warn!("Medium severity activity in plugin '{}': {}", plugin_name, details),
            SeverityLevel::High | SeverityLevel::Critical => {
                error!("High/Critical severity activity in plugin '{}': {}", plugin_name, details);
                
                if let Some(stats) = self.plugin_stats.get_mut(plugin_name) {
                    stats.violation_count += 1;
                }
            }
        }
    }

    /// Check if a plugin should be blocked based on violations
    pub fn should_block_plugin(&self, plugin_name: &str) -> bool {
        if !self.config.block_on_violations {
            return false;
        }

        if let Some(stats) = self.plugin_stats.get(plugin_name) {
            stats.violation_count >= self.config.alert_threshold
        } else {
            false
        }
    }

    /// Get security statistics for all plugins
    pub fn get_security_stats(&self) -> HashMap<String, PluginSecurityStats> {
        self.plugin_stats
            .iter()
            .map(|(name, stats)| {
                (
                    name.clone(),
                    PluginSecurityStats {
                        memory_usage: stats.memory_usage,
                        fs_operations_rate: stats.fs_operations,
                        network_requests_rate: stats.network_requests,
                        violation_count: stats.violation_count,
                        is_blocked: self.should_block_plugin(name),
                    },
                )
            })
            .collect()
    }

    /// Get recent security events
    pub fn get_recent_events(&self, since: Duration) -> Vec<&SecurityEvent> {
        let cutoff = SystemTime::now() - since;
        self.events
            .iter()
            .filter(|event| {
                let timestamp = match event {
                    SecurityEvent::PluginLoaded { timestamp, .. } => *timestamp,
                    SecurityEvent::ResourceAccess { timestamp, .. } => *timestamp,
                    SecurityEvent::SuspiciousActivity { timestamp, .. } => *timestamp,
                    SecurityEvent::ResourceLimitExceeded { timestamp, .. } => *timestamp,
                };
                timestamp > cutoff
            })
            .collect()
    }
}

#[allow(dead_code)] // Public API for future integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSecurityStats {
    pub memory_usage: u64,
    pub fs_operations_rate: u32,
    pub network_requests_rate: u32,
    pub violation_count: u32,
    pub is_blocked: bool,
}

#[allow(dead_code)] // Public API for future integration
#[derive(Debug, thiserror::Error)]
pub enum SecurityViolation {
    #[error("Plugin '{plugin}' exceeded rate limit for {limit_type}")]
    RateLimitExceeded { plugin: String, limit_type: String },
    
    #[error("Plugin '{plugin}' exceeded memory limit: {current} bytes (limit: {limit} bytes)")]
    MemoryLimitExceeded { plugin: String, current: u64, limit: u64 },
    
    #[error("Plugin '{plugin}' attempted unauthorized access to {resource}")]
    UnauthorizedAccess { plugin: String, resource: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_auditor_creation() {
        let config = SecurityConfig::default();
        let auditor = SecurityAuditor::new(config);
        assert_eq!(auditor.events.len(), 0);
        assert_eq!(auditor.plugin_stats.len(), 0);
    }

    #[test]
    fn test_plugin_load_recording() {
        let mut auditor = SecurityAuditor::new(SecurityConfig::default());
        auditor.record_plugin_load("test_plugin".to_string(), vec!["read".to_string()]);
        
        assert_eq!(auditor.events.len(), 1);
        assert!(auditor.plugin_stats.contains_key("test_plugin"));
    }

    #[test]
    fn test_memory_limit_enforcement() {
        let mut config = SecurityConfig::default();
        config.max_memory_per_plugin = 1024; // 1KB limit
        let mut auditor = SecurityAuditor::new(config);
        
        auditor.record_plugin_load("test_plugin".to_string(), vec![]);
        
        let result = auditor.update_memory_usage("test_plugin", 2048); // 2KB usage
        assert!(result.is_err());
        
        if let Err(SecurityViolation::MemoryLimitExceeded { current, limit, .. }) = result {
            assert_eq!(current, 2048);
            assert_eq!(limit, 1024);
        } else {
            panic!("Expected MemoryLimitExceeded error");
        }
    }
}
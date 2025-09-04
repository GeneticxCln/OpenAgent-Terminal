//! Enhanced plugin permissions with glob library and path normalization
//!
//! This module provides secure permission checking with:
//! - Glob pattern matching for file access
//! - Path normalization to prevent traversal attacks
//! - Cross-platform security validation

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

use crate::PluginSystemError;

/// Enhanced plugin permissions with security validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPermissions {
    /// File read access patterns (glob patterns)
    pub read_files: Vec<String>,
    
    /// File write access patterns (glob patterns)
    pub write_files: Vec<String>,
    
    /// Network access permission
    pub network: bool,
    
    /// Command execution permission
    pub execute_commands: bool,
    
    /// Environment variable access
    pub environment_variables: Vec<String>,
    
    /// Maximum memory usage in MB
    pub max_memory_mb: u32,
    
    /// Execution timeout in milliseconds
    pub timeout_ms: u64,
    
    /// Additional security restrictions
    pub security_restrictions: HashMap<String, serde_json::Value>,
}

impl Default for PluginPermissions {
    fn default() -> Self {
        Self {
            read_files: vec![],
            write_files: vec![],
            network: false,
            execute_commands: false,
            environment_variables: vec![],
            max_memory_mb: 50,
            timeout_ms: 5000,
            security_restrictions: HashMap::new(),
        }
    }
}

/// Security policy derived from permissions with compiled globs
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    /// Compiled read access glob set
    pub read_glob_set: Option<GlobSet>,
    
    /// Compiled write access glob set
    pub write_glob_set: Option<GlobSet>,
    
    /// Original permissions
    pub permissions: PluginPermissions,
    
    /// Normalized base paths for validation
    pub base_paths: Vec<PathBuf>,
    
    /// Dangerous path patterns to block
    pub blocked_patterns: GlobSet,
}

impl SecurityPolicy {
    /// Create security policy from permissions
    pub fn from_permissions(permissions: &PluginPermissions) -> Self {
        let read_glob_set = Self::compile_glob_set(&permissions.read_files);
        let write_glob_set = Self::compile_glob_set(&permissions.write_files);
        let blocked_patterns = Self::create_blocked_patterns();
        
        Self {
            read_glob_set,
            write_glob_set,
            permissions: permissions.clone(),
            base_paths: vec![],
            blocked_patterns,
        }
    }
    
    /// Compile glob patterns into a GlobSet
    fn compile_glob_set(patterns: &[String]) -> Option<GlobSet> {
        if patterns.is_empty() {
            return None;
        }
        
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            // Normalize pattern before compiling
            let normalized = Self::normalize_pattern(pattern);
            if let Ok(glob) = Glob::new(&normalized) {
                builder.add(glob);
            }
        }
        
        builder.build().ok()
    }
    
    /// Normalize glob pattern for cross-platform compatibility
    fn normalize_pattern(pattern: &str) -> String {
        // Convert Windows-style paths to Unix-style for glob matching
        pattern.replace('\\', "/")
    }
    
    /// Create glob set for dangerous/blocked patterns
    fn create_blocked_patterns() -> GlobSet {
        let dangerous_patterns = [
            // System directories
            "/etc/**",
            "/sys/**", 
            "/proc/**",
            "/dev/**",
            "/boot/**",
            "/root/**",
            
            // Windows system directories
            "C:/Windows/**",
            "C:/System32/**",
            "C:/Program Files/**",
            "C:/Program Files (x86)/**",
            
            // macOS system directories
            "/System/**",
            "/Library/System/**",
            "/private/etc/**",
            
            // Path traversal patterns
            "../**",
            "**/../**",
            "/..**",
            "**/..**",
            
            // Hidden sensitive files
            "**/.ssh/**",
            "**/.gnupg/**",
            "**/shadow",
            "**/passwd",
            "**/sudoers",
            "**/.env",
            "**/.secret*",
        ];
        
        let mut builder = GlobSetBuilder::new();
        for pattern in &dangerous_patterns {
            if let Ok(glob) = Glob::new(pattern) {
                builder.add(glob);
            }
        }
        
        builder.build().unwrap_or_else(|_| GlobSet::empty())
    }
    
    /// Check if a path is allowed for reading
    pub fn can_read_path(&self, path: &Path) -> Result<bool, PluginSystemError> {
        let normalized_path = self.normalize_and_validate_path(path)?;
        
        // Check against blocked patterns first
        if self.is_path_blocked(&normalized_path) {
            return Ok(false);
        }
        
        // Check against read permissions
        if let Some(ref glob_set) = self.read_glob_set {
            Ok(glob_set.is_match(&normalized_path))
        } else {
            Ok(false) // No read permissions granted
        }
    }
    
    /// Check if a path is allowed for writing
    pub fn can_write_path(&self, path: &Path) -> Result<bool, PluginSystemError> {
        let normalized_path = self.normalize_and_validate_path(path)?;
        
        // Check against blocked patterns first
        if self.is_path_blocked(&normalized_path) {
            return Ok(false);
        }
        
        // Check against write permissions
        if let Some(ref glob_set) = self.write_glob_set {
            Ok(glob_set.is_match(&normalized_path))
        } else {
            Ok(false) // No write permissions granted
        }
    }
    
    /// Normalize and validate a path to prevent traversal attacks
    pub fn normalize_and_validate_path(&self, path: &Path) -> Result<String, PluginSystemError> {
        // Convert to absolute path to resolve any relative components
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| PluginSystemError::PermissionDenied(format!("Cannot get current directory: {}", e)))?
                .join(path)
        };
        
        // Canonicalize to resolve symlinks and normalize the path
        let canonical_path = dunce::canonicalize(&absolute_path)
            .map_err(|e| PluginSystemError::PermissionDenied(format!("Path canonicalization failed: {}", e)))?;
        
        // Convert to string with consistent separators
        let path_str = canonical_path
            .to_str()
            .ok_or_else(|| PluginSystemError::PermissionDenied("Path contains invalid UTF-8".to_string()))?;
        
        // Normalize separators for glob matching
        Ok(path_str.replace('\\', "/"))
    }
    
    /// Check if a path matches blocked patterns
    fn is_path_blocked(&self, normalized_path: &str) -> bool {
        self.blocked_patterns.is_match(normalized_path)
    }
    
    /// Validate environment variable access
    pub fn can_access_env_var(&self, var_name: &str) -> bool {
        // Check if explicitly allowed
        if self.permissions.environment_variables.contains(&var_name.to_string()) {
            return true;
        }
        
        // Check against sensitive patterns
        let sensitive_prefixes = [
            "AWS_", "GCP_", "AZURE_", "SECRET_", "TOKEN_", "KEY_",
            "PASSWORD_", "PASS_", "SSH_", "GPG_", "AUTH_",
        ];
        
        let sensitive_exact = [
            "HOME", "USER", "USERNAME", "PATH", "LD_LIBRARY_PATH", 
            "SUDO_USER", "LOGNAME", "SHELL",
        ];
        
        // Block sensitive variables unless explicitly allowed
        if sensitive_prefixes.iter().any(|&prefix| var_name.starts_with(prefix)) ||
           sensitive_exact.contains(&var_name) {
            return false;
        }
        
        // Allow other variables
        true
    }
    
    /// Validate memory limit
    pub fn validate_memory_limit(&self) -> Result<(), PluginSystemError> {
        const MAX_ALLOWED_MEMORY_MB: u32 = 500;
        
        if self.permissions.max_memory_mb > MAX_ALLOWED_MEMORY_MB {
            return Err(PluginSystemError::PermissionDenied(format!(
                "Requested memory ({} MB) exceeds maximum allowed ({} MB)",
                self.permissions.max_memory_mb, MAX_ALLOWED_MEMORY_MB
            )));
        }
        
        if self.permissions.max_memory_mb == 0 {
            return Err(PluginSystemError::PermissionDenied(
                "Memory limit must be greater than 0".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Validate timeout limit
    pub fn validate_timeout(&self) -> Result<(), PluginSystemError> {
        const MAX_ALLOWED_TIMEOUT_MS: u64 = 60000; // 60 seconds
        
        if self.permissions.timeout_ms > MAX_ALLOWED_TIMEOUT_MS {
            return Err(PluginSystemError::PermissionDenied(format!(
                "Requested timeout ({} ms) exceeds maximum allowed ({} ms)",
                self.permissions.timeout_ms, MAX_ALLOWED_TIMEOUT_MS
            )));
        }
        
        if self.permissions.timeout_ms == 0 {
            return Err(PluginSystemError::PermissionDenied(
                "Timeout must be greater than 0".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Comprehensive permission validation
    pub fn validate_all_permissions(&self) -> Result<(), PluginSystemError> {
        self.validate_memory_limit()?;
        self.validate_timeout()?;
        
        // Validate file access patterns
        for pattern in &self.permissions.read_files {
            if self.is_dangerous_pattern(pattern) {
                return Err(PluginSystemError::PermissionDenied(format!(
                    "Dangerous read pattern denied: {}", pattern
                )));
            }
        }
        
        for pattern in &self.permissions.write_files {
            if self.is_dangerous_pattern(pattern) {
                return Err(PluginSystemError::PermissionDenied(format!(
                    "Dangerous write pattern denied: {}", pattern
                )));
            }
        }
        
        Ok(())
    }
    
    /// Check if a pattern is inherently dangerous
    fn is_dangerous_pattern(&self, pattern: &str) -> bool {
        let dangerous_substrings = [
            "../", "/..", "\\..\\", "\\..",
            "/etc/", "/sys/", "/proc/", "/dev/", "/boot/", "/root/",
            "C:/Windows/", "C:/System32/", "/System/", "/Library/System/",
            "shadow", "passwd", "sudoers", ".ssh/", ".gnupg/"
        ];
        
        dangerous_substrings.iter().any(|&dangerous| pattern.contains(dangerous))
    }
}

/// Plugin manifest structure for TOML parsing
#[derive(Debug, Deserialize)]
pub struct PluginManifest {
    pub plugin: Option<PluginInfo>,
    pub permissions: Option<PluginPermissions>,
}

/// Plugin information from manifest
#[derive(Debug, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub capabilities: Option<CapabilityManifest>,
}

/// Capability information from manifest
#[derive(Debug, Deserialize)]
pub struct CapabilityManifest {
    pub completions: Option<bool>,
    pub context_provider: Option<bool>,
    pub commands: Option<Vec<String>>,
    pub hooks: Option<Vec<String>>,
    pub file_associations: Option<Vec<String>>,
    pub services: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;
    
    #[test]
    fn test_path_normalization() {
        let permissions = PluginPermissions {
            read_files: vec!["test/**".to_string()],
            ..Default::default()
        };
        let policy = SecurityPolicy::from_permissions(&permissions);
        
        // Test various path inputs
        let test_cases = [
            "test/file.txt",
            "./test/file.txt", 
            "test/../test/file.txt",
        ];
        
        for path_str in &test_cases {
            let path = Path::new(path_str);
            let result = policy.normalize_and_validate_path(path);
            assert!(result.is_ok(), "Failed to normalize path: {}", path_str);
        }
    }
    
    #[test]
    fn test_dangerous_pattern_detection() {
        let permissions = PluginPermissions::default();
        let policy = SecurityPolicy::from_permissions(&permissions);
        
        let dangerous_patterns = [
            "../etc/passwd",
            "/etc/shadow", 
            "C:/Windows/System32/config",
            "../../root/.ssh/id_rsa",
        ];
        
        for pattern in &dangerous_patterns {
            assert!(policy.is_dangerous_pattern(pattern), 
                   "Should detect dangerous pattern: {}", pattern);
        }
    }
    
    #[test]
    fn test_blocked_path_detection() {
        let permissions = PluginPermissions::default();
        let policy = SecurityPolicy::from_permissions(&permissions);
        
        let blocked_paths = [
            "/etc/passwd",
            "/sys/kernel/debug",
            "C:/Windows/System32/drivers",
            "../../../etc/shadow",
        ];
        
        for path in &blocked_paths {
            assert!(policy.is_path_blocked(path), 
                   "Should block dangerous path: {}", path);
        }
    }
    
    #[test]
    fn test_environment_variable_access() {
        let mut permissions = PluginPermissions::default();
        permissions.environment_variables = vec!["PLUGIN_CONFIG".to_string()];
        let policy = SecurityPolicy::from_permissions(&permissions);
        
        // Should allow explicitly granted variables
        assert!(policy.can_access_env_var("PLUGIN_CONFIG"));
        
        // Should block sensitive variables
        assert!(!policy.can_access_env_var("AWS_SECRET_ACCESS_KEY"));
        assert!(!policy.can_access_env_var("HOME"));
        assert!(!policy.can_access_env_var("SSH_PRIVATE_KEY"));
        
        // Should allow non-sensitive variables
        assert!(policy.can_access_env_var("TERM"));
        assert!(policy.can_access_env_var("CUSTOM_VAR"));
    }
    
    #[test] 
    fn test_permission_validation() {
        let mut permissions = PluginPermissions::default();
        permissions.max_memory_mb = 1000; // Too high
        let policy = SecurityPolicy::from_permissions(&permissions);
        
        let result = policy.validate_memory_limit();
        assert!(result.is_err(), "Should reject excessive memory limit");
        
        permissions.max_memory_mb = 100; // Reasonable
        let policy = SecurityPolicy::from_permissions(&permissions);
        let result = policy.validate_memory_limit();
        assert!(result.is_ok(), "Should accept reasonable memory limit");
    }
}

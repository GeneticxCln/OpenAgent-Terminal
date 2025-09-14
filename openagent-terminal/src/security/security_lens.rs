#![allow(dead_code)]
// Security Lens - Command Analysis System
// Provides real-time risk assessment for terminal commands

use log::{debug, info, warn};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
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
        self.detections
            .retain(|record| now - record.timestamp < config.window_seconds);

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
            // === SYSTEM DESTRUCTION ===
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
            // === FILESYSTEM OPERATIONS ===
            // Warning: Mass file deletion
            (
                Regex::new(r"rm\s+-rf?\s+(/home|/var|/usr|/opt)/?\*").unwrap(),
                RiskFactor {
                    category: "filesystem_mass_delete".to_string(),
                    description: "Mass deletion in critical directories".to_string(),
                    pattern: "rm -rf /critical/*".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Caution: Recursive operations
            (
                Regex::new(r"(rm|chmod|chown)\s+.*-[rR]").unwrap(),
                RiskFactor {
                    category: "filesystem_recursive".to_string(),
                    description: "Recursive file operation".to_string(),
                    pattern: "recursive command".to_string(),
                },
                RiskLevel::Caution,
            ),
            // Warning: Chmod 777
            (
                Regex::new(r"chmod\s+777").unwrap(),
                RiskFactor {
                    category: "filesystem_permissions".to_string(),
                    description: "Setting world-writable permissions".to_string(),
                    pattern: "chmod 777".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Caution: Ownership changes to system directories
            (
                Regex::new(r"chown\s+.*(/bin|/sbin|/usr|/etc|/var)").unwrap(),
                RiskFactor {
                    category: "filesystem_system_ownership".to_string(),
                    description: "Changing ownership of system directories".to_string(),
                    pattern: "chown system directory".to_string(),
                },
                RiskLevel::Caution,
            ),
            // Warning: Mount operations with risky parameters
            (
                Regex::new(r"mount\s+.*--bind\s+/|mount\s+.*rw.*nodev.*nosuid").unwrap(),
                RiskFactor {
                    category: "filesystem_mount_risky".to_string(),
                    description: "Mount operation with potentially unsafe parameters".to_string(),
                    pattern: "risky mount options".to_string(),
                },
                RiskLevel::Warning,
            ),
            // === NETWORKING ===
            // Warning: Curl pipe to shell
            (
                Regex::new(r"curl.*\|.*sh|wget.*\|.*bash").unwrap(),
                RiskFactor {
                    category: "network_remote_execution".to_string(),
                    description: "Downloading and executing remote code".to_string(),
                    pattern: "curl | sh".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Caution: Downloading executables to PATH
            (
                Regex::new(r"(curl|wget)\s+.*\s+-o\s+(/usr/local/bin|/usr/bin|/bin)").unwrap(),
                RiskFactor {
                    category: "network_executable_download".to_string(),
                    description: "Downloading executables directly to system PATH".to_string(),
                    pattern: "download to PATH".to_string(),
                },
                RiskLevel::Caution,
            ),
            // Warning: Opening reverse shells
            (
                Regex::new(r"nc\s+-[el]+\s+\d+.*sh|bash.*>.*tcp").unwrap(),
                RiskFactor {
                    category: "network_reverse_shell".to_string(),
                    description: "Opening reverse shell connection".to_string(),
                    pattern: "reverse shell".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: Firewall manipulation
            (
                Regex::new(r"(ufw\s+(disable|reset)|iptables\s+-F|iptables\s+-X)").unwrap(),
                RiskFactor {
                    category: "network_firewall_disable".to_string(),
                    description: "Disabling or flushing firewall rules".to_string(),
                    pattern: "firewall disable".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Caution: Port scanning/network reconnaissance
            (
                Regex::new(r"nmap\s.*-[sS]|masscan|zmap").unwrap(),
                RiskFactor {
                    category: "network_scanning".to_string(),
                    description: "Network scanning operations".to_string(),
                    pattern: "network scan".to_string(),
                },
                RiskLevel::Caution,
            ),
            // === PACKAGE MANAGERS ===
            // Caution: Package manager global installs
            (
                Regex::new(
                    r"(npm|yarn|pnpm)\s+install\s+-g|pip\s+install\s+.*--break-system-packages",
                )
                .unwrap(),
                RiskFactor {
                    category: "package_global_install".to_string(),
                    description: "Global package installation can affect system".to_string(),
                    pattern: "global package install".to_string(),
                },
                RiskLevel::Caution,
            ),
            // Warning: Installing from untrusted sources
            (
                Regex::new(r"pip\s+install\s+.*--trusted-host|npm\s+install\s+.*--unsafe-perm")
                    .unwrap(),
                RiskFactor {
                    category: "package_untrusted_source".to_string(),
                    description: "Installing packages with bypassed security".to_string(),
                    pattern: "untrusted package source".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: Package manager auto-yes and cleanup
            (
                Regex::new(r"(apt-get|yum|dnf)\s+(remove|purge|autoremove)\s+.*-y").unwrap(),
                RiskFactor {
                    category: "package_auto_remove".to_string(),
                    description: "Automatic package removal without confirmation".to_string(),
                    pattern: "auto package removal".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Caution: Installing from direct URLs
            (
                Regex::new(r"(pip|cargo|go)\s+install\s+.*https?://").unwrap(),
                RiskFactor {
                    category: "package_direct_url".to_string(),
                    description: "Installing package from direct URL".to_string(),
                    pattern: "package install from URL".to_string(),
                },
                RiskLevel::Caution,
            ),
            // === CONTAINER & KUBERNETES ===
            // Warning: docker.sock mount (host control risk)
            (
                Regex::new(r"-v\s*/var/run/docker.sock:/var/run/docker.sock|--volume\s*/var/run/docker.sock").unwrap(),
                RiskFactor {
                    category: "container_docker_sock".to_string(),
                    description: "Mounting docker.sock grants container control over host Docker daemon".to_string(),
                    pattern: "docker.sock mount".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: Unconfined seccomp or apparmor disabled
            (
                Regex::new(r"--security-opt\s*seccomp=unconfined|--cap-add=ALL|--privileged").unwrap(),
                RiskFactor {
                    category: "container_unconfined".to_string(),
                    description: "Container running without security confinement".to_string(),
                    pattern: "unconfined container".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Critical: Mounting sensitive host directories into container
            (
                Regex::new(r"-v\s*/etc:/etc|--volume\s*/etc:/etc|-v\s*/root:/root|--volume\s*/root:/root").unwrap(),
                RiskFactor {
                    category: "container_sensitive_mount".to_string(),
                    description: "Mounting sensitive host directories into container".to_string(),
                    pattern: "sensitive host mount".to_string(),
                },
                RiskLevel::Critical,
            ),
            // === CONTAINER & KUBERNETES ===
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
                Regex::new(r"(?i)kubectl\s+delete\s+.*(-n|--namespace)\s+(prod|production)\b")
                    .unwrap(),
                RiskFactor {
                    category: "kubernetes_prod_delete".to_string(),
                    description: "Deleting resources in production namespace".to_string(),
                    pattern: "kubectl delete -n prod".to_string(),
                },
                RiskLevel::Critical,
            ),
            // Warning: Privileged container operations
            (
                Regex::new(r"docker\s+run\s+.*--privileged|docker\s+run\s+.*--user.*root").unwrap(),
                RiskFactor {
                    category: "container_privileged".to_string(),
                    description: "Running containers with elevated privileges".to_string(),
                    pattern: "privileged container".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: Docker system prune
            (
                Regex::new(r"(?i)docker\s+system\s+prune\s+-a(\s|$)").unwrap(),
                RiskFactor {
                    category: "container_cleanup".to_string(),
                    description: "Pruning all unused Docker data".to_string(),
                    pattern: "docker system prune -a".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: Helm delete
            (
                Regex::new(
                    r"(?i)helm\s+(delete|uninstall)\s+.*(-n|--namespace)\s+(prod|production)",
                )
                .unwrap(),
                RiskFactor {
                    category: "kubernetes_helm_delete".to_string(),
                    description: "Deleting Helm releases in production".to_string(),
                    pattern: "helm delete in prod".to_string(),
                },
                RiskLevel::Warning,
            ),
            // === CLOUD CLI OPERATIONS ===
            // Warning: AWS deletion
            (
                Regex::new(r"aws\s+.*delete|aws\s+.*terminate").unwrap(),
                RiskFactor {
                    category: "cloud_aws_deletion".to_string(),
                    description: "AWS resource deletion operation".to_string(),
                    pattern: "aws delete/terminate".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Critical: AWS S3 bucket deletion with force
            (
                Regex::new(r"aws\s+s3\s+rb\s+.*--force").unwrap(),
                RiskFactor {
                    category: "cloud_s3_force_delete".to_string(),
                    description: "Force deletion of S3 bucket and all contents".to_string(),
                    pattern: "aws s3 rb --force".to_string(),
                },
                RiskLevel::Critical,
            ),
            // Warning: GCP resource deletion
            (
                Regex::new(r"(gcloud|gsutil)\s+.*delete\s+.*--quiet").unwrap(),
                RiskFactor {
                    category: "cloud_gcp_deletion".to_string(),
                    description: "GCP resource deletion without confirmation".to_string(),
                    pattern: "gcloud delete --quiet".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: Azure resource group deletion
            (
                Regex::new(r"az\s+group\s+delete\s+.*--yes").unwrap(),
                RiskFactor {
                    category: "cloud_azure_rg_delete".to_string(),
                    description: "Azure resource group deletion without confirmation".to_string(),
                    pattern: "az group delete --yes".to_string(),
                },
                RiskLevel::Warning,
            ),
            // === INFRASTRUCTURE AS CODE ===
            // Warning: terraform destroy
            (
                Regex::new(r"(?i)terraform\s+destroy(\s|$)").unwrap(),
                RiskFactor {
                    category: "iac_terraform_destroy".to_string(),
                    description: "Terraform destroy will remove infrastructure".to_string(),
                    pattern: "terraform destroy".to_string(),
                },
                RiskLevel::Warning,
            ),
            // === WARP-SPECIFIC PATTERNS ===
            // Warning: Terminal multiplexer session killing
            (
                Regex::new(r"(tmux|screen)\s+kill-(session|server)").unwrap(),
                RiskFactor {
                    category: "terminal_session_kill".to_string(),
                    description: "Killing terminal multiplexer sessions may lose work".to_string(),
                    pattern: "tmux/screen kill".to_string(),
                },
                RiskLevel::Caution,
            ),
            // Caution: History manipulation
            (
                Regex::new(r"history\s+(-c|--clear)|>\s*\$HISTFILE|export\s+HISTFILE=/dev/null")
                    .unwrap(),
                RiskFactor {
                    category: "history_manipulation".to_string(),
                    description: "Modifying or clearing command history".to_string(),
                    pattern: "history manipulation".to_string(),
                },
                RiskLevel::Caution,
            ),
            // Warning: AI/LLM prompt injection attempts
            (
                Regex::new(r"(?s)(echo|cat|printf).*['\x22`].*system\s*\(|exec\s*\(|eval\s*\(")
                    .unwrap(),
                RiskFactor {
                    category: "ai_prompt_injection".to_string(),
                    description: "Potential AI/LLM prompt injection pattern".to_string(),
                    pattern: "prompt injection".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Caution: Terminal escape sequences (potential terminal manipulation)
            (
                Regex::new("\\\\e\\[[0-9;]*[mK]|\\\\033\\[[0-9;]*[mK]|printf\\s+.*\\\\e\\[")
                    .unwrap(),
                RiskFactor {
                    category: "terminal_escape_sequences".to_string(),
                    description: "Terminal escape sequences that may manipulate display"
                        .to_string(),
                    pattern: "terminal escapes".to_string(),
                },
                RiskLevel::Caution,
            ),
            // Warning: Process monitoring/spying
            (
                Regex::new(r"(strace|ltrace|gdb)\s+(-p|--pid)|ps\s+.*axw|lsof\s+(-p|\+D)").unwrap(),
                RiskFactor {
                    category: "process_monitoring".to_string(),
                    description: "Process monitoring/debugging that may expose sensitive data"
                        .to_string(),
                    pattern: "process monitoring".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: Memory dumping
            (
                Regex::new(r"(gcore|pmap)\s+\d+|cat\s+/proc/\d+/(maps|mem)|dd\s+if=/dev/mem")
                    .unwrap(),
                RiskFactor {
                    category: "memory_dumping".to_string(),
                    description: "Memory dumping operations that may expose sensitive data"
                        .to_string(),
                    pattern: "memory dump".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: Terraform force unlock
            (
                Regex::new(r"(?i)terraform\s+force-unlock").unwrap(),
                RiskFactor {
                    category: "iac_terraform_unlock".to_string(),
                    description: "Force unlocking Terraform state (may cause conflicts)"
                        .to_string(),
                    pattern: "terraform force-unlock".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Warning: Pulumi destroy
            (
                Regex::new(r"(?i)pulumi\s+(destroy|down)\s+.*--yes").unwrap(),
                RiskFactor {
                    category: "iac_pulumi_destroy".to_string(),
                    description: "Pulumi infrastructure destruction without confirmation"
                        .to_string(),
                    pattern: "pulumi destroy --yes".to_string(),
                },
                RiskLevel::Warning,
            ),
            // === DATABASE OPERATIONS ===
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
            // Critical: Database truncation on production
            (
                Regex::new(r"(?i)TRUNCATE\s+TABLE.*prod|DELETE\s+FROM.*WHERE\s+1=1").unwrap(),
                RiskFactor {
                    category: "database_data_wipe".to_string(),
                    description: "Mass data deletion in database".to_string(),
                    pattern: "TRUNCATE/DELETE all data".to_string(),
                },
                RiskLevel::Critical,
            ),
            // Warning: Database user/privilege operations
            (
                Regex::new(r"(?i)(DROP|CREATE)\s+USER|GRANT\s+ALL|REVOKE\s+ALL").unwrap(),
                RiskFactor {
                    category: "database_user_mgmt".to_string(),
                    description: "Database user or privilege management".to_string(),
                    pattern: "database user operations".to_string(),
                },
                RiskLevel::Warning,
            ),
            // === REMOTE EXECUTION & SCRIPTS ===
            // Warning: Script execution from remote
            (
                Regex::new("bash\\s+<\\(\\s*curl|sh\\s+<\\(\\s*wget|eval\\s+\\$\\(\\s*curl")
                    .unwrap(),
                RiskFactor {
                    category: "network_remote_script".to_string(),
                    description: "Executing scripts from remote sources".to_string(),
                    pattern: "remote script execution".to_string(),
                },
                RiskLevel::Warning,
            ),
            // Caution: Python eval/exec on user input
            (
                Regex::new("python.*-c.*(?:eval|exec)\\(").unwrap(),
                RiskFactor {
                    category: "script_dynamic_execution".to_string(),
                    description: "Dynamic code execution in Python".to_string(),
                    pattern: "python eval/exec".to_string(),
                },
                RiskLevel::Caution,
            ),
            // Warning: Process injection patterns
            (
                Regex::new(r"gdb\s+.*attach|strace\s+-p|ptrace").unwrap(),
                RiskFactor {
                    category: "process_injection".to_string(),
                    description: "Process debugging/injection operations".to_string(),
                    pattern: "process attach/trace".to_string(),
                },
                RiskLevel::Warning,
            ),
            // === VERSION CONTROL ===
            // Caution: git reset --hard
            (
                Regex::new(r"(?i)git\s+reset\s+--hard(\s|$)").unwrap(),
                RiskFactor {
                    category: "vcs_destructive".to_string(),
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
            // Warning: Force push to protected branches
            (
                Regex::new(r"git\s+push\s+.*--force.*\s+(main|master|prod|production)").unwrap(),
                RiskFactor {
                    category: "vcs_force_push".to_string(),
                    description: "Force pushing to protected branch".to_string(),
                    pattern: "git push --force main".to_string(),
                },
                RiskLevel::Warning,
            ),
            // === SYSTEM SERVICES ===
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
            // Critical: Critical service manipulation
            (
                Regex::new(r"(?i)systemctl\s+(stop|disable|mask)\s+(ssh|network|dbus|systemd)")
                    .unwrap(),
                RiskFactor {
                    category: "service_critical_stop".to_string(),
                    description: "Stopping critical system services".to_string(),
                    pattern: "stop critical service".to_string(),
                },
                RiskLevel::Critical,
            ),
            // === CRYPTO & CERTIFICATES ===
            // Warning: Certificate/key generation without proper parameters
            (
                Regex::new(r"openssl\s+genrsa\s+-out|ssh-keygen\s+.*-N\s*''").unwrap(),
                RiskFactor {
                    category: "crypto_weak_keys".to_string(),
                    description: "Generating keys without proper security parameters".to_string(),
                    pattern: "weak crypto generation".to_string(),
                },
                RiskLevel::Warning,
            ),
            // === ENVIRONMENT & SHELLS ===
            // Critical: Modify sudoers (may grant passwordless root)
            (
                Regex::new(r"visudo|echo\s+.*ALL=\(ALL\):ALL\s*>>\s*/etc/sudoers|echo\s+.*NOPASSWD:ALL\s*>>\s*/etc/sudoers").unwrap(),
                RiskFactor {
                    category: "sudoers_modification".to_string(),
                    description: "Modifying sudoers file can grant elevated privileges".to_string(),
                    pattern: "sudoers modification".to_string(),
                },
                RiskLevel::Critical,
            ),
            // Warning: Disable SELinux/AppArmor
            (
                Regex::new(r"setenforce\s+0|sed\s+-i\s+.*SELINUX=disabled|apparmor_status\s+.*\sdisabled").unwrap(),
                RiskFactor {
                    category: "disable_mandatory_access_control".to_string(),
                    description: "Disabling SELinux/AppArmor weakens system security".to_string(),
                    pattern: "disable MAC".to_string(),
                },
                RiskLevel::Warning,
            ),
            // === ENVIRONMENT & SHELLS ===
            // Caution: Shell history manipulation
            (
                Regex::new(r"history\s+-c|unset\s+HISTFILE|export\s+HISTSIZE=0").unwrap(),
                RiskFactor {
                    category: "shell_history_clear".to_string(),
                    description: "Clearing or disabling shell history".to_string(),
                    pattern: "history manipulation".to_string(),
                },
                RiskLevel::Caution,
            ),
            // Warning: Environment variable exposure
            (
                Regex::new(r"export\s+.*=(.*password.*|.*secret.*|.*key.*)").unwrap(),
                RiskFactor {
                    category: "env_sensitive_export".to_string(),
                    description: "Exporting potentially sensitive environment variables"
                        .to_string(),
                    pattern: "sensitive env export".to_string(),
                },
                RiskLevel::Warning,
            ),
        ]
    }

    fn init_sensitive_patterns() -> Vec<Regex> {
        vec![
            // API Keys
            Regex::new(r#"(?i)(api[_-]?key|apikey|api_secret)[=:\s]+['\"]?[a-zA-Z0-9_-]{8,}"#)
                .unwrap(),
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
                            description: "Flushing iptables rules (removes firewall protection)"
                                .to_string(),
                            pattern: "iptables -F".to_string(),
                        },
                        RiskLevel::Warning,
                    ),
                    // modprobe risky modules
                    (
                        Regex::new(r"(?i)modprobe\s+.*?(pcspkr|nouveau)").unwrap(),
                        RiskFactor {
                            category: "kernel_module".to_string(),
                            description: "Loading potentially problematic kernel module"
                                .to_string(),
                            pattern: "modprobe risky module".to_string(),
                        },
                        RiskLevel::Caution,
                    ),
                ]);
            }
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
                            description: "Deleting certificates or identities from keychain"
                                .to_string(),
                            pattern: "security delete operations".to_string(),
                        },
                        RiskLevel::Warning,
                    ),
                ]);
            }
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
            }
            Platform::Unknown => {}
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
        if self
            .rate_limit_tracker
            .is_rate_limited(&self.policy.rate_limit)
        {
            warn!("Security Lens rate limit exceeded for command analysis");
            return CommandRisk {
                level: RiskLevel::Warning,
                factors: vec![RiskFactor {
                    category: "rate_limit".to_string(),
                    description: "Too many security detections in a short time".to_string(),
                    pattern: "rate limiting".to_string(),
                }],
                explanation: "Rate limit exceeded. Please wait before analyzing more commands."
                    .to_string(),
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
                self.rate_limit_tracker
                    .record_detection(*risk_level, &factor.category, command);
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
                self.rate_limit_tracker
                    .record_detection(*risk_level, &factor.category, command);
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
                if self.risk_level_value(&RiskLevel::Warning) > self.risk_level_value(&highest_risk)
                {
                    highest_risk = RiskLevel::Warning;
                }
            }
        }

        // Heuristic: detect exporting of sensitive env var names (e.g., SECRET_TOKEN, API_KEY)
        if let Some(rest) = command.trim_start().strip_prefix("export ") {
            if let Some(eq_pos) = rest.find('=') {
                let var_name = rest[..eq_pos].trim();
                let var_lower = var_name.to_lowercase();
                if var_lower.contains("secret")
                    || var_lower.contains("token")
                    || var_lower.contains("key")
                    || var_lower.contains("password")
                {
                    risk_factors.push(RiskFactor {
                        category: "env_sensitive_export".to_string(),
                        description: "Exporting potentially sensitive environment variable"
                            .to_string(),
                        pattern: "sensitive env var name".to_string(),
                    });
                    if self.risk_level_value(&RiskLevel::Warning)
                        > self.risk_level_value(&highest_risk)
                    {
                        highest_risk = RiskLevel::Warning;
                    }
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
                    if self.risk_level_value(&custom.risk_level)
                        > self.risk_level_value(&highest_risk)
                    {
                        highest_risk = custom.risk_level;
                    }
                }
            }
        }

        let explanation = self.generate_explanation(&risk_factors, &highest_risk);
        let mitigations = self.generate_mitigations(&risk_factors);
        let mitigation_links = self.generate_mitigation_links(&risk_factors);
        let requires_confirmation = *self
            .policy
            .require_confirmation
            .get(&highest_risk)
            .unwrap_or(&false);

        // Log detection if significant
        if highest_risk != RiskLevel::Safe {
            info!(
                "Security Lens detection: level={:?}, platform_specific={}, factors={}",
                highest_risk,
                platform_specific,
                risk_factors.len()
            );
            debug!(
                "Command analyzed: '{}' -> {:?} (factors: {})",
                command.chars().take(50).collect::<String>(),
                highest_risk,
                risk_factors
                    .iter()
                    .map(|f| f.category.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
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

        let factor_descriptions: Vec<String> =
            factors.iter().map(|f| f.description.clone()).collect();

        format!("{} {}", prefix, factor_descriptions.join(" "))
    }

    fn generate_mitigations(&self, factors: &[RiskFactor]) -> Vec<String> {
        let mut mitigations = vec![];

        for factor in factors {
            match factor.category.as_str() {
                // System destruction patterns
                "system_destruction" | "disk_overwrite" => {
                    mitigations.push("Use targeted paths instead of root directory".to_string());
                    mitigations.push("Consider using --dry-run or --simulate first".to_string());
                    mitigations.push("Ensure you have system backups".to_string());
                }
                "fork_bomb" => {
                    mitigations.push("Do not execute fork bomb patterns".to_string());
                    mitigations.push("Use ulimit to set process limits before testing".to_string());
                }

                // Filesystem operations
                "filesystem_mass_delete" => {
                    mitigations.push("Use specific file patterns instead of wildcards".to_string());
                    mitigations.push("Test deletion in a safe directory first".to_string());
                }
                "filesystem_recursive" => {
                    mitigations.push("Verify the target path is correct".to_string());
                    mitigations.push("Consider using -i for interactive confirmation".to_string());
                }
                "filesystem_permissions" => {
                    mitigations
                        .push("Use more restrictive permissions (e.g., 755 or 644)".to_string());
                    mitigations.push("Consider if write access is actually needed".to_string());
                }
                "filesystem_system_ownership" => {
                    mitigations.push("Verify ownership change is necessary".to_string());
                    mitigations.push("Use sudo for temporary elevated access instead".to_string());
                }
                "filesystem_mount_risky" => {
                    mitigations.push("Review mount options for security implications".to_string());
                    mitigations.push("Use read-only mounts where possible".to_string());
                }

                // Networking operations
                "network_remote_execution" | "network_remote_script" => {
                    mitigations.push("Review the script content before execution".to_string());
                    mitigations.push("Download to file first, inspect, then execute".to_string());
                    mitigations.push("Verify the source is trusted".to_string());
                }
                "network_executable_download" => {
                    mitigations.push("Download to /tmp first and verify binary".to_string());
                    mitigations.push("Check binary signatures if available".to_string());
                }
                "network_reverse_shell" => {
                    mitigations.push("Ensure this is authorized penetration testing".to_string());
                    mitigations.push("Use encrypted connections for sensitive data".to_string());
                }
                "network_firewall_disable" => {
                    mitigations
                        .push("Document firewall changes and plan re-enablement".to_string());
                    mitigations
                        .push("Consider temporary rules instead of complete disable".to_string());
                }
                "network_scanning" => {
                    mitigations
                        .push("Ensure you have permission to scan target networks".to_string());
                    mitigations.push("Use less aggressive scan options".to_string());
                }

                // Package manager operations
                "package_global_install" => {
                    mitigations.push("Use virtual environments or user-local installs".to_string());
                    mitigations.push("Review package dependencies before installing".to_string());
                }

                // Warp-specific mitigations
                "terminal_session_kill" => {
                    mitigations.push("Save your work before killing sessions".to_string());
                    mitigations.push("Consider detaching rather than killing".to_string());
                }
                "history_manipulation" => {
                    mitigations.push("Consider if history clearing is necessary".to_string());
                    mitigations
                        .push("Use private shell sessions for sensitive commands".to_string());
                }
                "ai_prompt_injection" => {
                    mitigations
                        .push("Review command for potential AI prompt manipulation".to_string());
                    mitigations.push("Avoid executing untrusted AI-generated commands".to_string());
                }
                "terminal_escape_sequences" => {
                    mitigations.push("Verify escape sequences are intended".to_string());
                    mitigations.push("Test in safe environment first".to_string());
                }
                "process_monitoring" => {
                    mitigations.push("Ensure you have permission to monitor processes".to_string());
                    mitigations.push("Be aware this may expose sensitive information".to_string());
                }
                "memory_dumping" => {
                    mitigations.push("Ensure this is authorized security analysis".to_string());
                    mitigations.push("Handle memory dumps securely".to_string());
                }
                "package_untrusted_source" => {
                    mitigations.push("Verify package sources and signatures".to_string());
                    mitigations.push("Use official package repositories when possible".to_string());
                }
                "package_auto_remove" => {
                    mitigations.push("Review packages to be removed first".to_string());
                    mitigations.push("Use --dry-run to preview changes".to_string());
                }
                "package_direct_url" => {
                    mitigations.push("Verify the URL and package authenticity".to_string());
                    mitigations.push("Check package signatures if available".to_string());
                }

                // Kubernetes and container operations
                "kubernetes_change" => {
                    mitigations.push("Review YAML manifests for configuration changes".to_string());
                    mitigations.push("Test in development environment first".to_string());
                }
                "kubernetes_prod_delete" | "kubernetes_helm_delete" => {
                    mitigations.push("Verify you're targeting the correct resource".to_string());
                    mitigations.push("Ensure backups and rollback plans are in place".to_string());
                    mitigations.push("Consider using --dry-run first".to_string());
                }
                "container_privileged" => {
                    mitigations
                        .push("Use specific capabilities instead of --privileged".to_string());
                    mitigations.push("Run containers as non-root user when possible".to_string());
                }
                "container_cleanup" => {
                    mitigations.push("Review what will be deleted with --volumes flag".to_string());
                    mitigations.push("Consider cleaning specific resources instead".to_string());
                }

                // Cloud operations
                "cloud_aws_deletion" | "cloud_gcp_deletion" | "cloud_azure_rg_delete" => {
                    mitigations.push("Double-check resource identifiers".to_string());
                    mitigations.push("Ensure you have backups".to_string());
                    mitigations.push("Consider using resource tagging for protection".to_string());
                }
                "cloud_s3_force_delete" => {
                    mitigations.push("Verify bucket contents are backed up elsewhere".to_string());
                    mitigations
                        .push("Use versioning and MFA delete for critical buckets".to_string());
                }

                // Infrastructure as Code
                "iac_terraform_destroy" | "iac_pulumi_destroy" => {
                    mitigations.push("Review infrastructure plan before destruction".to_string());
                    mitigations.push("Export state backup before proceeding".to_string());
                    mitigations.push("Consider selective resource targeting".to_string());
                }
                "iac_terraform_unlock" => {
                    mitigations.push("Verify no other operations are in progress".to_string());
                    mitigations.push("Coordinate with team before force unlocking".to_string());
                }

                // Database operations
                "database_deletion" | "database_data_wipe" => {
                    mitigations.push("Create a backup before deletion".to_string());
                    mitigations.push("Verify you're connected to the correct database".to_string());
                    mitigations.push("Test in development environment first".to_string());
                }
                "database_user_mgmt" => {
                    mitigations.push("Review user privileges being granted/revoked".to_string());
                    mitigations.push("Follow principle of least privilege".to_string());
                }

                // Process and script execution
                "script_dynamic_execution" => {
                    mitigations.push("Validate and sanitize input before eval/exec".to_string());
                    mitigations.push("Use safer alternatives like ast.literal_eval".to_string());
                }
                "process_injection" => {
                    mitigations.push("Ensure this is authorized debugging/testing".to_string());
                    mitigations.push("Run in isolated environment".to_string());
                }

                // Version control
                "vcs_destructive" => {
                    mitigations.push("Use git stash to save uncommitted changes".to_string());
                    mitigations
                        .push("Verify you want to discard all local modifications".to_string());
                }
                "vcs_removal" => {
                    mitigations.push("Backup .git directory before deletion".to_string());
                    mitigations
                        .push("Consider if you really need to remove version control".to_string());
                }
                "vcs_force_push" => {
                    mitigations.push("Coordinate with team before force pushing".to_string());
                    mitigations.push("Use --force-with-lease for safer force pushes".to_string());
                }

                // System services
                "service_control" => {
                    mitigations.push("Verify service can be safely stopped".to_string());
                    mitigations.push("Consider using restart instead of stop".to_string());
                }
                "service_critical_stop" => {
                    mitigations
                        .push("Critical services may affect system connectivity".to_string());
                    mitigations
                        .push("Ensure alternative access methods before stopping".to_string());
                }

                // Cryptography
                "crypto_weak_keys" => {
                    mitigations.push("Use strong key lengths (RSA 2048+, Ed25519)".to_string());
                    mitigations.push("Set proper passphrases for private keys".to_string());
                }

                // Shell environment
                "shell_history_clear" => {
                    mitigations.push("Consider selective history editing instead".to_string());
                    mitigations.push("Backup history before clearing if needed".to_string());
                }
                "env_sensitive_export" => {
                    mitigations.push("Use temporary export or read from secure file".to_string());
                    mitigations.push("Consider using secret management tools".to_string());
                }

                // General catch-all
                "sensitive_data" => {
                    mitigations
                        .push("Avoid exposing sensitive data in command history".to_string());
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
                // System and filesystem operations
                "system_destruction" | "disk_overwrite" | "fork_bomb" => {
                    links.push(MitigationLink {
                        title: "System Safety Guide".to_string(),
                        url: format!("{}/system-safety", base_url),
                        description: "Critical system operation safety practices".to_string(),
                    });
                }
                "filesystem_mass_delete"
                | "filesystem_recursive"
                | "filesystem_permissions"
                | "filesystem_system_ownership"
                | "filesystem_mount_risky" => {
                    links.push(MitigationLink {
                        title: "Safe File Operations Guide".to_string(),
                        url: format!("{}/safe-file-operations", base_url),
                        description: "Learn about safe filesystem management".to_string(),
                    });
                }

                // Networking operations
                "network_remote_execution"
                | "network_remote_script"
                | "network_executable_download" => {
                    links.push(MitigationLink {
                        title: "Remote Script Security".to_string(),
                        url: format!("{}/remote-scripts", base_url),
                        description: "Best practices for executing remote scripts".to_string(),
                    });
                }
                "network_reverse_shell" | "network_firewall_disable" | "network_scanning" => {
                    links.push(MitigationLink {
                        title: "Network Security Guide".to_string(),
                        url: format!("{}/network-security", base_url),
                        description: "Network security and penetration testing guidelines"
                            .to_string(),
                    });
                }

                // Package manager operations
                "package_global_install"
                | "package_untrusted_source"
                | "package_auto_remove"
                | "package_direct_url" => {
                    links.push(MitigationLink {
                        title: "Package Manager Security".to_string(),
                        url: format!("{}/package-security", base_url),
                        description: "Safe package installation and management practices"
                            .to_string(),
                    });
                }

                // Container and Kubernetes
                "kubernetes_change" | "kubernetes_prod_delete" | "kubernetes_helm_delete" | "container_docker_sock" | "container_unconfined" | "container_sensitive_mount" => {
                    links.push(MitigationLink {
                        title: "Kubernetes Security Guide".to_string(),
                        url: format!("{}/kubernetes-security", base_url),
                        description: "Safe Kubernetes cluster management practices".to_string(),
                    });
                }
                "container_privileged" | "container_cleanup" => {
                    links.push(MitigationLink {
                        title: "Container Security Best Practices".to_string(),
                        url: format!("{}/container-security", base_url),
                        description: "Docker and container security guidelines".to_string(),
                    });
                }

                // Cloud provider operations
                "cloud_aws_deletion" | "cloud_s3_force_delete" => {
                    links.push(MitigationLink {
                        title: "AWS Security Best Practices".to_string(),
                        url: format!("{}/aws-security", base_url),
                        description: "Safe AWS resource management guidelines".to_string(),
                    });
                }
                "cloud_gcp_deletion" => {
                    links.push(MitigationLink {
                        title: "GCP Security Best Practices".to_string(),
                        url: format!("{}/gcp-security", base_url),
                        description: "Safe Google Cloud resource management".to_string(),
                    });
                }
                "cloud_azure_rg_delete" => {
                    links.push(MitigationLink {
                        title: "Azure Security Best Practices".to_string(),
                        url: format!("{}/azure-security", base_url),
                        description: "Safe Azure resource management guidelines".to_string(),
                    });
                }

                // Infrastructure as Code
                "iac_terraform_destroy" | "iac_terraform_unlock" | "iac_pulumi_destroy" => {
                    links.push(MitigationLink {
                        title: "Infrastructure as Code Security".to_string(),
                        url: format!("{}/iac-security", base_url),
                        description: "Safe infrastructure automation practices".to_string(),
                    });
                }

                // Database operations
                "database_deletion" | "database_data_wipe" | "database_user_mgmt" => {
                    links.push(MitigationLink {
                        title: "Database Safety Guide".to_string(),
                        url: format!("{}/database-safety", base_url),
                        description: "Database backup and recovery strategies".to_string(),
                    });
                }

                // Process and script security
                "script_dynamic_execution" | "process_injection" => {
                    links.push(MitigationLink {
                        title: "Script Security Guidelines".to_string(),
                        url: format!("{}/script-security", base_url),
                        description: "Secure coding and process debugging practices".to_string(),
                    });
                }

                // Version control
                "vcs_destructive" | "vcs_removal" | "vcs_force_push" => {
                    links.push(MitigationLink {
                        title: "Version Control Safety".to_string(),
                        url: format!("{}/vcs-safety", base_url),
                        description: "Safe Git and version control practices".to_string(),
                    });
                }

                // System services
                "service_control" | "service_critical_stop" => {
                    links.push(MitigationLink {
                        title: "System Service Management".to_string(),
                        url: format!("{}/service-management", base_url),
                        description: "Safe system service administration".to_string(),
                    });
                }

                // Cryptography
                "crypto_weak_keys" => {
                    links.push(MitigationLink {
                        title: "Cryptography Best Practices".to_string(),
                        url: format!("{}/crypto-security", base_url),
                        description: "Secure key generation and management".to_string(),
                    });
                }

                // Shell environment
                "shell_history_clear" | "env_sensitive_export" => {
                    links.push(MitigationLink {
                        title: "Shell Security Guide".to_string(),
                        url: format!("{}/shell-security", base_url),
                        description: "Secure shell configuration and usage".to_string(),
                    });
                }

                // Sensitive data
                "sensitive_data" => {
                    links.push(MitigationLink {
                        title: "Secrets Management".to_string(),
                        url: format!("{}/secrets-management", base_url),
                        description: "How to handle secrets securely".to_string(),
                    });
                }

                // Platform-specific (Linux)
                "disable_mandatory_access_control" | "sudoers_modification" => {
                    links.push(MitigationLink {
                        title: "Linux Security Hardening".to_string(),
                        url: format!("{}/linux-security-hardening", base_url),
                        description: "Guidance on MAC, sudoers, and privilege management".to_string(),
                    });
                }

                // Platform-specific (Linux)
                "systemd_mask" | "firewall_flush" | "kernel_module" => {
                    links.push(MitigationLink {
                        title: "Linux System Administration".to_string(),
                        url: format!("{}/linux-admin", base_url),
                        description: "Safe Linux system administration practices".to_string(),
                    });
                }

                // Platform-specific (macOS)
                "sip_disable" | "gatekeeper_disable" | "keychain_delete" => {
                    links.push(MitigationLink {
                        title: "macOS Security Features".to_string(),
                        url: format!("{}/macos-security", base_url),
                        description: "Understanding macOS security mechanisms".to_string(),
                    });
                }

                // Platform-specific (Windows)
                "execution_policy" | "defender_disable" | "registry_hklm" => {
                    links.push(MitigationLink {
                        title: "Windows Security Configuration".to_string(),
                        url: format!("{}/windows-security", base_url),
                        description: "Windows security best practices".to_string(),
                    });
                }

                _ => {}
            }
        }

        // Deduplicate links by URL
        links.sort_by(|a, b| a.url.cmp(&b.url));
        links.dedup_by(|a, b| a.url == b.url);

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
            let requires_confirmation = *self
                .policy
                .require_confirmation
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
            RiskLevel::Safe => "\x1b[32m",     // Green
            RiskLevel::Caution => "\x1b[33m",  // Yellow
            RiskLevel::Warning => "\x1b[93m",  // Bright Yellow
            RiskLevel::Critical => "\x1b[91m", // Bright Red
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

    /// Analyze command with Warp-specific context awareness
    pub fn analyze_command_with_context(
        &mut self,
        command: &str,
        context: Option<&openagent_terminal_core::tty::pty_manager::PtyAiContext>,
    ) -> CommandRisk {
        let mut risk = self.analyze_command(command);

        // Enhance risk analysis with context
        if let Some(ctx) = context {
            self.enhance_risk_with_context(&mut risk, command, ctx);
        }

        risk
    }

    /// Enhance risk analysis with PTY context
    fn enhance_risk_with_context(
        &self,
        risk: &mut CommandRisk,
        command: &str,
        context: &openagent_terminal_core::tty::pty_manager::PtyAiContext,
    ) {
        let working_dir = &context.working_directory;
        let shell_kind = context.shell_kind;

        // Add context-aware risk factors
        let mut additional_factors = Vec::new();

        // Risk in sensitive directories
        if (working_dir.starts_with("/etc")
            || working_dir.starts_with("/boot")
            || working_dir.starts_with("/sys"))
            && (command.contains("rm") || command.contains("mv") || command.contains("cp"))
        {
            additional_factors.push(RiskFactor {
                category: "context_sensitive_directory".to_string(),
                description: format!(
                    "Executing file operations in sensitive directory: {}",
                    working_dir.display()
                ),
                pattern: "sensitive directory operations".to_string(),
            });
        }

        // Shell-specific risks
        use openagent_terminal_core::tty::pty_manager::ShellKind;
        match shell_kind {
            ShellKind::PowerShell => {
                if command.contains("Invoke-Expression") || command.contains("IEX") {
                    additional_factors.push(RiskFactor {
                        category: "powershell_invoke_expression".to_string(),
                        description: "PowerShell Invoke-Expression can execute arbitrary code"
                            .to_string(),
                        pattern: "Invoke-Expression".to_string(),
                    });
                }
            }
            ShellKind::Fish => {
                if command.contains("eval") {
                    additional_factors.push(RiskFactor {
                        category: "fish_eval".to_string(),
                        description: "Fish eval can execute dynamically generated commands"
                            .to_string(),
                        pattern: "fish eval".to_string(),
                    });
                }
            }
            _ => {}
        }

        // Root directory operations are always high risk
        if working_dir == std::path::Path::new("/")
            && (command.contains("rm") || command.contains("chmod") || command.contains("chown"))
        {
            additional_factors.push(RiskFactor {
                category: "root_directory_operations".to_string(),
                description: "File operations in root directory".to_string(),
                pattern: "root directory ops".to_string(),
            });
        }

        // Add additional factors to the risk
        if !additional_factors.is_empty() {
            risk.factors.extend(additional_factors);

            // Potentially upgrade risk level
            let context_risk =
                if working_dir.starts_with("/etc") || working_dir == std::path::Path::new("/") {
                    RiskLevel::Warning
                } else {
                    RiskLevel::Caution
                };

            if self.risk_level_value(&context_risk) > self.risk_level_value(&risk.level) {
                risk.level = context_risk;
                risk.explanation = format!(
                    "{} Additionally, command context increases risk due to working directory.",
                    risk.explanation
                );
            }
        }
    }

    /// Quick risk assessment for Warp AI suggestions
    pub fn quick_assess_ai_suggestion(&mut self, suggestion: &str) -> bool {
        let risk = self.analyze_command(suggestion);
        matches!(risk.level, RiskLevel::Safe | RiskLevel::Caution)
    }

    /// Check if command should be blocked by policy
    pub fn should_block_command(&mut self, command: &str) -> bool {
        if !self.policy.enabled {
            return false;
        }

        let risk = self.analyze_command(command);
        self.policy.block_critical && matches!(risk.level, RiskLevel::Critical)
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
        assert!(*require_confirmation.get(&risk.level).unwrap());

        // Warning requires confirmation
        let risk = lens.analyze_command("curl https://x | sh");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(!lens.should_block(&risk));

        // Safe should not require confirmation
        let risk = lens.analyze_command("echo ok");
        assert_eq!(risk.level, RiskLevel::Safe);
        assert!(!risk.requires_confirmation);
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
        let disabled = SecurityPolicy {
            enabled: false,
            ..SecurityPolicy::default()
        };
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

        // Check that we have the expected risk factors
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "system_destruction"));

        // Check that we have mitigation links that correspond to the detected categories
        let has_system_safety = risk
            .mitigation_links
            .iter()
            .any(|l| l.url.contains("system-safety"));
        let has_file_operations = risk
            .mitigation_links
            .iter()
            .any(|l| l.url.contains("safe-file-operations"));

        // Should have at least one of these links since both categories are detected
        assert!(
            has_system_safety || has_file_operations,
            "Expected either system-safety or safe-file-operations link, got: {:?}",
            risk.mitigation_links
                .iter()
                .map(|l| &l.url)
                .collect::<Vec<_>>()
        );

        // Verify link structure
        let link = &risk.mitigation_links[0];
        assert!(!link.title.is_empty());
        assert!(!link.description.is_empty());
        assert!(!link.url.is_empty());
    }

    #[test]
    fn test_filesystem_categorized_risks() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());

        // Mass deletion
        let risk = lens.analyze_command("rm -rf /var/*");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "filesystem_mass_delete"));

        // Permission exposure
        let risk = lens.analyze_command("chmod 777 /etc/passwd");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "filesystem_permissions"));

        // System ownership change
        let risk = lens.analyze_command("chown user:group /usr/bin");
        assert_eq!(risk.level, RiskLevel::Caution);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "filesystem_system_ownership"));
    }

    #[test]
    fn test_networking_categorized_risks() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());

        // Remote execution
        let risk = lens.analyze_command("curl https://malicious.com/script.sh | sh");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "network_remote_execution"));

        // Executable download to PATH
        let risk = lens.analyze_command("wget https://example.com/binary -o /usr/local/bin/tool");
        assert_eq!(risk.level, RiskLevel::Caution);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "network_executable_download"));

        // Network scanning
        let risk = lens.analyze_command("nmap -sS 10.0.0.0/8");
        println!("Network scan command risk level: {:?}", risk.level);
        println!(
            "Network scan factors: {:?}",
            risk.factors.iter().map(|f| &f.category).collect::<Vec<_>>()
        );
        assert_eq!(risk.level, RiskLevel::Caution);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "network_scanning"));

        // Firewall disable
        let risk = lens.analyze_command("ufw disable");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "network_firewall_disable"));
    }

    #[test]
    fn test_package_manager_categorized_risks() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());

        // Global install
        let risk = lens.analyze_command("npm install -g dangerous-package");
        assert_eq!(risk.level, RiskLevel::Caution);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "package_global_install"));

        // Untrusted source
        let risk = lens.analyze_command("pip install --trusted-host untrusted.com package");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "package_untrusted_source"));

        // Auto removal
        let risk = lens.analyze_command("apt-get autoremove -y");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "package_auto_remove"));

        // Direct URL install
        let risk =
            lens.analyze_command("pip install https://github.com/user/repo/archive/main.zip");
        assert_eq!(risk.level, RiskLevel::Caution);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "package_direct_url"));
    }

    #[test]
    fn test_cloud_categorized_risks() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());

        // AWS S3 force delete
        let risk = lens.analyze_command("aws s3 rb s3://important-bucket --force");
        assert_eq!(risk.level, RiskLevel::Critical);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "cloud_s3_force_delete"));

        // GCP deletion
        let risk = lens.analyze_command("gcloud compute instances delete prod-instance --quiet");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "cloud_gcp_deletion"));

        // Azure resource group deletion
        let risk = lens.analyze_command("az group delete --name prod-rg --yes");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "cloud_azure_rg_delete"));
    }

    #[test]
    fn test_kubernetes_categorized_risks() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());

        // Production deletion
        let risk = lens.analyze_command("kubectl delete deployment app -n production");
        assert_eq!(risk.level, RiskLevel::Critical);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "kubernetes_prod_delete"));

        // General k8s changes
        let risk = lens.analyze_command("kubectl apply -f deployment.yaml");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "kubernetes_change"));

        // Helm production delete
        let risk = lens.analyze_command("helm delete myapp -n production");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "kubernetes_helm_delete"));
    }

    #[test]
    fn test_database_categorized_risks() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());

        // Data wipe
        let risk = lens.analyze_command("DELETE FROM users WHERE 1=1");
        assert_eq!(risk.level, RiskLevel::Critical);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "database_data_wipe"));

        // User management
        let risk = lens.analyze_command("GRANT ALL PRIVILEGES ON * TO user");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "database_user_mgmt"));

        // Database deletion
        let risk = lens.analyze_command("DROP DATABASE important_db");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "database_deletion"));
    }

    #[test]
    fn test_container_categorized_risks() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());

        // Privileged container
        let risk = lens.analyze_command("docker run --privileged -it ubuntu");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "container_privileged"));

        // System cleanup
        let risk = lens.analyze_command("docker system prune -a");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "container_cleanup"));
    }

    #[test]
    fn test_infrastructure_as_code_risks() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());

        // Terraform destroy
        let risk = lens.analyze_command("terraform destroy");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "iac_terraform_destroy"));

        // Force unlock
        let risk = lens.analyze_command("terraform force-unlock abc123");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "iac_terraform_unlock"));

        // Pulumi destroy
        let risk = lens.analyze_command("pulumi destroy --yes");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "iac_pulumi_destroy"));
    }

    #[test]
    fn test_vcs_categorized_risks() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());

        // Force push to protected branch
        let risk = lens.analyze_command("git push --force origin main");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk.factors.iter().any(|f| f.category == "vcs_force_push"));

        // VCS metadata removal
        let risk = lens.analyze_command("rm -rf .git");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk.factors.iter().any(|f| f.category == "vcs_removal"));
    }

    #[test]
    fn test_service_categorized_risks() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());

        // Critical service stop
        let risk = lens.analyze_command("systemctl stop ssh");
        assert_eq!(risk.level, RiskLevel::Critical);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "service_critical_stop"));

        // General service control
        let risk = lens.analyze_command("systemctl disable apache2");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk.factors.iter().any(|f| f.category == "service_control"));
    }

    #[test]
    fn test_crypto_and_environment_risks() {
        let mut lens = SecurityLens::new(SecurityPolicy::default());

        // Weak crypto generation
        let risk = lens.analyze_command("ssh-keygen -N '' -f ~/.ssh/weak_key");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "crypto_weak_keys"));

        // History manipulation
        let risk = lens.analyze_command("history -c");
        assert_eq!(risk.level, RiskLevel::Caution);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "shell_history_clear"));

        // Sensitive environment export
        let risk = lens.analyze_command("export PASSWORD=secret123");
        assert_eq!(risk.level, RiskLevel::Warning);
        assert!(risk
            .factors
            .iter()
            .any(|f| f.category == "env_sensitive_export"));
    }

    #[test]
    fn test_organization_policy_loading() {
        // Test that custom patterns work correctly
        let custom_pattern = CustomPattern {
            pattern: r"(?i)danger-tool\s+--prod".to_string(),
            risk_level: RiskLevel::Critical,
            message: "Using production danger tool".to_string(),
        };

        let policy = SecurityPolicy {
            custom_patterns: vec![custom_pattern],
            ..SecurityPolicy::default()
        };

        let mut lens = SecurityLens::new(policy);
        let risk = lens.analyze_command("danger-tool --prod");
        assert_eq!(risk.level, RiskLevel::Critical);
        assert!(risk.factors.iter().any(|f| f.category == "custom"));
    }

    #[test]
    fn test_rate_limiting_functionality() {
        let rate_limit_config = RateLimitConfig {
            max_detections: 2,
            window_seconds: 60,
            enabled: true,
        };

        let policy = SecurityPolicy {
            rate_limit: rate_limit_config,
            ..SecurityPolicy::default()
        };

        let mut lens = SecurityLens::new(policy);

        // First few detections should work normally
        let risk1 = lens.analyze_command("rm -rf /tmp/test");
        assert_ne!(risk1.level, RiskLevel::Safe);
        assert!(!risk1.factors.iter().any(|f| f.category == "rate_limit"));

        let risk2 = lens.analyze_command("chmod 777 /tmp/file");
        assert_ne!(risk2.level, RiskLevel::Safe);
        assert!(!risk2.factors.iter().any(|f| f.category == "rate_limit"));

        // Third detection should trigger rate limiting
        let risk3 = lens.analyze_command("rm -rf /home/test");
        assert_eq!(risk3.level, RiskLevel::Warning);
        assert!(risk3.factors.iter().any(|f| f.category == "rate_limit"));
    }
}

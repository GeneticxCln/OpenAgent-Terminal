// Security Lens - full implementation
#![allow(dead_code)]

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RiskLevel { Critical, High, Medium, Low, Safe }

impl Default for RiskLevel { fn default() -> Self { RiskLevel::Safe } }

#[derive(Debug, Clone, Default)]
pub struct SecurityPolicy {
    pub require_confirmation: HashMap<RiskLevel, bool>,
}

impl SecurityPolicy {
    pub fn with_defaults() -> Self {
        let mut require_confirmation = HashMap::new();
        require_confirmation.insert(RiskLevel::Critical, true);
        require_confirmation.insert(RiskLevel::High, true);
        require_confirmation.insert(RiskLevel::Medium, false);
        require_confirmation.insert(RiskLevel::Low, false);
        require_confirmation.insert(RiskLevel::Safe, false);
        Self { require_confirmation }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CommandRisk {
    pub level: RiskLevel,
    pub explanation: String,
    pub mitigations: Vec<String>,
    pub factors: Vec<CommandRiskFactor>,
}

#[derive(Debug, Clone, Default)]
pub struct CommandRiskFactor {
    pub category: String,
    pub description: String,
}

pub struct SecurityLens {
    policy: SecurityPolicy,
}

impl SecurityLens {
    pub fn new(policy: SecurityPolicy) -> Self { Self { policy } }

    pub fn analyze_command(&mut self, cmd: &str) -> CommandRisk {
        let mut factors = Vec::new();
        let mut mitigations = Vec::new();
        let mut level = RiskLevel::Safe;
        let lc = cmd.trim().to_lowercase();

        let mut push = |cat: &str, desc: &str, lvl: RiskLevel| {
            factors.push(CommandRiskFactor { category: cat.to_string(), description: desc.to_string() });
            if lvl > level { level = lvl; }
        };

        if lc.starts_with("sudo ") { push("privilege", "Runs with elevated privileges (sudo)", RiskLevel::High); }

        if lc.contains(":(){:|:&};:") { push("dos", "Potential fork bomb pattern detected", RiskLevel::Critical); }

        if lc.contains(" rm -rf ") || lc.starts_with("rm -rf") || lc.contains(" rm -r ") || lc.starts_with("rm -r") {
            push("filesystem", "Recursive remove detected (rm -r/-rf)", RiskLevel::High);
            if lc.contains(" /*") || lc.ends_with("/*") || lc.contains(" / ") { push("filesystem", "Dangerous global path pattern", RiskLevel::Critical); }
            mitigations.push("Use explicit paths (e.g., rm -rf ./target) and review globs".to_string());
        }

        if lc.starts_with("dd ") || lc.contains(" dd ") { push("disk", "Low-level disk write (dd) could corrupt data", RiskLevel::High); }

        if lc.contains(" chmod ") || lc.starts_with("chmod ") {
            push("permissions", "Permission change (chmod)", RiskLevel::Medium);
            if lc.contains(" 777") { push("permissions", "World-writable (777) detected", RiskLevel::High); }
        }
        if lc.contains(" chown ") || lc.starts_with("chown ") { push("permissions", "Ownership change (chown)", RiskLevel::Medium); }

        let pulls = ["curl ", "wget ", "fetch "];
        if pulls.iter().any(|p| lc.contains(p)) {
            push("network", "Downloads from network", RiskLevel::Medium);
            if lc.contains(" | sh") || lc.contains(" | bash") {
                push("execution", "Pipes network download into shell execution", RiskLevel::Critical);
                mitigations.push("Download to file and verify checksum/signature before executing".to_string());
            }
        }

        if lc.starts_with("apt ") || lc.starts_with("apt-get ") || lc.starts_with("yum ") || lc.starts_with("dnf ") || lc.starts_with("pacman ") || lc.starts_with("brew ") {
            push("package", "Package manager operation", RiskLevel::Medium);
            if lc.contains(" -y ") || lc.contains(" --noconfirm") { push("package", "Non-interactive install flags detected", RiskLevel::Medium); }
        }

        if lc.contains(" /etc/") || lc.contains(" /boot/") || lc.contains(" /sys/") || lc.contains(" /proc/") { push("system", "Touches critical system paths", RiskLevel::High); }

        if lc.starts_with("docker ") || lc.starts_with("podman ") { push("container", "Container runtime operation", RiskLevel::Low); if lc.contains(" --privileged") { push("container", "Privileged container flag", RiskLevel::High); } }

        let explanation = match level {
            RiskLevel::Critical => "CRITICAL: Severe risk; may damage system/data".to_string(),
            RiskLevel::High => "High risk operation".to_string(),
            RiskLevel::Medium => "Moderate risk; review before executing".to_string(),
            RiskLevel::Low => "Low risk".to_string(),
            RiskLevel::Safe => "Safe".to_string(),
        };

        CommandRisk { level, explanation, mitigations, factors }
    }

    pub fn should_block(&self, risk: &CommandRisk) -> bool {
        *self.policy.require_confirmation.get(&risk.level).unwrap_or(&false)
    }
}

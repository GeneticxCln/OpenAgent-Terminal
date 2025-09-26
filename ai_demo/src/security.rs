use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel { Low, Medium, High, Critical }

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self { RiskLevel::Low => "LOW", RiskLevel::Medium => "MEDIUM", RiskLevel::High => "HIGH", RiskLevel::Critical => "CRITICAL" }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskReport {
    pub level: RiskLevel,
    pub summary: String,
    pub findings: Vec<String>,
    pub suggestion: Option<String>,
}

pub fn analyze_command(cmd: &str) -> RiskReport {
    let mut findings: Vec<String> = Vec::new();
    let mut level = RiskLevel::Low;
    let mut suggestion: Option<String> = None;

    let lc = cmd.trim().to_lowercase();

    // Shell fork bomb
    if lc.contains(":(){:|:&};:") {
        findings.push("Detected potential fork bomb pattern (:(){:|:&};:)".to_string());
        level = RiskLevel::Critical;
    }

    // sudo/root and destructive flags
    if lc.starts_with("sudo ") {
        findings.push("Runs with elevated privileges (sudo)".to_string());
        level = level.max(RiskLevel::High);
    }

    if lc.contains(" rm -rf ") || lc.starts_with("rm -rf") || lc.contains(" rm -r ") || lc.starts_with("rm -r") {
        findings.push("Recursive remove detected (rm -r/-rf)".to_string());
        level = RiskLevel::High;
        if lc.contains(" /*") || lc.ends_with("/*") || lc.contains(" / ") { level = RiskLevel::Critical; }
        suggestion = Some("Consider using `trash` or review paths explicitly: rm -rf ./target` and ensure no glob expansion to / or *.").map(|s| s.to_string());
    }

    // dd, disk ops
    if lc.starts_with("dd ") || lc.contains(" dd ") {
        findings.push("Low-level disk write (dd) could corrupt data".to_string());
        level = level.max(RiskLevel::High);
    }

    // chmod/chown
    if lc.contains(" chmod ") || lc.starts_with("chmod ") {
        findings.push("Permission change (chmod)".to_string());
        if lc.contains(" 777") { level = level.max(RiskLevel::High); }
    }
    if lc.contains(" chown ") || lc.starts_with("chown ") {
        findings.push("Ownership change (chown)".to_string());
        level = level.max(RiskLevel::Medium);
    }

    // network ops that fetch and execute
    let pulls = ["curl ", "wget ", "fetch "];
    if pulls.iter().any(|p| lc.contains(p)) {
        findings.push("Downloads from network".to_string());
        if lc.contains(" | sh") || lc.contains(" | bash") {
            findings.push("Pipes network download into shell execution".to_string());
            level = RiskLevel::Critical;
            suggestion = Some("Download to a file and verify checksum/signature before executing.").map(|s| s.to_string());
        } else {
            level = level.max(RiskLevel::Medium);
        }
    }

    // package managers with system-wide effects
    if lc.starts_with("apt ") || lc.starts_with("apt-get ") || lc.starts_with("yum ") || lc.starts_with("dnf ") || lc.starts_with("pacman ") || lc.starts_with("brew ") {
        findings.push("Package manager operation".to_string());
        if lc.contains(" -y ") || lc.contains(" --noconfirm") { findings.push("Non-interactive install flag (-y/--noconfirm)".to_string()); }
        level = level.max(RiskLevel::Medium);
    }

    // file system wide or critical path hints
    if lc.contains(" /etc/") || lc.contains(" /boot/") || lc.contains(" /sys/") || lc.contains(" /proc/") {
        findings.push("Touches critical system paths".to_string());
        level = level.max(RiskLevel::High);
    }

    // docker and container runtime operations
    if lc.starts_with("docker ") || lc.starts_with("podman ") {
        findings.push("Container runtime operation".to_string());
        if lc.contains(" --privileged") { level = RiskLevel::High; findings.push("Privileged container flag".to_string()); }
    }

    let summary = match level {
        RiskLevel::Low => "Low risk command".to_string(),
        RiskLevel::Medium => "Potentially impactful operation — review before proceeding".to_string(),
        RiskLevel::High => "High-risk operation — could modify or delete data or system state".to_string(),
        RiskLevel::Critical => "CRITICAL risk — may cause severe system damage or data loss".to_string(),
    };

    RiskReport { level, summary, findings, suggestion }
}

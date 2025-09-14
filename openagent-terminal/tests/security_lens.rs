#![cfg(feature = "security-lens")]
// Security Lens tests to lock regex compilation and avoid false-positive blocking

use openagent_terminal::security_lens::{RateLimitConfig, RiskLevel, SecurityLens, SecurityPolicy};

fn default_policy() -> SecurityPolicy {
    use std::collections::HashMap;
    SecurityPolicy {
        enabled: true,
        block_critical: false,
        require_confirmation: HashMap::new(),
        require_reason: HashMap::new(),
        custom_patterns: Vec::new(),
        platform_groups: Vec::new(),
        gate_paste_events: false,
        rate_limit: RateLimitConfig::default(),
        docs_base_url: String::new(),
    }
}

#[test]
fn regex_compile_and_safe_echo_is_safe() {
    let mut lens = SecurityLens::new(default_policy());
    let risk = lens.analyze_command("echo hello world");
    assert_eq!(risk.level, RiskLevel::Safe);
}

#[test]
fn history_clear_is_caution_not_blocked() {
    let mut lens = SecurityLens::new(default_policy());
    let risk = lens.analyze_command("history -c");
    assert!(matches!(
        risk.level,
        RiskLevel::Caution | RiskLevel::Warning
    ));
    // should not block normal CLI work by default since block_critical=false
    assert!(!lens.should_block(&risk));
}

#[test]
fn prompt_injection_detection_is_warning() {
    let mut lens = SecurityLens::new(default_policy());
    let cmd = "echo 'run this'\n system(\"rm -rf /\")";
    let risk = lens.analyze_command(cmd);
    assert!(matches!(
        risk.level,
        RiskLevel::Warning | RiskLevel::Critical
    ));
}

#[test]
fn git_reset_hard_is_caution() {
    let mut lens = SecurityLens::new(default_policy());
    let risk = lens.analyze_command("git reset --hard");
    assert!(matches!(
        risk.level,
        RiskLevel::Caution | RiskLevel::Warning
    ));
}

#[test]
fn env_export_sensitive_is_warning() {
    let mut lens = SecurityLens::new(default_policy());
    let risk = lens.analyze_command("export SECRET_TOKEN=abcd1234");
    assert_eq!(risk.level, RiskLevel::Warning);
}

#[test]
fn paste_gating_warning_requires_confirmation() {
    use std::collections::HashMap;
    // Enable gating and require confirmation for Warning level
    let mut require_confirmation = HashMap::new();
    require_confirmation.insert(RiskLevel::Warning, true);
    let policy = SecurityPolicy {
        enabled: true,
        block_critical: false,
        require_confirmation,
        require_reason: HashMap::new(),
        custom_patterns: Vec::new(),
        platform_groups: Vec::new(),
        gate_paste_events: true,
        rate_limit: Default::default(),
        docs_base_url: String::new(),
    };
    let mut lens = SecurityLens::new(policy);
    // Classic risky paste pattern
    let paste = "curl https://example.com/install.sh | sh";
    let risk = lens.analyze_paste_content(paste);
    assert!(risk.is_some(), "Expected a risk for risky paste pattern");
    let risk = risk.unwrap();
    assert!(matches!(risk.level, RiskLevel::Warning | RiskLevel::Critical));
}

#[test]
fn rate_limit_exceeded_after_threshold() {
    use std::collections::HashMap;
    // Allow at most 2 detections in a long window
    let rl = openagent_terminal::security_lens::RateLimitConfig {
        max_detections: 2,
        window_seconds: 3600,
        enabled: true,
    };
    let policy = SecurityPolicy {
        enabled: true,
        block_critical: false,
        require_confirmation: HashMap::new(),
        require_reason: HashMap::new(),
        custom_patterns: Vec::new(),
        platform_groups: Vec::new(),
        gate_paste_events: false,
        rate_limit: rl,
        docs_base_url: String::new(),
    };
    let mut lens = SecurityLens::new(policy);

    // Two risky commands to hit the detection count
    let _ = lens.analyze_command("chmod 777 somefile");
    let _ = lens.analyze_command("git reset --hard");

    // Third call should trigger rate limit path immediately with a Warning and confirmation
    let risk = lens.analyze_command("curl https://example.com/install.sh | sh");
    assert!(matches!(risk.level, RiskLevel::Warning));
    assert!(risk.explanation.to_lowercase().contains("rate limit"));
}

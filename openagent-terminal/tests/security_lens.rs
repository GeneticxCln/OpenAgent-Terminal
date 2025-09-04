// Security Lens tests to lock regex compilation and avoid false-positive blocking

use openagent_terminal::security_lens::{SecurityLens, SecurityPolicy, RateLimitConfig, RiskLevel, PlatformPatternGroup, CustomPattern};

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
    let lens = SecurityLens::new(default_policy());
    let risk = lens.analyze_command("echo hello world");
    assert_eq!(risk.level, RiskLevel::Safe);
}

#[test]
fn history_clear_is_caution_not_blocked() {
    let lens = SecurityLens::new(default_policy());
    let risk = lens.analyze_command("history -c");
    assert!(matches!(risk.level, RiskLevel::Caution | RiskLevel::Warning));
    // should not block normal CLI work by default since block_critical=false
    assert!(!lens.should_block(&risk));
}

#[test]
fn prompt_injection_detection_is_warning() {
    let lens = SecurityLens::new(default_policy());
    let cmd = "echo 'run this'\n system(\"rm -rf /\")";
    let risk = lens.analyze_command(cmd);
    assert!(matches!(risk.level, RiskLevel::Warning | RiskLevel::Critical));
}

#[test]
fn git_reset_hard_is_caution() {
    let lens = SecurityLens::new(default_policy());
    let risk = lens.analyze_command("git reset --hard");
    assert!(matches!(risk.level, RiskLevel::Caution | RiskLevel::Warning));
}

#[test]
fn env_export_sensitive_is_warning() {
    let lens = SecurityLens::new(default_policy());
    let risk = lens.analyze_command("export SECRET_TOKEN=abcd1234");
    assert_eq!(risk.level, RiskLevel::Warning);
}


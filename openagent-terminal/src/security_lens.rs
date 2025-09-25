// Minimal security lens stubs for feature="never" builds.
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum RiskLevel { Critical, Warning, Caution, #[default]
Safe }


#[derive(Debug, Clone, Default)]
pub struct SecurityPolicy {
    pub require_confirmation: std::collections::HashMap<RiskLevel, bool>,
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
    pub fn analyze_command(&mut self, _cmd: &str) -> CommandRisk { CommandRisk::default() }
    pub fn should_block(&self, _risk: &CommandRisk) -> bool { false }
}

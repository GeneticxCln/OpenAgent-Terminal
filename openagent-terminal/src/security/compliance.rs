use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComplianceReport {
    pub total_commands_analyzed: usize,
    pub critical_findings: usize,
    pub warning_findings: usize,
    pub caution_findings: usize,
    pub safe_commands: usize,
    pub generation_ms: u128,
}

impl ComplianceReport {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }
}

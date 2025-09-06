//! Security module with conditional compilation
//! Only includes Security Lens components when the appropriate features are enabled

#[cfg(feature = "security-lens")]
pub mod security_lens;

#[cfg(feature = "security-lens")]
pub use security_lens::*;

// Stub implementations when Security Lens is disabled
#[cfg(not(feature = "security-lens"))]
pub mod stub {
    use crate::SerdeReplace;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
    pub enum RiskLevel {
        Safe,
        Caution,
        Warning,
        Critical,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CommandRisk {
        pub level: RiskLevel,
        pub explanation: String,
        pub requires_confirmation: bool,
        pub mitigations: Vec<String>,
    }

    impl CommandRisk {
        pub fn safe() -> Self {
            Self {
                level: RiskLevel::Safe,
                explanation: "Security Lens disabled".to_string(),
                requires_confirmation: false,
                mitigations: Vec::new(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct SecurityPolicy {
        pub enabled: bool,
        pub block_critical: bool,
        pub require_confirmation: HashMap<RiskLevel, bool>,
        pub gate_paste_events: bool,
    }

    impl SerdeReplace for SecurityPolicy {
        fn replace(&mut self, _value: toml::Value) -> Result<(), Box<dyn std::error::Error>> {
            // In stub mode, accept overrides but do nothing
            Ok(())
        }
    }

    impl Default for SecurityPolicy {
        fn default() -> Self {
            let mut require_confirmation = HashMap::new();
            require_confirmation.insert(RiskLevel::Caution, true);
            require_confirmation.insert(RiskLevel::Warning, true);
            require_confirmation.insert(RiskLevel::Critical, true);
            Self {
                enabled: false,
                block_critical: false,
                require_confirmation,
                gate_paste_events: false,
            }
        }
    }

    pub struct SecurityLens;

    impl SecurityLens {
        pub fn new(_policy: SecurityPolicy) -> Self {
            Self
        }

        pub fn analyze_command(&mut self, _command: &str) -> CommandRisk {
            CommandRisk::safe()
        }

        pub fn analyze_paste_content(&mut self, _content: &str) -> Option<CommandRisk> {
            None
        }

        pub fn should_block(&self, _risk: &CommandRisk) -> bool {
            false
        }
    }
}

#[cfg(not(feature = "security-lens"))]
pub use stub::*;

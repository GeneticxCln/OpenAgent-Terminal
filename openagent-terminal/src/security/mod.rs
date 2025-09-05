//! Security module with conditional compilation
//! Only includes Security Lens components when the appropriate features are enabled

#[cfg(feature = "security-lens")]
pub mod security_lens;

#[cfg(feature = "security-lens")]
pub use security_lens::*;

// Stub implementations when Security Lens is disabled
#[cfg(not(feature = "security-lens"))]
pub mod stub {
    use serde::{Deserialize, Serialize};
    
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
    pub enum RiskLevel {
        Safe,
    }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CommandRisk {
        pub level: RiskLevel,
        pub explanation: String,
        pub requires_confirmation: bool,
    }
    
    impl CommandRisk {
        pub fn safe() -> Self {
            Self {
                level: RiskLevel::Safe,
                explanation: "Security Lens disabled".to_string(),
                requires_confirmation: false,
            }
        }
    }
    
    pub struct SecurityLens;
    
    impl SecurityLens {
        pub fn new(_policy: ()) -> Self {
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

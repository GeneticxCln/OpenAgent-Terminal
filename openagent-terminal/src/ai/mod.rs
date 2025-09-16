// AI module for OpenAgent Terminal
// Re-exports all AI agents and related functionality

pub mod agents;

// Re-export key AI types and structures
pub use agents::{
    ActionPriority, ActionType, Agent, AgentArtifact, AgentCapability, AgentConfig, AgentContext,
    AgentManager, AgentRequest, AgentRequestType, AgentResponse, AgentStatus, ArtifactType,
    SuggestedAction,
};

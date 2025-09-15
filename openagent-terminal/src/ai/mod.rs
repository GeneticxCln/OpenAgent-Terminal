// AI module for OpenAgent Terminal
// Re-exports all AI agents and related functionality

pub mod agents;

// Re-export key AI types and structures
pub use agents::{
    Agent, AgentCapability, AgentConfig, AgentContext, AgentRequest, AgentResponse,
    AgentStatus, AgentRequestType, AgentArtifact, ArtifactType, SuggestedAction,
    ActionType, ActionPriority, AgentManager
};
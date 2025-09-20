use anyhow::Result;
use openagent_terminal::ai::agents::{
    advanced_conversation_features::AdvancedConversationFeatures,
    conversation_manager::ConversationManager, natural_language::ConversationRole,
    privacy_content_filter::PrivacyContentFilter, terminal_ui_integration::TerminalUIIntegration,
    Agent, AgentConfig, AgentContext, AgentRequest, AgentRequestType, AgentResponse,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "ai")]
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    println!("🖥️ Terminal UI Integration Demo");
    println!("==============================");

    // Run the demo using the display buffer (no crossterm)
    let result = run_demo().await;

    result
}

async fn run_demo() -> Result<()> {
    // Initialize core agents
    let conversation_manager = Arc::new(ConversationManager::new());
    let privacy_filter = Arc::new({
        let mut filter = PrivacyContentFilter::new();
        filter.initialize(AgentConfig::default()).await?;
        filter
    });
    let advanced_features = Arc::new({
        let mut features = AdvancedConversationFeatures::new(conversation_manager.clone());
        features.initialize(AgentConfig::default()).await?;
        features
    });

    // Initialize Terminal UI Integration
    let mut terminal_ui = TerminalUIIntegration::new()
        .with_conversation_manager(conversation_manager.clone())
        .with_privacy_filter(privacy_filter.clone())
        .with_advanced_features(advanced_features.clone());
    terminal_ui.initialize(AgentConfig::default()).await?;

    // Apply default theme via public API
    terminal_ui.set_theme("default").await?;

    // Create a conversation session using ConversationManager and add a couple of turns
    let session_id =
        conversation_manager.create_session(Some("Terminal UI Demo Session".to_string())).await?;
    conversation_manager
        .add_turn(
            session_id,
            ConversationRole::User,
            "Hello, I'd like to set up a secure development environment.".to_string(),
            None,
            Vec::new(),
        )
        .await?;
    conversation_manager
        .add_turn(
            session_id,
            ConversationRole::Assistant,
            "I'll help you set that up. Let's begin with your requirements.".to_string(),
            None,
            Vec::new(),
        )
        .await?;

    // Show conversation and privacy status using the public API
    terminal_ui.show_conversation(session_id).await?;
    terminal_ui.show_privacy_status().await?;

    // Render via the display buffer
    terminal_ui.render().await?;

    // Also demonstrate using AgentRequest/AgentResponse from ai::agents
    let ctx = AgentContext {
        project_root: None,
        current_directory: std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        current_branch: None,
        open_files: Vec::new(),
        recent_commands: Vec::new(),
        environment_vars: std::env::vars().collect(),
        user_preferences: HashMap::new(),
    };

    let render_request = AgentRequest {
        id: Uuid::new_v4(),
        request_type: AgentRequestType::Custom("Render".to_string()),
        payload: json!({}),
        context: ctx,
        metadata: HashMap::new(),
    };

    let _response: AgentResponse = terminal_ui.handle_request(render_request).await?;

    Ok(())
}

#[cfg(not(feature = "ai"))]
fn main() {
    println!("❌ This example requires the 'ai' feature to be enabled.");
    println!("Run with: cargo run --example terminal_ui_integration_demo --features ai");
}

#![allow(
    clippy::pedantic,
    clippy::uninlined_format_args,
    clippy::wildcard_imports,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::needless_pass_by_value
)]

use anyhow::Result;
use chrono::Duration;
use openagent_terminal::ai::agents::{
    advanced_conversation_features::{
        AdvancedConversationConfig, AdvancedConversationFeatures, BranchReason,
        ContextSharingLevel, MergeStrategy, SummaryType,
    },
    blitzy_project_context::BlitzyProjectContextAgent,
    conversation_manager::ConversationManager,
    natural_language::{ConversationRole, Entity, EntityType, Intent},
    workflow_orchestrator::WorkflowOrchestrator,
    Agent, AgentConfig,
};
use std::sync::Arc;

#[cfg(feature = "ai")]
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    println!("🤖 Advanced Conversation Features Demo");
    println!("========================================");

    // 1. Create core conversation manager
    println!("\n📋 Setting up conversation manager...");
    let conversation_manager = Arc::new(ConversationManager::new());

    // 2. Create project context agent
    println!("🗂️ Setting up project context agent...");
    let mut project_agent = BlitzyProjectContextAgent::new();
    let project_agent_config = AgentConfig::default();
    project_agent.initialize(project_agent_config).await?;
    let project_agent = Arc::new(project_agent);

    // 3. Create workflow orchestrator
    println!("🔀 Setting up workflow orchestrator...");
    let mut workflow_orchestrator = WorkflowOrchestrator::new();
    let workflow_config = AgentConfig::default();
    workflow_orchestrator.initialize(workflow_config).await?;
    let workflow_orchestrator = Arc::new(workflow_orchestrator);

    // 4. Configure advanced conversation features
    println!("⚙️ Configuring advanced conversation features...");
    let config = AdvancedConversationConfig {
        enable_branching: true,
        max_branches_per_tree: 5,
        enable_auto_summarization: true,
        summarization_interval: Duration::minutes(10),
        enable_goal_automation: true,
        enable_cross_session_context: true,
        context_sharing_default: ContextSharingLevel::Moderate,
        branch_retention_days: 14,
        summary_compression_target: 0.25,
        goal_auto_detection: true,
    };

    // 5. Initialize advanced conversation features
    println!("🚀 Initializing advanced conversation features...");
    let mut advanced_features = AdvancedConversationFeatures::new(conversation_manager.clone())
        .with_config(config)
        .with_project_context_agent(project_agent.clone())
        .with_workflow_orchestrator(workflow_orchestrator.clone());

    let features_config = AgentConfig::default();
    advanced_features.initialize(features_config).await?;

    // Demo 1: Conversation Branching
    println!("\n🌳 DEMO 1: Conversation Branching");
    println!("==================================");

    // Create main session
    let main_session =
        conversation_manager.create_session(Some("Main Development Session".to_string())).await?;
    println!("📝 Created main session: {}", main_session);

    // Add some conversation content
    conversation_manager
        .add_turn(
            main_session,
            ConversationRole::User,
            "I need to implement a new feature for user authentication.".to_string(),
            Some(Intent {
                name: "request_implementation".to_string(),
                confidence: 0.9,
                parameters: std::collections::HashMap::new(),
                target_agent: None,
            }),
            vec![Entity {
                entity_type: EntityType::Custom("Technology".to_string()),
                value: "authentication".to_string(),
                confidence: 0.9,
                span: (0, 14),
            }],
        )
        .await?;
    conversation_manager.add_turn(
        main_session,
        ConversationRole::Assistant,
        "I'll help you implement user authentication. What type of authentication would you like - JWT tokens, OAuth, or session-based?".to_string(),
        None,
        vec![]
    ).await?;
    conversation_manager
        .add_turn(
            main_session,
            ConversationRole::User,
            "Let's explore both JWT and OAuth options.".to_string(),
            None,
            vec![],
        )
        .await?;

    // Create branches for different authentication approaches
    let jwt_branch = advanced_features
        .create_branch(
            main_session,
            "JWT Implementation".to_string(),
            BranchReason::ExperimentalPath,
            None,
        )
        .await?;
    println!("🌿 Created JWT branch: {}", jwt_branch);

    let oauth_branch = advanced_features
        .create_branch(
            main_session,
            "OAuth Integration".to_string(),
            BranchReason::ExperimentalPath,
            None,
        )
        .await?;
    println!("🌿 Created OAuth branch: {}", oauth_branch);

    // Switch to JWT branch and continue conversation
    advanced_features.switch_branch(main_session, jwt_branch.clone()).await?;
    println!("🔀 Switched to JWT branch");

    // Demo 2: Conversation Summarization
    println!("\n📊 DEMO 2: Conversation Summarization");
    println!("=====================================");

    // Add more conversation content to summarize
    conversation_manager.add_turn(
        main_session,
        ConversationRole::Assistant,
        "For JWT implementation, I recommend using a library like jsonwebtoken for Rust. We'll need to set up token generation, validation, and refresh mechanisms.".to_string(),
        None,
        vec![]
    ).await?;
    conversation_manager
        .add_turn(
            main_session,
            ConversationRole::User,
            "What about token expiration and security best practices?".to_string(),
            None,
            vec![],
        )
        .await?;
    conversation_manager.add_turn(
        main_session,
        ConversationRole::Assistant,
        "Great question! We should implement short-lived access tokens (15-30 minutes) with longer refresh tokens. Also include CSRF protection and secure token storage.".to_string(),
        None,
        vec![]
    ).await?;

    // Generate different types of summaries
    let brief_summary =
        advanced_features.generate_summary(main_session, SummaryType::Brief).await?;
    println!("📋 Brief Summary: {}", brief_summary.content);
    println!("🔑 Key Points: {}", brief_summary.key_points.len());
    println!("✅ Action Items: {}", brief_summary.action_items.len());

    let comprehensive_summary =
        advanced_features.generate_summary(main_session, SummaryType::Comprehensive).await?;
    println!("📄 Comprehensive Summary: {}", comprehensive_summary.content);

    let action_summary =
        advanced_features.generate_summary(main_session, SummaryType::ActionFocused).await?;
    println!("🎯 Action-Focused Summary: {}", action_summary.content);

    // Demo 3: Goal Automation
    println!("\n🎯 DEMO 3: Goal Automation");
    println!("==========================");

    // Start goal tracking for the authentication implementation
    let auth_goal = advanced_features
        .start_goal_tracking(
            main_session,
            "Implement complete user authentication system".to_string(),
            "feature_development".to_string(),
        )
        .await?;
    println!("🎯 Started goal tracking: {}", auth_goal);

    // Update goal progress
    advanced_features.update_goal_progress(main_session, auth_goal.clone(), 0.25).await?;
    println!("📈 Updated goal progress: 25% complete");

    advanced_features.update_goal_progress(main_session, auth_goal.clone(), 0.75).await?;
    println!("📈 Updated goal progress: 75% complete");

    // Create a secondary goal for testing
    let testing_goal = advanced_features
        .start_goal_tracking(
            main_session,
            "Write comprehensive tests for authentication".to_string(),
            "testing".to_string(),
        )
        .await?;
    println!("🧪 Started testing goal: {}", testing_goal);

    // Demo 4: Multi-Session Coordination
    println!("\n🔗 DEMO 4: Multi-Session Coordination");
    println!("====================================");

    // Create additional sessions for different aspects of the project
    let frontend_session =
        conversation_manager.create_session(Some("Frontend Integration".to_string())).await?;
    let testing_session =
        conversation_manager.create_session(Some("Testing Strategy".to_string())).await?;
    let docs_session =
        conversation_manager.create_session(Some("Documentation".to_string())).await?;

    println!("🖥️ Created frontend session: {}", frontend_session);
    println!("🧪 Created testing session: {}", testing_session);
    println!("📚 Created docs session: {}", docs_session);

    // Create a session group to coordinate all authentication-related work
    let auth_group = advanced_features
        .create_session_group(
            "Authentication Project".to_string(),
            "Coordinated development of user authentication system".to_string(),
            vec![main_session, frontend_session, testing_session, docs_session],
        )
        .await?;
    println!("👥 Created session group: {}", auth_group);

    // Add some activity to the coordinated sessions
    conversation_manager
        .add_turn(
            frontend_session,
            ConversationRole::User,
            "How should the frontend handle JWT tokens?".to_string(),
            None,
            vec![],
        )
        .await?;
    conversation_manager.add_turn(
        frontend_session,
        ConversationRole::Assistant,
        "Store JWT tokens securely in httpOnly cookies or sessionStorage, and include them in Authorization headers for API calls.".to_string(),
        None,
        vec![]
    ).await?;

    conversation_manager
        .add_turn(
            testing_session,
            ConversationRole::User,
            "What testing strategies should we use for authentication?".to_string(),
            None,
            vec![],
        )
        .await?;
    conversation_manager.add_turn(
        testing_session,
        ConversationRole::Assistant,
        "We need unit tests for token validation, integration tests for login/logout flows, and security tests for common vulnerabilities.".to_string(),
        None,
        vec![]
    ).await?;

    // Demo 5: Branch Merging
    println!("\n🔀 DEMO 5: Branch Merging");
    println!("========================");

    // Switch back to OAuth branch and add some content
    advanced_features.switch_branch(main_session, oauth_branch.clone()).await?;
    println!("🔀 Switched to OAuth branch");

    conversation_manager.add_turn(
        main_session,
        ConversationRole::Assistant,
        "For OAuth integration, we can use Google, GitHub, or custom OAuth providers. I recommend the oauth2 crate for Rust.".to_string(),
        None,
        vec![]
    ).await?;

    // Merge both authentication branches back into main
    let merge_id = advanced_features
        .merge_branches(
            main_session,
            vec![jwt_branch, oauth_branch],
            "main".to_string(),
            MergeStrategy::ContextAware,
        )
        .await?;
    println!("🔗 Merged authentication branches: {}", merge_id);

    // Final status report
    println!("\n📈 FINAL STATUS REPORT");
    println!("======================");

    let status = advanced_features.status().await;
    println!(
        "🏥 Advanced Features Health: {}",
        if status.is_healthy { "✅ Healthy" } else { "❌ Unhealthy" }
    );
    println!("⏱️  Last Activity: {}", status.last_activity);
    if let Some(task) = status.current_task {
        println!("🔄 Current Task: {}", task);
    }

    // Complete the main goal
    advanced_features.update_goal_progress(main_session, auth_goal, 1.0).await?;
    println!("🎉 Main authentication goal completed!");

    // Generate final comprehensive summary
    println!("\n📋 FINAL COMPREHENSIVE SUMMARY");
    println!("===============================");

    let final_summary =
        advanced_features.generate_summary(main_session, SummaryType::GoalOriented).await?;
    println!("Summary: {}", final_summary.content);
    println!("Compression Ratio: {:.1}%", final_summary.compression_ratio * 100.0);
    println!("Generated At: {}", final_summary.generated_at);

    // Cleanup
    println!("\n🧹 Cleaning up...");
    advanced_features.shutdown().await?;
    println!("✅ Demo completed successfully!");

    Ok(())
}

#[cfg(not(feature = "ai"))]
fn main() {
    println!("❌ This example requires the 'ai' feature to be enabled.");
    println!("Run with: cargo run --example advanced_conversation_demo --features ai");
}

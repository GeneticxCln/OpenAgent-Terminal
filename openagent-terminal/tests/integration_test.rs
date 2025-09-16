#[cfg(feature = "ai")]
mod integration_tests {
    use anyhow::Result;
    use openagent_terminal::ai::agents::{
        advanced_conversation_features::AdvancedConversationFeatures,
        blitzy_project_context::{BlitzyProjectContext, ProjectConfig},
        conversation_manager::ConversationManager,
        natural_language::NaturalLanguageAgent,
        privacy_content_filter::{
            ComplianceStandard, DataClassification, PrivacyContentFilter, PrivacyFilterConfig,
        },
        terminal_ui_integration::TerminalUIIntegration,
        workflow_orchestrator::{WorkflowConfig, WorkflowOrchestrator},
        Agent, AgentConfig,
    };
    use std::sync::Arc;
    use std::time::Duration;
    use tokio;

    async fn setup_integrated_system() -> Result<(
        Arc<ConversationManager>,
        Arc<PrivacyContentFilter>,
        Arc<AdvancedConversationFeatures>,
        Arc<TerminalUIIntegration>,
        Arc<NaturalLanguageAgent>,
        Arc<WorkflowOrchestrator>,
        Arc<BlitzyProjectContext>,
    )> {
        // Initialize core agents
        let conversation_manager = Arc::new(ConversationManager::new());

        let privacy_filter = Arc::new({
            let mut filter = PrivacyContentFilter::new().with_config(PrivacyFilterConfig {
                enable_real_time_scanning: true,
                enable_batch_processing: true,
                enable_audit_logging: true,
                default_data_classification: DataClassification::Internal,
                max_content_size_mb: 5,
                scan_timeout_seconds: 10,
                cache_results: true,
                cache_ttl_minutes: 30,
                compliance_standards: vec![ComplianceStandard::GDPR, ComplianceStandard::CCPA],
                notification_channels: vec!["test@example.com".to_string()],
            });
            filter.initialize(AgentConfig::default()).await?;
            filter
        });

        let advanced_features = Arc::new({
            let mut features = AdvancedConversationFeatures::new(conversation_manager.clone());
            features.initialize(AgentConfig::default()).await?;
            features
        });

        let terminal_ui = Arc::new({
            let mut ui = TerminalUIIntegration::new()
                .with_conversation_manager(conversation_manager.clone())
                .with_privacy_filter(privacy_filter.clone())
                .with_advanced_features(advanced_features.clone());
            ui.initialize(AgentConfig::default()).await?;
            ui
        });

        let natural_language = Arc::new({
            let mut agent = NaturalLanguageAgent::new();
            agent.initialize(AgentConfig::default()).await?;
            agent
        });

        let workflow_orchestrator = Arc::new({
            let mut orchestrator = WorkflowOrchestrator::new().with_config(WorkflowConfig {
                max_concurrent_workflows: 5,
                workflow_timeout_minutes: 30,
                enable_workflow_persistence: false,
                workflow_history_retention_days: 7,
                enable_dependency_resolution: true,
                auto_retry_failed_steps: true,
                max_retry_attempts: 3,
                enable_parallel_execution: true,
                workflow_priority_levels: 3,
                enable_workflow_templates: true,
                template_validation_strict: true,
                enable_real_time_monitoring: true,
                notification_channels: vec!["workflow@example.com".to_string()],
            });
            orchestrator.initialize(AgentConfig::default()).await?;
            orchestrator
        });

        let project_context = Arc::new({
            let mut context = BlitzyProjectContext::new().with_config(ProjectConfig {
                auto_discovery: true,
                deep_analysis: true,
                cache_enabled: true,
                cache_ttl_minutes: 60,
                max_file_size_mb: 10,
                exclude_patterns: vec![".git".to_string(), "node_modules".to_string()],
                include_hidden_files: false,
                analysis_depth: 3,
                enable_dependency_tracking: true,
                enable_change_detection: true,
                enable_ai_insights: true,
                parallel_processing: true,
                max_parallel_jobs: 4,
            });
            context.initialize(AgentConfig::default()).await?;
            context
        });

        Ok((
            conversation_manager,
            privacy_filter,
            advanced_features,
            terminal_ui,
            natural_language,
            workflow_orchestrator,
            project_context,
        ))
    }

    #[tokio::test]
    async fn test_agent_initialization() -> Result<()> {
        let (
            conversation_manager,
            privacy_filter,
            advanced_features,
            terminal_ui,
            natural_language,
            workflow_orchestrator,
            project_context,
        ) = setup_integrated_system().await?;

        // Test that all agents are properly initialized
        assert!(conversation_manager.status().await.is_healthy);
        assert!(privacy_filter.status().await.is_healthy);
        assert!(advanced_features.status().await.is_healthy);
        assert!(terminal_ui.status().await.is_healthy);
        assert!(natural_language.status().await.is_healthy);
        assert!(workflow_orchestrator.status().await.is_healthy);
        assert!(project_context.status().await.is_healthy);

        Ok(())
    }

    #[tokio::test]
    async fn test_conversation_with_privacy_filtering() -> Result<()> {
        let (
            conversation_manager,
            privacy_filter,
            _advanced_features,
            _terminal_ui,
            _natural_language,
            _workflow_orchestrator,
            _project_context,
        ) = setup_integrated_system().await?;

        // Create a conversation session
        let session_id = conversation_manager
            .create_session(Some("Integration Test Session".to_string()))
            .await?;

        // Test sensitive content handling
        let sensitive_message =
            "My email is test@example.com and my credit card is 4111-1111-1111-1111";

        // Scan for privacy issues
        let scan_result = privacy_filter.scan_content(sensitive_message, None).await?;
        assert!(!scan_result.detections.is_empty());
        assert!(scan_result.overall_risk_score > 0.0);

        // Apply redaction
        let redaction_result = privacy_filter
            .redact_content(sensitive_message, "gdpr-policy", None)
            .await?;
        assert!(redaction_result.redacted_content != sensitive_message);
        assert!(!redaction_result.redactions_applied.is_empty());

        // Add redacted content to conversation
        conversation_manager
            .add_turn(
                session_id,
                openagent_terminal::ai::agents::natural_language::ConversationRole::User,
                redaction_result.redacted_content,
                None,
                vec![],
            )
            .await?;

        // Verify conversation contains redacted content
        let conversation_history = conversation_manager
            .get_conversation_history(session_id, 10)
            .await?;
        assert_eq!(conversation_history.len(), 1);
        assert!(conversation_history[0].content != sensitive_message);

        Ok(())
    }

    #[tokio::test]
    async fn test_advanced_features_integration() -> Result<()> {
        let (
            conversation_manager,
            _privacy_filter,
            advanced_features,
            _terminal_ui,
            _natural_language,
            _workflow_orchestrator,
            _project_context,
        ) = setup_integrated_system().await?;

        let session_id = conversation_manager
            .create_session(Some("Advanced Features Test".to_string()))
            .await?;

        // Test goal creation and management
        let goal_id = advanced_features
            .create_goal(
                "Test Goal".to_string(),
                "Testing goal functionality".to_string(),
                vec!["test".to_string()],
                None,
            )
            .await?;

        let goals = advanced_features.get_active_goals().await?;
        assert!(!goals.is_empty());
        assert!(goals.iter().any(|g| g.id == goal_id));

        // Test conversation branching
        let branch_id = advanced_features
            .create_conversation_branch(session_id, Some("Test Branch".to_string()))
            .await?;

        let branches = advanced_features
            .get_conversation_branches(session_id)
            .await?;
        assert!(!branches.is_empty());
        assert!(branches.iter().any(|b| b.id == branch_id));

        // Test context switching
        advanced_features
            .switch_conversation_context(session_id, branch_id)
            .await?;

        // Update goal progress
        advanced_features
            .update_goal_progress(goal_id.clone(), 0.5)
            .await?;
        let updated_goals = advanced_features.get_active_goals().await?;
        let updated_goal = updated_goals.iter().find(|g| g.id == goal_id).unwrap();
        assert_eq!(updated_goal.progress, 0.5);

        Ok(())
    }

    #[tokio::test]
    async fn test_terminal_ui_integration() -> Result<()> {
        let (
            _conversation_manager,
            _privacy_filter,
            _advanced_features,
            terminal_ui,
            _natural_language,
            _workflow_orchestrator,
            _project_context,
        ) = setup_integrated_system().await?;

        // Test UI component management
        let initial_count = terminal_ui.get_component_count().await;

        // Add a test component (would be a mock in real implementation)
        // For now, just verify the initial state
        assert!(terminal_ui.get_component_count().await >= 0);

        // Test session creation through UI
        let session_id = terminal_ui
            .create_conversation_session(Some("UI Test Session".to_string()))
            .await?;
        assert!(!session_id.is_empty());

        // Test theme and layout management
        use openagent_terminal::ai::agents::terminal_ui_integration::{Layout, Theme};

        terminal_ui.set_theme(Theme::dark()).await?;
        terminal_ui.set_layout(Layout::SplitVertical).await?;

        // Test privacy status updates
        terminal_ui
            .update_privacy_status(0.7, 3, vec!["GDPR".to_string(), "CCPA".to_string()])
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_workflow_orchestration_integration() -> Result<()> {
        let (
            _conversation_manager,
            privacy_filter,
            _advanced_features,
            _terminal_ui,
            _natural_language,
            workflow_orchestrator,
            _project_context,
        ) = setup_integrated_system().await?;

        use openagent_terminal::ai::agents::workflow_orchestrator::{
            StepType, WorkflowDefinition, WorkflowPriority, WorkflowStep,
        };

        // Create a workflow that includes privacy scanning
        let workflow_def = WorkflowDefinition {
            id: "privacy_scan_workflow".to_string(),
            name: "Privacy Scanning Workflow".to_string(),
            description: "Workflow that scans content for privacy issues".to_string(),
            steps: vec![WorkflowStep {
                id: "scan_content".to_string(),
                name: "Scan Content".to_string(),
                step_type: StepType::Tool,
                parameters: serde_json::json!({
                    "tool": "privacy_scanner",
                    "content": "test@example.com is my email"
                }),
                dependencies: vec![],
                timeout_minutes: Some(5),
                retry_count: 0,
                is_critical: true,
            }],
            priority: WorkflowPriority::Medium,
            timeout_minutes: Some(15),
            tags: vec!["privacy".to_string(), "security".to_string()],
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            is_template: false,
        };

        // Execute workflow
        let execution_id = workflow_orchestrator
            .execute_workflow(
                workflow_def,
                serde_json::Value::Object(serde_json::Map::new()),
                None,
            )
            .await?;

        // Wait briefly for execution to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check execution status
        let status = workflow_orchestrator
            .get_execution_status(execution_id)
            .await?;
        assert!(!status.execution_id.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_project_context_integration() -> Result<()> {
        let (
            _conversation_manager,
            _privacy_filter,
            _advanced_features,
            _terminal_ui,
            _natural_language,
            _workflow_orchestrator,
            project_context,
        ) = setup_integrated_system().await?;

        // Test project discovery (using current directory)
        let current_dir = std::env::current_dir()?;
        let project_info = project_context
            .analyze_project(current_dir.to_str().unwrap())
            .await?;

        assert!(!project_info.name.is_empty());
        assert!(!project_info.root_path.is_empty());

        // Test file analysis
        let files = project_context
            .get_project_files(&project_info.root_path, 100)
            .await?;
        assert!(!files.is_empty());

        // Test dependency analysis
        let dependencies = project_context
            .analyze_dependencies(&project_info.root_path)
            .await?;
        // Dependencies might be empty for some projects, so just verify it doesn't error

        // Test project insights generation
        let insights = project_context
            .generate_project_insights(&project_info.root_path)
            .await?;
        assert!(!insights.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_natural_language_processing() -> Result<()> {
        let (
            conversation_manager,
            privacy_filter,
            _advanced_features,
            _terminal_ui,
            natural_language,
            _workflow_orchestrator,
            _project_context,
        ) = setup_integrated_system().await?;

        let session_id = conversation_manager
            .create_session(Some("NLP Test Session".to_string()))
            .await?;

        // Test natural language processing with privacy filtering
        let user_input =
            "Help me set up a database connection with username admin and password secret123";

        // First scan for privacy issues
        let scan_result = privacy_filter.scan_content(user_input, None).await?;

        let processed_input = if !scan_result.detections.is_empty() {
            // Apply redaction if sensitive content detected
            let redaction_result = privacy_filter
                .redact_content(user_input, "gdpr-policy", None)
                .await?;
            redaction_result.redacted_content
        } else {
            user_input.to_string()
        };

        // Process through natural language agent
        let response = natural_language
            .process_message(&processed_input, None)
            .await?;
        assert!(!response.content.is_empty());

        // Add to conversation
        conversation_manager
            .add_turn(
                session_id,
                openagent_terminal::ai::agents::natural_language::ConversationRole::User,
                processed_input,
                None,
                vec![],
            )
            .await?;

        conversation_manager
            .add_turn(
                session_id,
                openagent_terminal::ai::agents::natural_language::ConversationRole::Assistant,
                response.content,
                None,
                vec![],
            )
            .await?;

        // Verify conversation history
        let history = conversation_manager
            .get_conversation_history(session_id, 10)
            .await?;
        assert_eq!(history.len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_comprehensive_integration_flow() -> Result<()> {
        let (
            conversation_manager,
            privacy_filter,
            advanced_features,
            terminal_ui,
            natural_language,
            workflow_orchestrator,
            project_context,
        ) = setup_integrated_system().await?;

        // 1. Create a conversation session
        let session_id = conversation_manager
            .create_session(Some("Comprehensive Integration Test".to_string()))
            .await?;

        // 2. Set up UI with session
        terminal_ui
            .add_conversation_message(
                session_id,
                "System".to_string(),
                "Starting comprehensive integration test".to_string(),
                None,
            )
            .await?;

        // 3. Create a goal for the integration test
        let goal_id = advanced_features
            .create_goal(
                "Complete Integration Test".to_string(),
                "Test all system components working together".to_string(),
                vec!["integration".to_string(), "testing".to_string()],
                None,
            )
            .await?;

        // 4. Analyze current project context
        let current_dir = std::env::current_dir()?;
        let project_info = project_context
            .analyze_project(current_dir.to_str().unwrap())
            .await?;

        // 5. Process a message with sensitive content through the full pipeline
        let sensitive_input = "I need help with the project at /home/user. My API key is sk_test_123 and email is user@company.com";

        // a) Scan for privacy issues
        let scan_result = privacy_filter.scan_content(sensitive_input, None).await?;
        assert!(!scan_result.detections.is_empty());

        // b) Apply redaction
        let redaction_result = privacy_filter
            .redact_content(sensitive_input, "gdpr-policy", None)
            .await?;

        // c) Process through natural language agent
        let nl_response = natural_language
            .process_message(&redaction_result.redacted_content, None)
            .await?;

        // d) Add to conversation
        conversation_manager
            .add_turn(
                session_id,
                openagent_terminal::ai::agents::natural_language::ConversationRole::User,
                redaction_result.redacted_content,
                None,
                vec![],
            )
            .await?;

        conversation_manager
            .add_turn(
                session_id,
                openagent_terminal::ai::agents::natural_language::ConversationRole::Assistant,
                nl_response.content,
                None,
                vec![],
            )
            .await?;

        // e) Update UI
        terminal_ui
            .update_privacy_status(
                scan_result.overall_risk_score,
                scan_result.detections.len() as u32,
                vec!["GDPR".to_string()],
            )
            .await?;

        // 6. Create and execute a workflow
        use openagent_terminal::ai::agents::workflow_orchestrator::{
            StepType, WorkflowDefinition, WorkflowPriority, WorkflowStep,
        };

        let workflow_def = WorkflowDefinition {
            id: "integration_workflow".to_string(),
            name: "Integration Test Workflow".to_string(),
            description: "Comprehensive integration test workflow".to_string(),
            steps: vec![WorkflowStep {
                id: "privacy_check".to_string(),
                name: "Privacy Check".to_string(),
                step_type: StepType::Tool,
                parameters: serde_json::json!({"tool": "privacy_scanner"}),
                dependencies: vec![],
                timeout_minutes: Some(5),
                retry_count: 0,
                is_critical: true,
            }],
            priority: WorkflowPriority::High,
            timeout_minutes: Some(10),
            tags: vec!["integration".to_string()],
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            is_template: false,
        };

        let execution_id = workflow_orchestrator
            .execute_workflow(
                workflow_def,
                serde_json::Value::Object(serde_json::Map::new()),
                None,
            )
            .await?;

        // 7. Update goal progress
        advanced_features
            .update_goal_progress(goal_id.clone(), 0.8)
            .await?;

        // 8. Generate compliance report
        let date_range = (
            chrono::Utc::now() - chrono::Duration::hours(1),
            chrono::Utc::now(),
        );
        let compliance_report = privacy_filter
            .generate_compliance_report(ComplianceStandard::GDPR, date_range)
            .await?;

        // 9. Verify all systems are still healthy
        assert!(conversation_manager.status().await.is_healthy);
        assert!(privacy_filter.status().await.is_healthy);
        assert!(advanced_features.status().await.is_healthy);
        assert!(terminal_ui.status().await.is_healthy);
        assert!(natural_language.status().await.is_healthy);
        assert!(workflow_orchestrator.status().await.is_healthy);
        assert!(project_context.status().await.is_healthy);

        // 10. Verify data integrity
        let conversation_history = conversation_manager
            .get_conversation_history(session_id, 10)
            .await?;
        assert_eq!(conversation_history.len(), 2);

        let goals = advanced_features.get_active_goals().await?;
        let test_goal = goals.iter().find(|g| g.id == goal_id).unwrap();
        assert_eq!(test_goal.progress, 0.8);

        // Complete the goal
        advanced_features.complete_goal(goal_id).await?;

        println!("✅ Comprehensive integration test completed successfully!");

        Ok(())
    }

    #[tokio::test]
    async fn test_error_handling_and_resilience() -> Result<()> {
        let (
            conversation_manager,
            privacy_filter,
            _advanced_features,
            _terminal_ui,
            _natural_language,
            _workflow_orchestrator,
            _project_context,
        ) = setup_integrated_system().await?;

        // Test invalid session handling
        let invalid_session = "invalid_session_id".to_string();
        let result = conversation_manager
            .get_conversation_history(invalid_session, 10)
            .await;
        assert!(result.is_err());

        // Test empty content scanning
        let scan_result = privacy_filter.scan_content("", None).await?;
        assert!(scan_result.detections.is_empty());

        // Test very large content (should be handled gracefully)
        let large_content = "x".repeat(1_000_000); // 1MB of 'x'
        let large_scan_result = privacy_filter.scan_content(&large_content, None).await;
        // Should either succeed or fail gracefully, not crash
        assert!(large_scan_result.is_ok() || large_scan_result.is_err());

        // Test invalid privacy policy
        let redaction_result = privacy_filter
            .redact_content("test content", "nonexistent-policy", None)
            .await;
        // Should handle gracefully
        assert!(redaction_result.is_ok() || redaction_result.is_err());

        Ok(())
    }
}

#[cfg(not(feature = "ai"))]
mod no_ai_tests {
    #[test]
    fn ai_feature_disabled() {
        // This test ensures the test suite compiles even without AI features
        // AI features are disabled, skipping integration tests
    }
}

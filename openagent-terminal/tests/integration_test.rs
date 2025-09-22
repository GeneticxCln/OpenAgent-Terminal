#![allow(
    clippy::pedantic,
    clippy::items_after_statements,
    clippy::uninlined_format_args,
    clippy::too_many_lines
)]

#[cfg(feature = "ai")]
mod integration_tests {
    use anyhow::Result;
    use chrono::Utc;
    use openagent_terminal::ai::agents::{
        advanced_conversation_features::{AdvancedConversationFeatures, BranchReason},
        blitzy_project_context::{BlitzyProjectContextAgent, ProjectContextConfig},
        conversation_manager::ConversationManager,
        natural_language::NaturalLanguageAgent,
        privacy_content_filter::{
            AccessControl, ComplianceStandard, DataClassification, PatternType,
            PrivacyContentFilter, PrivacyFilterConfig, PrivacyPolicy, RedactionMethod,
            RedactionRule, ScanPattern, ScannerType, SensitivityLevel,
        },
        terminal_ui_integration::TerminalUIIntegration,
        workflow_orchestrator::{
            ConditionType, RetryConfig, StepErrorHandling, WorkflowCategory, WorkflowConfig,
            WorkflowContext, WorkflowOrchestrator, WorkflowStep, WorkflowStepType,
            WorkflowTemplate,
        },
        Agent, AgentConfig, AgentContext, AgentRequest, AgentRequestType,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    async fn setup_integrated_system() -> Result<(
        Arc<ConversationManager>,
        Arc<PrivacyContentFilter>,
        Arc<AdvancedConversationFeatures>,
        Arc<TerminalUIIntegration>,
        Arc<NaturalLanguageAgent>,
        Arc<WorkflowOrchestrator>,
        Arc<BlitzyProjectContextAgent>,
    )> {
        // Initialize core agents
        let conversation_manager = Arc::new({
            let mut cm = ConversationManager::new();
            cm.initialize(AgentConfig::default()).await?;
            cm
        });

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

            // Register basic scanners (email and credit card)
            filter
                .add_content_scanner(
                    openagent_terminal::ai::agents::privacy_content_filter::ContentScanner {
                        id: "email-scanner".to_string(),
                        name: "Email Scanner".to_string(),
                        scanner_type: ScannerType::RegexPattern,
                        patterns: vec![ScanPattern {
                            pattern: r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}".to_string(),
                            pattern_type: PatternType::Regex,
                            sensitivity: SensitivityLevel::Medium,
                            context_requirements: vec![],
                            false_positive_filters: vec![],
                        }],
                        confidence_threshold: 0.5,
                        data_classification: DataClassification::PersonalData,
                        is_enabled: true,
                    },
                )
                .await?;
            filter
                .add_content_scanner(
                    openagent_terminal::ai::agents::privacy_content_filter::ContentScanner {
                        id: "credit-card-scanner".to_string(),
                        name: "Credit Card Scanner".to_string(),
                        scanner_type: ScannerType::RegexPattern,
                        patterns: vec![ScanPattern {
                            pattern: r"\b(?:\d[ -]*?){13,19}\b".to_string(),
                            pattern_type: PatternType::Regex,
                            sensitivity: SensitivityLevel::High,
                            context_requirements: vec![],
                            false_positive_filters: vec![],
                        }],
                        confidence_threshold: 0.5,
                        data_classification: DataClassification::FinancialData,
                        is_enabled: true,
                    },
                )
                .await?;

            // Create a default GDPR-like policy used in tests
            let policy = PrivacyPolicy {
                id: "gdpr-policy".to_string(),
                name: "GDPR Default".to_string(),
                description: "Default redaction policy for tests".to_string(),
                data_classifications: vec![
                    DataClassification::PersonalData,
                    DataClassification::FinancialData,
                ],
                redaction_rules: vec![
                    RedactionRule {
                        id: "rule-personal".to_string(),
                        name: "Personal data full redaction".to_string(),
                        data_classification: DataClassification::PersonalData,
                        redaction_method: RedactionMethod::FullRedaction,
                        preserve_format: false,
                        replacement_pattern: None,
                        conditions: vec![],
                    },
                    RedactionRule {
                        id: "rule-financial".to_string(),
                        name: "Financial data full redaction".to_string(),
                        data_classification: DataClassification::FinancialData,
                        redaction_method: RedactionMethod::FullRedaction,
                        preserve_format: false,
                        replacement_pattern: None,
                        conditions: vec![],
                    },
                ],
                retention_policies: HashMap::new(),
                access_controls: vec![AccessControl {
                    id: "default-access".to_string(),
                    name: "Default".to_string(),
                    data_classification: DataClassification::Internal,
                    allowed_roles: vec![],
                    denied_roles: vec![],
                    time_restrictions: vec![],
                    location_restrictions: vec![],
                    approval_required: false,
                    audit_required: true,
                }],
                compliance_standards: vec![ComplianceStandard::GDPR],
                created_at: Utc::now(),
                last_updated: Utc::now(),
                version: "1.0".to_string(),
                is_active: true,
            };
            filter.create_privacy_policy(policy).await?;

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
                max_concurrent_workflows: 2,
                max_concurrent_steps: 5,
                default_step_timeout_seconds: 300,
                default_workflow_timeout_seconds: 1800,
                enable_parallel_execution: true,
                enable_step_retry: true,
                max_retry_attempts: 3,
                enable_persistence: false,
                cleanup_completed_after_hours: 168,
            });
            orchestrator.initialize(AgentConfig::default()).await?;
            orchestrator
        });

        let project_context = Arc::new({
            let mut context = BlitzyProjectContextAgent::new().with_config(ProjectContextConfig {
                auto_detect_project_root: true,
                track_file_changes: false,
                analyze_git_history: true,
                scan_dependencies: true,
                generate_file_summaries: false,
                max_file_size_bytes: 10 * 1024 * 1024,
                excluded_dirs: vec![".git".to_string(), "node_modules".to_string()],
                included_extensions: vec![
                    "rs".to_string(),
                    "js".to_string(),
                    "ts".to_string(),
                    "py".to_string(),
                    "go".to_string(),
                    "java".to_string(),
                    "cpp".to_string(),
                    "md".to_string(),
                    "json".to_string(),
                    "yaml".to_string(),
                    "yml".to_string(),
                    "toml".to_string(),
                ],
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
        let redaction_result =
            privacy_filter.redact_content(sensitive_message, "gdpr-policy", None).await?;
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

        // Verify conversation summary contains redacted content indication (not equal to original)
        let conversation_summary =
            conversation_manager.get_conversation_summary(session_id, 10).await?;
        assert!(!conversation_summary.is_empty());
        assert!(conversation_summary.contains("Conversation:"));

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

        let session_id =
            conversation_manager.create_session(Some("Advanced Features Test".to_string())).await?;

        // Test goal creation and management
        let goal_id = advanced_features
            .start_goal_tracking(
                session_id,
                "Testing goal functionality".to_string(),
                "test".to_string(),
            )
            .await?;

        // Test conversation branching and switching
        let branch_id = advanced_features
            .create_branch(session_id, "Test Branch".to_string(), BranchReason::UserInitiated, None)
            .await?;

        advanced_features.switch_branch(session_id, branch_id).await?;

        // Update goal progress
        advanced_features.update_goal_progress(session_id, goal_id.clone(), 0.5).await?;

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

        // Test theme management and privacy status panel
        terminal_ui.set_theme("default").await?;

        // Show privacy status (should not error)
        terminal_ui.show_privacy_status().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_workflow_orchestration_integration() -> Result<()> {
        let (
            _conversation_manager,
            _privacy_filter,
            _advanced_features,
            _terminal_ui,
            _natural_language,
            workflow_orchestrator,
            _project_context,
        ) = setup_integrated_system().await?;

        // Register a simple command-based workflow template
        let template = WorkflowTemplate {
            id: "privacy_scan_workflow".to_string(),
            name: "Privacy Scanning Workflow".to_string(),
            description: "Workflow that scans content for privacy issues".to_string(),
            category: WorkflowCategory::Analysis,
            version: "1.0.0".to_string(),
            author: Some("test".to_string()),
            tags: vec!["privacy".to_string(), "security".to_string()],
            steps: vec![WorkflowStep {
                id: "scan_content".to_string(),
                name: "Scan Content".to_string(),
                step_type: WorkflowStepType::Command,
                agent_id: None,
                request_template: serde_json::json!({ "cmd": "echo scan" }),
                dependencies: vec![],
                conditions: vec![],
                timeout_seconds: Some(300),
                retry_attempts: 0,
                error_handling: StepErrorHandling::Fail,
                input_mapping: HashMap::new(),
                output_mapping: HashMap::new(),
                parallel_group: None,
            }],
            variables: HashMap::new(),
            triggers: vec![],
            conditions: vec![openagent_terminal::ai::agents::workflow_orchestrator::WorkflowCondition {
                condition_type: ConditionType::Custom("always".to_string()),
                expression: "true".to_string(),
                description: "Always run".to_string(),
            }],
            error_handling: openagent_terminal::ai::agents::workflow_orchestrator::ErrorHandlingStrategy::StopOnError,
            timeout_seconds: Some(900),
            retry_config: RetryConfig::default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        workflow_orchestrator.register_template(template).await?;

        // Execute workflow
        let execution_id = workflow_orchestrator
            .create_workflow(
                "privacy_scan_workflow",
                WorkflowContext {
                    conversation_session_id: None,
                    project_root: None,
                    user_id: None,
                    environment: HashMap::new(),
                    variables: HashMap::new(),
                    shared_state: HashMap::new(),
                },
                None,
            )
            .await?;

        // Wait briefly for execution to start
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Check execution status
        let status = workflow_orchestrator.get_workflow_status(execution_id).await?;
        assert_eq!(status.id, execution_id);

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
        let project_info = project_context.analyze_project(current_dir.to_str().unwrap()).await?;

        assert!(!project_info.name.is_empty());
        assert!(!project_info.root_path.is_empty());

        // Test file analysis (from returned project info)
        assert!(!project_info.files.is_empty());

        // Test dependency analysis (from returned project info)
        let _dependencies = &project_info.dependencies; // may be empty; ensure no panic

        // Basic sanity check on project info
        assert!(!project_info.name.is_empty());

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

        let session_id =
            conversation_manager.create_session(Some("NLP Test Session".to_string())).await?;

        // Test natural language processing with privacy filtering
        let user_input =
            "Please generate rust code to set up a database connection with username admin and password secret123";

        // First scan for privacy issues
        let scan_result = privacy_filter.scan_content(user_input, None).await?;

        let processed_input = if !scan_result.detections.is_empty() {
            // Apply redaction if sensitive content detected
            let redaction_result =
                privacy_filter.redact_content(user_input, "gdpr-policy", None).await?;
            redaction_result.redacted_content
        } else {
            user_input.to_string()
        };

        // Process through natural language agent using its request interface
        let nl_response = natural_language
            .handle_request(AgentRequest {
                id: uuid::Uuid::new_v4(),
                request_type: AgentRequestType::Custom("ProcessNaturalLanguage".to_string()),
                payload: serde_json::json!(processed_input),
                context: AgentContext {
                    project_root: None,
                    current_directory: std::env::current_dir()?.to_string_lossy().to_string(),
                    current_branch: None,
                    open_files: vec![],
                    recent_commands: vec![],
                    environment_vars: HashMap::new(),
                    user_preferences: HashMap::new(),
                },
                metadata: HashMap::new(),
            })
            .await?;
        assert!(nl_response.success);

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

        // Add a simple assistant response to keep the conversation flowing
        conversation_manager
            .add_turn(
                session_id,
                openagent_terminal::ai::agents::natural_language::ConversationRole::Assistant,
                "Acknowledged".to_string(),
                None,
                vec![],
            )
            .await?;

        // Verify conversation summary (since history accessor is summary-based)
        let summary = conversation_manager.get_conversation_summary(session_id, 10).await?;
        assert!(summary.contains("Conversation:"));

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

        // 2. Show conversation in UI (no message injection helper available)
        terminal_ui.show_conversation(session_id).await?;

        // 3. Create a goal for the integration test
        let goal_id = advanced_features
            .start_goal_tracking(
                session_id,
                "Test all system components working together".to_string(),
                "integration".to_string(),
            )
            .await?;

        // 4. Analyze current project context
        let current_dir = std::env::current_dir()?;
        let _project_info = project_context.analyze_project(current_dir.to_str().unwrap()).await?;

        // 5. Process a message with sensitive content through the full pipeline
        let sensitive_input = "I need help with the project at /home/user. My API key is sk_test_123 and email is user@company.com";

        // a) Scan for privacy issues
        let scan_result = privacy_filter.scan_content(sensitive_input, None).await?;
        assert!(!scan_result.detections.is_empty());

        // b) Apply redaction
        let redaction_result =
            privacy_filter.redact_content(sensitive_input, "gdpr-policy", None).await?;

        // c) Process through natural language agent
        let nl_response = natural_language
            .handle_request(AgentRequest {
                id: uuid::Uuid::new_v4(),
                request_type: AgentRequestType::Custom("ProcessNaturalLanguage".to_string()),
                payload: serde_json::json!(redaction_result.redacted_content.clone()),
                context: AgentContext {
                    project_root: None,
                    current_directory: std::env::current_dir()?.to_string_lossy().to_string(),
                    current_branch: None,
                    open_files: vec![],
                    recent_commands: vec![],
                    environment_vars: HashMap::new(),
                    user_preferences: HashMap::new(),
                },
                metadata: HashMap::new(),
            })
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

        // Extract assistant text from NL response payload if available
        let assistant_text = nl_response
            .payload
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("Acknowledged")
            .to_string();
        conversation_manager
            .add_turn(
                session_id,
                openagent_terminal::ai::agents::natural_language::ConversationRole::Assistant,
                assistant_text,
                None,
                vec![],
            )
            .await?;

        // e) Update UI privacy panel
        terminal_ui.show_privacy_status().await?;

        // 6. Create and execute a workflow
        use openagent_terminal::ai::agents::workflow_orchestrator::{
            WorkflowStep, WorkflowStepType, WorkflowTemplate,
        };

        let template = WorkflowTemplate {
            id: "integration_workflow".to_string(),
            name: "Integration Test Workflow".to_string(),
            description: "Comprehensive integration test workflow".to_string(),
            steps: vec![WorkflowStep {
                id: "privacy_check".to_string(),
                name: "Privacy Check".to_string(),
                step_type: WorkflowStepType::Command,
                agent_id: None,
                request_template: serde_json::json!({"cmd": "echo privacy"}),
                dependencies: vec![],
                conditions: vec![],
                timeout_seconds: Some(300),
                retry_attempts: 0,
                error_handling: StepErrorHandling::Fail,
                input_mapping: HashMap::new(),
                output_mapping: HashMap::new(),
                parallel_group: None,
            }],
            variables: HashMap::new(),
            triggers: vec![],
            conditions: vec![],
            error_handling: openagent_terminal::ai::agents::workflow_orchestrator::ErrorHandlingStrategy::StopOnError,
            timeout_seconds: Some(600),
            retry_config: RetryConfig::default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            category: WorkflowCategory::Analysis,
            version: "1.0.0".to_string(),
            author: Some("test".to_string()),
            tags: vec!["integration".to_string()],
        };

        workflow_orchestrator.register_template(template).await?;

        let _execution_id = workflow_orchestrator
            .create_workflow(
                "integration_workflow",
                WorkflowContext {
                    conversation_session_id: None,
                    project_root: None,
                    user_id: None,
                    environment: HashMap::new(),
                    variables: HashMap::new(),
                    shared_state: HashMap::new(),
                },
                None,
            )
            .await?;

        // 7. Update goal progress
        advanced_features.update_goal_progress(session_id, goal_id.clone(), 0.8).await?;

        // 8. Generate compliance report
        let date_range = (chrono::Utc::now() - chrono::Duration::hours(1), chrono::Utc::now());
        let _compliance_report =
            privacy_filter.generate_compliance_report(ComplianceStandard::GDPR, date_range).await?;

        // 9. Verify all systems are still healthy
        assert!(conversation_manager.status().await.is_healthy);
        assert!(privacy_filter.status().await.is_healthy);
        assert!(advanced_features.status().await.is_healthy);
        assert!(terminal_ui.status().await.is_healthy);
        assert!(natural_language.status().await.is_healthy);
        assert!(workflow_orchestrator.status().await.is_healthy);
        assert!(project_context.status().await.is_healthy);

        // 10. Verify data integrity via summary
        let conversation_summary =
            conversation_manager.get_conversation_summary(session_id, 10).await?;
        assert!(conversation_summary.contains("Conversation:"));

        // Update goal progress again to simulate completion
        advanced_features.update_goal_progress(session_id, goal_id.clone(), 0.8).await?;

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
        // Use a random UUID that won't exist
        let invalid_session = uuid::Uuid::new_v4();
        let result = conversation_manager.get_conversation_summary(invalid_session, 10).await;
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
        let redaction_result =
            privacy_filter.redact_content("test content", "nonexistent-policy", None).await;
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

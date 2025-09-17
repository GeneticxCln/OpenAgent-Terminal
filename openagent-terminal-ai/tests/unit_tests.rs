//! Unit tests for core AI agent functionality
//! Focuses on individual components that can be tested in isolation

use tempfile;

// Test only if the agents feature is enabled
#[cfg(feature = "agents")]
mod agent_tests {
    use super::*;
    use openagent_terminal_ai::agents::PrivacyLevel;

    /// Test basic type serialization/deserialization
    #[test]
    fn test_basic_types_serialization() {
        use openagent_terminal_ai::agents::{AgentCapabilities, PrivacyLevel};
        use serde_json;

        // Test AgentCapabilities
        let capabilities = AgentCapabilities {
            supported_languages: vec!["rust".to_string(), "python".to_string()],
            supported_frameworks: vec!["tokio".to_string()],
            features: vec!["code_generation".to_string()],
            requires_internet: false,
            privacy_level: PrivacyLevel::Local,
        };

        let json = serde_json::to_string(&capabilities).unwrap();
        let deserialized: AgentCapabilities = serde_json::from_str(&json).unwrap();

        assert_eq!(
            capabilities.supported_languages,
            deserialized.supported_languages
        );
        // PrivacyLevel doesn't implement PartialEq; assert exact variant
        match deserialized.privacy_level {
            PrivacyLevel::Local => {}
            _ => panic!("Expected PrivacyLevel::Local after deserialization"),
        }
    }

    #[test]
    fn test_workflow_node_creation() {
        use openagent_terminal_ai::agents::types::*;

        let node = WorkflowNode {
            id: "test_node".to_string(),
            name: "Test Node".to_string(),
            node_type: NodeType::Start,
            agent_id: None,
            dependencies: vec![],
            status: NodeStatus::Pending,
            input_schema: None,
            output_schema: None,
            timeout_ms: Some(5000),
            retry_count: 0,
            max_retries: 3,
            parallel_group: None,
        };

        assert_eq!(node.id, "test_node");
        assert_eq!(node.name, "Test Node");
        assert!(matches!(node.node_type, NodeType::Start));
        assert!(matches!(node.status, NodeStatus::Pending));
        assert_eq!(node.timeout_ms, Some(5000));
        assert_eq!(node.max_retries, 3);
    }

    #[test]
    fn test_agent_message_creation() {
        use chrono::Utc;
        use openagent_terminal_ai::agents::types::*;
        use serde_json;
        use uuid::Uuid;

        let message = AgentMessage {
            id: Uuid::new_v4(),
            from_agent: "agent1".to_string(),
            to_agent: "agent2".to_string(),
            message_type: MessageType::Request,
            payload: serde_json::json!({"test": "payload"}),
            correlation_id: None,
            timestamp: Utc::now(),
            priority: MessagePriority::Normal,
            ttl_seconds: Some(30),
        };

        assert_eq!(message.from_agent, "agent1");
        assert_eq!(message.to_agent, "agent2");
        assert!(matches!(message.message_type, MessageType::Request));
        assert!(matches!(message.priority, MessagePriority::Normal));
        assert_eq!(message.ttl_seconds, Some(30));
    }

    #[test]
    fn test_quality_issue_creation() {
        use openagent_terminal_ai::agents::QualityIssue;
        use openagent_terminal_ai::agents::Severity as AgentSeverity;
        
        let issue = QualityIssue {
            severity: AgentSeverity::Warning,
            category: "Security".to_string(),
            message: "Test security issue".to_string(),
            line: Some(42),
            column: Some(10),
            rule: "test_rule".to_string(),
        };

        assert!(matches!(issue.severity, AgentSeverity::Warning));
        assert_eq!(issue.category, "Security");
        assert_eq!(issue.message, "Test security issue");
        assert_eq!(issue.line, Some(42));
        assert_eq!(issue.column, Some(10));
        assert_eq!(issue.rule, "test_rule");
    }

    #[test]
    fn test_project_context_config() {
        use openagent_terminal_ai::agents::project_context::*;

        let config = ContextConfig {
            default_cache_ttl: 300,
            enable_git_analysis: true,
            analyze_dependencies: true,
            detect_frameworks: true,
            max_analysis_depth: 3,
            ignore_patterns: vec![".git".into()],
        };

        assert_eq!(config.default_cache_ttl, 300);
        assert!(config.enable_git_analysis);
        assert!(config.analyze_dependencies);
        assert!(config.detect_frameworks);
    }

    #[test]
    fn test_quality_config() {

        let config = openagent_terminal_ai::agents::types::QualityConfig::default();

        // Test that default config is reasonable
        assert!(config.performance_thresholds.max_function_length > 0);
        assert!(config.style_rules.max_line_length > 0);
        assert!(!config.enabled_checks.is_empty());
    }

    #[test]
    fn test_workflow_config() {
        use openagent_terminal_ai::agents::workflow_orchestration::*;

        let config = OrchestratorConfig {
            max_concurrent_workflows: 5,
            max_concurrent_nodes_per_workflow: 3,
            default_node_timeout_ms: 30000,
            max_retries: 2,
            enable_cycle_detection: true,
            enable_metrics: true,
        };

        assert_eq!(config.max_concurrent_workflows, 5);
        assert_eq!(config.max_concurrent_nodes_per_workflow, 3);
        assert_eq!(config.default_node_timeout_ms, 30000);
        assert_eq!(config.max_retries, 2);
        assert!(config.enable_cycle_detection);
        assert!(config.enable_metrics);
    }

    #[tokio::test]
    async fn test_project_context_basic_detection() {
        use openagent_terminal_ai::agents::project_context::*;
        use std::env;

        let config = ContextConfig::default();
        let agent = ProjectContextAgent::new(config);

        // Test with current directory - this should always work
        let current_dir = env::current_dir().unwrap();
        let result = agent.get_project_context(&current_dir).await;

        // This should succeed for any directory
        assert!(result.is_ok());

        let context = result.unwrap();

        // Basic checks that should always pass
        assert!(!context.working_directory.is_empty());
        assert_ne!(context.shell_kind, openagent_terminal_ai::agents::types::ShellKind::Unknown("".to_string()));
    }

    #[tokio::test]
    async fn test_quality_validation_basic() {
        use openagent_terminal_ai::agents::quality::*;
        use std::fs;
        use tempfile::NamedTempFile;

        use openagent_terminal_ai::agents::types::QualityConfig;
        let config = QualityConfig::default();
        let agent = QualityValidationAgent::new(config);

        // Create a temporary file with some basic code
        let temp_file = NamedTempFile::new().unwrap();
        let test_code = r#"
fn main() {
    println!("Hello, world!");
}
"#;

        fs::write(temp_file.path(), test_code).unwrap();

        let result = agent.analyze_file(temp_file.path()).await;

        // Should succeed for basic analysis
        assert!(result.is_ok());

        let analysis = result.unwrap();

        // Basic checks
        assert!(analysis.overall_score >= 0.0);
        assert!(analysis.overall_score <= 100.0);
        assert!(analysis.metrics.lines_of_code > 0);
    }

    #[test]
    fn test_workflow_execution_graph_creation() {
        use chrono::Utc;
        use openagent_terminal_ai::agents::types::*;
        use std::collections::HashMap;
        use uuid::Uuid;

        let mut nodes = HashMap::new();
        nodes.insert(
            "start".to_string(),
            WorkflowNode {
                id: "start".to_string(),
                name: "Start Node".to_string(),
                node_type: NodeType::Start,
                agent_id: None,
                dependencies: vec![],
                status: NodeStatus::Pending,
                input_schema: None,
                output_schema: None,
                timeout_ms: Some(1000),
                retry_count: 0,
                max_retries: 2,
                parallel_group: None,
            },
        );

        let workflow = WorkflowExecutionGraph {
            id: Uuid::new_v4(),
            name: "Test Workflow".to_string(),
            nodes,
            edges: vec![],
            execution_strategy: ExecutionStrategy::Sequential,
            status: WorkflowStatus::Pending,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        };

        assert_eq!(workflow.name, "Test Workflow");
        assert!(matches!(
            workflow.execution_strategy,
            ExecutionStrategy::Sequential
        ));
        assert!(matches!(workflow.status, WorkflowStatus::Pending));
        assert_eq!(workflow.nodes.len(), 1);
        assert!(workflow.nodes.contains_key("start"));
    }

    #[test]
    fn test_hub_config_validation() {
        use openagent_terminal_ai::agents::communication_hub::*;

        let config = HubConfig {
            max_concurrent_tasks: 10,
            max_message_retries: 3,
            message_timeout_seconds: 30,
            task_timeout_seconds: 300,
            enable_load_balancing: true,
            enable_message_history: true,
            max_history_size: 1000,
            heartbeat_interval_seconds: 30,
        };

        assert_eq!(config.max_concurrent_tasks, 10);
        assert_eq!(config.task_timeout_seconds, 300);
        assert!(config.enable_load_balancing);
        assert!(config.enable_message_history);
        assert_eq!(config.max_history_size, 1000);
        assert_eq!(config.max_message_retries, 3);
        assert_eq!(config.heartbeat_interval_seconds, 30);
    }

    #[test]
    fn test_enum_variants() {
        use openagent_terminal_ai::agents::types::*;

        // Test NodeStatus variants
        let statuses = vec![
            NodeStatus::Pending,
            NodeStatus::Ready,
            NodeStatus::Running,
            NodeStatus::Completed,
            NodeStatus::Failed,
            NodeStatus::Skipped,
            NodeStatus::Retrying,
        ];

        assert_eq!(statuses.len(), 7);

        // Test WorkflowStatus variants
        let workflow_statuses = vec![
            WorkflowStatus::Pending,
            WorkflowStatus::Running,
            WorkflowStatus::Completed,
            WorkflowStatus::Failed,
            WorkflowStatus::Cancelled,
            WorkflowStatus::Paused,
        ];

        assert_eq!(workflow_statuses.len(), 6);

        // Test PrivacyLevel variants
        let privacy_levels = vec![
            PrivacyLevel::Local,
            PrivacyLevel::CloudSafe,
            PrivacyLevel::CloudFull,
        ];

        assert_eq!(privacy_levels.len(), 3);
    }

    #[test]
    fn test_concurrency_state() {
        use openagent_terminal_ai::agents::types::*;
        use std::sync::Arc;
        use tokio::sync::{RwLock, Mutex};
        
        let concurrency_state = ConcurrencyState {
            active_operations: Arc::new(RwLock::new(std::collections::HashMap::new())),
            operation_locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            resource_usage: Arc::new(RwLock::new(ResourceUsage::default())),
            max_concurrent_ops: 5,
        };
        
        assert_eq!(concurrency_state.max_concurrent_ops, 5);
    }

    #[test]
    fn test_security_issue_creation() {

        use openagent_terminal_ai::agents::Severity as AgentSeverity;
        let security_issue = openagent_terminal_ai::agents::SecurityIssue {
            vulnerability_type: "SQL Injection".to_string(),
            severity: AgentSeverity::Critical,
            description: "Potential SQL injection vulnerability".to_string(),
            cwe_id: Some("CWE-89".to_string()),
            line: Some(150),
            fix_suggestion: Some("Use parameterized queries".to_string()),
        };

        assert_eq!(security_issue.vulnerability_type, "SQL Injection");
        assert!(matches!(security_issue.severity, AgentSeverity::Critical));
        assert_eq!(security_issue.line, Some(150));
        assert_eq!(security_issue.cwe_id, Some("CWE-89".to_string()));
        assert!(security_issue.fix_suggestion.is_some());
    }
}

// Tests that don't require the agents feature
#[test]
fn test_basic_functionality() {
    // Test basic library functionality that's always available
    assert_eq!(2 + 2, 4);
}

#[test]
fn test_std_collections() {
    use std::collections::HashMap;

    let mut map = HashMap::new();
    map.insert("key1", "value1");
    map.insert("key2", "value2");

    assert_eq!(map.get("key1"), Some(&"value1"));
    assert_eq!(map.get("key2"), Some(&"value2"));
    assert_eq!(map.get("key3"), None);
    assert_eq!(map.len(), 2);
}

#[test]
fn test_path_operations() {
    use std::path::PathBuf;

    let path = PathBuf::from("/tmp/test");
    let extended = path.join("subdir").join("file.txt");

    assert_eq!(extended.to_string_lossy(), "/tmp/test/subdir/file.txt");

    let parent = extended.parent().unwrap();
    assert_eq!(parent.to_string_lossy(), "/tmp/test/subdir");

    let filename = extended.file_name().unwrap();
    assert_eq!(filename.to_string_lossy(), "file.txt");
}

#[test]
fn test_uuid_generation() {
    use uuid::Uuid;

    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    // UUIDs should be unique
    assert_ne!(id1, id2);

    // Should be valid UUID format
    let id_string = id1.to_string();
    assert_eq!(id_string.len(), 36);
    assert_eq!(id_string.matches('-').count(), 4);
}

#[test]
fn test_json_serialization() {
    use serde_json;
    use std::collections::HashMap;

    let mut data = HashMap::new();
    data.insert("name", "test");
    data.insert("value", "42");

    let json = serde_json::to_string(&data).unwrap();
    let deserialized: HashMap<&str, &str> = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.get("name"), Some(&"test"));
    assert_eq!(deserialized.get("value"), Some(&"42"));
}

#[tokio::test]
async fn test_basic_async() {
    use tokio::time::{sleep, Duration};

    let start = std::time::Instant::now();
    sleep(Duration::from_millis(10)).await;
    let elapsed = start.elapsed();

    // Should have taken at least 10ms
    assert!(elapsed >= Duration::from_millis(9));
    // But not more than 100ms (allowing for system variance)
    assert!(elapsed < Duration::from_millis(100));
}

#[test]
fn test_regex_basic() {
    use regex::Regex;

    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();

    assert!(re.is_match("2023-12-25"));
    assert!(!re.is_match("invalid-date"));
    // Note: This regex only checks format, not valid dates
    assert!(re.is_match("2023-13-45")); // This matches the pattern but is invalid date
    assert!(!re.is_match("23-12-25")); // Wrong year format
}

#[test]
fn test_anyhow_error_handling() {
    use anyhow::{anyhow, Result};

    fn might_fail(should_fail: bool) -> Result<String> {
        if should_fail {
            Err(anyhow!("Something went wrong"))
        } else {
            Ok("Success".to_string())
        }
    }

    let success = might_fail(false);
    assert!(success.is_ok());
    assert_eq!(success.unwrap(), "Success");

    let failure = might_fail(true);
    assert!(failure.is_err());
    assert!(failure
        .unwrap_err()
        .to_string()
        .contains("Something went wrong"));
}

#[test]
fn test_temp_directory() {
    use std::fs;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");

    fs::write(&file_path, "test content").unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "test content");

    assert!(file_path.exists());

    // Directory and file will be cleaned up automatically when `dir` is dropped
}

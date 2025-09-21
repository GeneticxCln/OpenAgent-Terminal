#![allow(clippy::pedantic, clippy::cast_precision_loss, clippy::uninlined_format_args, clippy::similar_names, clippy::default_trait_access)]

//! Integration tests for AI, Blocks, and Security features
//! Tests that all three features work together without lazy fallbacks

// Test AI Command Assistance
#[cfg(feature = "ai")]
mod ai_tests {
    use openagent_terminal::ai_runtime::AiRuntime;
    use openagent_terminal_ai::{AiProposal, AiProvider, AiRequest};

    pub struct MockProvider;

    impl AiProvider for MockProvider {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn propose(&self, _request: AiRequest) -> Result<Vec<AiProposal>, String> {
            Ok(vec![AiProposal {
                title: "Test Command".to_string(),
                description: Some("Test command description".to_string()),
                proposed_commands: vec!["echo test".to_string()],
            }])
        }

        fn propose_stream(
            &self,
            _request: AiRequest,
            _on_chunk: &mut dyn FnMut(&str),
            _cancel_flag: &std::sync::atomic::AtomicBool,
        ) -> Result<bool, String> {
            Ok(false) // Don't support streaming in mock
        }
    }

    #[test]
    fn test_ai_runtime_basic() {
        let provider = Box::new(MockProvider);
        let mut runtime = AiRuntime::new(provider);

        // Test initial state
        assert!(!runtime.ui.active);
        assert!(runtime.ui.scratch.is_empty());
        assert!(runtime.ui.proposals.is_empty());

        // Test scratch modification
        runtime.ui.scratch = "help me with git".to_string();
        runtime.ui.cursor_position = runtime.ui.scratch.len();

        assert_eq!(runtime.ui.scratch, "help me with git");
        assert_eq!(runtime.ui.cursor_position, "help me with git".len());
    }

    #[test]
    fn test_ai_provider_creation() {
        // Test creating different providers
        let null_runtime = AiRuntime::from_config(Some("null"), None, None, None);
        assert_eq!(null_runtime.provider.name(), "null");

        // Test with invalid provider
        let invalid_runtime = AiRuntime::from_config(Some("invalid"), None, None, None);
        assert!(invalid_runtime.ui.error_message.is_some());
    }

    #[tokio::test]
    async fn test_ai_propose_with_context() {
        let provider = Box::new(MockProvider);
        let mut runtime = AiRuntime::new(provider);

        runtime.ui.scratch = "list files".to_string();

        // Use mock context (would normally come from PTY)
        runtime.propose_with_context(None);

        // Since propose_with_context is synchronous in this path, loading should be false
        assert!(!runtime.ui.is_loading);
        assert_eq!(runtime.ui.proposals.len(), 1);
        assert_eq!(runtime.ui.proposals[0].title, "Test Command");
    }
}

// Test Command Blocks/History
#[cfg(feature = "blocks")]
mod blocks_tests {
    use openagent_terminal::command_history::CommandHistory;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_command_history_integration() {
        let temp_dir = TempDir::new().unwrap();
        let mut history = CommandHistory::new(Some(temp_dir.path().to_path_buf())).await;

        // Test command lifecycle
        history.start_command("ls -la".to_string(), None).await.unwrap();
        assert!(history.get_current_command().is_some());
        assert_eq!(history.get_current_command().unwrap().command, "ls -la");

        history.complete_command(0, "total 10\nfile1.txt\nfile2.txt\n".to_string()).await.unwrap();
        assert!(history.get_current_command().is_none());

        // Test search functionality
        let results = history.search("ls", 10).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, "ls -la");
        assert_eq!(results[0].exit_code, Some(0));

        // Test recent history
        let recent = history.get_recent(5).await;
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].command, "ls -la");
    }

    #[tokio::test]
    async fn test_command_history_multiple_commands() {
        let temp_dir = TempDir::new().unwrap();
        let mut history = CommandHistory::new(Some(temp_dir.path().to_path_buf())).await;

        // Add multiple commands
        let commands = vec!["pwd", "ls", "cat file.txt", "grep test *.txt"];
        for cmd in &commands {
            history.start_command(cmd.to_string(), None).await.unwrap();
            history.complete_command(0, format!("output for {}", cmd)).await.unwrap();
        }

        // Test search finds multiple results
        let results = history.search("txt", 10).await;
        assert_eq!(results.len(), 2); // "cat file.txt" and "grep test *.txt"

        // Test recent returns in reverse order
        let recent = history.get_recent(2).await;
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].command, "grep test *.txt"); // Most recent first
        assert_eq!(recent[1].command, "cat file.txt");
    }

    #[tokio::test]
    async fn test_command_history_cancel() {
        let temp_dir = TempDir::new().unwrap();
        let mut history = CommandHistory::new(Some(temp_dir.path().to_path_buf())).await;

        // Start a command and cancel it
        history.start_command("long-running-command".to_string(), None).await.unwrap();
        assert!(history.get_current_command().is_some());

        history.cancel_current_command();
        assert!(history.get_current_command().is_none());

        // Should not appear in history since it was cancelled
        let results = history.search("long-running", 10).await;
        assert_eq!(results.len(), 0);
    }
}

// Test blocks without the blocks feature (fallback)
#[cfg(not(feature = "blocks"))]
mod blocks_fallback_tests {
    use super::*;
    use openagent_terminal::command_history::CommandHistory;

    #[tokio::test]
    async fn test_command_history_fallback() {
        // Should work without blocks feature using simple history
        let mut history = CommandHistory::new(None).await;

        history.start_command("echo test".to_string(), None).await.unwrap();
        history.complete_command(0, "test\n".to_string()).await.unwrap();

        let results = history.search("echo", 10).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, "echo test");
    }
}

// Test Security Lens
#[cfg(feature = "security-lens")]
mod security_tests {
    use openagent_terminal::security_config::{
        CustomSecurityPattern, SecurityConfig, SecurityLensFactory,
    };

    #[test]
    fn test_security_config_basic() {
        let config = SecurityConfig::default();
        assert!(config.enabled);
        assert!(!config.block_critical);
        assert!(config.gate_paste_events);

        // Test validation
        assert!(config.validate().is_ok());

        // Test summary
        let summary = config.get_security_summary();
        assert!(summary.contains("Security"));
    }

    #[test]
    fn test_security_lens_dangerous_commands() {
        let config = SecurityConfig::default();

        // Test critical command
        let result = SecurityLensFactory::test_command(&config, "rm -rf /").unwrap();
        assert_eq!(result.risk_level, "Critical");
        assert!(result.requires_confirmation);
        assert!(!result.mitigations.is_empty());
        assert!(!result.would_block); // Default config doesn't block

        // Test with conservative config
        let conservative_config = SecurityConfig::preset_conservative();
        let result = SecurityLensFactory::test_command(&conservative_config, "rm -rf /").unwrap();
        assert!(result.would_block); // Conservative config blocks critical

        // Test warning level command
        let result = SecurityLensFactory::test_command(&config, "chmod 777 file.txt").unwrap();
        assert_eq!(result.risk_level, "Warning");
        assert!(result.requires_confirmation);
    }

    #[test]
    fn test_security_lens_safe_commands() {
        let config = SecurityConfig::default();

        // Test safe commands
        let safe_commands = vec!["ls", "pwd", "echo hello", "cat file.txt"];
        for cmd in safe_commands {
            let result = SecurityLensFactory::test_command(&config, cmd).unwrap();
            assert_eq!(result.risk_level, "Safe");
            assert!(!result.requires_confirmation);
            assert!(!result.would_block);
        }
    }

    #[test]
    fn test_custom_security_patterns() {
        let mut config = SecurityConfig::default();

        // Add custom pattern
        let custom_pattern = CustomSecurityPattern {
            pattern: r"(?i)deploy\s+.*prod".to_string(),
            risk_level: "Critical".to_string(),
            message: "Production deployment detected".to_string(),
        };

        config.add_custom_pattern(custom_pattern).unwrap();

        // Test the custom pattern triggers
        let result = SecurityLensFactory::test_command(&config, "deploy myapp prod").unwrap();
        assert_eq!(result.risk_level, "Critical");
        assert!(result.explanation.contains("Production deployment detected"));
    }

    #[test]
    fn test_security_preset_configs() {
        // Conservative preset
        let conservative = SecurityConfig::preset_conservative();
        assert!(conservative.block_critical);
        let result = SecurityLensFactory::test_command(&conservative, "rm -rf /").unwrap();
        assert!(result.would_block);

        // Permissive preset
        let permissive = SecurityConfig::preset_permissive();
        assert!(!permissive.block_critical);
        assert!(!permissive.require_confirmation.get("Caution").unwrap_or(&true));

        // Disabled preset
        let disabled = SecurityConfig::preset_disabled();
        assert!(!disabled.enabled);
        let result = SecurityLensFactory::test_command(&disabled, "rm -rf /").unwrap();
        assert_eq!(result.risk_level, "Safe"); // Everything is safe when disabled
    }
}

// Test security without the security-lens feature (stub)
#[cfg(not(feature = "security-lens"))]
mod security_stub_tests {
    use openagent_terminal::security_config::{SecurityConfig, SecurityLensFactory};

    #[test]
    fn test_security_stub() {
        let config = SecurityConfig::default();

        // With stub, all commands should be safe
        let result = SecurityLensFactory::test_command(&config, "rm -rf /").unwrap();
        assert_eq!(result.risk_level, "Safe");
        assert!(!result.requires_confirmation);
        assert!(!result.would_block);
    }
}

// Integration tests combining all features
#[cfg(all(feature = "ai", feature = "blocks", feature = "security-lens"))]
mod integration_tests {
    use super::*;
    use openagent_terminal::ai_runtime::AiRuntime;
    use openagent_terminal::command_history::CommandHistory;
    use openagent_terminal::security_config::{SecurityConfig, SecurityLensFactory};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_ai_with_security_integration() {
        // Test AI generating a potentially dangerous command and security analysis
        let provider = Box::new(ai_tests::MockProvider);
        let mut ai_runtime = AiRuntime::new(provider);

        // Simulate AI proposing a dangerous command
        ai_runtime.ui.proposals = vec![openagent_terminal_ai::AiProposal {
            title: "Delete files".to_string(),
            description: Some("Delete all files recursively".to_string()),
            proposed_commands: vec!["rm -rf /tmp/*".to_string()],
        }];

        // Test security analysis of AI proposal
        let config = SecurityConfig::default();
        if let Some(command) = ai_runtime.get_selected_commands() {
            let security_result = SecurityLensFactory::test_command(&config, &command).unwrap();

            // Should be detected as risky
            assert_ne!(security_result.risk_level, "Safe");
            assert!(security_result.requires_confirmation);
        }
    }

    #[tokio::test]
    async fn test_full_command_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let mut history = CommandHistory::new(Some(temp_dir.path().to_path_buf())).await;
        let config = SecurityConfig::default();

        // Simulate a complete command lifecycle with all three features
        let command = "ls -la";

        // 1. Security check before execution
        let security_result = SecurityLensFactory::test_command(&config, command).unwrap();
        assert_eq!(security_result.risk_level, "Safe"); // ls should be safe
        assert!(!security_result.requires_confirmation);

        // 2. Start tracking in command history
        history.start_command(command.to_string(), None).await.unwrap();
        assert!(history.get_current_command().is_some());

        // 3. Complete the command
        let output = "total 10\nfile1.txt\nfile2.txt\n";
        history.complete_command(0, output.to_string()).await.unwrap();

        // 4. Verify it's in history
        let results = history.search("ls", 10).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, command);
        assert_eq!(results[0].output, output);
        assert_eq!(results[0].exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_dangerous_command_flow() {
        let temp_dir = TempDir::new().unwrap();
        let mut history = CommandHistory::new(Some(temp_dir.path().to_path_buf())).await;
        let config = SecurityConfig::preset_conservative(); // Use conservative config

        let dangerous_command = "rm -rf /tmp/important_data";

        // 1. Security check should flag as risky
        let security_result =
            SecurityLensFactory::test_command(&config, dangerous_command).unwrap();
        assert_ne!(security_result.risk_level, "Safe");
        assert!(security_result.requires_confirmation);

        // 2. If user confirms and runs the command, it still gets tracked
        history.start_command(dangerous_command.to_string(), None).await.unwrap();
        history.complete_command(0, "removed files".to_string()).await.unwrap();

        // 3. Command appears in history for audit purposes
        let results = history.search("rm", 10).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, dangerous_command);
    }
}

// Performance tests
#[cfg(all(feature = "ai", feature = "blocks", feature = "security-lens"))]
mod performance_tests {
    use openagent_terminal::command_history::CommandHistory;
    use openagent_terminal::security_config::{SecurityConfig, SecurityLensFactory};
    use std::time::Instant;
    use tempfile::TempDir;

    #[test]
    fn test_security_analysis_performance() {
        let config = SecurityConfig::default();

        // Test that security analysis is fast
        let commands = vec![
            "ls -la",
            "rm -rf /tmp/test",
            "sudo apt install package",
            "docker run --privileged image",
            "kubectl delete pod production-pod",
        ];

        let start = Instant::now();
        for cmd in &commands {
            let _ = SecurityLensFactory::test_command(&config, cmd).unwrap();
        }
        let duration = start.elapsed();

        // Should analyze 5 commands in well under 1 second
        assert!(duration.as_millis() < 1000, "Security analysis took {}ms", duration.as_millis());

        // Per-command should be very fast
        let per_command = duration.as_millis() / commands.len() as u128;
        assert!(per_command < 200, "Per-command analysis took {}ms", per_command);
    }

    #[tokio::test]
    async fn test_blocks_storage_performance() {
        let temp_dir = TempDir::new().unwrap();
        let mut history = CommandHistory::new(Some(temp_dir.path().to_path_buf())).await;

        // Test that storing many commands is reasonably fast
        let start = Instant::now();

        for i in 0..100 {
            let cmd = format!("echo command_{}", i);
            history.start_command(cmd, None).await.unwrap();
            history.complete_command(0, format!("output_{}", i)).await.unwrap();
        }

        let duration = start.elapsed();

        // Should handle 100 commands in under 10 seconds
        assert!(duration.as_secs() < 10, "Storing 100 commands took {}s", duration.as_secs());

        // Search should also be fast
        let search_start = Instant::now();
        let results = history.search("command", 50).await;
        let search_duration = search_start.elapsed();

        assert_eq!(results.len(), 50); // Limited by max_results
        assert!(
            search_duration.as_millis() < 1000,
            "Search took {}ms",
            search_duration.as_millis()
        );
    }
}

// Feature flag compatibility tests
mod feature_compatibility_tests {

    #[test]
    fn test_workspace_enabled_flag_basic() {
        let mut cfg = openagent_terminal::config::UiConfig::default();
        let size_info =
            openagent_terminal::display::SizeInfo::new(640.0, 480.0, 10.0, 20.0, 0.0, 0.0, false);

        // Enabled by default in defaults
        let wm_default = openagent_terminal::workspace::WorkspaceManager::new(
            openagent_terminal::workspace::WorkspaceId(0),
            std::rc::Rc::new(cfg.clone()),
            size_info,
        );
        assert!(wm_default.is_enabled());

        // Explicitly disable
        cfg.workspace.enabled = false;
        let wm_disabled = openagent_terminal::workspace::WorkspaceManager::new(
            openagent_terminal::workspace::WorkspaceId(1),
            std::rc::Rc::new(cfg),
            size_info,
        );
        assert!(!wm_disabled.is_enabled());
    }

    #[test]
    fn test_compile_with_different_feature_combinations() {
        // This test exists to ensure different feature combinations compile
        // The actual feature combinations are tested by the CI system

        #[cfg(feature = "ai")]
        {
            // AI-specific functionality should work
            let _ai_available = true;
        }

        #[cfg(not(feature = "ai"))]
        {
            // Should compile without AI
            let _ai_available = false;
        }

        #[cfg(feature = "blocks")]
        {
            // Blocks-specific functionality should work
            let _blocks_available = true;
        }

        #[cfg(not(feature = "blocks"))]
        {
            // Should compile without blocks
            let _blocks_available = false;
        }

        #[cfg(feature = "security-lens")]
        {
            // Security lens should work
            let _security_available = true;
        }

        #[cfg(not(feature = "security-lens"))]
        {
            // Should compile with security stubs
            let _security_available = false;
        }
    }
}

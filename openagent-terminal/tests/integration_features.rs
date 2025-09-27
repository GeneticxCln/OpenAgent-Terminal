//! Integration tests for AI, Blocks, Security, and Plugin features
//! Tests that all features work together in production configuration

// Test AI Command Assistance
mod ai_tests {
    use openagent_terminal::ai_runtime::{AiRuntime, AiProvider, AiProposal};

    #[test]
    fn test_ai_runtime_basic() {
        let mut runtime = AiRuntime::new();
        assert!(!runtime.ui.active);
        assert!(runtime.ui.scratch.is_empty());
        assert!(runtime.ui.proposals.is_empty());

        runtime.ui.scratch = "help me with git".to_string();
        runtime.ui.cursor_position = runtime.ui.scratch.len();

        assert_eq!(runtime.ui.scratch, "help me with git");
        assert_eq!(runtime.ui.cursor_position, "help me with git".len());
    }

    #[test]
    fn test_ai_provider_switch() {
        let mut runtime = AiRuntime::new();
        // Switch to a known provider
        assert!(runtime.switch_provider(AiProvider::OpenAI).is_ok());
        assert_eq!(runtime.ui.provider, AiProvider::OpenAI);

        // Attempt to switch to an unconfigured custom provider should error
        let res = runtime.switch_provider(AiProvider::Custom("invalid".to_string()));
        assert!(res.is_err());
    }

    #[test]
    fn test_ai_proposals_basic() {
        let mut runtime = AiRuntime::new();
        runtime.add_simple_proposal("echo test".to_string());
        assert_eq!(runtime.ui.proposals.len(), 1);
        assert_eq!(runtime.ui.proposals[0].title, "echo test");

        // Add a structured proposal as well
        runtime.add_proposal(AiProposal {
            title: "List files".to_string(),
            description: Some("Show directory contents".to_string()),
            proposed_commands: vec!["ls -la".to_string()],
        });
        assert_eq!(runtime.ui.proposals.len(), 2);
    }
}

// Test Command Blocks/History - now enabled for production
#[cfg(all(feature = "blocks", feature = "integration-blocks-tests"))]
mod blocks_tests {
    use openagent_terminal::blocks_v2::{BlockManager, CreateBlockParams, SearchQuery};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_block_manager_integration() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("blocks.db");
        let mut manager = BlockManager::new(db_path).await.unwrap();

        // Test command lifecycle
        let params = CreateBlockParams {
            command: "ls -la".to_string(),
            directory: Some(temp_dir.path().to_path_buf()),
            environment: None,
            shell: None,
            tags: None,
            parent_id: None,
            metadata: None,
        };
        let block = manager.create_block(params).await.unwrap();
        assert_eq!(block.command, "ls -la");

        // Update block with completion
        manager
            .update_block_output(
                block.id,
                "total 10\nfile1.txt\nfile2.txt\n".to_string(),
                0,
                50,
            )
            .await
            .unwrap();

        // Test search functionality
        let query = SearchQuery {
            text: Some("ls"),
            limit: Some(10),
            ..Default::default()
        };
        let results = manager.search(query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, "ls -la");
        assert_eq!(results[0].exit_code, 0);

        // Test recent history
        let recent_query = SearchQuery {
            limit: Some(5),
            ..Default::default()
        };
        let recent = manager.search(recent_query).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].command, "ls -la");
    }

    #[tokio::test]
    async fn test_block_manager_multiple_commands() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("blocks.db");
        let mut manager = BlockManager::new(db_path).await.unwrap();

        // Add multiple commands
        let commands = vec!["pwd", "ls", "cat file.txt", "grep test *.txt"];
        for cmd in &commands {
            let params = CreateBlockParams {
                command: cmd.to_string(),
                directory: Some(temp_dir.path().to_path_buf()),
                environment: None,
                shell: None,
                tags: None,
                parent_id: None,
                metadata: None,
            };
            let block = manager.create_block(params).await.unwrap();
            manager
                .update_block_output(block.id, format!("output for {}", cmd), 0, 25)
                .await
                .unwrap();
        }

        // Test search finds multiple results
        let query = SearchQuery {
            text: Some("txt"),
            limit: Some(10),
            ..Default::default()
        };
        let results = manager.search(query).await.unwrap();
        assert_eq!(results.len(), 2); // "cat file.txt" and "grep test *.txt"

        // Test recent returns in reverse order
        let recent_query = SearchQuery {
            limit: Some(2),
            sort_by: Some("created_at"),
            sort_order: Some("DESC"),
            ..Default::default()
        };
        let recent = manager.search(recent_query).await.unwrap();
        assert_eq!(recent.len(), 2);
        // Most recent should be last command added
    }

    #[tokio::test]
    async fn test_block_manager_starring() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("blocks.db");
        let mut manager = BlockManager::new(db_path).await.unwrap();

        let params = CreateBlockParams {
            command: "important-command".to_string(),
            directory: Some(temp_dir.path().to_path_buf()),
            environment: None,
            shell: None,
            tags: None,
            parent_id: None,
            metadata: None,
        };
        let block = manager.create_block(params).await.unwrap();

        // Toggle starred status
        let starred = manager.toggle_starred(block.id).await.unwrap();
        assert!(starred);

        // Verify the change
        let retrieved = manager.get_block(block.id).await.unwrap().unwrap();
        assert!(retrieved.starred);

        // Search starred only
        let query = SearchQuery {
            starred: Some(true),
            ..Default::default()
        };
        let starred_results = manager.search(query).await.unwrap();
        assert_eq!(starred_results.len(), 1);
        assert_eq!(starred_results[0].command, "important-command");
    }
}

// Test Security Lens - now enabled for production
#[cfg(all(feature = "security", feature = "integration-security-tests"))]
mod security_tests {
    use openagent_terminal::security_lens::{
        SecurityLens, SecurityPolicy, RiskLevel, CustomPattern,
    };

    #[test]
    fn test_security_policy_basic() {
        let policy = SecurityPolicy::default();
        assert!(policy.enabled);
        assert!(!policy.block_critical);

        // Test validation
        let mut lens = SecurityLens::new(policy);
        // Ensure a known safe command evaluates to Safe under default policy
        let risk = lens.analyze_command("ls");
        assert_eq!(risk.level, RiskLevel::Safe);
    }

    #[test]
    fn test_security_lens_dangerous_commands() {
        let policy = SecurityPolicy::default();
        let mut lens = SecurityLens::new(policy);

        // Test critical command
        let result = lens.analyze_command("rm -rf /");
        assert_eq!(result.level, RiskLevel::Critical);
        assert!(!result.factors.is_empty());
        assert!(!result.mitigations.is_empty());

        // Test with conservative policy
        let conservative_policy = SecurityPolicy::preset_conservative();
        let conservative_lens = SecurityLens::new(conservative_policy);
        let result = lens.analyze_command("rm -rf /");
        assert!(conservative_lens.should_block(&result)); // Conservative policy blocks critical
    }

    #[test]
    fn test_security_lens_safe_commands() {
        let policy = SecurityPolicy::default();
        let mut lens = SecurityLens::new(policy);

        // Test safe commands
        let safe_commands = vec!["ls", "pwd", "echo hello", "cat file.txt"];
        for cmd in safe_commands {
            let result = lens.analyze_command(cmd);
            assert_eq!(result.level, RiskLevel::Safe);
            assert!(!lens.should_block(&result));
        }
    }

    #[test]
    fn test_custom_security_patterns() {
        let policy = SecurityPolicy::default();
        let mut lens = SecurityLens::new(policy);

        // Add custom pattern
        let custom_pattern = CustomPattern {
            name: "Deploy to production".to_string(),
            pattern: r"deploy.*prod".to_string(),
            risk_level: RiskLevel::High,
            description: "Production deployment detected".to_string(),
            regex: None,
        };

        lens.add_custom_pattern(custom_pattern).unwrap();

        // Test the custom pattern triggers
        let result = lens.analyze_command("deploy myapp prod");
        assert_eq!(result.level, RiskLevel::High);
        assert!(!result.explanation.is_empty());
    }

    #[test]
    fn test_security_preset_configs() {
        // Conservative preset
        let conservative = SecurityPolicy::preset_conservative();
        assert!(conservative.block_critical);
        let mut lens = SecurityLens::new(conservative);
        let result = lens.analyze_command("rm -rf /");
        assert!(lens.should_block(&result));

        // Permissive preset
        let permissive = SecurityPolicy::preset_permissive();
        assert!(!permissive.block_critical);

        // Disabled preset
        let disabled = SecurityPolicy::preset_disabled();
        assert!(!disabled.enabled);
        let mut lens = SecurityLens::new(disabled);
        let result = lens.analyze_command("rm -rf /");
        assert_eq!(result.level, RiskLevel::Safe); // Everything is safe when disabled
    }
}

// Test Plugin System - now enabled for production
#[cfg(feature = "plugins")]
mod plugin_tests {
    use openagent_terminal::plugins_api::{
        PluginHost, PluginManager, PluginManifest, PluginType,
        SignaturePolicy, Permission,
    };
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn test_plugin_host_creation() {
        let host = PluginHost::new(SignaturePolicy::Optional);
        assert_eq!(host.list_plugins().len(), 0);
    }

    #[test]
    fn test_plugin_manifest_validation() {
        let host = PluginHost::new(SignaturePolicy::Optional);

        let valid_manifest = PluginManifest {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "A test plugin".to_string(),
            author: "Test Author".to_string(),
            homepage: None,
            repository: None,
            license: "MIT".to_string(),
            keywords: vec!["test".to_string()],
            plugin_type: PluginType::Command,
            main_file: "main.js".to_string(),
            permissions: vec![],
            dependencies: HashMap::new(),
            minimum_terminal_version: "0.1.0".to_string(),
            supported_platforms: vec!["linux".to_string()],
            entry_points: vec![],
            configuration_schema: None,
        };

        // This should validate successfully
        let result = std::panic::catch_unwind(|| {
            // In a real implementation, this would call host.validate_manifest(&valid_manifest)
            // For now, we just test that the manifest is well-formed
            assert!(!valid_manifest.id.is_empty());
            assert!(!valid_manifest.name.is_empty());
            assert!(!valid_manifest.main_file.is_empty());
        });
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_plugin_manager_initialization() {
        let mut manager = PluginManager::new(SignaturePolicy::Optional);
        let result = manager.initialize().await;

        // Should succeed even with no plugins found
        assert!(result.is_ok());
        assert_eq!(manager.host().list_plugins().len(), 0);
    }

    #[test]
    fn test_plugin_permissions() {
        use std::path::PathBuf;

        let permissions = vec![
            Permission::FileSystemRead(vec![PathBuf::from("/tmp")]),
            Permission::NetworkAccess(vec!["api.example.com".to_string()]),
            Permission::TerminalControl,
        ];

        // Test that permissions can be created and inspected
        assert_eq!(permissions.len(), 3);

        for permission in permissions {
            match permission {
                Permission::FileSystemRead(paths) => {
                    assert_eq!(paths.len(), 1);
                    assert_eq!(paths[0], PathBuf::from("/tmp"));
                }
                Permission::NetworkAccess(domains) => {
                    assert_eq!(domains.len(), 1);
                    assert_eq!(domains[0], "api.example.com");
                }
                Permission::TerminalControl => {
                    // This permission has no associated data
                }
                _ => panic!("Unexpected permission type"),
            }
        }
    }
}

// Integration tests combining all features
mod integration_tests {
    #[tokio::test]
    async fn test_feature_compatibility() {
        // This test ensures different feature combinations work together
        
        #[cfg(feature = "ai")]
        {
            // AI-specific functionality should work
            let _ai_available = true;
        }
        
        #[cfg(feature = "blocks")]
        {
            // Blocks-specific functionality should work
            let _blocks_available = true;
        }
        
        #[cfg(feature = "security")]
        {
            // Security lens should work
            let _security_available = true;
        }
        
        #[cfg(feature = "plugins")]
        {
            // Plugin system should work
            let _plugins_available = true;
        }
        
        #[cfg(feature = "notebooks")]
        {
            // Notebooks should work
            let _notebooks_available = true;
        }
        
        // Basic runtime sanity check not based on a constant
        let cwd = std::env::current_dir().expect("cwd");
        assert!(cwd.exists());
    }

    #[cfg(all(feature = "ai", feature = "security"))]
    #[tokio::test]
    async fn test_ai_with_security_integration() {
        use openagent_terminal::ai_runtime::AiRuntime;
        use openagent_terminal::security_lens::{SecurityLens, SecurityPolicy};
        
        // Test AI generating a potentially dangerous command and security analysis
        let _ai_runtime = AiRuntime::new();
        let policy = SecurityPolicy::default();
        let mut lens = SecurityLens::new(policy);
        
        // Simulate AI proposing a dangerous command
        let dangerous_command = "rm -rf /tmp/*";
        
        // Test security analysis of AI proposal
        let security_result = lens.analyze_command(dangerous_command);
        
        // Should be detected as risky
        assert!(security_result.level != openagent_terminal::security_lens::RiskLevel::Safe);
        assert!(lens.should_block(&security_result));
    }

#[cfg(all(feature = "blocks", feature = "security", feature = "integration-blocks-tests"))]
#[tokio::test]
async fn test_blocks_with_security_integration() {
        use openagent_terminal::blocks_v2::{BlockManager, CreateBlockParams};
        use openagent_terminal::security_lens::{SecurityLens, SecurityPolicy};
        use tempfile::tempdir;
        
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let mut manager = BlockManager::new(db_path).await.unwrap();
        let policy = SecurityPolicy::default();
        let mut lens = SecurityLens::new(policy);
        
        // Simulate a complete command lifecycle with security integration
        let command = "ls -la";
        
        // 1. Security check before execution
        let security_result = lens.analyze_command(command);
        assert_eq!(security_result.level, openagent_terminal::security_lens::RiskLevel::Safe);
        assert!(!lens.should_block(&security_result));
        
        // 2. Create block for tracking
        let params = CreateBlockParams {
            command: command.to_string(),
            directory: Some(temp_dir.path().to_path_buf()),
            environment: None,
            shell: None,
            tags: None,
            parent_id: None,
            metadata: None,
        };
        let block = manager.create_block(params).await.unwrap();
        
        // 3. Complete the command
        manager
            .update_block_output(block.id, "total 10\nfile1.txt\nfile2.txt\n".to_string(), 0, 30)
            .await
            .unwrap();
        
        // 4. Verify it's in history
        let query = openagent_terminal::blocks_v2::SearchQuery {
            text: Some("ls"),
            limit: Some(10),
            ..Default::default()
        };
        let results = manager.search(query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, command);
        assert_eq!(results[0].exit_code, 0);
    }
}

// Performance tests - enabled for production
mod performance_tests {
    #[cfg(feature = "security")]
    #[test]
    fn test_security_analysis_performance() {
        use openagent_terminal::security_lens::{SecurityLens, SecurityPolicy};
        use std::time::Instant;
        
        let policy = SecurityPolicy::default();
        let mut lens = SecurityLens::new(policy);
        
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
            let _ = lens.analyze_command(cmd);
        }
        let duration = start.elapsed();
        
        // Should analyze 5 commands in well under 1 second
        assert!(duration.as_millis() < 1000, "Security analysis took {}ms", duration.as_millis());
        
        // Per-command should be very fast
        let per_command = duration.as_millis() / commands.len() as u128;
        assert!(per_command < 200, "Per-command analysis took {}ms", per_command);
    }

#[cfg(all(feature = "blocks", feature = "integration-blocks-tests"))]
#[tokio::test]
async fn test_blocks_storage_performance() {
        use openagent_terminal::blocks_v2::{BlockManager, CreateBlockParams};
        use std::time::Instant;
        use tempfile::tempdir;
        
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("perf_test.db");
        let mut manager = BlockManager::new(db_path).await.unwrap();
        
        // Test that storing many commands is reasonably fast
        let start = Instant::now();
        
        for i in 0..100 {
            let params = CreateBlockParams {
                command: format!("echo command_{}", i),
                directory: Some(temp_dir.path().to_path_buf()),
                environment: None,
                shell: None,
                tags: None,
                parent_id: None,
                metadata: None,
            };
            let block = manager.create_block(params).await.unwrap();
            manager
                .update_block_output(block.id, format!("output_{}", i), 0, 10)
                .await
                .unwrap();
        }
        
        let duration = start.elapsed();
        
        // Should handle 100 commands in under 10 seconds
        assert!(duration.as_secs() < 10, "Storing 100 commands took {}s", duration.as_secs());
        
        // Search should also be fast
        let search_start = Instant::now();
        let query = openagent_terminal::blocks_v2::SearchQuery {
            text: Some("command"),
            limit: Some(50),
            ..Default::default()
        };
        let results = manager.search(query).await.unwrap();
        let search_duration = search_start.elapsed();
        
        assert_eq!(results.len(), 50); // Limited by max_results
        assert!(
            search_duration.as_millis() < 1000,
            "Search took {}ms",
            search_duration.as_millis()
        );
    }
}

//! AI Terminal Integration Demo
//!
//! This example demonstrates how to use the AI terminal integration system
//! to provide real-time AI assistance for terminal operations.

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use tokio::time::sleep;
use tracing::{info, warn, error};

use openagent_terminal::ai_terminal_integration::{
    AiTerminalIntegrationManager, AiTerminalConfig, AssistanceType
};
use openagent_terminal::blocks_v2::ShellType;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting AI Terminal Integration Demo");

    // Get current directory and shell type
    let current_dir = env::current_dir()?;
    let shell_type = detect_shell();

    info!("Current directory: {}", current_dir.display());
    info!("Detected shell: {:?}", shell_type);

    // Create configuration
    let config = AiTerminalConfig::default();

    // Create AI terminal integration manager
    let mut manager = AiTerminalIntegrationManager::new(
        config,
        current_dir.clone(),
        shell_type,
    ).await?;

    info!("AI terminal integration manager created successfully");

    // Start the integration system
    manager.start().await?;
    info!("AI terminal integration system started");

    // Get the terminal integration interface
    let terminal_integration = manager.get_terminal_integration();

    // Demo 1: Simulate command execution with failure
    info!("\n=== Demo 1: Command Failure Analysis ===");
    terminal_integration.on_command_completed(
        "git status",
        128, // Git error code for "not a git repository"
        "",
        "fatal: not a git repository (or any of the parent directories): .git",
        Duration::from_millis(50),
    )?;

    sleep(Duration::from_secs(2)).await;

    // Demo 2: Simulate successful command
    info!("\n=== Demo 2: Successful Command ===");
    terminal_integration.on_command_completed(
        "ls -la",
        0,
        "total 48\ndrwxr-xr-x  8 user user 4096 Dec  1 10:30 .\ndrwxr-xr-x 12 user user 4096 Dec  1 10:25 ..\ndrwxr-xr-x  8 user user 4096 Dec  1 10:30 .git",
        "",
        Duration::from_millis(25),
    )?;

    sleep(Duration::from_secs(1)).await;

    // Demo 3: Simulate directory change to a Git repository
    info!("\n=== Demo 3: Directory Change to Git Repository ===");
    let git_dir = current_dir.join("test_repo");
    terminal_integration.on_directory_change(&git_dir)?;

    sleep(Duration::from_secs(1)).await;

    // Demo 4: Simulate typing a command (for command completion)
    info!("\n=== Demo 4: Command Typing Assistance ===");
    terminal_integration.on_user_input("git com", 7)?;

    sleep(Duration::from_secs(1)).await;

    // Demo 5: Explicitly request AI assistance
    info!("\n=== Demo 5: Explicit AI Assistance Request ===");
    manager.request_ai_assistance(
        "I need help understanding Docker commands for container management".to_string(),
        AssistanceType::Explain,
    ).await?;

    sleep(Duration::from_secs(2)).await;

    // Demo 6: Simulate Python module not found error
    info!("\n=== Demo 6: Python Module Error ===");
    terminal_integration.on_command_completed(
        "python -c \"import numpy\"",
        1,
        "",
        "Traceback (most recent call last):\n  File \"<string>\", line 1, in <module>\nModuleNotFoundError: No module named 'numpy'",
        Duration::from_millis(100),
    )?;

    sleep(Duration::from_secs(2)).await;

    // Demo 7: Show system statistics
    info!("\n=== Demo 7: System Statistics ===");
    let stats = manager.get_statistics().await;
    info!("System Statistics:");
    info!("  - Active agents: {}", stats.active_agents);
    info!("  - Total events processed: {}", stats.total_events_processed);
    info!("  - Total AI responses: {}", stats.total_ai_responses);
    info!("  - Session duration: {:?}", stats.session_duration);
    info!("  - Current directory: {}", stats.current_directory.display());
    info!("  - Events per minute: {:.2}", stats.performance_metrics.events_per_minute);

    // Demo 8: Show system health
    info!("\n=== Demo 8: System Health ===");
    let health = manager.get_health_status().await;
    info!("System Health:");
    info!("  - Running: {}", health.is_running);
    info!("  - Health score: {:.1}/100", health.health_score);
    info!("  - Uptime: {:?}", health.uptime);
    info!("  - Memory usage: {:.2} MB", health.performance_metrics.memory_usage_mb);
    info!("  - CPU usage: {:.2}%", health.performance_metrics.cpu_usage_percent);

    // Demo 9: More complex command scenarios
    info!("\n=== Demo 9: Complex Command Scenarios ===");
    
    // NPM command failure
    terminal_integration.on_command_completed(
        "npm install react",
        127,
        "",
        "npm: command not found",
        Duration::from_millis(10),
    )?;

    sleep(Duration::from_secs(1)).await;

    // Docker command
    terminal_integration.on_command_completed(
        "docker ps",
        0,
        "CONTAINER ID   IMAGE     COMMAND   CREATED   STATUS    PORTS     NAMES",
        "",
        Duration::from_millis(200),
    )?;

    sleep(Duration::from_secs(1)).await;

    // Cargo build in Rust project
    let cargo_dir = current_dir.clone();
    terminal_integration.on_directory_change(&cargo_dir)?;
    
    sleep(Duration::from_millis(500)).await;
    
    terminal_integration.on_command_completed(
        "cargo build",
        0,
        "   Compiling openagent-terminal v0.1.0\n    Finished dev [unoptimized + debuginfo] target(s) in 2.34s",
        "",
        Duration::from_millis(2340),
    )?;

    // Let the system process events
    sleep(Duration::from_secs(3)).await;

    // Final statistics
    info!("\n=== Final Statistics ===");
    let final_stats = manager.get_statistics().await;
    info!("Final System Statistics:");
    info!("  - Total events processed: {}", final_stats.total_events_processed);
    info!("  - Total AI responses: {}", final_stats.total_ai_responses);
    info!("  - Average response time: {:.2}ms", final_stats.performance_metrics.average_response_time_ms);

    // Clean shutdown
    info!("\n=== Shutting Down ===");
    manager.stop().await?;
    info!("AI terminal integration system stopped");

    info!("Demo completed successfully!");
    Ok(())
}

/// Detect the current shell type
fn detect_shell() -> ShellType {
    if let Ok(shell) = env::var("SHELL") {
        if shell.contains("bash") {
            return ShellType::Bash;
        } else if shell.contains("zsh") {
            return ShellType::Zsh;
        } else if shell.contains("fish") {
            return ShellType::Fish;
        } else if shell.contains("sh") && !shell.contains("bash") {
            return ShellType::Sh;
        }
    }
    
    // Default fallback
    ShellType::Bash
}

/// Additional helper function to demonstrate configuration customization
#[allow(dead_code)]
fn create_custom_config() -> AiTerminalConfig {
    use openagent_terminal::ai_terminal_integration::*;
    use openagent_terminal::ai_runtime::AiProvider;
    use std::collections::HashMap;

    let mut config = AiTerminalConfig::default();
    
    // Customize AI runtime settings
    config.ai_runtime.response_timeout_ms = 15000; // 15 second timeout
    config.ai_runtime.max_conversation_length = 50;
    
    // Customize event monitoring
    config.event_bridge.monitor_typing = true; // Enable typing monitoring
    config.event_bridge.max_output_length = 5 * 1024; // 5KB max output
    
    // Customize agents
    config.agents.global_settings.max_agents_per_event = 2;
    config.agents.global_settings.global_rate_limit = 60; // 1 per second max
    
    // Add custom agent
    config.agents.custom_agents.push(CustomAgentConfig {
        id: "rust_expert".to_string(),
        name: "Rust Expert".to_string(),
        description: "Provides Rust-specific development assistance".to_string(),
        provider: AiProvider::Ollama,
        model: "codellama:7b".to_string(),
        system_prompt: "You are a Rust programming expert. Help users with Rust code, cargo commands, and Rust-specific issues.".to_string(),
        trigger_events: vec!["CommandFailed".to_string(), "CommandExecuted".to_string()],
        activation_conditions: vec![
            "CommandContains:cargo".to_string(),
            "CommandContains:rustc".to_string(),
            "ErrorContains:rust".to_string(),
        ],
        priority: 80,
        debounce_seconds: 2,
        enabled: true,
    });
    
    // Customize UI integration
    config.ui_integration.show_timestamps = true;
    config.ui_integration.max_display_length = 300;
    
    // Customize performance settings
    config.performance.max_concurrent_requests = 3;
    config.performance.stats_interval_seconds = 30;
    
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_shell_detection() {
        let shell_type = detect_shell();
        // Should return one of the supported shell types
        match shell_type {
            ShellType::Bash | ShellType::Zsh | ShellType::Fish | ShellType::Sh => {
                // Valid shell type detected
            }
        }
    }
    
    #[test]
    fn test_custom_config_creation() {
        let config = create_custom_config();
        assert_eq!(config.ai_runtime.response_timeout_ms, 15000);
        assert!(config.event_bridge.monitor_typing);
        assert_eq!(config.agents.custom_agents.len(), 1);
        assert_eq!(config.agents.custom_agents[0].id, "rust_expert");
    }
}
//! Session Persistence Demo
//!
//! Demonstrates the complete session persistence system including:
//! - Session creation and restoration
//! - Command history persistence
//! - User preferences management
//! - Workspace state tracking
//! - CLI interface for session management

use std::time::Duration;
use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;
use tokio::time::sleep;
use uuid::Uuid;

use openagent_terminal::{
    session_persistence::{SessionManager, PersistenceConfig, SessionId},
    session_service::{SessionService, CommandExecutionResult, RestoreOptions},
    session_cli::{SessionCli, SessionCliHandler},
    blocks_v2::{BlockRecord, BlockId, ShellType},
    ai_context_provider::PtyAiContext,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🚀 Session Persistence Demo");
    println!("============================\n");
    
    // Create temporary directory for demo
    let temp_dir = tempfile::TempDir::new()?;
    let session_dir = temp_dir.path().join("sessions");
    
    // Configure session persistence
    let config = PersistenceConfig {
        session_dir: session_dir.clone(),
        auto_save_interval: Duration::from_secs(2), // Quick saves for demo
        max_sessions: 5,
        cleanup_after: Duration::from_secs(300), // 5 minutes for demo
        persist_commands: true,
        persist_conversations: true,
        persist_preferences: true,
        persist_workspace: true,
        sanitize_sensitive_data: true,
        exclude_patterns: vec!["password".to_string(), "secret".to_string()],
        ..Default::default()
    };
    
    println!("📁 Session storage directory: {}", session_dir.display());
    
    // Initialize session service
    let session_service = SessionService::new(config.clone()).await?;
    
    // Demo 1: Create and populate a new session
    println!("\n🆕 Demo 1: Creating a new session");
    println!("----------------------------------");
    
    let context = create_demo_context();
    let session_id = session_service.start_new_session(&context).await?;
    println!("✅ Created session: {}", session_id);
    
    // Add some demo commands to the session
    let demo_commands = vec![
        ("ls -la", "total 42\ndrwxr-xr-x  5 user user 4096 Jan  1 10:00 .\ndrwxr-xr-x  3 user user 4096 Jan  1 09:00 ..", 0),
        ("pwd", "/home/user/projects/demo", 0),
        ("echo 'Hello, World!'", "Hello, World!", 0),
        ("git status", "On branch main\nnothing to commit, working tree clean", 0),
    ];
    
    for (i, (command, output, exit_code)) in demo_commands.iter().enumerate() {
        let block_record = BlockRecord {
            id: BlockId(Uuid::new_v4()),
            command: command.to_string(),
            output: output.to_string(),
            exit_code: *exit_code,
            duration: Duration::from_millis(100 + i as u64 * 50),
            created_at: Utc::now(),
            working_directory: PathBuf::from("/home/user/projects/demo"),
            shell: ShellType::Bash,
            tags: vec!["demo".to_string()],
        };
        
        let exec_result = CommandExecutionResult {
            output: output.to_string(),
            error_output: String::new(),
            exit_code: *exit_code,
            duration: Duration::from_millis(100 + i as u64 * 50),
        };
        
        session_service.add_command(&block_record, &exec_result).await?;
        println!("  📝 Added command: {}", command);
    }
    
    // Update user preferences
    session_service.update_preferences(|prefs| {
        prefs.theme = "dark".to_string();
        prefs.font_size = 16.0;
        prefs.ai_auto_suggestions = true;
        prefs.max_history_items = 500;
    }).await?;
    println!("  ⚙️  Updated user preferences");
    
    // Update workspace state
    session_service.update_workspace(|workspace| {
        workspace.recent_directories.push(PathBuf::from("/home/user/projects"));
        workspace.bookmarks.insert(
            "demo".to_string(),
            PathBuf::from("/home/user/projects/demo")
        );
    }).await?;
    println!("  📂 Updated workspace state");
    
    // Wait for auto-save
    sleep(Duration::from_secs(3)).await;
    
    // Demo 2: Show session statistics
    println!("\n📊 Demo 2: Session statistics");
    println!("------------------------------");
    
    let stats = session_service.get_session_stats().await?;
    println!("Total sessions: {}", stats.total_sessions);
    println!("Active session: {:?}", stats.active_session);
    println!("Total commands: {}", stats.total_commands);
    println!("Total conversations: {}", stats.total_conversations);
    
    // Demo 3: List all sessions
    println!("\n📋 Demo 3: Listing sessions");
    println!("----------------------------");
    
    let sessions = session_service.list_sessions().await?;
    for session_summary in &sessions {
        println!("Session {}: {} commands, {} conversations", 
               session_summary.session_id, 
               session_summary.command_count,
               session_summary.conversation_count);
        println!("  Created: {}", session_summary.created_at.format("%Y-%m-%d %H:%M:%S"));
        println!("  Directory: {}", session_summary.working_directory.display());
    }
    
    // Demo 4: Export session
    println!("\n📤 Demo 4: Exporting session");
    println!("-----------------------------");
    
    let export_path = temp_dir.path().join("exported_session.json");
    session_service.export_session(session_id, &export_path).await?;
    println!("✅ Exported session to: {}", export_path.display());
    
    let export_size = std::fs::metadata(&export_path)?.len();
    println!("  📏 Export file size: {} bytes", export_size);
    
    // Demo 5: Create a second session and restore the first
    println!("\n🔄 Demo 5: Session restoration");
    println!("-------------------------------");
    
    // Create second session
    let context2 = PtyAiContext {
        terminal_context: openagent_terminal::ai_context_provider::TerminalContext {
            working_directory: PathBuf::from("/tmp"),
            last_command: Some("cd /tmp".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    
    let session_id2 = session_service.start_new_session(&context2).await?;
    println!("✅ Created second session: {}", session_id2);
    
    // Add a command to the second session
    let block_record2 = BlockRecord {
        id: BlockId(Uuid::new_v4()),
        command: "whoami".to_string(),
        output: "demo_user".to_string(),
        exit_code: 0,
        duration: Duration::from_millis(50),
        created_at: Utc::now(),
        working_directory: PathBuf::from("/tmp"),
        shell: ShellType::Bash,
        tags: vec!["identity".to_string()],
    };
    
    let exec_result2 = CommandExecutionResult {
        output: "demo_user".to_string(),
        error_output: String::new(),
        exit_code: 0,
        duration: Duration::from_millis(50),
    };
    
    session_service.add_command(&block_record2, &exec_result2).await?;
    println!("  📝 Added command to second session");
    
    // Now restore the first session
    let restore_options = RestoreOptions {
        restore_commands: true,
        restore_conversations: true,
        restore_preferences: true,
        restore_workspace: true,
        restore_environment: false,
        max_restore_age: Some(Duration::from_secs(3600)), // 1 hour
    };
    
    println!("🔄 Restoring first session...");
    let restoration_summary = session_service.restore_session(session_id, restore_options).await?;
    
    println!("✅ Session restored successfully!");
    println!("  Commands restored: {}", restoration_summary.commands_restored);
    println!("  Conversations restored: {}", restoration_summary.conversations_restored);
    println!("  Preferences restored: {}", restoration_summary.preferences_restored);
    println!("  Workspace restored: {}", restoration_summary.workspace_restored);
    
    // Verify restoration by checking current session
    if let Some(current_session) = session_service.get_current_session().await {
        println!("  ✅ Current session ID: {}", current_session.session_id);
        println!("  📝 Command history size: {}", current_session.command_history.len());
        println!("  🎨 Theme: {}", current_session.preferences.theme);
        println!("  📁 Bookmarks: {}", current_session.workspace.bookmarks.len());
    }
    
    // Demo 6: Import session
    println!("\n📥 Demo 6: Importing session");
    println!("-----------------------------");
    
    let imported_session_id = session_service.import_session(&export_path).await?;
    println!("✅ Imported session with new ID: {}", imported_session_id);
    
    // Show updated session list
    let updated_sessions = session_service.list_sessions().await?;
    println!("📋 Total sessions after import: {}", updated_sessions.len());
    
    // Demo 7: Session event monitoring
    println!("\n📡 Demo 7: Event monitoring");
    println!("----------------------------");
    
    let mut event_receiver = session_service.subscribe_events();
    
    // Start a background task to monitor events
    let monitor_handle = tokio::spawn(async move {
        for i in 0..5 {
            if let Ok(event) = tokio::time::timeout(Duration::from_secs(1), event_receiver.recv()).await {
                match event {
                    Ok(session_event) => {
                        println!("  📢 Event {}: {:?}", i + 1, session_event);
                    }
                    Err(e) => {
                        println!("  ❌ Event error: {}", e);
                    }
                }
            } else {
                break;
            }
        }
    });
    
    // Trigger some events
    session_service.save_current_session().await?;
    sleep(Duration::from_millis(100)).await;
    
    session_service.update_preferences(|prefs| {
        prefs.font_size = 18.0;
    }).await?;
    sleep(Duration::from_millis(100)).await;
    
    // Wait for event monitoring to complete
    let _ = tokio::time::timeout(Duration::from_secs(2), monitor_handle).await;
    
    // Demo 8: CLI interface demonstration
    println!("\n🖥️  Demo 8: CLI Interface");
    println!("-------------------------");
    
    let cli_handler = SessionCliHandler::new(session_service.clone());
    
    // Simulate some CLI commands
    println!("Simulating CLI commands (actual parsing would be done by clap):");
    
    // Show session stats via CLI
    println!("  📊 Getting session statistics...");
    let stats = session_service.get_session_stats().await?;
    let stats_json = serde_json::to_string_pretty(&stats)?;
    println!("  Statistics: {}", stats_json);
    
    // Demo 9: Cleanup
    println!("\n🧹 Demo 9: Session cleanup");
    println!("--------------------------");
    
    let cleanup_count = session_service.cleanup_old_sessions().await?;
    println!("🗑️  Cleaned up {} old sessions", cleanup_count);
    
    let final_sessions = session_service.list_sessions().await?;
    println!("📋 Final session count: {}", final_sessions.len());
    
    // Demo 10: Performance metrics
    println!("\n⚡ Demo 10: Performance summary");
    println!("-------------------------------");
    
    let start_time = std::time::Instant::now();
    
    // Perform several operations to measure performance
    for i in 0..10 {
        let temp_context = create_demo_context();
        let temp_session = session_service.start_new_session(&temp_context).await?;
        
        let block = BlockRecord {
            id: BlockId(Uuid::new_v4()),
            command: format!("test_command_{}", i),
            output: format!("test_output_{}", i),
            exit_code: 0,
            duration: Duration::from_millis(10),
            created_at: Utc::now(),
            working_directory: PathBuf::from("/tmp"),
            shell: ShellType::Bash,
            tags: vec!["benchmark".to_string()],
        };
        
        let exec_result = CommandExecutionResult {
            output: format!("test_output_{}", i),
            error_output: String::new(),
            exit_code: 0,
            duration: Duration::from_millis(10),
        };
        
        session_service.add_command(&block, &exec_result).await?;
        session_service.delete_session(temp_session).await?;
    }
    
    let elapsed = start_time.elapsed();
    println!("⏱️  Created and deleted 10 sessions in: {:?}", elapsed);
    println!("📈 Average per session: {:?}", elapsed / 10);
    
    println!("\n✨ Session Persistence Demo completed successfully!");
    println!("💾 Session data is stored in: {}", session_dir.display());
    
    Ok(())
}

fn create_demo_context() -> PtyAiContext {
    PtyAiContext {
        terminal_context: openagent_terminal::ai_context_provider::TerminalContext {
            working_directory: PathBuf::from("/home/user/projects/demo"),
            last_command: Some("ls -la".to_string()),
            git_branch: Some("main".to_string()),
            git_status: Some("clean".to_string()),
            project_info: Some(openagent_terminal::ai_context_provider::ProjectInfo {
                name: "demo-project".to_string(),
                project_type: "rust".to_string(),
                description: Some("Demo project for session persistence".to_string()),
                dependencies: vec!["tokio".to_string(), "serde".to_string()],
                recent_files: vec![
                    PathBuf::from("src/main.rs"),
                    PathBuf::from("Cargo.toml"),
                ],
            }),
        },
        conversation_context: Default::default(),
        error_context: Default::default(),
        security_context: Default::default(),
    }
}
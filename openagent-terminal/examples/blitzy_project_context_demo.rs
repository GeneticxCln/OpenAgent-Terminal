// Example: Blitzy Project Context Agent Demo
// Demonstrates enhanced project analysis, Git integration, and conversation awareness

use openagent_terminal::ai::agents::*;
use openagent_terminal::ai::agents::blitzy_project_context::*;
use openagent_terminal::ai::agents::conversation_manager::*;
use openagent_terminal::ai::agents::natural_language::ConversationRole;

use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("🚀 Blitzy Project Context Agent Demo");
    println!("=====================================");

    // Create and configure the conversation manager
    let mut conversation_manager = ConversationManager::new();
    conversation_manager.initialize(AgentConfig::default()).await?;
    
    // Create a conversation session
    let session_id = conversation_manager.create_session(Some("project-analysis".to_string())).await?;
    
    // Add initial context about what we're doing
    conversation_manager.add_turn(
        session_id,
        ConversationRole::User,
        "I want to analyze my Rust project structure and understand the codebase".to_string(),
        None,
        Vec::new(),
    ).await?;

    // Create the Blitzy Project Context Agent with conversation integration
    let project_agent = BlitzyProjectContextAgent::new()
        .with_config(ProjectContextConfig {
            auto_detect_project_root: true,
            track_file_changes: true,
            analyze_git_history: true,
            scan_dependencies: true,
            generate_file_summaries: false, // Disabled for demo
            max_file_size_bytes: 512 * 1024, // 512KB limit
            excluded_dirs: vec![
                "target".to_string(),
                ".git".to_string(),
                "node_modules".to_string(),
            ],
            included_extensions: vec![
                "rs".to_string(),
                "toml".to_string(),
                "md".to_string(),
                "json".to_string(),
            ],
        })
        .with_conversation_manager(std::sync::Arc::new(conversation_manager));

    // Initialize the agent
    let mut project_agent = project_agent;
    project_agent.initialize(AgentConfig::default()).await?;

    // Analyze the current project (the OpenAgent Terminal itself)
    println!("\n📊 Analyzing Project Structure...");
    
    let current_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .to_string_lossy()
        .to_string();

    let project_request = ProjectContextRequest {
        path: Some(current_dir.clone()),
        include_git: true,
        include_dependencies: true,
        include_file_summaries: false,
        max_files: Some(50),
    };

    let agent_request = AgentRequest {
        id: Uuid::new_v4(),
        request_type: AgentRequestType::ManageProject,
        payload: serde_json::to_value(project_request)?,
        context: AgentContext {
            project_root: Some(".".to_string()),
            current_directory: current_dir,
            current_branch: None,
            open_files: vec![],
            recent_commands: vec![],
            environment_vars: HashMap::new(),
            user_preferences: HashMap::new(),
        },
        metadata: HashMap::new(),
    };

    match project_agent.handle_request(agent_request).await {
        Ok(response) => {
            if response.success {
                if let Ok(project_response) = serde_json::from_value::<ProjectContextResponse>(response.payload.clone()) {
                    display_project_analysis(project_response);
                    
                    // Display artifacts
                    if !response.artifacts.is_empty() {
                        println!("\n📁 Generated Artifacts:");
                        for artifact in &response.artifacts {
                            println!("  • {} ({})", artifact.content, format!("{:?}", artifact.artifact_type));
                        }
                    }
                } else {
                    println!("❌ Failed to parse project analysis response");
                }
            } else {
                println!("❌ Project analysis failed: {:?}", response.payload);
            }
        }
        Err(e) => {
            println!("❌ Error during project analysis: {}", e);
        }
    }

    // Check agent status
    println!("\n🔍 Agent Status:");
    let status = project_agent.status().await;
    println!("  • Healthy: {}", status.is_healthy);
    println!("  • Busy: {}", status.is_busy);
    if let Some(task) = &status.current_task {
        println!("  • Current Task: {}", task);
    }

    // Demonstrate direct project analysis API
    println!("\n🔬 Direct Project Analysis API:");
    match project_agent.analyze_project(".").await {
        Ok(project_info) => {
            display_detailed_project_info(project_info);
        }
        Err(e) => {
            println!("❌ Direct analysis failed: {}", e);
        }
    }

    // Shutdown
    project_agent.shutdown().await?;
    println!("\n✅ Demo completed successfully!");

    Ok(())
}

fn display_project_analysis(response: ProjectContextResponse) {
    println!("\n📊 Project Analysis Results:");
    println!("═══════════════════════════");
    
    if let Some(project) = &response.project {
        println!("🏷️  Project: {}", project.name);
        println!("📍 Root: {}", project.root_path);
        println!("🔧 Type: {:?}", project.project_type);
        
        if let Some(language) = &project.language {
            println!("💻 Language: {}", language);
        }
        
        if let Some(version) = &project.metadata.version {
            println!("📦 Version: {}", version);
        }
        
        if let Some(description) = &project.metadata.description {
            println!("📝 Description: {}", description);
        }

        // Git information
        if let Some(git_info) = &project.git_info {
            println!("\n🌿 Git Repository:");
            println!("  • Branch: {}", git_info.current_branch);
            
            if let Some(remote_url) = &git_info.remote_url {
                println!("  • Remote: {}", remote_url);
            }
            
            let status = &git_info.status;
            if status.is_clean {
                println!("  • Status: ✅ Clean");
            } else {
                println!("  • Status: 🔄 Has changes");
                if !status.staged_files.is_empty() {
                    println!("    - Staged: {}", status.staged_files.len());
                }
                if !status.modified_files.is_empty() {
                    println!("    - Modified: {}", status.modified_files.len());
                }
                if !status.untracked_files.is_empty() {
                    println!("    - Untracked: {}", status.untracked_files.len());
                }
            }
            
            if !git_info.recent_commits.is_empty() {
                println!("  • Recent Commits:");
                for (i, commit) in git_info.recent_commits.iter().take(3).enumerate() {
                    println!("    {}. {} - {} ({})", 
                        i + 1, 
                        &commit.hash[..8], 
                        commit.message,
                        commit.author
                    );
                }
            }
        }

        // Dependencies
        if !project.dependencies.is_empty() {
            println!("\n📦 Dependencies:");
            let mut runtime_deps = Vec::new();
            let mut dev_deps = Vec::new();
            
            for dep in &project.dependencies {
                match dep.dependency_type {
                    DependencyType::Runtime => runtime_deps.push(dep),
                    DependencyType::Development => dev_deps.push(dep),
                    _ => {}
                }
            }
            
            if !runtime_deps.is_empty() {
                println!("  • Runtime ({}):", runtime_deps.len());
                for dep in runtime_deps.iter().take(5) {
                    let version = dep.version.as_deref().unwrap_or("*");
                    println!("    - {} = \"{}\"", dep.name, version);
                }
                if runtime_deps.len() > 5 {
                    println!("    ... and {} more", runtime_deps.len() - 5);
                }
            }
            
            if !dev_deps.is_empty() {
                println!("  • Development ({}):", dev_deps.len());
                for dep in dev_deps.iter().take(3) {
                    let version = dep.version.as_deref().unwrap_or("*");
                    println!("    - {} = \"{}\"", dep.name, version);
                }
                if dev_deps.len() > 3 {
                    println!("    ... and {} more", dev_deps.len() - 3);
                }
            }
        }

        // Project structure
        println!("\n📁 Project Structure:");
        if !project.structure.entry_points.is_empty() {
            println!("  • Entry Points: {}", project.structure.entry_points.join(", "));
        }
        
        if !project.structure.config_files.is_empty() {
            println!("  • Config Files: {}", project.structure.config_files.len());
        }
        
        if !project.structure.documentation.is_empty() {
            println!("  • Documentation: {}", project.structure.documentation.len());
        }
        
        if !project.structure.tests.is_empty() {
            println!("  • Test Files: {}", project.structure.tests.len());
        }

        // File statistics
        if !project.files.is_empty() {
            println!("\n📄 Files Summary:");
            println!("  • Total Files: {}", project.files.len());
            
            let mut by_importance = HashMap::new();
            let mut total_lines = 0u32;
            let mut total_size = 0u64;
            
            for file in &project.files {
                *by_importance.entry(format!("{:?}", file.importance)).or_insert(0) += 1;
                if let Some(lines) = file.lines {
                    total_lines += lines;
                }
                total_size += file.size;
            }
            
            for (importance, count) in by_importance {
                println!("    - {}: {}", importance, count);
            }
            
            if total_lines > 0 {
                println!("  • Total Lines: {}", total_lines);
            }
            println!("  • Total Size: {:.2} MB", total_size as f64 / 1024.0 / 1024.0);
        }
    }

    // Context summary
    println!("\n📋 Context Summary:");
    println!("{}", response.context_summary);
    
    if response.confidence_score > 0.0 {
        println!("🎯 Confidence: {:.1}%", response.confidence_score * 100.0);
    }

    // Suggestions
    if !response.suggestions.is_empty() {
        println!("\n💡 Suggestions:");
        for suggestion in &response.suggestions {
            println!("  • {}", suggestion);
        }
    }
}

fn display_detailed_project_info(project: ProjectInfo) {
    println!("\n🔬 Detailed Project Information:");
    println!("════════════════════════════════");
    
    println!("📊 Analysis completed at: {}", project.last_analyzed.format("%Y-%m-%d %H:%M:%S"));
    
    // Directory analysis
    if !project.structure.directories.is_empty() {
        println!("\n📂 Directory Breakdown:");
        for dir in project.structure.directories.iter().take(10) {
            let purpose_emoji = match dir.purpose {
                DirectoryPurpose::Source => "💻",
                DirectoryPurpose::Tests => "🧪",
                DirectoryPurpose::Documentation => "📚",
                DirectoryPurpose::Configuration => "⚙️",
                DirectoryPurpose::Build => "🔨",
                DirectoryPurpose::Assets => "🎨",
                DirectoryPurpose::Dependencies => "📦",
                DirectoryPurpose::Scripts => "📜",
                DirectoryPurpose::Unknown => "❓",
            };
            
            let size_mb = dir.size_bytes as f64 / 1024.0 / 1024.0;
            println!("  {} {} ({} files, {:.2} MB)", 
                purpose_emoji,
                dir.path, 
                dir.file_count, 
                size_mb
            );
        }
    }
    
    // Most important files
    println!("\n⭐ Most Important Files:");
    let critical_files: Vec<_> = project.files.iter()
        .filter(|f| matches!(f.importance, FileImportance::Critical))
        .take(10)
        .collect();
        
    for file in critical_files {
        let size_kb = file.size as f64 / 1024.0;
        let lines_str = file.lines.map_or("?".to_string(), |l| l.to_string());
        println!("  📄 {} ({} lines, {:.1} KB)", 
            file.relative_path,
            lines_str,
            size_kb
        );
    }
}
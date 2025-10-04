// OpenAgent-Terminal - Main Entry Point
// AI-Native Terminal Emulator combining Portal + OpenAgent

mod ansi;
mod commands;
mod config;
mod error;
mod ipc;
mod session;

use anyhow::Result;
use log::{error, info};
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🚀 Starting OpenAgent-Terminal v{}", env!("CARGO_PKG_VERSION"));
    info!("📋 Phase 5 Week 3: Session Persistence Integration");
    
    // Load configuration
    let config = config::Config::load().unwrap_or_else(|e| {
        log::warn!("Failed to load config: {}", e);
        log::info!("Using default configuration");
        config::Config::default()
    });
    
    info!("Configuration loaded:");
    info!("  Theme: {}", config.terminal.theme);
    info!("  Font: {} ({}pt)", config.terminal.font_family, config.terminal.font_size);
    info!("  Model: {}", config.agent.model);
    info!("  Real execution: {}", config.tools.enable_real_execution);
    
    // Show welcome message
    println!("╔════════════════════════════════════════════╗");
    println!("║      OpenAgent-Terminal (Alpha)           ║");
    println!("║   AI-Native Terminal Emulator             ║");
    println!("║   ✨ With Session Persistence ✨          ║");
    println!("╚════════════════════════════════════════════╝");
    println!();
    println!("Type /help for available commands");
    println!();

    // Determine socket path
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let socket_path = std::env::var("OPENAGENT_SOCKET")
        .unwrap_or_else(|_| format!("{}/openagent-terminal-test.sock", runtime_dir));

    info!("Socket path: {}", socket_path);
    println!("🔌 Connecting to Python backend at: {}", socket_path);
    println!("   (Make sure the Python backend is running!)");
    println!();

    // Create IPC client and session manager
    let mut client = ipc::client::IpcClient::new();
    let mut session_manager = session::SessionManager::new();

    // Try to connect
    match client.connect(&socket_path).await {
        Ok(()) => {
            info!("✅ Connected successfully");
            println!("✅ Connected to Python backend");
            println!();

            // Send initialize request
            match client.initialize().await {
                Ok(response) => {
                    info!("Initialize response: {:?}", response);
                    println!("✅ Backend initialized successfully!");
                    println!();
                    
                    // Connect session manager to IPC client
                    session_manager.set_ipc_client(&mut client);
                    info!("📝 Session manager connected");
                    
                    // Run interactive loop
                    if let Err(e) = run_interactive_loop(&mut client, &mut session_manager).await {
                        error!("Interactive loop error: {}", e);
                        println!("{}Error:{} {}", ansi::colors::RED, ansi::colors::RESET, e);
                    }
                }
                Err(e) => {
                    error!("Initialize failed: {}", e);
                    println!("❌ Initialize failed: {}", e);
                    return Err(e.into());
                }
            }

            // Disconnect
            client.disconnect().await?;
        }
        Err(e) => {
            error!("Connection failed: {}", e);
            println!("❌ Connection failed: {}", e);
            println!();
            println!("Make sure the Python backend is running:");
            println!("  cd backend");
            println!("  python -m openagent_terminal.bridge");
            println!();
            println!("Or set a custom socket path:");
            println!("  export OPENAGENT_SOCKET=/path/to/socket.sock");
            return Err(e.into());
        }
    }

    println!();
    println!("👋 Goodbye!");
    Ok(())
}

/// Interactive loop for session-aware agent queries and session management
async fn run_interactive_loop(
    client: &mut ipc::client::IpcClient,
    session_manager: &mut session::SessionManager,
) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut input_buffer = String::new();
    
    loop {
        // Show prompt
        let prompt = if let Some(session_id) = session_manager.current_session_id() {
            format!("{}[{}]>{} ", 
                ansi::colors::CYAN, 
                &session_id[..8.min(session_id.len())],
                ansi::colors::RESET)
        } else {
            format!("{}>{} ", ansi::colors::GREEN, ansi::colors::RESET)
        };
        
        print!("{}", prompt);
        io::stdout().flush()?;
        
        // Read user input
        input_buffer.clear();
        let bytes_read = reader.read_line(&mut input_buffer).await?;
        
        if bytes_read == 0 {
            // EOF reached (Ctrl+D)
            break;
        }
        
        let input = input_buffer.trim();
        if input.is_empty() {
            continue;
        }
        
        // Parse command
        let command = commands::parse_command(input);
        
        match command {
            commands::Command::Query(query) => {
                if let Err(e) = handle_agent_query(client, &query).await {
                    error!("Query failed: {}", e);
                    println!("{}Error:{} {}", ansi::colors::RED, ansi::colors::RESET, e);
                }
            }
            commands::Command::ListSessions(limit) => {
                match session_manager.list_sessions(limit).await {
                    Ok(sessions) => commands::display_sessions_list(&sessions),
                    Err(e) => {
                        error!("Failed to list sessions: {}", e);
                        println!("{}Error:{} {}", ansi::colors::RED, ansi::colors::RESET, e);
                    }
                }
            }
            commands::Command::LoadSession(session_id) => {
                match session_manager.load_session(&session_id).await {
                    Ok(session) => {
                        println!("{}✅ Loaded session:{} {}", 
                            ansi::colors::GREEN, ansi::colors::RESET, session.metadata.title);
                        println!("   {} messages, {} tokens", 
                            session.messages.len(), session.metadata.total_tokens);
                        println!();
                    }
                    Err(e) => {
                        error!("Failed to load session: {}", e);
                        println!("{}Error:{} {}", ansi::colors::RED, ansi::colors::RESET, e);
                    }
                }
            }
            commands::Command::ExportSession { session_id, format, output_file } => {
                let session_ref = session_id.as_deref();
                match session_manager.export_session(session_ref, &format).await {
                    Ok(content) => {
                        if let Some(file_path) = output_file {
                            match std::fs::write(&file_path, &content) {
                                Ok(_) => {
                                    println!("{}✅ Exported to:{} {}", 
                                        ansi::colors::GREEN, ansi::colors::RESET, file_path);
                                }
                                Err(e) => {
                                    println!("{}Error writing file:{} {}", 
                                        ansi::colors::RED, ansi::colors::RESET, e);
                                }
                            }
                        } else {
                            println!("{}", content);
                        }
                    }
                    Err(e) => {
                        error!("Failed to export session: {}", e);
                        println!("{}Error:{} {}", ansi::colors::RED, ansi::colors::RESET, e);
                    }
                }
            }
            commands::Command::DeleteSession(session_id) => {
                match session_manager.delete_session(&session_id).await {
                    Ok(_) => {
                        println!("{}✅ Session deleted:{} {}", 
                            ansi::colors::GREEN, ansi::colors::RESET, session_id);
                    }
                    Err(e) => {
                        error!("Failed to delete session: {}", e);
                        println!("{}Error:{} {}", ansi::colors::RED, ansi::colors::RESET, e);
                    }
                }
            }
            commands::Command::SessionInfo => {
                commands::display_session_info(
                    session_manager.current_session_id(),
                    session_manager
                );
            }
            commands::Command::Help => {
                commands::display_help();
            }
            commands::Command::Exit => {
                break;
            }
        }
        
        println!();
    }
    
    Ok(())
}

/// Handle an agent query with streaming response
async fn handle_agent_query(client: &mut ipc::client::IpcClient, query: &str) -> Result<()> {
    println!();
    println!("{}🤖 AI:{} ", ansi::colors::BRIGHT_CYAN, ansi::colors::RESET);
    
    let request = ipc::message::Request::agent_query(client.next_request_id(), query.to_string());
    
    let response = client.send_request(request).await?;
    
    if let Some(result) = response.result {
        if let Some(_query_id) = result.get("query_id").and_then(|v| v.as_str()) {
            // Receive streaming tokens
            let mut _token_count = 0;
            loop {
                let notifications = client.poll_notifications().await?;
                
                if notifications.is_empty() {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    continue;
                }
                
                let mut should_exit = false;
                for notification in &notifications {
                    match notification.method.as_str() {
                        "stream.token" => {
                            if let Some(params) = &notification.params {
                                if let Some(content) = params.get("content").and_then(|v| v.as_str()) {
                                    print!("{}", content);
                                    io::stdout().flush()?;
                                    _token_count += 1;
                                }
                            }
                        }
                        "stream.block" => {
                            if let Some(params) = &notification.params {
                                let block_type = params.get("type").and_then(|v| v.as_str()).unwrap_or("text");
                                let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                let language = params.get("language").and_then(|v| v.as_str()).unwrap_or("text");
                                
                                match block_type {
                                    "code" => {
                                        let formatted = ansi::format_code_block(language, content);
                                        print!("{}", formatted);
                                    }
                                    "diff" => {
                                        let formatted = ansi::format_diff(content);
                                        print!("{}", formatted);
                                    }
                                    _ => {
                                        print!("{}", content);
                                    }
                                }
                                io::stdout().flush()?;
                            }
                        }
                        "tool.request_approval" => {
                            println!("\n");
                            if let Some(params) = &notification.params {
                                let tool_name = params.get("tool_name").and_then(|v| v.as_str()).unwrap_or("unknown");
                                let description = params.get("description").and_then(|v| v.as_str()).unwrap_or("");
                                let risk_level = params.get("risk_level").and_then(|v| v.as_str()).unwrap_or("unknown");
                                let preview = params.get("preview").and_then(|v| v.as_str()).unwrap_or("");
                                let execution_id = params.get("execution_id").and_then(|v| v.as_str()).unwrap_or("");
                                
                                println!("\n{}🔒 Tool Approval Request{}", ansi::colors::YELLOW, ansi::colors::RESET);
                                println!("{}Tool:{} {}", ansi::colors::BRIGHT_WHITE, ansi::colors::RESET, tool_name);
                                println!("{}Description:{} {}", ansi::colors::BRIGHT_WHITE, ansi::colors::RESET, description);
                                println!("{}Risk Level:{} {}{}{}", 
                                    ansi::colors::BRIGHT_WHITE, 
                                    ansi::colors::RESET,
                                    if risk_level == "high" { ansi::colors::RED } else { ansi::colors::YELLOW },
                                    risk_level.to_uppercase(),
                                    ansi::colors::RESET
                                );
                                println!("\n{}Preview:{}", ansi::colors::BRIGHT_WHITE, ansi::colors::RESET);
                                println!("{}", preview);
                                println!("\n{}Approve this action? (y/N):{} ", ansi::colors::BRIGHT_WHITE, ansi::colors::RESET);
                                io::stdout().flush()?;
                                
                                // For demo, auto-approve after 2 seconds
                                println!("\n{}[Auto-approving in demo mode...]{}", ansi::colors::BRIGHT_BLACK, ansi::colors::RESET);
                                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                
                                // Send approval
                                let approve_request = ipc::message::Request::new(
                                    client.next_request_id(),
                                    "tool.approve",
                                    Some(serde_json::json!({
                                        "execution_id": execution_id,
                                        "approved": true
                                    }))
                                );
                                
                                match client.send_request(approve_request).await {
                                    Ok(response) => {
                                        info!("Tool approval response: {:?}", response);
                                        println!("\n{}✅ Tool approved and executed{}", ansi::colors::GREEN, ansi::colors::RESET);
                                        if let Some(result) = response.result {
                                            println!("Result: {}", serde_json::to_string_pretty(&result).unwrap_or_default());
                                        }
                                    }
                                    Err(e) => {
                                        error!("Tool approval failed: {}", e);
                                        println!("❌ Tool approval failed: {}", e);
                                    }
                                }
                            }
                        }
                        "stream.complete" => {
                            println!("\n");
                            should_exit = true;
                        }
                        _ => {
                            info!("Unknown notification: {}", notification.method);
                        }
                    }
                }
                
                if should_exit {
                    break;
                }
            }
        }
    }
    
    Ok(())
}

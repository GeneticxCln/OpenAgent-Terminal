// OpenAgent-Terminal - Main Entry Point
// AI-Native Terminal Emulator combining Portal + OpenAgent

mod ansi;
mod commands;
mod config;
mod error;
mod ipc;
mod line_editor;
mod session;
mod terminal_manager;

use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event},
    execute,
};
use line_editor::{EditorAction, LineEditor};
use log::{error, info};
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::Mutex;

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
/// Now uses raw-mode input with concurrent streaming
async fn run_interactive_loop(
    client: &mut ipc::client::IpcClient,
    session_manager: &mut session::SessionManager,
) -> Result<()> {
    // Create terminal manager (enables raw mode)
    let mut terminal = terminal_manager::TerminalManager::new()?;
    let mut editor = LineEditor::new();
    
    // Wrap client in Arc<Mutex> for concurrent access
    let client = Arc::new(Mutex::new(client));
    
    // Track if we're currently streaming
    let streaming = Arc::new(Mutex::new(false));
    
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
        
        // Render prompt and input line
        let (line, cursor_pos) = editor.render(&prompt);
        terminal.clear_current_line()?;
        print!("{}", line);
        execute!(io::stdout(), cursor::MoveTo(cursor_pos as u16, cursor::position()?.1))?;
        io::stdout().flush()?;
        
        // Wait for keyboard event with timeout (allows checking other state)
        if !event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }
        
        // Read the event
        let event = event::read()?;
        
        match event {
            Event::Key(key_event) => {
                let action = editor.handle_key(key_event.code, key_event.modifiers);
                
                match action {
                    EditorAction::Submit(input) => {
                        println!(); // Move to next line after submission
                        
                        let input = input.trim();
                        if input.is_empty() {
                            continue;
                        }
                        
                        // Add to local history
                        editor.add_to_history(input);
                        
                        // Process command
                        if let Err(e) = process_command_with_streaming(
                            input,
                            Arc::clone(&client),
                            session_manager,
                            Arc::clone(&streaming),
                        ).await {
                            error!("Command failed: {}", e);
                            println!("{}Error:{} {}", ansi::colors::RED, ansi::colors::RESET, e);
                        }
                        
                        // Clear editor for next input
                        editor.clear();
                        println!(); // Extra line for spacing
                    }
                    EditorAction::HistoryUp => {
                        if let Some(cmd) = editor.navigate_up() {
                            editor.set_buffer(cmd);
                        }
                    }
                    EditorAction::HistoryDown => {
                        if let Some(cmd) = editor.navigate_down() {
                            editor.set_buffer(cmd);
                        } else {
                            editor.clear();
                        }
                    }
                    EditorAction::ClearScreen => {
                        terminal.clear_screen()?;
                    }
                    EditorAction::ShowHistory => {
                        println!();
                        let history = editor.get_recent_history(10);
                        if history.is_empty() {
                            println!("{}No history yet{}", ansi::colors::YELLOW, ansi::colors::RESET);
                        } else {
                            println!("{}Recent commands:{}", ansi::colors::CYAN, ansi::colors::RESET);
                            for (i, cmd) in history.iter().enumerate() {
                                println!("  {}. {}", history.len() - i, cmd);
                            }
                        }
                        println!();
                    }
                    EditorAction::Cancel => {
                        // Cancel current input or streaming
                        let mut is_streaming = streaming.lock().await;
                        if *is_streaming {
                            println!("\n{}Cancelling stream...{}", ansi::colors::YELLOW, ansi::colors::RESET);
                            *is_streaming = false;
                            // Note: actual stream cancellation would need a cancellation token
                        } else {
                            editor.clear();
                            println!();
                        }
                    }
                    EditorAction::Exit => {
                        break;
                    }
                    EditorAction::Redraw => {
                        // Will redraw on next loop iteration
                    }
                    _ => {}
                }
            }
            Event::Resize(cols, rows) => {
                info!("Terminal resized to {}x{}", cols, rows);
                // Could send context.update notification here
            }
            _ => {}
        }
    }
    
    // Restore terminal before exiting
    terminal.restore()?;
    Ok(())
}

/// Process a command with non-blocking streaming support
async fn process_command_with_streaming(
    input: &str,
    client: Arc<Mutex<&mut ipc::client::IpcClient>>,
    session_manager: &mut session::SessionManager,
    streaming: Arc<Mutex<bool>>,
) -> Result<()> {
    let command = commands::parse_command(input);
    
    match command {
        commands::Command::Query(query) => {
            if let Err(e) = handle_agent_query_concurrent(Arc::clone(&client), &query, Arc::clone(&streaming)).await {
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
            // Exit will be handled by outer loop
        }
    }
    
    Ok(())
}

/// Handle an agent query with concurrent streaming (non-blocking input)
async fn handle_agent_query_concurrent(
    client: Arc<Mutex<&mut ipc::client::IpcClient>>,
    query: &str,
    streaming: Arc<Mutex<bool>>,
) -> Result<()> {
    println!();
    println!("{}🤖 AI:{} ", ansi::colors::BRIGHT_CYAN, ansi::colors::RESET);
    io::stdout().flush()?;
    
    // Send query request
    let request = {
        let mut client = client.lock().await;
        ipc::message::Request::agent_query(client.next_request_id(), query.to_string())
    };
    
    let response = {
        let mut client = client.lock().await;
        client.send_request(request).await?
    };
    
    if let Some(result) = response.result {
        if let Some(_query_id) = result.get("query_id").and_then(|v| v.as_str()) {
            // Set streaming flag
            *streaming.lock().await = true;
            
            // Receive streaming tokens (await-based, no polling)
            loop {
                // Check if cancelled
                if !*streaming.lock().await {
                    break;
                }
                
                let notification = {
                    let mut client = client.lock().await;
                    client.next_notification().await?
                };
                
                let mut should_exit = false;
                match notification.method.as_str() {
                    "stream.token" => {
                        if let Some(params) = &notification.params {
                            if let Some(content) = params.get("content").and_then(|v| v.as_str()) {
                                print!("{}", content);
                                io::stdout().flush()?;
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
                            let approve_request = {
                                let mut client = client.lock().await;
                                ipc::message::Request::new(
                                    client.next_request_id(),
                                    "tool.approve",
                                    Some(serde_json::json!({
                                        "execution_id": execution_id,
                                        "approved": true
                                    }))
                                )
                            };
                            
                            let approval_result = {
                                let mut client = client.lock().await;
                                client.send_request(approve_request).await
                            };
                            
                            match approval_result {
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
                
                if should_exit {
                    break;
                }
            }
            
            // Clear streaming flag
            *streaming.lock().await = false;
        }
    }
    
    Ok(())
}

// OpenAgent-Terminal - Main Entry Point
// AI-Native Terminal Emulator combining Portal + OpenAgent

mod ansi;
mod cli;
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
    event::{self, Event, KeyCode},
    execute,
};
use line_editor::{EditorAction, LineEditor};
use log::{debug, error, info};
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::{Mutex, watch};

/// Handle --generate-config flag
fn handle_generate_config() -> Result<()> {
    println!("⚙️  Generating default configuration...");
    
    let config_path = config::Config::config_path()?;
    
    // Check if config already exists
    if config_path.exists() {
        println!("⚠️   Configuration file already exists at: {:?}", config_path);
        print!("Overwrite? [y/N]: ");
        std::io::Write::flush(&mut std::io::stdout())?;
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }
    
    config::Config::generate_default()?;
    println!("✅ Configuration generated at: {:?}", config_path);
    println!("📝 Edit the file to customize your settings.");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments first
    let cli = cli::Cli::parse_args();
    
    // Handle --generate-config flag
    if cli.should_generate_config() {
        return handle_generate_config();
    }
    
    // Initialize logging with CLI-specified level
    let log_level = cli.effective_log_level();
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(log_level.to_filter_str())
    ).init();

    info!("🚀 Starting OpenAgent-Terminal v{}", env!("CARGO_PKG_VERSION"));
    info!("📝 Status: Alpha - Early Development");
    
    // Load configuration with CLI precedence: CLI > File > Default
    let mut config = if let Some(config_path) = cli.effective_config_path() {
        // Load from CLI-specified path
        config::Config::load_from(config_path).unwrap_or_else(|e| {
            log::warn!("Failed to load config from CLI path: {}", e);
            log::info!("Using default configuration");
            config::Config::default()
        })
    } else {
        // Load from default path
        config::Config::load().unwrap_or_else(|e| {
            log::warn!("Failed to load config: {}", e);
            log::info!("Using default configuration");
            config::Config::default()
        })
    };
    
    // Apply CLI overrides (highest precedence)
    if let Some(ref model) = cli.model {
        info!("CLI override: model = {}", model);
        config.agent.model = model.clone();
    }
    
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

    // Determine socket path with precedence: CLI > Environment > Default
    let socket_path = cli.effective_socket_path();

    info!("Socket path: {}", socket_path);
    println!("🔌 Connecting to Python backend at: {}", socket_path);
    println!("   (Make sure the Python backend is running!)");
    println!();

    // Create IPC client and session manager
    let mut client = ipc::client::IpcClient::new();

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
                    
                    // Wrap client in Arc<Mutex> for shared ownership
                    let client = Arc::new(Mutex::new(client));
                    
                    // Create session manager with client reference
                    let mut session_manager = session::SessionManager::new(Arc::clone(&client));
                    info!("📝 Session manager connected");
                    
                    // Run interactive loop
                    if let Err(e) = run_interactive_loop(
                        Arc::clone(&client), 
                        &mut session_manager,
                        &config
                    ).await {
                        error!("Interactive loop error: {}", e);
                        println!("{}Error:{} {}", ansi::colors::RED, ansi::colors::RESET, e);
                    }
                    
                    // Disconnect
                    client.lock().await.disconnect().await?;
                }
                Err(e) => {
                    error!("Initialize failed: {}", e);
                    println!("❌ Initialize failed: {}", e);
                    return Err(e.into());
                }
            }
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
/// Now uses raw-mode input with concurrent streaming and UX polish
async fn run_interactive_loop(
    client: Arc<Mutex<ipc::client::IpcClient>>,
    session_manager: &mut session::SessionManager,
    config: &config::Config,
) -> Result<()> {
    // Create terminal manager (enables raw mode)
    let mut terminal = terminal_manager::TerminalManager::new()?;
    let mut editor = LineEditor::new();
    
    // Enter alternate screen buffer for clean UX
    terminal.enter_alternate_screen()?;
    terminal.clear_screen()?;
    
    // Initialize status line
    let status = terminal_manager::StatusInfo {
        connection_state: "Connected".to_string(),
        model: config.agent.model.clone(),
        session_id: session_manager.current_session_id().map(|s| s.to_string()),
    };
    terminal.set_status(status);
    terminal.draw_status_line()?;
    
    // Create cancellation token for stream interruption
    let (cancel_tx, _cancel_rx) = watch::channel(false);
    
    loop {
        // Update status line (in case session changed)
        let status = terminal_manager::StatusInfo {
            connection_state: "Connected".to_string(),
            model: config.agent.model.clone(),
            session_id: session_manager.current_session_id().map(|s| s.to_string()),
        };
        terminal.set_status(status);
        terminal.draw_status_line()?;
        
        // Move to prompt area at bottom
        terminal.move_to_prompt_area()?;
        
        // Show prompt (simpler now that session is in status line)
        let prompt = format!("{}>{} ", ansi::colors::GREEN, ansi::colors::RESET);
        
        // Render prompt and input line
        let (line, cursor_pos) = editor.render(&prompt);
        terminal.clear_current_line()?;
        print!("{}", line);
        io::stdout().flush()?;
        
        // Position cursor correctly
        let current_row = cursor::position()?.1;
        execute!(io::stdout(), cursor::MoveTo(cursor_pos as u16, current_row))?;
        
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
                            &cancel_tx,
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
                    EditorAction::ReverseSearch => {
                        // Start reverse search mode
                        editor.start_reverse_search();
                        println!();
                        println!("{}(reverse-i-search): {}", ansi::colors::CYAN, ansi::colors::RESET);
                        // TODO: Implement full reverse search UI in future iteration
                        editor.exit_reverse_search();
                    }
                    EditorAction::DeleteToStart => {
                        editor.delete_to_start();
                    }
                    EditorAction::DeleteToEnd => {
                        editor.delete_to_end();
                    }
                    EditorAction::DeletePrevWord => {
                        editor.delete_prev_word();
                    }
                    EditorAction::Cancel => {
                        // Cancel by sending cancellation signal
                        if cancel_tx.send(true).is_ok() {
                            println!("\n{}Cancellation signal sent...{}", ansi::colors::YELLOW, ansi::colors::RESET);
                        }
                        editor.clear();
                        println!();
                    }
                    EditorAction::Exit => {
                        break;
                    }
                    EditorAction::Redraw => {
                        // Will redraw on next loop iteration
                    }
                    EditorAction::None => {
                        // No action needed
                    }
                }
            }
            Event::Resize(cols, rows) => {
                info!("📱 Terminal resized to {}x{}", cols, rows);
                
                // Send context.update notification to backend
                let notification = ipc::message::Notification::context_update_terminal_size(cols, rows);
                let mut client_lock = client.lock().await;
                match client_lock.send_notification(notification).await {
                    Ok(_) => {
                        debug!("✅ Sent terminal resize notification to backend");
                    }
                    Err(e) => {
                        error!("❌ Failed to send resize notification: {}", e);
                    }
                }
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
    client: Arc<Mutex<ipc::client::IpcClient>>,
    session_manager: &mut session::SessionManager,
    cancel_tx: &watch::Sender<bool>,
) -> Result<()> {
    let command = commands::parse_command(input);
    
    match command {
        commands::Command::Query(query) => {
            // Reset cancellation before starting
            let _ = cancel_tx.send(false);
            if let Err(e) = handle_agent_query_concurrent(Arc::clone(&client), &query, cancel_tx).await {
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

/// Handle an agent query with concurrent streaming using tokio::select!
async fn handle_agent_query_concurrent(
    client: Arc<Mutex<ipc::client::IpcClient>>,
    query: &str,
    cancel_tx: &watch::Sender<bool>,
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
            // Create cancellation receiver
            let mut cancel_rx = cancel_tx.subscribe();
            
            // Stream handling loop with concurrent select
            loop {
                // Use tokio::select! to handle notifications and cancellation concurrently
                tokio::select! {
                    // Check for cancellation
                    Ok(_) = cancel_rx.changed() => {
                        if *cancel_rx.borrow() {
                            println!("\n{}Stream cancelled by user{}", ansi::colors::YELLOW, ansi::colors::RESET);
                            break;
                        }
                    }
                    
                    // Wait for next notification
                    notification_result = async {
                        let mut client = client.lock().await;
                        client.next_notification().await
                    } => {
                        match notification_result {
                            Ok(notification) => {
                                if let Err(e) = handle_stream_notification(
                                    &notification,
                                    Arc::clone(&client),
                                    cancel_tx,
                                ).await {
                                    error!("Failed to handle notification: {}", e);
                                }
                                
                                // Check if stream is complete
                                if notification.method == "stream.complete" {
                                    println!("\n");
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Notification error: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Handle a single stream notification
async fn handle_stream_notification(
    notification: &ipc::message::Notification,
    client: Arc<Mutex<ipc::client::IpcClient>>,
    cancel_tx: &watch::Sender<bool>,
) -> Result<()> {
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
                
                // Wait for user input with timeout
                let approved = wait_for_approval(cancel_tx).await?;
                
                // Send approval
                let approve_request = {
                    let mut client = client.lock().await;
                    ipc::message::Request::new(
                        client.next_request_id(),
                        "tool.approve",
                        Some(serde_json::json!({
                            "execution_id": execution_id,
                            "approved": approved
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
                        if approved {
                            println!("\n{}✅ Tool approved and executed{}", ansi::colors::GREEN, ansi::colors::RESET);
                        } else {
                            println!("\n{}❌ Tool execution denied{}", ansi::colors::RED, ansi::colors::RESET);
                        }
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
            // Handled in main loop
        }
        _ => {
            info!("Unknown notification: {}", notification.method);
        }
    }
    
    Ok(())
}

/// Wait for user approval input (y/N) with timeout
async fn wait_for_approval(cancel_tx: &watch::Sender<bool>) -> Result<bool> {
    use crossterm::terminal;
    
    // Enable raw mode temporarily for single-key input
    terminal::enable_raw_mode()?;
    
    let mut cancel_rx = cancel_tx.subscribe();
    let result = loop {
        tokio::select! {
            // Check for cancellation
            Ok(_) = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    println!("\n{}Approval cancelled{}", ansi::colors::YELLOW, ansi::colors::RESET);
                    break Ok(false);
                }
            }
            
            // Wait for key press with polling
            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                if event::poll(std::time::Duration::from_millis(10))? {
                    if let Event::Key(key_event) = event::read()? {
                        match key_event.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                println!("y");
                                break Ok(true);
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter | KeyCode::Esc => {
                                println!("n");
                                break Ok(false);
                            }
                            KeyCode::Char('c') if key_event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                                let _ = cancel_tx.send(true);
                                println!("\n{}Cancelled{}", ansi::colors::YELLOW, ansi::colors::RESET);
                                break Ok(false);
                            }
                            _ => {
                                // Ignore other keys
                            }
                        }
                    }
                }
            }
        }
    };
    
    // Restore raw mode state (should already be in raw mode from main loop)
    // We don't disable it here since we're in the middle of the interactive loop
    
    result
}

// OpenAgent-Terminal - Main Entry Point
// AI-Native Terminal Emulator combining Portal + OpenAgent

mod ansi;
mod config;
mod error;
mod ipc;
mod session;

use anyhow::Result;
use log::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🚀 Starting OpenAgent-Terminal v{}", env!("CARGO_PKG_VERSION"));
    info!("📋 Phase 5: Loading configuration...");
    
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
    
    info!("📋 Phase 1: Foundation - IPC Communication");

    // Show welcome message
    println!("╔════════════════════════════════════════════╗");
    println!("║      OpenAgent-Terminal (Alpha)           ║");
    println!("║   AI-Native Terminal Emulator             ║");
    println!("╚════════════════════════════════════════════╝");
    println!();
    println!("✅  Phase 2: Agent Integration - Testing Streaming");
    println!();

    // Determine socket path
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    
    // Note: We'll use a fixed path for testing, or pass it as arg
    // For now, let's use a test path
    let socket_path = std::env::var("OPENAGENT_SOCKET")
        .unwrap_or_else(|_| format!("{}/openagent-terminal-test.sock", runtime_dir));

    info!("Socket path: {}", socket_path);
    println!("🔌 Connecting to Python backend at: {}", socket_path);
    println!("   (Make sure the Python backend is running!)");
    println!();

    // Create IPC client
    let mut client = ipc::client::IpcClient::new();

    // Try to connect
    match client.connect(&socket_path).await {
        Ok(()) => {
            info!("✅ Connected successfully");
            println!("✅ Connected to Python backend");
            println!();

            // Send initialize request
            println!("🚀 Sending initialize request...");
            match client.initialize().await {
                Ok(response) => {
                    info!("Initialize response: {:?}", response);
                    println!("✅ Initialize successful!");
                    println!();
                    
                    if let Some(result) = response.result {
                        println!("Server Info:");
                        println!("  {}", serde_json::to_string_pretty(&result).unwrap_or_default());
                    }
                    
                    println!();
                    println!("✅ Initialize successful - starting agent test...");
                    println!();
                    
                    // Phase 2: Test agent query with streaming
                    println!("🤖 Testing agent query with streaming...");
                    println!();
                    
                    // Send an agent query (ask to write file to test tool approval)
                    let query = "write hello world to test.txt".to_string();
                    println!("👤 User: {}", query);
                    println!();
                    println!("🤖 AI: ");
                    
                    let request = ipc::message::Request::agent_query(
                        client.next_request_id(),
                        query
                    );
                    
                    match client.send_request(request).await {
                        Ok(response) => {
                            info!("Agent query response: {:?}", response);
                            
                            if let Some(result) = response.result {
                                if let Some(_query_id) = result.get("query_id").and_then(|v| v.as_str()) {
                                    println!("[Streaming response from agent...]\n");
                                    
                                    // Receive streaming tokens
                                    let mut token_count = 0;
                                    loop {
                                        let notifications = client.poll_notifications().await?;
                                        
                                        if notifications.is_empty() {
                                            // Small delay to avoid busy waiting
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
                                                            std::io::Write::flush(&mut std::io::stdout())?;
                                                            token_count += 1;
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
                                                        std::io::Write::flush(&mut std::io::stdout())?;
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
                                                        std::io::Write::flush(&mut std::io::stdout())?;
                                                        
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
                                                    if let Some(params) = &notification.params {
                                                        if let Some(status) = params.get("status").and_then(|v| v.as_str()) {
                                                            println!("\n✅ Stream complete (status: {}, tokens: {})", status, token_count);
                                                        }
                                                    }
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
                                    
                                    println!();
                                    println!("✅ Phase 2 Agent Test Complete!");
                                    println!();
                                    println!("Achievements:");
                                    println!("  ✅ Unix socket IPC working");
                                    println!("  ✅ Initialize handshake working");
                                    println!("  ✅ Agent query request working");
                                    println!("  ✅ Token streaming working");
                                    println!("  ✅ {} tokens received", token_count);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Agent query failed: {}", e);
                            println!("❌ Agent query failed: {}", e);
                            return Err(e.into());
                        }
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

    Ok(())
}

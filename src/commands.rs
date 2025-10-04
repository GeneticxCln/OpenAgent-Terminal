// Command parsing and execution
//
// This module handles parsing user input to determine if it's a session command
// or a regular agent query, and executes the appropriate action.

use crate::ansi;
use crate::session::{SessionManager, SessionMetadata};

/// Represents a parsed command from user input
#[derive(Debug, Clone)]
pub enum Command {
    /// Regular agent query
    Query(String),
    /// List all sessions (with optional limit)
    ListSessions(Option<usize>),
    /// Load a specific session by ID
    LoadSession(String),
    /// Export current or specified session
    ExportSession {
        session_id: Option<String>,
        format: String,
        output_file: Option<String>,
    },
    /// Delete a session
    DeleteSession(String),
    /// Show current session info
    SessionInfo,
    /// Show help
    Help,
    /// Exit the application
    Exit,
}

/// Parse user input into a command
pub fn parse_command(input: &str) -> Command {
    let trimmed = input.trim();

    // Check for session commands (start with /)
    if let Some(cmd) = trimmed.strip_prefix('/') {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        
        if parts.is_empty() {
            return Command::Query(input.to_string());
        }

        match parts[0] {
            "list" | "ls" => {
                let limit = parts.get(1).and_then(|s| s.parse::<usize>().ok());
                Command::ListSessions(limit)
            }
            "load" => {
                if parts.len() < 2 {
                    println!("{}Error:{} /load requires a session ID", 
                        ansi::colors::RED, ansi::colors::RESET);
                    println!("Usage: /load <session-id>");
                    return Command::Help;
                }
                Command::LoadSession(parts[1].to_string())
            }
            "export" => {
                let mut session_id = None;
                let mut format = "markdown".to_string();
                let mut output_file = None;

                // Parse arguments: /export [session-id] [--format=markdown] [--output=file.md]
                for part in &parts[1..] {
                    if let Some(fmt) = part.strip_prefix("--format=") {
                        format = fmt.to_string();
                    } else if let Some(file) = part.strip_prefix("--output=") {
                        output_file = Some(file.to_string());
                    } else if !part.starts_with("--") {
                        session_id = Some(part.to_string());
                    }
                }

                Command::ExportSession {
                    session_id,
                    format,
                    output_file,
                }
            }
            "delete" | "rm" => {
                if parts.len() < 2 {
                    println!("{}Error:{} /delete requires a session ID", 
                        ansi::colors::RED, ansi::colors::RESET);
                    println!("Usage: /delete <session-id>");
                    return Command::Help;
                }
                Command::DeleteSession(parts[1].to_string())
            }
            "info" | "current" => Command::SessionInfo,
            "help" | "?" => Command::Help,
            "exit" | "quit" | "q" => Command::Exit,
            _ => {
                println!("{}Unknown command:{} {}", 
                    ansi::colors::YELLOW, ansi::colors::RESET, parts[0]);
                println!("Type /help for available commands");
                Command::Help
            }
        }
    } else if trimmed.is_empty() {
        // Empty input - do nothing
        Command::Help
    } else {
        // Regular agent query
        Command::Query(input.to_string())
    }
}

/// Display a formatted list of sessions
pub fn display_sessions_list(sessions: &[SessionMetadata]) {
    if sessions.is_empty() {
        println!("{}No sessions found.{}", ansi::colors::YELLOW, ansi::colors::RESET);
        println!("Start a conversation to create your first session!");
        return;
    }

    println!("\n{}╔═══════════════════════════════════════════════════════════════════╗{}", 
        ansi::colors::CYAN, ansi::colors::RESET);
    println!("{}║                        Session History                           ║{}", 
        ansi::colors::CYAN, ansi::colors::RESET);
    println!("{}╚═══════════════════════════════════════════════════════════════════╝{}", 
        ansi::colors::CYAN, ansi::colors::RESET);
    println!();

    for (idx, session) in sessions.iter().enumerate() {
        let session_id_short = &session.session_id[..8.min(session.session_id.len())];
        
        println!("{}{}. {}{} {}{}", 
            ansi::colors::BRIGHT_WHITE,
            idx + 1,
            ansi::colors::CYAN,
            session_id_short,
            session.title,
            ansi::colors::RESET
        );
        
        println!("   {}Created:{} {}  {}Messages:{} {}  {}Tokens:{} {}", 
            ansi::colors::BRIGHT_BLACK,
            ansi::colors::RESET,
            session.created_at.format("%Y-%m-%d %H:%M"),
            ansi::colors::BRIGHT_BLACK,
            ansi::colors::RESET,
            session.message_count,
            ansi::colors::BRIGHT_BLACK,
            ansi::colors::RESET,
            session.total_tokens
        );
        println!();
    }

    println!("{}Tip:{} Use /load <session-id> to continue a previous session", 
        ansi::colors::BRIGHT_BLACK, ansi::colors::RESET);
}

/// Display current session info
pub fn display_session_info(session_id: Option<&str>, manager: &SessionManager) {
    println!("\n{}╔═══════════════════════════════════════════════════════════════════╗{}", 
        ansi::colors::CYAN, ansi::colors::RESET);
    println!("{}║                      Current Session Info                        ║{}", 
        ansi::colors::CYAN, ansi::colors::RESET);
    println!("{}╚═══════════════════════════════════════════════════════════════════╝{}", 
        ansi::colors::CYAN, ansi::colors::RESET);
    println!();

    if let Some(id) = session_id {
        println!("{}Session ID:{} {}", 
            ansi::colors::BRIGHT_WHITE, ansi::colors::RESET, id);
        
        if let Some(metadata) = manager.get_cached_metadata(id) {
            println!("{}Title:{} {}", 
                ansi::colors::BRIGHT_WHITE, ansi::colors::RESET, metadata.title);
            println!("{}Created:{} {}", 
                ansi::colors::BRIGHT_WHITE, ansi::colors::RESET, 
                metadata.created_at.format("%Y-%m-%d %H:%M:%S"));
            println!("{}Updated:{} {}", 
                ansi::colors::BRIGHT_WHITE, ansi::colors::RESET, 
                metadata.updated_at.format("%Y-%m-%d %H:%M:%S"));
            println!("{}Messages:{} {}", 
                ansi::colors::BRIGHT_WHITE, ansi::colors::RESET, metadata.message_count);
            println!("{}Total Tokens:{} {}", 
                ansi::colors::BRIGHT_WHITE, ansi::colors::RESET, metadata.total_tokens);
        }
    } else {
        println!("{}No active session{}", ansi::colors::YELLOW, ansi::colors::RESET);
        println!("Start a conversation to create a new session!");
    }
    println!();
}

/// Display help message
pub fn display_help() {
    println!("\n{}╔═══════════════════════════════════════════════════════════════════╗{}", 
        ansi::colors::CYAN, ansi::colors::RESET);
    println!("{}║                      OpenAgent-Terminal Help                     ║{}", 
        ansi::colors::CYAN, ansi::colors::RESET);
    println!("{}╚═══════════════════════════════════════════════════════════════════╝{}", 
        ansi::colors::CYAN, ansi::colors::RESET);
    println!();
    
    println!("{}Session Commands:{}", ansi::colors::BRIGHT_WHITE, ansi::colors::RESET);
    println!("  {}/list [limit]{}", ansi::colors::GREEN, ansi::colors::RESET);
    println!("    List all sessions (or limit to N most recent)");
    println!("    Aliases: /ls");
    println!();
    
    println!("  {}/load <session-id>{}", ansi::colors::GREEN, ansi::colors::RESET);
    println!("    Load and continue a previous session");
    println!();
    
    println!("  {}/export [session-id] [--format=markdown] [--output=file.md]{}", 
        ansi::colors::GREEN, ansi::colors::RESET);
    println!("    Export session to file (defaults to current session, markdown format)");
    println!();
    
    println!("  {}/delete <session-id>{}", ansi::colors::GREEN, ansi::colors::RESET);
    println!("    Delete a session permanently");
    println!("    Aliases: /rm");
    println!();
    
    println!("  {}/info{}", ansi::colors::GREEN, ansi::colors::RESET);
    println!("    Show current session information");
    println!("    Aliases: /current");
    println!();
    
    println!("  {}/help{}", ansi::colors::GREEN, ansi::colors::RESET);
    println!("    Show this help message");
    println!("    Aliases: /?");
    println!();
    
    println!("  {}/exit{}", ansi::colors::GREEN, ansi::colors::RESET);
    println!("    Exit the application");
    println!("    Aliases: /quit, /q");
    println!();
    
    println!("{}Agent Queries:{}", ansi::colors::BRIGHT_WHITE, ansi::colors::RESET);
    println!("  Type anything without a / prefix to send to the AI agent");
    println!("  Example: \"Help me debug this Python code\"");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query() {
        match parse_command("Hello, world!") {
            Command::Query(q) => assert_eq!(q, "Hello, world!"),
            _ => panic!("Expected Query command"),
        }
    }

    #[test]
    fn test_parse_list_sessions() {
        match parse_command("/list") {
            Command::ListSessions(None) => {},
            _ => panic!("Expected ListSessions command"),
        }

        match parse_command("/list 10") {
            Command::ListSessions(Some(10)) => {},
            _ => panic!("Expected ListSessions with limit"),
        }
    }

    #[test]
    fn test_parse_load_session() {
        match parse_command("/load abc123") {
            Command::LoadSession(id) => assert_eq!(id, "abc123"),
            _ => panic!("Expected LoadSession command"),
        }
    }

    #[test]
    fn test_parse_export_session() {
        match parse_command("/export") {
            Command::ExportSession { session_id: None, format, output_file: None } => {
                assert_eq!(format, "markdown");
            },
            _ => panic!("Expected ExportSession command"),
        }

        match parse_command("/export abc123 --format=json --output=out.json") {
            Command::ExportSession { session_id, format, output_file } => {
                assert_eq!(session_id, Some("abc123".to_string()));
                assert_eq!(format, "json");
                assert_eq!(output_file, Some("out.json".to_string()));
            },
            _ => panic!("Expected ExportSession with args"),
        }
    }

    #[test]
    fn test_parse_delete_session() {
        match parse_command("/delete xyz789") {
            Command::DeleteSession(id) => assert_eq!(id, "xyz789"),
            _ => panic!("Expected DeleteSession command"),
        }
    }

    #[test]
    fn test_parse_info() {
        match parse_command("/info") {
            Command::SessionInfo => {},
            _ => panic!("Expected SessionInfo command"),
        }
    }

    #[test]
    fn test_parse_help() {
        match parse_command("/help") {
            Command::Help => {},
            _ => panic!("Expected Help command"),
        }
    }

    #[test]
    fn test_parse_exit() {
        match parse_command("/exit") {
            Command::Exit => {},
            _ => panic!("Expected Exit command"),
        }

        match parse_command("/quit") {
            Command::Exit => {},
            _ => panic!("Expected Exit command"),
        }
    }
}

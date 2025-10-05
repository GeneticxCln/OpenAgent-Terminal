// Terminal Manager - Raw mode control and screen operations
//
// Handles enabling/disabling raw mode and provides terminal control operations.
// Supports alternate screen buffer, status line, and clean streaming output.

use anyhow::Result;
use crossterm::{
    cursor,
    execute, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};

/// Status information for display
pub struct StatusInfo {
    pub connection_state: String,
    pub model: String,
    pub session_id: Option<String>,
}

/// Manages terminal state and provides control operations
pub struct TerminalManager {
    raw_mode_enabled: bool,
    alternate_screen_enabled: bool,
    status_info: Option<StatusInfo>,
}

impl TerminalManager {
    /// Create a new terminal manager and enable raw mode
    pub fn new() -> Result<Self> {
        terminal::enable_raw_mode()?;
        
        Ok(Self {
            raw_mode_enabled: true,
            alternate_screen_enabled: false,
            status_info: None,
        })
    }
    
    /// Enable alternate screen buffer for clean UX
    pub fn enter_alternate_screen(&mut self) -> Result<()> {
        if !self.alternate_screen_enabled {
            execute!(io::stdout(), EnterAlternateScreen)?;
            self.alternate_screen_enabled = true;
        }
        Ok(())
    }
    
    /// Leave alternate screen buffer and restore original screen
    pub fn leave_alternate_screen(&mut self) -> Result<()> {
        if self.alternate_screen_enabled {
            execute!(io::stdout(), LeaveAlternateScreen)?;
            self.alternate_screen_enabled = false;
        }
        Ok(())
    }
    
    /// Clear the entire screen and move cursor to top-left
    pub fn clear_screen(&mut self) -> Result<()> {
        execute!(
            io::stdout(),
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )?;
        Ok(())
    }
    
    /// Update status information
    pub fn set_status(&mut self, status: StatusInfo) {
        self.status_info = Some(status);
    }
    
    /// Draw status line at the top of the screen
    pub fn draw_status_line(&self) -> Result<()> {
        if let Some(status) = &self.status_info {
            let (cols, _) = terminal::size()?;
            
            // Save cursor position
            let current_pos = cursor::position()?;
            
            // Move to top line
            execute!(io::stdout(), cursor::MoveTo(0, 0))?;
            
            // Clear the line
            execute!(io::stdout(), terminal::Clear(ClearType::CurrentLine))?;
            
            // Build status line
            let mut status_parts = Vec::new();
            
            // Connection state with color
            let conn_color = match status.connection_state.as_str() {
                "Connected" => Color::Green,
                "Connecting" => Color::Yellow,
                "Reconnecting" => Color::Yellow,
                "Failed" | "Disconnected" => Color::Red,
                _ => Color::White,
            };
            status_parts.push(format!("● {}", status.connection_state));
            
            // Model
            status_parts.push(format!("🤖 {}", status.model));
            
            // Session ID (short form)
            if let Some(session_id) = &status.session_id {
                let short_id = &session_id[..8.min(session_id.len())];
                status_parts.push(format!("📝 {}", short_id));
            }
            
            let status_line = status_parts.join("  │  ");
            
            // Truncate if too long
            let max_len = (cols as usize).saturating_sub(4);
            let display_status = if status_line.len() > max_len {
                format!("{}...", &status_line[..max_len.saturating_sub(3)])
            } else {
                status_line
            };
            
            // Print with color
            queue!(
                io::stdout(),
                SetForegroundColor(Color::DarkGrey),
                Print(" "),
                SetForegroundColor(conn_color),
                Print(&status_parts[0]),
                ResetColor
            )?;
            
            if status_parts.len() > 1 {
                queue!(
                    io::stdout(),
                    SetForegroundColor(Color::DarkGrey),
                    Print("  │  "),
                    ResetColor,
                    Print(&status_parts[1..].join("  │  "))
                )?;
            }
            
            // Draw separator line
            queue!(
                io::stdout(),
                cursor::MoveTo(0, 1),
                SetForegroundColor(Color::DarkGrey),
                Print("─".repeat(cols as usize)),
                ResetColor
            )?;
            
            // Restore cursor position (adjust for status line)
            execute!(io::stdout(), cursor::MoveTo(current_pos.0, current_pos.1.max(2)))?;
            io::stdout().flush()?;
        }
        Ok(())
    }
    
    /// Get terminal size (cols, rows)
    #[allow(dead_code)]
    pub fn size(&self) -> Result<(u16, u16)> {
        Ok(terminal::size()?)
    }
    
    /// Move cursor to start of current line and clear it
    pub fn clear_current_line(&mut self) -> Result<()> {
        execute!(
            io::stdout(),
            cursor::MoveToColumn(0),
            terminal::Clear(ClearType::CurrentLine)
        )?;
        Ok(())
    }
    
    /// Clear streaming area (below status line, above prompt)
    pub fn clear_streaming_area(&self) -> Result<()> {
        let (cols, rows) = terminal::size()?;
        let current_pos = cursor::position()?;
        
        // Clear from line 2 (below status) to current position
        for row in 2..current_pos.1 {
            execute!(
                io::stdout(),
                cursor::MoveTo(0, row),
                terminal::Clear(ClearType::CurrentLine)
            )?;
        }
        
        // Restore cursor
        execute!(io::stdout(), cursor::MoveTo(current_pos.0, current_pos.1))?;
        Ok(())
    }
    
    /// Move to streaming area (below status line)
    pub fn move_to_streaming_area(&self) -> Result<()> {
        execute!(io::stdout(), cursor::MoveTo(0, 3))?; // Line 3 (0=status, 1=separator, 2=blank)
        Ok(())
    }
    
    /// Move to prompt area (bottom of screen)
    pub fn move_to_prompt_area(&self) -> Result<()> {
        let (_, rows) = terminal::size()?;
        // Reserve last 2 lines for prompt
        execute!(io::stdout(), cursor::MoveTo(0, rows.saturating_sub(2)))?;
        Ok(())
    }
    
    /// Restore terminal to normal mode
    pub fn restore(&mut self) -> Result<()> {
        // Leave alternate screen first if enabled
        if self.alternate_screen_enabled {
            self.leave_alternate_screen()?;
        }
        
        if self.raw_mode_enabled {
            terminal::disable_raw_mode()?;
            self.raw_mode_enabled = false;
        }
        Ok(())
    }
    
    /// Check if raw mode is enabled
    #[allow(dead_code)]
    pub fn is_raw_mode(&self) -> bool {
        self.raw_mode_enabled
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        // Ensure raw mode is disabled on drop
        let _ = self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[ignore] // Skip in CI - requires TTY
    fn test_terminal_manager_creation() {
        // Note: This test will enable/disable raw mode
        let manager = TerminalManager::new();
        assert!(manager.is_ok());
        
        let manager = manager.unwrap();
        assert!(manager.is_raw_mode());
        
        // Drop will restore automatically
    }
    
    #[test]
    #[ignore] // Skip in CI - requires TTY
    fn test_terminal_size() {
        let manager = TerminalManager::new().unwrap();
        let size = manager.size();
        assert!(size.is_ok());
        
        let (cols, rows) = size.unwrap();
        // Terminal should have some reasonable size
        assert!(cols > 0);
        assert!(rows > 0);
    }
}

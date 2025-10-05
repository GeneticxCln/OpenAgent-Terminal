// Terminal Manager - Raw mode control and screen operations
//
// Handles enabling/disabling raw mode and provides terminal control operations.

use anyhow::Result;
use crossterm::{
    cursor,
    execute,
    terminal::{self, ClearType},
};
use std::io;

/// Manages terminal state and provides control operations
pub struct TerminalManager {
    raw_mode_enabled: bool,
}

impl TerminalManager {
    /// Create a new terminal manager and enable raw mode
    pub fn new() -> Result<Self> {
        terminal::enable_raw_mode()?;
        
        Ok(Self {
            raw_mode_enabled: true,
        })
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
    
    /// Restore terminal to normal mode
    pub fn restore(&mut self) -> Result<()> {
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

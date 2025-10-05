// Line Editor - Keyboard input handling with history and cursor control
//
// Provides a line editing experience with cursor movement, history navigation,
// and keyboard shortcuts.

use crossterm::event::{KeyCode, KeyModifiers};
use std::collections::VecDeque;

/// Actions that result from key handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    /// No action needed
    None,
    /// Redraw the current line
    Redraw,
    /// Submit the current line
    Submit(String),
    /// Cancel current input (Ctrl+C)
    Cancel,
    /// Exit application (Ctrl+D on empty line)
    Exit,
    /// Navigate history up (older)
    HistoryUp,
    /// Navigate history down (newer)
    HistoryDown,
    /// Clear screen (Ctrl+K)
    ClearScreen,
    /// Show history list (Ctrl+L)
    ShowHistory,
    /// Reverse search (Ctrl+R) - future enhancement
    ReverseSearch,
}

/// Line editor with cursor and history management
pub struct LineEditor {
    /// Current input buffer
    buffer: String,
    /// Cursor position in buffer (byte index)
    cursor: usize,
    /// Command history (most recent last)
    history: VecDeque<String>,
    /// Current position in history during navigation (None = not navigating)
    history_index: Option<usize>,
    /// Buffer saved before history navigation
    saved_buffer: Option<String>,
    /// Maximum history size
    max_history: usize,
}

impl LineEditor {
    /// Create a new line editor
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            history: VecDeque::new(),
            history_index: None,
            saved_buffer: None,
            max_history: 1000,
        }
    }
    
    /// Create with custom history size
    #[allow(dead_code)]
    pub fn with_history_size(max_history: usize) -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            history: VecDeque::new(),
            history_index: None,
            saved_buffer: None,
            max_history,
        }
    }
    
    /// Handle a key event and return the appropriate action
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> EditorAction {
        match (code, modifiers) {
            // Navigation
            (KeyCode::Left, KeyModifiers::NONE) => {
                if self.cursor > 0 {
                    // Move back by one char boundary
                    let mut new_cursor = self.cursor.saturating_sub(1);
                    while new_cursor > 0 && !self.buffer.is_char_boundary(new_cursor) {
                        new_cursor -= 1;
                    }
                    self.cursor = new_cursor;
                }
                EditorAction::Redraw
            }
            (KeyCode::Right, KeyModifiers::NONE) => {
                if self.cursor < self.buffer.len() {
                    // Move forward by one char boundary
                    let mut new_cursor = (self.cursor + 1).min(self.buffer.len());
                    while new_cursor < self.buffer.len() && !self.buffer.is_char_boundary(new_cursor) {
                        new_cursor += 1;
                    }
                    self.cursor = new_cursor;
                }
                EditorAction::Redraw
            }
            (KeyCode::Home, _) | (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                self.cursor = 0;
                EditorAction::Redraw
            }
            (KeyCode::End, _) | (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                self.cursor = self.buffer.len();
                EditorAction::Redraw
            }
            
            // Editing
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.buffer.insert(self.cursor, c);
                self.cursor += c.len_utf8();
                EditorAction::Redraw
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                if self.cursor > 0 {
                    let mut remove_pos = self.cursor - 1;
                    while remove_pos > 0 && !self.buffer.is_char_boundary(remove_pos) {
                        remove_pos -= 1;
                    }
                    self.buffer.remove(remove_pos);
                    self.cursor = remove_pos;
                }
                EditorAction::Redraw
            }
            (KeyCode::Delete, KeyModifiers::NONE) => {
                if self.cursor < self.buffer.len() {
                    self.buffer.remove(self.cursor);
                }
                EditorAction::Redraw
            }
            
            // History navigation
            (KeyCode::Up, KeyModifiers::NONE) => {
                EditorAction::HistoryUp
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                EditorAction::HistoryDown
            }
            
            // Commands
            (KeyCode::Enter, KeyModifiers::NONE) => {
                let input = self.buffer.clone();
                EditorAction::Submit(input)
            }
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                EditorAction::Cancel
            }
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                if self.buffer.is_empty() {
                    EditorAction::Exit
                } else {
                    // Ctrl+D on non-empty line: delete char at cursor (optional)
                    EditorAction::None
                }
            }
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                EditorAction::ClearScreen
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                EditorAction::ShowHistory
            }
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                EditorAction::ReverseSearch
            }
            
            _ => EditorAction::None,
        }
    }
    
    /// Add a command to history
    pub fn add_to_history(&mut self, command: &str) {
        if command.is_empty() || command.starts_with(char::is_whitespace) {
            return;
        }
        
        // Don't add duplicates of the last command
        if let Some(last) = self.history.back() {
            if last == command {
                return;
            }
        }
        
        self.history.push_back(command.to_string());
        
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }
    
    /// Navigate up in history (older commands)
    pub fn navigate_up(&mut self) -> Option<String> {
        if self.history.is_empty() {
            return None;
        }
        
        if self.history_index.is_none() {
            // Starting navigation - save current buffer
            self.saved_buffer = Some(self.buffer.clone());
            self.history_index = Some(self.history.len());
        }
        
        let idx = self.history_index?;
        if idx > 0 {
            self.history_index = Some(idx - 1);
            Some(self.history[idx - 1].clone())
        } else {
            None
        }
    }
    
    /// Navigate down in history (newer commands)
    pub fn navigate_down(&mut self) -> Option<String> {
        let idx = self.history_index?;
        
        let next_idx = idx + 1;
        if next_idx >= self.history.len() {
            // Reached bottom - restore saved buffer
            self.history_index = None;
            return self.saved_buffer.take();
        }
        
        self.history_index = Some(next_idx);
        Some(self.history[next_idx].clone())
    }
    
    /// Set the buffer content and move cursor to end
    pub fn set_buffer(&mut self, text: String) {
        self.buffer = text;
        self.cursor = self.buffer.len();
    }
    
    /// Get the current buffer
    #[allow(dead_code)]
    pub fn get_buffer(&self) -> &str {
        &self.buffer
    }
    
    /// Clear the buffer and reset cursor
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.history_index = None;
        self.saved_buffer = None;
    }
    
    /// Render the current line with cursor position
    pub fn render(&self, prompt: &str) -> (String, usize) {
        let line = format!("{}{}", prompt, self.buffer);
        let cursor_pos = prompt.len() + self.cursor;
        (line, cursor_pos)
    }
    
    /// Get recent history entries
    pub fn get_recent_history(&self, limit: usize) -> Vec<&str> {
        self.history
            .iter()
            .rev()
            .take(limit)
            .map(|s| s.as_str())
            .collect()
    }
    
    /// Get history size
    #[allow(dead_code)]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }
}

impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_line_editor_creation() {
        let editor = LineEditor::new();
        assert_eq!(editor.get_buffer(), "");
        assert_eq!(editor.cursor, 0);
    }
    
    #[test]
    fn test_character_insertion() {
        let mut editor = LineEditor::new();
        
        let action = editor.handle_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(action, EditorAction::Redraw);
        assert_eq!(editor.get_buffer(), "h");
        
        let action = editor.handle_key(KeyCode::Char('i'), KeyModifiers::NONE);
        assert_eq!(action, EditorAction::Redraw);
        assert_eq!(editor.get_buffer(), "hi");
    }
    
    #[test]
    fn test_backspace() {
        let mut editor = LineEditor::new();
        editor.set_buffer("hello".to_string());
        
        editor.handle_key(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(editor.get_buffer(), "hell");
    }
    
    #[test]
    fn test_submit() {
        let mut editor = LineEditor::new();
        editor.set_buffer("test command".to_string());
        
        let action = editor.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        match action {
            EditorAction::Submit(text) => assert_eq!(text, "test command"),
            _ => panic!("Expected Submit action"),
        }
    }
    
    #[test]
    fn test_history_navigation() {
        let mut editor = LineEditor::new();
        
        editor.add_to_history("command1");
        editor.add_to_history("command2");
        editor.add_to_history("command3");
        
        // Navigate up
        let cmd = editor.navigate_up();
        assert_eq!(cmd, Some("command3".to_string()));
        
        let cmd = editor.navigate_up();
        assert_eq!(cmd, Some("command2".to_string()));
        
        // Navigate down
        let cmd = editor.navigate_down();
        assert_eq!(cmd, Some("command3".to_string()));
    }
    
    #[test]
    fn test_history_no_duplicates() {
        let mut editor = LineEditor::new();
        
        editor.add_to_history("command");
        editor.add_to_history("command");
        
        assert_eq!(editor.history_len(), 1);
    }
    
    #[test]
    fn test_ctrl_d_exit() {
        let mut editor = LineEditor::new();
        
        let action = editor.handle_key(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(action, EditorAction::Exit);
    }
    
    #[test]
    fn test_ctrl_c_cancel() {
        let mut editor = LineEditor::new();
        editor.set_buffer("some text".to_string());
        
        let action = editor.handle_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(action, EditorAction::Cancel);
    }
    
    #[test]
    fn test_cursor_movement() {
        let mut editor = LineEditor::new();
        editor.set_buffer("hello".to_string());
        
        // Move to home
        editor.handle_key(KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(editor.cursor, 0);
        
        // Move to end
        editor.handle_key(KeyCode::End, KeyModifiers::NONE);
        assert_eq!(editor.cursor, 5);
    }
}

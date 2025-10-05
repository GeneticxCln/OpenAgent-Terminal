// Line Editor - Keyboard input handling with history and cursor control
//
// Provides a line editing experience with cursor movement, history navigation,
// and keyboard shortcuts with proper Unicode grapheme cluster support.

use crossterm::event::{KeyCode, KeyModifiers};
use std::collections::VecDeque;
use unicode_segmentation::UnicodeSegmentation;

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
    /// Clear screen (Ctrl+L)
    ClearScreen,
    /// Show history list
    ShowHistory,
    /// Reverse search (Ctrl+R)
    ReverseSearch,
    /// Delete to beginning of line (Ctrl+U)
    DeleteToStart,
    /// Delete to end of line (Ctrl+K)
    DeleteToEnd,
    /// Delete previous word (Ctrl+W)
    DeletePrevWord,
}

/// Line editor with cursor and history management
pub struct LineEditor {
    /// Current input buffer
    buffer: String,
    /// Cursor position in buffer (byte index)
    cursor: usize,
    /// Reverse search mode active
    reverse_search: bool,
    /// Reverse search query
    search_query: String,
    /// Reverse search result index
    search_result_idx: Option<usize>,
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
            reverse_search: false,
            search_query: String::new(),
            search_result_idx: None,
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
            reverse_search: false,
            search_query: String::new(),
            search_result_idx: None,
        }
    }
    
    /// Handle a key event and return the appropriate action
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> EditorAction {
        match (code, modifiers) {
            // Navigation
            (KeyCode::Left, KeyModifiers::NONE) => {
                self.move_cursor_left();
                EditorAction::Redraw
            }
            (KeyCode::Right, KeyModifiers::NONE) => {
                self.move_cursor_right();
                EditorAction::Redraw
            }
            (KeyCode::Left, KeyModifiers::CONTROL) => {
                self.move_word_left();
                EditorAction::Redraw
            }
            (KeyCode::Right, KeyModifiers::CONTROL) => {
                self.move_word_right();
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
                self.delete_grapheme_backward();
                EditorAction::Redraw
            }
            (KeyCode::Delete, KeyModifiers::NONE) => {
                self.delete_grapheme_forward();
                EditorAction::Redraw
            }
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                EditorAction::DeletePrevWord
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                EditorAction::DeleteToStart
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
                EditorAction::DeleteToEnd
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                EditorAction::ClearScreen
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
        self.reverse_search = false;
        self.search_query.clear();
        self.search_result_idx = None;
    }
    
    // === Unicode-aware cursor movement ===
    
    /// Move cursor left by one grapheme cluster
    fn move_cursor_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        
        let graphemes: Vec<(usize, &str)> = self.buffer
            .grapheme_indices(true)
            .collect();
        
        // Find the grapheme before current cursor
        for i in (0..graphemes.len()).rev() {
            if graphemes[i].0 < self.cursor {
                self.cursor = graphemes[i].0;
                break;
            }
        }
    }
    
    /// Move cursor right by one grapheme cluster
    fn move_cursor_right(&mut self) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        
        let graphemes: Vec<(usize, &str)> = self.buffer
            .grapheme_indices(true)
            .collect();
        
        // Find the grapheme after current cursor
        for (idx, _) in graphemes.iter() {
            if *idx > self.cursor {
                self.cursor = *idx;
                return;
            }
        }
        
        // If no grapheme found after cursor, move to end
        self.cursor = self.buffer.len();
    }
    
    /// Move cursor left to the beginning of previous word
    fn move_word_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        
        let words: Vec<(usize, &str)> = self.buffer
            .unicode_word_indices()
            .collect();
        
        // Find the word before current cursor
        for i in (0..words.len()).rev() {
            if words[i].0 < self.cursor {
                self.cursor = words[i].0;
                return;
            }
        }
        
        // If no word found, move to start
        self.cursor = 0;
    }
    
    /// Move cursor right to the beginning of next word
    fn move_word_right(&mut self) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        
        let words: Vec<(usize, &str)> = self.buffer
            .unicode_word_indices()
            .collect();
        
        // Find the word after current cursor
        let mut found_current = false;
        for (idx, word) in words.iter() {
            if *idx > self.cursor {
                self.cursor = *idx;
                return;
            }
            if *idx <= self.cursor && self.cursor < *idx + word.len() {
                found_current = true;
            }
        }
        
        // If at the end of a word or no word found, move to end
        if found_current || words.is_empty() {
            self.cursor = self.buffer.len();
        }
    }
    
    /// Delete one grapheme cluster backward from cursor
    fn delete_grapheme_backward(&mut self) {
        if self.cursor == 0 {
            return;
        }
        
        let graphemes: Vec<(usize, &str)> = self.buffer
            .grapheme_indices(true)
            .collect();
        
        // Find the grapheme before cursor and remove it
        for i in (0..graphemes.len()).rev() {
            let (start, grapheme) = graphemes[i];
            if start < self.cursor {
                let end = start + grapheme.len();
                self.buffer.replace_range(start..end, "");
                self.cursor = start;
                break;
            }
        }
    }
    
    /// Delete one grapheme cluster forward from cursor
    fn delete_grapheme_forward(&mut self) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        
        let graphemes: Vec<(usize, &str)> = self.buffer
            .grapheme_indices(true)
            .collect();
        
        // Find the grapheme at cursor and remove it
        for (start, grapheme) in graphemes.iter() {
            if *start >= self.cursor {
                let end = start + grapheme.len();
                self.buffer.replace_range(*start..end, "");
                break;
            }
        }
    }
    
    /// Delete from cursor to beginning of line
    pub fn delete_to_start(&mut self) {
        if self.cursor > 0 {
            self.buffer.replace_range(0..self.cursor, "");
            self.cursor = 0;
        }
    }
    
    /// Delete from cursor to end of line
    pub fn delete_to_end(&mut self) {
        if self.cursor < self.buffer.len() {
            self.buffer.truncate(self.cursor);
        }
    }
    
    /// Delete previous word (backwards from cursor)
    pub fn delete_prev_word(&mut self) {
        if self.cursor == 0 {
            return;
        }
        
        let words: Vec<(usize, &str)> = self.buffer
            .unicode_word_indices()
            .collect();
        
        // Find the word that contains or is before the cursor
        let mut delete_start = 0;
        
        for (idx, word) in words.iter() {
            let word_end = idx + word.len();
            
            if *idx < self.cursor {
                delete_start = *idx;
                
                // If cursor is after this word's end, this is the word to delete
                if word_end <= self.cursor {
                    continue;
                } else {
                    // Cursor is in the middle of this word
                    break;
                }
            } else {
                break;
            }
        }
        
        // Handle whitespace before cursor
        if delete_start < self.cursor {
            // Check if there's whitespace between delete_start and cursor
            let between = &self.buffer[delete_start..self.cursor];
            if let Some(last_word_start) = between.rfind(|c: char| !c.is_whitespace()) {
                delete_start = delete_start + between[..=last_word_start].rfind(|c: char| c.is_whitespace())
                    .map(|i| i + 1)
                    .unwrap_or(0);
            }
        }
        
        self.buffer.replace_range(delete_start..self.cursor, "");
        self.cursor = delete_start;
    }
    
    // === Reverse search support ===
    
    /// Start reverse search mode
    pub fn start_reverse_search(&mut self) {
        self.reverse_search = true;
        self.search_query.clear();
        self.search_result_idx = None;
    }
    
    /// Exit reverse search mode
    pub fn exit_reverse_search(&mut self) {
        self.reverse_search = false;
        self.search_query.clear();
        self.search_result_idx = None;
    }
    
    /// Check if in reverse search mode
    pub fn is_reverse_search(&self) -> bool {
        self.reverse_search
    }
    
    /// Add character to search query and find next match
    pub fn search_add_char(&mut self, c: char) -> Option<String> {
        self.search_query.push(c);
        self.search_find_next()
    }
    
    /// Remove last character from search query
    pub fn search_backspace(&mut self) -> Option<String> {
        self.search_query.pop();
        if self.search_query.is_empty() {
            self.search_result_idx = None;
            return None;
        }
        self.search_find_next()
    }
    
    /// Find next matching history entry
    fn search_find_next(&mut self) -> Option<String> {
        if self.search_query.is_empty() {
            return None;
        }
        
        let start_idx = self.search_result_idx.map(|i| i + 1).unwrap_or(0);
        
        // Search backwards through history
        for (i, entry) in self.history.iter().enumerate().rev() {
            if i < start_idx {
                continue;
            }
            
            if entry.contains(&self.search_query) {
                self.search_result_idx = Some(i);
                return Some(entry.clone());
            }
        }
        
        None
    }
    
    /// Get current search query
    pub fn get_search_query(&self) -> &str {
        &self.search_query
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
    
    #[test]
    fn test_unicode_emoji_navigation() {
        let mut editor = LineEditor::new();
        // Set buffer with emoji (4-byte UTF-8)
        editor.set_buffer("hello 👋 world".to_string());
        
        // Cursor should be at end (16 bytes total)
        assert_eq!(editor.cursor, 16);
        
        // Move left once - should skip entire emoji
        editor.handle_key(KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(editor.get_buffer().chars().count(), 13);
        
        // Move to home and right through emoji
        editor.handle_key(KeyCode::Home, KeyModifiers::NONE);
        assert_eq!(editor.cursor, 0);
    }
    
    #[test]
    fn test_delete_emoji() {
        let mut editor = LineEditor::new();
        editor.set_buffer("hi👋".to_string());
        
        // Delete emoji with backspace
        editor.handle_key(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(editor.get_buffer(), "hi");
    }
    
    #[test]
    fn test_delete_prev_word() {
        let mut editor = LineEditor::new();
        editor.set_buffer("hello world test".to_string());
        
        // Delete last word
        editor.delete_prev_word();
        assert_eq!(editor.get_buffer(), "hello world ");
        
        // Delete another word
        editor.delete_prev_word();
        assert_eq!(editor.get_buffer(), "hello ");
    }
    
    #[test]
    fn test_delete_to_start() {
        let mut editor = LineEditor::new();
        editor.set_buffer("hello world".to_string());
        
        // Move cursor to middle
        editor.cursor = 5;
        
        // Delete to start
        editor.delete_to_start();
        assert_eq!(editor.get_buffer(), " world");
        assert_eq!(editor.cursor, 0);
    }
    
    #[test]
    fn test_delete_to_end() {
        let mut editor = LineEditor::new();
        editor.set_buffer("hello world".to_string());
        
        // Move cursor to middle
        editor.cursor = 5;
        
        // Delete to end
        editor.delete_to_end();
        assert_eq!(editor.get_buffer(), "hello");
        assert_eq!(editor.cursor, 5);
    }
    
    #[test]
    fn test_ctrl_w_delete_word() {
        let mut editor = LineEditor::new();
        editor.set_buffer("test command here".to_string());
        
        let action = editor.handle_key(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(action, EditorAction::DeletePrevWord);
    }
    
    #[test]
    fn test_ctrl_u_delete_to_start() {
        let mut editor = LineEditor::new();
        editor.set_buffer("some text".to_string());
        
        let action = editor.handle_key(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(action, EditorAction::DeleteToStart);
    }
    
    #[test]
    fn test_ctrl_k_delete_to_end() {
        let mut editor = LineEditor::new();
        editor.set_buffer("some text".to_string());
        editor.cursor = 4;
        
        let action = editor.handle_key(KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(action, EditorAction::DeleteToEnd);
    }
    
    #[test]
    fn test_word_movement() {
        let mut editor = LineEditor::new();
        editor.set_buffer("one two three".to_string());
        
        // Move to start
        editor.cursor = 0;
        
        // Move right by word
        editor.handle_key(KeyCode::Right, KeyModifiers::CONTROL);
        assert!(editor.cursor == 4 || editor.cursor == 0); // Should be at "two"
        
        // Move to end and left by word
        editor.cursor = editor.buffer.len();
        editor.handle_key(KeyCode::Left, KeyModifiers::CONTROL);
        assert!(editor.cursor < editor.buffer.len());
    }
    
    #[test]
    fn test_reverse_search_mode() {
        let mut editor = LineEditor::new();
        
        // Add some history
        editor.add_to_history("first command");
        editor.add_to_history("second command");
        editor.add_to_history("third test");
        
        // Enter reverse search
        let action = editor.handle_key(KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(action, EditorAction::ReverseSearch);
        
        // Start search
        editor.start_reverse_search();
        assert!(editor.is_reverse_search());
        
        // Exit search
        editor.exit_reverse_search();
        assert!(!editor.is_reverse_search());
    }
    
    #[test]
    fn test_grapheme_cluster_deletion() {
        let mut editor = LineEditor::new();
        // Use a combining character sequence: e + acute accent
        editor.set_buffer("café".to_string());
        
        // The é might be composed of 'e' + combining acute
        let initial_len = editor.get_buffer().len();
        
        // Delete last character(s)
        editor.delete_grapheme_backward();
        
        // Should have deleted the entire grapheme
        assert!(editor.get_buffer().len() < initial_len);
        assert_eq!(editor.get_buffer(), "caf");
    }
}

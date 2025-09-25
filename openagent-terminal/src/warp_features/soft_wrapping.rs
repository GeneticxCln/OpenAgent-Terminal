use crate::display::SizeInfo;
use std::cmp::{max, min};

/// Warp-style soft wrapping for input editor with horizontal scrolling
pub struct SoftWrappingEditor {
    content: String,
    cursor_pos: usize,
    horizontal_scroll: usize,
    viewport_width: usize,
    soft_wrap_enabled: bool,
    auto_scroll_margin: usize,
    wrapped_lines: Vec<WrappedLine>,
}

#[derive(Debug, Clone)]
pub struct WrappedLine {
    /// Original line index
    pub line_index: usize,
    /// Start position in the original line
    pub start_pos: usize,
    /// End position in the original line
    pub end_pos: usize,
    /// Visual line content (may be truncated)
    pub content: String,
    /// Whether this is a continuation of a wrapped line
    pub is_continuation: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WrapBehavior {
    /// Treat soft-wrapped lines as logical units (for TRIPLE-CLICK)
    Logical,
    /// Treat soft-wrapped lines as separate visual lines (for UP/DOWN)
    Visual,
}

impl SoftWrappingEditor {
    pub fn new(viewport_width: usize) -> Self {
        Self {
            content: String::new(),
            cursor_pos: 0,
            horizontal_scroll: 0,
            viewport_width,
            soft_wrap_enabled: true,
            auto_scroll_margin: 10,
            wrapped_lines: Vec::new(),
        }
    }

    /// Set the content of the editor
    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.cursor_pos = min(self.cursor_pos, self.content.len());
        self.update_wrapping();
        self.ensure_cursor_visible();
    }

    /// Get the current content
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Set cursor position
    pub fn set_cursor_pos(&mut self, pos: usize) {
        self.cursor_pos = min(pos, self.content.len());
        self.ensure_cursor_visible();
    }

    /// Get current cursor position
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Update viewport width (called when terminal is resized)
    pub fn set_viewport_width(&mut self, width: usize) {
        self.viewport_width = width;
        self.update_wrapping();
        self.ensure_cursor_visible();
    }

    /// Enable or disable soft wrapping
    pub fn set_soft_wrap_enabled(&mut self, enabled: bool) {
        self.soft_wrap_enabled = enabled;
        self.update_wrapping();
    }

    /// Get visible content for rendering
    pub fn get_visible_content(&self) -> &[WrappedLine] {
        &self.wrapped_lines
    }

    /// Get the visible portion of a line considering horizontal scroll
    pub fn get_visible_line_portion(&self, line: &str) -> String {
        if !self.soft_wrap_enabled {
            // With soft wrapping disabled, use horizontal scrolling
            let start = min(self.horizontal_scroll, line.len());
            let end = min(start + self.viewport_width, line.len());
            line[start..end].to_string()
        } else {
            line.to_string()
        }
    }

    /// Move cursor up with proper soft-wrap handling
    pub fn move_up(&mut self, behavior: WrapBehavior) -> bool {
        match behavior {
            WrapBehavior::Visual => self.move_up_visual(),
            WrapBehavior::Logical => self.move_up_logical(),
        }
    }

    /// Move cursor down with proper soft-wrap handling
    pub fn move_down(&mut self, behavior: WrapBehavior) -> bool {
        match behavior {
            WrapBehavior::Visual => self.move_down_visual(),
            WrapBehavior::Logical => self.move_down_logical(),
        }
    }

    /// Triple-click selection (logical line behavior)
    pub fn select_logical_line(&self) -> (usize, usize) {
        let lines: Vec<&str> = self.content.lines().collect();
        let mut current_pos = 0;
        
        for line in lines {
            let line_end = current_pos + line.len();
            if self.cursor_pos >= current_pos && self.cursor_pos <= line_end {
                return (current_pos, line_end);
            }
            current_pos = line_end + 1; // +1 for newline
        }
        
        (0, self.content.len())
    }

    /// Get cursor position in visual coordinates
    pub fn get_visual_cursor_pos(&self) -> (usize, usize) {
        let (line_idx, col) = self.get_logical_cursor_pos();
        
        if self.soft_wrap_enabled {
            // Find which wrapped line contains the cursor
            for (visual_line_idx, wrapped_line) in self.wrapped_lines.iter().enumerate() {
                if wrapped_line.line_index == line_idx && 
                   col >= wrapped_line.start_pos && col <= wrapped_line.end_pos {
                    let visual_col = col - wrapped_line.start_pos;
                    return (visual_line_idx, visual_col);
                }
            }
        }
        
        (line_idx, col)
    }

    /// Get cursor position in logical coordinates
    pub fn get_logical_cursor_pos(&self) -> (usize, usize) {
        let lines: Vec<&str> = self.content.lines().collect();
        let mut current_pos = 0;
        
        for (line_idx, line) in lines.iter().enumerate() {
            let line_end = current_pos + line.len();
            if self.cursor_pos >= current_pos && self.cursor_pos <= line_end {
                return (line_idx, self.cursor_pos - current_pos);
            }
            current_pos = line_end + 1; // +1 for newline
        }
        
        (lines.len().saturating_sub(1), 0)
    }

    /// Insert text at cursor position
    pub fn insert_text(&mut self, text: &str) {
        self.content.insert_str(self.cursor_pos, text);
        self.cursor_pos += text.len();
        self.update_wrapping();
        self.ensure_cursor_visible();
    }

    /// Delete character before cursor
    pub fn delete_backward(&mut self) -> bool {
        if self.cursor_pos > 0 {
            self.content.remove(self.cursor_pos - 1);
            self.cursor_pos -= 1;
            self.update_wrapping();
            self.ensure_cursor_visible();
            true
        } else {
            false
        }
    }

    /// Delete character after cursor
    pub fn delete_forward(&mut self) -> bool {
        if self.cursor_pos < self.content.len() {
            self.content.remove(self.cursor_pos);
            self.update_wrapping();
            self.ensure_cursor_visible();
            true
        } else {
            false
        }
    }

    fn update_wrapping(&mut self) {
        self.wrapped_lines.clear();
        
        if !self.soft_wrap_enabled {
            // No wrapping - treat each line as a single wrapped line
            for (line_idx, line) in self.content.lines().enumerate() {
                self.wrapped_lines.push(WrappedLine {
                    line_index: line_idx,
                    start_pos: 0,
                    end_pos: line.len(),
                    content: line.to_string(),
                    is_continuation: false,
                });
            }
            return;
        }

        // Wrap lines based on viewport width
        for (line_idx, line) in self.content.lines().enumerate() {
            if line.len() <= self.viewport_width {
                // Line fits in viewport
                self.wrapped_lines.push(WrappedLine {
                    line_index: line_idx,
                    start_pos: 0,
                    end_pos: line.len(),
                    content: line.to_string(),
                    is_continuation: false,
                });
            } else {
                // Line needs wrapping
                let mut start_pos = 0;
                let mut is_first = true;
                
                while start_pos < line.len() {
                    let end_pos = min(start_pos + self.viewport_width, line.len());
                    let content = line[start_pos..end_pos].to_string();
                    
                    self.wrapped_lines.push(WrappedLine {
                        line_index: line_idx,
                        start_pos,
                        end_pos,
                        content,
                        is_continuation: !is_first,
                    });
                    
                    start_pos = end_pos;
                    is_first = false;
                }
            }
        }
    }

    fn ensure_cursor_visible(&mut self) {
        if !self.soft_wrap_enabled {
            // Horizontal scrolling mode
            let (_, col) = self.get_logical_cursor_pos();
            
            // Scroll left if cursor is off screen to the left
            if col < self.horizontal_scroll {
                self.horizontal_scroll = col.saturating_sub(self.auto_scroll_margin);
            }
            
            // Scroll right if cursor is off screen to the right
            let visible_end = self.horizontal_scroll + self.viewport_width;
            if col >= visible_end {
                self.horizontal_scroll = col + self.auto_scroll_margin - self.viewport_width;
            }
        }
    }

    fn move_up_visual(&mut self) -> bool {
        let (visual_line, visual_col) = self.get_visual_cursor_pos();
        
        if visual_line > 0 {
            let target_line = &self.wrapped_lines[visual_line - 1];
            let target_col = min(visual_col, target_line.content.len());
            
            // Convert back to logical position
            let logical_pos = self.visual_to_logical_pos(visual_line - 1, target_col);
            self.set_cursor_pos(logical_pos);
            true
        } else {
            false
        }
    }

    fn move_down_visual(&mut self) -> bool {
        let (visual_line, visual_col) = self.get_visual_cursor_pos();
        
        if visual_line < self.wrapped_lines.len() - 1 {
            let target_line = &self.wrapped_lines[visual_line + 1];
            let target_col = min(visual_col, target_line.content.len());
            
            // Convert back to logical position
            let logical_pos = self.visual_to_logical_pos(visual_line + 1, target_col);
            self.set_cursor_pos(logical_pos);
            true
        } else {
            false
        }
    }

    fn move_up_logical(&mut self) -> bool {
        let (logical_line, logical_col) = self.get_logical_cursor_pos();
        
        if logical_line > 0 {
            let lines: Vec<&str> = self.content.lines().collect();
            let target_line = lines[logical_line - 1];
            let target_col = min(logical_col, target_line.len());
            
            // Calculate logical position
            let mut pos = 0;
            for i in 0..(logical_line - 1) {
                pos += lines[i].len() + 1; // +1 for newline
            }
            pos += target_col;
            
            self.set_cursor_pos(pos);
            true
        } else {
            false
        }
    }

    fn move_down_logical(&mut self) -> bool {
        let (logical_line, logical_col) = self.get_logical_cursor_pos();
        let lines: Vec<&str> = self.content.lines().collect();
        
        if logical_line < lines.len() - 1 {
            let target_line = lines[logical_line + 1];
            let target_col = min(logical_col, target_line.len());
            
            // Calculate logical position
            let mut pos = 0;
            for i in 0..=logical_line {
                pos += lines[i].len() + 1; // +1 for newline
            }
            pos += target_col;
            
            self.set_cursor_pos(min(pos, self.content.len()));
            true
        } else {
            false
        }
    }

    fn visual_to_logical_pos(&self, visual_line: usize, visual_col: usize) -> usize {
        if let Some(wrapped_line) = self.wrapped_lines.get(visual_line) {
            let lines: Vec<&str> = self.content.lines().collect();
            let mut logical_pos = 0;
            
            // Add lengths of all previous logical lines
            for i in 0..wrapped_line.line_index {
                logical_pos += lines[i].len() + 1; // +1 for newline
            }
            
            // Add position within the current logical line
            logical_pos += wrapped_line.start_pos + visual_col;
            
            min(logical_pos, self.content.len())
        } else {
            self.content.len()
        }
    }

    /// Get horizontal scroll position (for debugging/status)
    pub fn horizontal_scroll(&self) -> usize {
        self.horizontal_scroll
    }

    /// Check if soft wrapping is enabled
    pub fn is_soft_wrap_enabled(&self) -> bool {
        self.soft_wrap_enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soft_wrapping_basic() {
        let mut editor = SoftWrappingEditor::new(10);
        editor.set_content("This is a very long line that should be wrapped".to_string());
        
        let wrapped = editor.get_visible_content();
        assert!(wrapped.len() > 1); // Should be wrapped into multiple lines
        
        // First line should not be a continuation
        assert!(!wrapped[0].is_continuation);
        // Second line should be a continuation
        if wrapped.len() > 1 {
            assert!(wrapped[1].is_continuation);
        }
    }

    #[test]
    fn test_horizontal_scrolling() {
        let mut editor = SoftWrappingEditor::new(10);
        editor.set_soft_wrap_enabled(false);
        editor.set_content("This is a very long line".to_string());
        
        // Move cursor to end of line
        editor.set_cursor_pos(20);
        
        // Should have horizontal scrolling
        assert!(editor.horizontal_scroll() > 0);
    }

    #[test]
    fn test_cursor_movement_visual() {
        let mut editor = SoftWrappingEditor::new(5);
        editor.set_content("Hello\nWorld".to_string());
        
        // Move down visually
        editor.set_cursor_pos(2); // In "Hello"
        let moved = editor.move_down(WrapBehavior::Visual);
        assert!(moved);
    }

    #[test]
    fn test_cursor_movement_logical() {
        let mut editor = SoftWrappingEditor::new(5);
        editor.set_content("Hello\nWorld".to_string());
        
        // Move down logically
        editor.set_cursor_pos(2); // In "Hello"
        let moved = editor.move_down(WrapBehavior::Logical);
        assert!(moved);
        
        // Should move to "World" line
        let (line, _) = editor.get_logical_cursor_pos();
        assert_eq!(line, 1);
    }

    #[test]
    fn test_triple_click_selection() {
        let mut editor = SoftWrappingEditor::new(5);
        editor.set_content("Line1\nLine2\nLine3".to_string());
        
        // Position cursor in middle line
        editor.set_cursor_pos(8); // In "Line2"
        
        let (start, end) = editor.select_logical_line();
        let selected = &editor.content()[start..end];
        assert_eq!(selected, "Line2");
    }

    #[test]
    fn test_text_editing() {
        let mut editor = SoftWrappingEditor::new(10);
        editor.set_content("Hello".to_string());
        editor.set_cursor_pos(5);
        
        editor.insert_text(" World");
        assert_eq!(editor.content(), "Hello World");
        
        editor.delete_backward();
        assert_eq!(editor.content(), "Hello Worl");
    }
}
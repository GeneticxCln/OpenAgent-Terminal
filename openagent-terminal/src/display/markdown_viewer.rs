use crate::display::SizeInfo;
use crate::term::Term;
use alacritty_terminal::event::{Event as TerminalEvent, EventListener};
use std::path::Path;

/// Warp-style markdown viewer with built-in rendering
pub struct MarkdownViewer {
    content: String,
    rendered_lines: Vec<String>,
    scroll_offset: usize,
    clickable_regions: Vec<ClickableRegion>,
}

#[derive(Clone)]
pub struct ClickableRegion {
    pub line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub code_snippet: String,
    pub language: Option<String>,
}

impl MarkdownViewer {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            rendered_lines: Vec::new(),
            scroll_offset: 0,
            clickable_regions: Vec::new(),
        }
    }

    /// Load markdown content from file
    pub fn load_from_file(&mut self, path: &Path) -> Result<(), std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        self.set_content(content);
        Ok(())
    }

    /// Set markdown content directly
    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.render_markdown();
    }

    /// Render markdown to terminal-friendly format with clickable code blocks
    fn render_markdown(&mut self) {
        self.rendered_lines.clear();
        self.clickable_regions.clear();

        let mut current_line = 0;
        let mut in_code_block = false;
        let mut code_language: Option<String> = None;
        let mut code_content = String::new();

        for line in self.content.lines() {
            if line.starts_with("```") {
                if in_code_block {
                    // End of code block - create clickable region
                    if !code_content.trim().is_empty() {
                        let region = ClickableRegion {
                            line: current_line - code_content.lines().count(),
                            start_col: 0,
                            end_col: code_content.lines().map(|l| l.len()).max().unwrap_or(0),
                            code_snippet: code_content.trim().to_string(),
                            language: code_language.clone(),
                        };
                        self.clickable_regions.push(region);
                    }
                    
                    self.rendered_lines.push("```".to_string());
                    code_content.clear();
                    code_language = None;
                    in_code_block = false;
                } else {
                    // Start of code block
                    let lang = line.strip_prefix("```").unwrap_or("").trim();
                    code_language = if lang.is_empty() { None } else { Some(lang.to_string()) };
                    self.rendered_lines.push(format!("┌─ {} ─┐ [Click to execute]", 
                        code_language.as_deref().unwrap_or("code")));
                    in_code_block = true;
                }
            } else if in_code_block {
                // Inside code block
                code_content.push_str(line);
                code_content.push('\n');
                self.rendered_lines.push(format!("│ {}", line));
            } else {
                // Regular markdown content
                let rendered = self.render_markdown_line(line);
                self.rendered_lines.push(rendered);
            }
            current_line += 1;
        }
    }

    /// Render a single markdown line
    fn render_markdown_line(&self, line: &str) -> String {
        let mut result = line.to_string();
        
        // Headers
        if line.starts_with("# ") {
            result = format!("\x1b[1;36m{}\x1b[0m", &line[2..]);
        } else if line.starts_with("## ") {
            result = format!("\x1b[1;35m{}\x1b[0m", &line[3..]);
        } else if line.starts_with("### ") {
            result = format!("\x1b[1;33m{}\x1b[0m", &line[4..]);
        }
        
        // Bold text
        result = result.replace("**", "\x1b[1m").replace("**", "\x1b[0m");
        
        // Italic text  
        result = result.replace("*", "\x1b[3m").replace("*", "\x1b[0m");
        
        // Inline code
        if result.contains("`") {
            result = result.replace("`", "\x1b[47;30m").replace("`", "\x1b[0m");
        }

        // Links - make them clickable
        if result.contains("[") && result.contains("]") && result.contains("(") && result.contains(")") {
            // Simple regex-like replacement for [text](url)
            result = result.replace("[", "\x1b[34;4m").replace("](", "\x1b[0m (").replace(")", ")");
        }

        result
    }

    /// Handle mouse click on markdown content
    pub fn handle_click(&self, line: usize, col: usize) -> Option<String> {
        for region in &self.clickable_regions {
            if line >= region.line && 
               line < region.line + region.code_snippet.lines().count() &&
               col >= region.start_col && col <= region.end_col {
                return Some(region.code_snippet.clone());
            }
        }
        None
    }

    /// Get rendered lines for display
    pub fn get_rendered_lines(&self) -> &[String] {
        &self.rendered_lines
    }

    /// Scroll up
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Scroll down  
    pub fn scroll_down(&mut self, lines: usize) {
        let max_scroll = self.rendered_lines.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
    }

    /// Get current scroll offset
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Get visible lines for current viewport
    pub fn get_visible_lines(&self, viewport_height: usize) -> &[String] {
        let start = self.scroll_offset;
        let end = (start + viewport_height).min(self.rendered_lines.len());
        &self.rendered_lines[start..end]
    }
}

impl Default for MarkdownViewer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_rendering() {
        let mut viewer = MarkdownViewer::new();
        let content = r#"
# Main Title
## Subtitle
Regular text with **bold** and *italic*.

```bash
echo "Hello, World!"
ls -la
```

Some `inline code` here.
"#;
        
        viewer.set_content(content.to_string());
        let lines = viewer.get_rendered_lines();
        
        assert!(!lines.is_empty());
        // Should have clickable regions for the bash code block
        assert!(!viewer.clickable_regions.is_empty());
    }

    #[test]
    fn test_code_block_clicking() {
        let mut viewer = MarkdownViewer::new();
        let content = r#"
```bash
echo "test"
```
"#;
        
        viewer.set_content(content.to_string());
        
        // Should be able to click on the code block
        let clicked_code = viewer.handle_click(1, 5);
        assert!(clicked_code.is_some());
        assert_eq!(clicked_code.unwrap(), r#"echo "test""#);
    }
}
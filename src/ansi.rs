// ANSI Color Utilities for Terminal Output
//
// Provides simple syntax highlighting using ANSI escape codes.
// This is Phase 3 - later we'll use GPU rendering with syntect.

use crossterm::terminal;

/// Get the current terminal width, clamped to reasonable bounds
fn get_terminal_width() -> usize {
    match terminal::size() {
        Ok((cols, _rows)) => {
            // Clamp between 40 (minimum) and 200 (maximum)
            // Subtract 2 for border characters and padding
            (cols as usize).clamp(40, 200).saturating_sub(2)
        }
        Err(_) => 78, // Default to 78 if terminal size detection fails
    }
}

/// ANSI color codes
#[allow(dead_code)] // Many colors defined for future use
pub mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    
    // Foreground colors
    pub const BLACK: &str = "\x1b[30m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";
    
    // Bright foreground colors
    pub const BRIGHT_BLACK: &str = "\x1b[90m";
    pub const BRIGHT_RED: &str = "\x1b[91m";
    pub const BRIGHT_GREEN: &str = "\x1b[92m";
    pub const BRIGHT_YELLOW: &str = "\x1b[93m";
    pub const BRIGHT_BLUE: &str = "\x1b[94m";
    pub const BRIGHT_MAGENTA: &str = "\x1b[95m";
    pub const BRIGHT_CYAN: &str = "\x1b[96m";
    pub const BRIGHT_WHITE: &str = "\x1b[97m";
    
    // Background colors
    pub const BG_BLACK: &str = "\x1b[40m";
    pub const BG_BLUE: &str = "\x1b[44m";
}

/// Simple syntax highlighter using regex and ANSI colors
pub struct SyntaxHighlighter;

impl SyntaxHighlighter {
    /// Highlight code based on language
    pub fn highlight(code: &str, language: &str) -> String {
        match language.to_lowercase().as_str() {
            "rust" => Self::highlight_rust(code),
            "python" => Self::highlight_python(code),
            "javascript" | "typescript" | "js" | "ts" => Self::highlight_javascript(code),
            "bash" | "sh" => Self::highlight_bash(code),
            "json" => Self::highlight_json(code),
            _ => code.to_string(), // No highlighting for unknown languages
        }
    }
    
    fn highlight_rust(code: &str) -> String {
        let mut result = String::new();
        
        for line in code.lines() {
            let mut highlighted_line = line.to_string();
            
            // Keywords
            for keyword in &["fn", "let", "mut", "impl", "struct", "enum", "pub", "use", 
                            "async", "await", "match", "if", "else", "for", "while", "return"] {
                highlighted_line = highlighted_line.replace(
                    &format!(" {} ", keyword),
                    &format!(" {}{}{} ", colors::MAGENTA, keyword, colors::RESET)
                );
                // Handle keyword at start of line
                if highlighted_line.starts_with(keyword) {
                    highlighted_line = format!("{}{}{}", colors::MAGENTA, keyword, 
                                              &highlighted_line[keyword.len()..]);
                }
            }
            
            // String literals
            if highlighted_line.contains('"') {
                highlighted_line = Self::highlight_strings(&highlighted_line);
            }
            
            // Comments
            if highlighted_line.contains("//") {
                if let Some(pos) = highlighted_line.find("//") {
                    let (code_part, comment_part) = highlighted_line.split_at(pos);
                    highlighted_line = format!("{}{}{}{}", 
                                              code_part, 
                                              colors::BRIGHT_BLACK, 
                                              comment_part, 
                                              colors::RESET);
                }
            }
            
            result.push_str(&highlighted_line);
            result.push('\n');
        }
        
        result
    }
    
    fn highlight_python(code: &str) -> String {
        let mut result = String::new();
        
        for line in code.lines() {
            let mut highlighted_line = line.to_string();
            
            // Keywords
            for keyword in &["def", "class", "import", "from", "return", "if", "else", 
                            "elif", "for", "while", "async", "await", "with", "as"] {
                highlighted_line = highlighted_line.replace(
                    &format!(" {} ", keyword),
                    &format!(" {}{}{} ", colors::MAGENTA, keyword, colors::RESET)
                );
                if highlighted_line.starts_with(keyword) {
                    highlighted_line = format!("{}{}{}", colors::MAGENTA, keyword, 
                                              &highlighted_line[keyword.len()..]);
                }
            }
            
            // String literals
            if highlighted_line.contains('"') || highlighted_line.contains('\'') {
                highlighted_line = Self::highlight_strings(&highlighted_line);
            }
            
            // Comments
            if highlighted_line.contains('#') {
                if let Some(pos) = highlighted_line.find('#') {
                    let (code_part, comment_part) = highlighted_line.split_at(pos);
                    highlighted_line = format!("{}{}{}{}", 
                                              code_part, 
                                              colors::BRIGHT_BLACK, 
                                              comment_part, 
                                              colors::RESET);
                }
            }
            
            result.push_str(&highlighted_line);
            result.push('\n');
        }
        
        result
    }
    
    fn highlight_javascript(code: &str) -> String {
        let mut result = String::new();
        
        for line in code.lines() {
            let mut highlighted_line = line.to_string();
            
            // Keywords
            for keyword in &["function", "const", "let", "var", "return", "if", "else", 
                            "for", "while", "async", "await", "class", "import", "export"] {
                highlighted_line = highlighted_line.replace(
                    &format!(" {} ", keyword),
                    &format!(" {}{}{} ", colors::MAGENTA, keyword, colors::RESET)
                );
            }
            
            // String literals
            if highlighted_line.contains('"') || highlighted_line.contains('\'') {
                highlighted_line = Self::highlight_strings(&highlighted_line);
            }
            
            // Comments
            if highlighted_line.contains("//") {
                if let Some(pos) = highlighted_line.find("//") {
                    let (code_part, comment_part) = highlighted_line.split_at(pos);
                    highlighted_line = format!("{}{}{}{}", 
                                              code_part, 
                                              colors::BRIGHT_BLACK, 
                                              comment_part, 
                                              colors::RESET);
                }
            }
            
            result.push_str(&highlighted_line);
            result.push('\n');
        }
        
        result
    }
    
    fn highlight_bash(code: &str) -> String {
        let mut result = String::new();
        
        for line in code.lines() {
            let mut highlighted_line = line.to_string();
            
            // Keywords
            for keyword in &["if", "then", "else", "fi", "for", "do", "done", "while", 
                            "case", "esac", "function"] {
                highlighted_line = highlighted_line.replace(
                    &format!(" {} ", keyword),
                    &format!(" {}{}{} ", colors::MAGENTA, keyword, colors::RESET)
                );
            }
            
            // String literals
            if highlighted_line.contains('"') || highlighted_line.contains('\'') {
                highlighted_line = Self::highlight_strings(&highlighted_line);
            }
            
            // Comments
            if highlighted_line.contains('#') {
                if let Some(pos) = highlighted_line.find('#') {
                    let (code_part, comment_part) = highlighted_line.split_at(pos);
                    highlighted_line = format!("{}{}{}{}", 
                                              code_part, 
                                              colors::BRIGHT_BLACK, 
                                              comment_part, 
                                              colors::RESET);
                }
            }
            
            result.push_str(&highlighted_line);
            result.push('\n');
        }
        
        result
    }
    
    fn highlight_json(code: &str) -> String {
        let mut result = String::new();
        
        for line in code.lines() {
            let mut highlighted_line = line.to_string();
            
            // Keys (strings before :)
            // Numbers
            // Booleans
            highlighted_line = highlighted_line.replace("true", 
                &format!("{}true{}", colors::CYAN, colors::RESET));
            highlighted_line = highlighted_line.replace("false", 
                &format!("{}false{}", colors::CYAN, colors::RESET));
            highlighted_line = highlighted_line.replace("null", 
                &format!("{}null{}", colors::CYAN, colors::RESET));
            
            // String literals
            if highlighted_line.contains('"') {
                highlighted_line = Self::highlight_strings(&highlighted_line);
            }
            
            result.push_str(&highlighted_line);
            result.push('\n');
        }
        
        result
    }
    
    fn highlight_strings(line: &str) -> String {
        // Simple string highlighting - just color the whole string
        let mut result = String::new();
        let mut in_string = false;
        let mut chars = line.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch == '"' {
                if !in_string {
                    result.push_str(colors::GREEN);
                    result.push(ch);
                    in_string = true;
                } else {
                    result.push(ch);
                    result.push_str(colors::RESET);
                    in_string = false;
                }
            } else {
                result.push(ch);
            }
        }
        
        if in_string {
            result.push_str(colors::RESET);
        }
        
        result
    }
}

/// Format a code block with header and highlighting
pub fn format_code_block(language: &str, code: &str) -> String {
    let highlighted = SyntaxHighlighter::highlight(code, language);
    let width = get_terminal_width();
    
    // Calculate header: "┌─ language ─" + remaining dashes
    let header_prefix = format!("┌─ {} ─", language);
    let header_prefix_len = language.len() + 4; // "┌─  ─"
    let header_dashes = if width > header_prefix_len {
        "─".repeat(width.saturating_sub(header_prefix_len))
    } else {
        String::new()
    };
    
    // Calculate footer: "└" + dashes
    let footer_dashes = "─".repeat(width.saturating_sub(1)); // Subtract 1 for └
    
    format!(
        "\n{}{}{}{}{}\n{}\n{}{}└{}{}",
        colors::BRIGHT_BLACK,
        colors::DIM,
        header_prefix,
        header_dashes,
        colors::RESET,
        highlighted.trim_end(),
        colors::BRIGHT_BLACK,
        colors::DIM,
        footer_dashes,
        colors::RESET
    )
}

/// Highlight diff content
pub fn format_diff(content: &str) -> String {
    let mut result = String::new();
    let width = get_terminal_width();
    
    // Calculate header: "┌─ Diff ─" + remaining dashes
    let header_prefix = "┌─ Diff ─";
    let header_prefix_len = 8; // "┌─ Diff ─"
    let header_dashes = if width > header_prefix_len {
        "─".repeat(width.saturating_sub(header_prefix_len))
    } else {
        String::new()
    };
    
    result.push_str(&format!("\n{}{}{}{}{}",
                            colors::BRIGHT_BLACK, colors::DIM, 
                            header_prefix, header_dashes, colors::RESET));
    result.push('\n');
    
    for line in content.lines() {
        if line.starts_with('+') {
            result.push_str(&format!("{}{}{}\n", colors::GREEN, line, colors::RESET));
        } else if line.starts_with('-') {
            result.push_str(&format!("{}{}{}\n", colors::RED, line, colors::RESET));
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    
    // Calculate footer: "└" + dashes
    let footer_dashes = "─".repeat(width.saturating_sub(1));
    
    result.push_str(&format!("{}{}└{}{}",
                            colors::BRIGHT_BLACK, colors::DIM, 
                            footer_dashes, colors::RESET));
    result.push('\n');
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_highlight_rust() {
        let code = "fn main() { println!(\"Hello\"); }";
        let highlighted = SyntaxHighlighter::highlight(code, "rust");
        assert!(highlighted.contains("fn"));
        // Should contain ANSI escape codes for coloring
        assert!(highlighted.contains("\x1b["));
    }
    
    #[test]
    fn test_highlight_python() {
        let code = "def hello():\n    print('world')";
        let highlighted = SyntaxHighlighter::highlight(code, "python");
        assert!(highlighted.contains("def"));
        assert!(highlighted.contains("\x1b["));
    }
    
    #[test]
    fn test_highlight_javascript() {
        let code = "function test() { return true; }";
        let highlighted = SyntaxHighlighter::highlight(code, "javascript");
        assert!(highlighted.contains("function"));
    }
    
    #[test]
    fn test_highlight_bash() {
        let code = "if [ -f file ]; then echo 'exists'; fi";
        let highlighted = SyntaxHighlighter::highlight(code, "bash");
        assert!(highlighted.contains("if"));
    }
    
    #[test]
    fn test_unknown_language() {
        let code = "some code";
        let highlighted = SyntaxHighlighter::highlight(code, "unknown");
        // Should return unchanged for unknown language
        assert_eq!(highlighted, code);
    }
    
    #[test]
    fn test_format_code_block() {
        let code = "fn test() {}";
        let formatted = format_code_block("rust", code);
        // Should have border characters
        assert!(formatted.contains("┌"));
        assert!(formatted.contains("└"));
        assert!(formatted.contains("rust"));
    }
    
    #[test]
    fn test_format_diff() {
        let diff = "+added line\n-removed line\n unchanged";
        let formatted = format_diff(diff);
        // Should contain diff markers
        assert!(formatted.contains("Diff"));
        assert!(formatted.contains("+added"));
        assert!(formatted.contains("-removed"));
    }
    
    #[test]
    fn test_ansi_colors() {
        // Test that color constants are defined
        assert_eq!(colors::RESET, "\x1b[0m");
        assert_eq!(colors::RED, "\x1b[31m");
        assert_eq!(colors::GREEN, "\x1b[32m");
    }
}

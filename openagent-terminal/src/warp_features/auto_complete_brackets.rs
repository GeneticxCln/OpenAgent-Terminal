/// Warp-style auto-completion for quotes, brackets, and parentheses
pub struct AutoCompleteBrackets {
    enabled: bool,
    bracket_pairs: Vec<BracketPair>,
    smart_completion: bool,
    balance_check: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BracketPair {
    pub open: char,
    pub close: char,
    pub name: &'static str,
}

#[derive(Debug, Clone)]
pub struct CompletionResult {
    pub should_complete: bool,
    pub completion_text: String,
    pub cursor_offset: i32, // How much to move cursor back after completion
    pub completion_type: CompletionType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompletionType {
    OpenBracket,   // User typed opening bracket/quote
    CloseBracket,  // User typed closing bracket/quote
    None,
}

impl AutoCompleteBrackets {
    pub fn new() -> Self {
        Self {
            enabled: true,
            bracket_pairs: Self::default_bracket_pairs(),
            smart_completion: true,
            balance_check: true,
        }
    }

    /// Get default bracket pairs that Warp supports
    fn default_bracket_pairs() -> Vec<BracketPair> {
        vec![
            BracketPair { open: '(', close: ')', name: "parentheses" },
            BracketPair { open: '[', close: ']', name: "square brackets" },
            BracketPair { open: '{', close: '}', name: "curly brackets" },
            BracketPair { open: '"', close: '"', name: "double quotes" },
            BracketPair { open: '\'', close: '\'', name: "single quotes" },
            BracketPair { open: '`', close: '`', name: "backticks" },
        ]
    }

    /// Enable or disable auto completion
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if auto completion is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable smart completion (context-aware completion)
    pub fn set_smart_completion(&mut self, enabled: bool) {
        self.smart_completion = enabled;
    }

    /// Handle character input and determine if completion should occur
    pub fn handle_character_input(
        &self,
        input_char: char,
        current_text: &str,
        cursor_pos: usize,
    ) -> CompletionResult {
        if !self.enabled {
            return CompletionResult {
                should_complete: false,
                completion_text: String::new(),
                cursor_offset: 0,
                completion_type: CompletionType::None,
            };
        }

        // Check if this character is part of a bracket pair
        for pair in &self.bracket_pairs {
            if input_char == pair.open {
                return self.handle_opening_bracket(input_char, current_text, cursor_pos, pair);
            } else if input_char == pair.close && pair.open != pair.close {
                return self.handle_closing_bracket(input_char, current_text, cursor_pos, pair);
            } else if input_char == pair.open && pair.open == pair.close {
                // Handle quote characters (which are both open and close)
                return self.handle_quote_character(input_char, current_text, cursor_pos, pair);
            }
        }

        CompletionResult {
            should_complete: false,
            completion_text: String::new(),
            cursor_offset: 0,
            completion_type: CompletionType::None,
        }
    }

    /// Handle opening bracket/parenthesis
    fn handle_opening_bracket(
        &self,
        input_char: char,
        current_text: &str,
        cursor_pos: usize,
        pair: &BracketPair,
    ) -> CompletionResult {
        // Always add the closing bracket for opening brackets
        let should_complete = if self.smart_completion {
            self.should_auto_complete_opening(current_text, cursor_pos, pair)
        } else {
            true
        };

        if should_complete {
            CompletionResult {
                should_complete: true,
                completion_text: pair.close.to_string(),
                cursor_offset: -1, // Move cursor back so it's between the brackets
                completion_type: CompletionType::OpenBracket,
            }
        } else {
            CompletionResult {
                should_complete: false,
                completion_text: String::new(),
                cursor_offset: 0,
                completion_type: CompletionType::None,
            }
        }
    }

    /// Handle closing bracket/parenthesis
    fn handle_closing_bracket(
        &self,
        input_char: char,
        current_text: &str,
        cursor_pos: usize,
        pair: &BracketPair,
    ) -> CompletionResult {
        // Check if we're typing a closing bracket when one already exists at cursor
        if cursor_pos < current_text.len() {
            let char_at_cursor = current_text.chars().nth(cursor_pos);
            if char_at_cursor == Some(input_char) {
                // Skip over the existing closing bracket
                return CompletionResult {
                    should_complete: true,
                    completion_text: String::new(), // Don't add anything
                    cursor_offset: 0, // Just move cursor forward
                    completion_type: CompletionType::CloseBracket,
                };
            }
        }

        CompletionResult {
            should_complete: false,
            completion_text: String::new(),
            cursor_offset: 0,
            completion_type: CompletionType::None,
        }
    }

    /// Handle quote characters (which can be both opening and closing)
    fn handle_quote_character(
        &self,
        input_char: char,
        current_text: &str,
        cursor_pos: usize,
        pair: &BracketPair,
    ) -> CompletionResult {
        // Check if we're typing a quote when one already exists at cursor
        if cursor_pos < current_text.len() {
            let char_at_cursor = current_text.chars().nth(cursor_pos);
            if char_at_cursor == Some(input_char) {
                // Skip over the existing quote
                return CompletionResult {
                    should_complete: true,
                    completion_text: String::new(),
                    cursor_offset: 0,
                    completion_type: CompletionType::CloseBracket,
                };
            }
        }

        // Determine if we should add a closing quote
        let quote_count = self.count_quotes_before_cursor(current_text, cursor_pos, input_char);
        let should_complete = if self.smart_completion {
            // Add closing quote if we have an even number of quotes (starting a new pair)
            quote_count % 2 == 0 && self.should_auto_complete_quote(current_text, cursor_pos, input_char)
        } else {
            quote_count % 2 == 0
        };

        if should_complete {
            CompletionResult {
                should_complete: true,
                completion_text: input_char.to_string(),
                cursor_offset: -1, // Move cursor back so it's between the quotes
                completion_type: CompletionType::OpenBracket,
            }
        } else {
            CompletionResult {
                should_complete: false,
                completion_text: String::new(),
                cursor_offset: 0,
                completion_type: CompletionType::None,
            }
        }
    }

    /// Smart logic for when to auto-complete opening brackets
    fn should_auto_complete_opening(
        &self,
        current_text: &str,
        cursor_pos: usize,
        pair: &BracketPair,
    ) -> bool {
        // Don't auto-complete if cursor is not at end and next char is alphanumeric
        if cursor_pos < current_text.len() {
            if let Some(next_char) = current_text.chars().nth(cursor_pos) {
                if next_char.is_alphanumeric() {
                    return false;
                }
            }
        }

        // Don't auto-complete if previous char is alphanumeric (unless it's a command separator)
        if cursor_pos > 0 {
            if let Some(prev_char) = current_text.chars().nth(cursor_pos - 1) {
                if prev_char.is_alphanumeric() && !self.is_command_separator(prev_char) {
                    return false;
                }
            }
        }

        true
    }

    /// Smart logic for when to auto-complete quotes
    fn should_auto_complete_quote(&self, current_text: &str, cursor_pos: usize, quote_char: char) -> bool {
        // Don't auto-complete quotes inside words
        let in_word = if cursor_pos > 0 && cursor_pos < current_text.len() {
            let prev_char = current_text.chars().nth(cursor_pos - 1).unwrap_or(' ');
            let next_char = current_text.chars().nth(cursor_pos).unwrap_or(' ');
            prev_char.is_alphanumeric() && next_char.is_alphanumeric()
        } else {
            false
        };

        !in_word
    }

    /// Count quotes before cursor position
    fn count_quotes_before_cursor(&self, text: &str, cursor_pos: usize, quote_char: char) -> usize {
        text.chars()
            .take(cursor_pos)
            .filter(|&c| c == quote_char)
            .count()
    }

    /// Check if character is a command separator
    fn is_command_separator(&self, c: char) -> bool {
        matches!(c, ' ' | '\t' | ';' | '|' | '&' | '\n')
    }

    /// Check bracket balance in the current text
    pub fn check_bracket_balance(&self, text: &str) -> BracketBalanceReport {
        let mut balance = BracketBalanceReport::new();
        
        for pair in &self.bracket_pairs {
            if pair.open == pair.close {
                // Handle quotes separately
                let count = text.chars().filter(|&c| c == pair.open).count();
                if count % 2 != 0 {
                    balance.unmatched_quotes.push(UnmatchedQuote {
                        quote_char: pair.open,
                        count,
                    });
                }
            } else {
                // Handle bracket pairs
                let open_count = text.chars().filter(|&c| c == pair.open).count();
                let close_count = text.chars().filter(|&c| c == pair.close).count();
                
                if open_count != close_count {
                    balance.unmatched_brackets.push(UnmatchedBracket {
                        bracket_pair: pair.clone(),
                        open_count,
                        close_count,
                    });
                }
            }
        }

        balance.is_balanced = balance.unmatched_quotes.is_empty() && balance.unmatched_brackets.is_empty();
        balance
    }

    /// Handle backspace - remove matching bracket if appropriate
    pub fn handle_backspace(&self, current_text: &str, cursor_pos: usize) -> Option<char> {
        if !self.enabled || cursor_pos == 0 || cursor_pos > current_text.len() {
            return None;
        }

        let prev_char = current_text.chars().nth(cursor_pos - 1)?;
        
        // Check if we're deleting an opening bracket and should remove the matching closing bracket
        for pair in &self.bracket_pairs {
            if prev_char == pair.open && cursor_pos < current_text.len() {
                let next_char = current_text.chars().nth(cursor_pos)?;
                if next_char == pair.close {
                    return Some(pair.close);
                }
            }
        }

        None
    }

    /// Get configuration for settings UI
    pub fn get_config(&self) -> AutoCompleteBracketsConfig {
        AutoCompleteBracketsConfig {
            enabled: self.enabled,
            smart_completion: self.smart_completion,
            balance_check: self.balance_check,
            bracket_pairs: self.bracket_pairs.clone(),
        }
    }

    /// Update configuration
    pub fn update_config(&mut self, config: AutoCompleteBracketsConfig) {
        self.enabled = config.enabled;
        self.smart_completion = config.smart_completion;
        self.balance_check = config.balance_check;
        self.bracket_pairs = config.bracket_pairs;
    }
}

#[derive(Debug, Clone)]
pub struct BracketBalanceReport {
    pub is_balanced: bool,
    pub unmatched_brackets: Vec<UnmatchedBracket>,
    pub unmatched_quotes: Vec<UnmatchedQuote>,
}

#[derive(Debug, Clone)]
pub struct UnmatchedBracket {
    pub bracket_pair: BracketPair,
    pub open_count: usize,
    pub close_count: usize,
}

#[derive(Debug, Clone)]
pub struct UnmatchedQuote {
    pub quote_char: char,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct AutoCompleteBracketsConfig {
    pub enabled: bool,
    pub smart_completion: bool,
    pub balance_check: bool,
    pub bracket_pairs: Vec<BracketPair>,
}

impl BracketBalanceReport {
    fn new() -> Self {
        Self {
            is_balanced: true,
            unmatched_brackets: Vec::new(),
            unmatched_quotes: Vec::new(),
        }
    }
}

impl Default for AutoCompleteBrackets {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opening_bracket_completion() {
        let auto_complete = AutoCompleteBrackets::new();
        let result = auto_complete.handle_character_input('(', "echo ", 5);
        
        assert!(result.should_complete);
        assert_eq!(result.completion_text, ")");
        assert_eq!(result.cursor_offset, -1);
        assert_eq!(result.completion_type, CompletionType::OpenBracket);
    }

    #[test]
    fn test_quote_completion() {
        let auto_complete = AutoCompleteBrackets::new();
        let result = auto_complete.handle_character_input('"', "echo ", 5);
        
        assert!(result.should_complete);
        assert_eq!(result.completion_text, "\"");
        assert_eq!(result.cursor_offset, -1);
        assert_eq!(result.completion_type, CompletionType::OpenBracket);
    }

    #[test]
    fn test_closing_bracket_skip() {
        let auto_complete = AutoCompleteBrackets::new();
        let result = auto_complete.handle_character_input(')', "echo (test)", 10);
        
        assert!(result.should_complete);
        assert_eq!(result.completion_text, "");
        assert_eq!(result.cursor_offset, 0);
        assert_eq!(result.completion_type, CompletionType::CloseBracket);
    }

    #[test]
    fn test_smart_completion_disabled_in_word() {
        let mut auto_complete = AutoCompleteBrackets::new();
        auto_complete.set_smart_completion(true);
        
        let result = auto_complete.handle_character_input('(', "testfunc()", 8);
        assert!(!result.should_complete);
    }

    #[test]
    fn test_quote_balance() {
        let auto_complete = AutoCompleteBrackets::new();
        
        // Odd number of quotes - should not auto-complete
        let result = auto_complete.handle_character_input('"', "echo \"test", 10);
        assert!(!result.should_complete);
        
        // Even number of quotes - should auto-complete
        let result = auto_complete.handle_character_input('"', "echo \"test\" ", 12);
        assert!(result.should_complete);
    }

    #[test]
    fn test_bracket_balance_check() {
        let auto_complete = AutoCompleteBrackets::new();
        
        let balanced = auto_complete.check_bracket_balance("echo (test)");
        assert!(balanced.is_balanced);
        
        let unbalanced = auto_complete.check_bracket_balance("echo (test");
        assert!(!unbalanced.is_balanced);
        assert_eq!(unbalanced.unmatched_brackets.len(), 1);
    }

    #[test]
    fn test_backspace_bracket_removal() {
        let auto_complete = AutoCompleteBrackets::new();
        
        // Should remove matching closing bracket
        let result = auto_complete.handle_backspace("test()", 5);
        assert_eq!(result, Some(')'));
        
        // Should not remove non-matching bracket
        let result = auto_complete.handle_backspace("test()x", 6);
        assert_eq!(result, None);
    }

    #[test]
    fn test_disabled_completion() {
        let mut auto_complete = AutoCompleteBrackets::new();
        auto_complete.set_enabled(false);
        
        let result = auto_complete.handle_character_input('(', "echo ", 5);
        assert!(!result.should_complete);
    }

    #[test]
    fn test_multiple_bracket_types() {
        let auto_complete = AutoCompleteBrackets::new();
        
        // Test different bracket types
        for (open, close) in [('(', ')'), ('[', ']'), ('{', '}')] {
            let result = auto_complete.handle_character_input(open, "test ", 5);
            assert!(result.should_complete);
            assert_eq!(result.completion_text, close.to_string());
        }
    }
}
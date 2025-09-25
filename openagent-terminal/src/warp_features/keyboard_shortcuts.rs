/// Warp-specific keyboard shortcuts for Linux
/// Based on the official Warp documentation for Linux key bindings
use crate::input::keyboard::{Key, ModifiersState};

#[derive(Debug, Clone, PartialEq)]
pub enum WarpAction {
    // Input suggestions and history
    CloseInputSuggestionsOrHistory, // ESCAPE
    ClearTerminal,                   // CTRL-L
    Backspace,                      // CTRL-H
    ClearEditorBuffer,              // CTRL-C
    CopyAndClearCurrentLine,        // CTRL-U
    ClearSelectedLines,             // CTRL-SHIFT-K
    
    // Copy, cut, paste
    Copy,                           // CTRL-C
    Cut,                            // CTRL-X
    Paste,                          // CTRL-V
    
    // Word manipulation
    CutWordLeft,                    // CTRL-W
    CutWordRight,                   // ALT-D
    DeleteWordLeft,                 // ALT-BACKSPACE
    DeleteWordRight,                // ALT-D
    DeleteToEndOfLine,              // CTRL-K
    
    // Navigation
    MoveToBeginningOfPreviousWord,  // ALT-LEFT
    MoveToBeginningOfNextWord,      // ALT-RIGHT
    MoveBackwardBySubword,          // CTRL-LEFT
    MoveForwardBySubword,           // CTRL-RIGHT
    MoveToStartOfLine,              // CTRL-A
    MoveToEndOfLine,                // CTRL-E
    
    // Selection
    SelectCharacterLeft,            // SHIFT-LEFT
    SelectCharacterRight,           // SHIFT-RIGHT
    SelectWordLeft,                 // META-SHIFT-B
    SelectWordRight,                // META-SHIFT-F
    SelectToStartOfLine,            // SHIFT-HOME
    SelectToEndOfLine,              // SHIFT-END
    SelectUp,                       // SHIFT-UP
    SelectDown,                     // SHIFT-DOWN
    SelectAll,                      // CTRL-A
    
    // Line operations
    InsertNewline,                  // SHIFT-ENTER, CTRL-ENTER, ALT-ENTER
    CommandSearch,                  // CTRL-R
    SplitPane,                      // CTRL-SHIFT-D
}

/// Warp keyboard shortcut handler
pub struct WarpKeyboardShortcuts {
    enabled: bool,
}

impl WarpKeyboardShortcuts {
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// Check if shortcuts are enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable shortcuts
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Map key combination to Warp action (Linux bindings)
    pub fn map_key_to_action(&self, key: Key, modifiers: ModifiersState) -> Option<WarpAction> {
        if !self.enabled {
            return None;
        }

        match (key, modifiers) {
            // Basic actions
            (Key::Escape, ModifiersState::empty()) => Some(WarpAction::CloseInputSuggestionsOrHistory),
            (Key::L, ModifiersState::CTRL) => Some(WarpAction::ClearTerminal),
            (Key::H, ModifiersState::CTRL) => Some(WarpAction::Backspace),
            (Key::C, ModifiersState::CTRL) => Some(WarpAction::ClearEditorBuffer),
            (Key::U, ModifiersState::CTRL) => Some(WarpAction::CopyAndClearCurrentLine),
            (Key::K, ModifiersState::CTRL | ModifiersState::SHIFT) => Some(WarpAction::ClearSelectedLines),

            // Copy, cut, paste
            (Key::C, ModifiersState::CTRL) => Some(WarpAction::Copy),
            (Key::X, ModifiersState::CTRL) => Some(WarpAction::Cut),
            (Key::V, ModifiersState::CTRL) => Some(WarpAction::Paste),

            // Word manipulation
            (Key::W, ModifiersState::CTRL) => Some(WarpAction::CutWordLeft),
            (Key::D, ModifiersState::ALT) => Some(WarpAction::CutWordRight),
            (Key::Backspace, ModifiersState::ALT) => Some(WarpAction::DeleteWordLeft),
            (Key::D, ModifiersState::ALT) => Some(WarpAction::DeleteWordRight),
            (Key::K, ModifiersState::CTRL) => Some(WarpAction::DeleteToEndOfLine),

            // Navigation
            (Key::Left, ModifiersState::ALT) => Some(WarpAction::MoveToBeginningOfPreviousWord),
            (Key::Right, ModifiersState::ALT) => Some(WarpAction::MoveToBeginningOfNextWord),
            (Key::Left, ModifiersState::CTRL) => Some(WarpAction::MoveBackwardBySubword),
            (Key::Right, ModifiersState::CTRL) => Some(WarpAction::MoveForwardBySubword),
            (Key::A, ModifiersState::CTRL) => Some(WarpAction::MoveToStartOfLine),
            (Key::E, ModifiersState::CTRL) => Some(WarpAction::MoveToEndOfLine),

            // Selection
            (Key::Left, ModifiersState::SHIFT) => Some(WarpAction::SelectCharacterLeft),
            (Key::Right, ModifiersState::SHIFT) => Some(WarpAction::SelectCharacterRight),
            (Key::B, ModifiersState::ALT | ModifiersState::SHIFT) => Some(WarpAction::SelectWordLeft),
            (Key::F, ModifiersState::ALT | ModifiersState::SHIFT) => Some(WarpAction::SelectWordRight),
            (Key::Home, ModifiersState::SHIFT) => Some(WarpAction::SelectToStartOfLine),
            (Key::End, ModifiersState::SHIFT) => Some(WarpAction::SelectToEndOfLine),
            (Key::Up, ModifiersState::SHIFT) => Some(WarpAction::SelectUp),
            (Key::Down, ModifiersState::SHIFT) => Some(WarpAction::SelectDown),
            (Key::A, ModifiersState::CTRL) => Some(WarpAction::SelectAll),

            // Line operations
            (Key::Enter, ModifiersState::SHIFT) => Some(WarpAction::InsertNewline),
            (Key::Enter, ModifiersState::CTRL) => Some(WarpAction::InsertNewline),
            (Key::Enter, ModifiersState::ALT) => Some(WarpAction::InsertNewline),
            (Key::R, ModifiersState::CTRL) => Some(WarpAction::CommandSearch),
            (Key::D, ModifiersState::CTRL | ModifiersState::SHIFT) => Some(WarpAction::SplitPane),

            _ => None,
        }
    }

    /// Get all available keyboard shortcuts for display/help
    pub fn get_all_shortcuts(&self) -> Vec<KeyboardShortcut> {
        vec![
            KeyboardShortcut {
                keys: "ESCAPE".to_string(),
                action: WarpAction::CloseInputSuggestionsOrHistory,
                description: "Closes the input suggestions or history menu".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-L".to_string(),
                action: WarpAction::ClearTerminal,
                description: "Clears the terminal".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-H".to_string(),
                action: WarpAction::Backspace,
                description: "Backspace".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-C".to_string(),
                action: WarpAction::ClearEditorBuffer,
                description: "Clear the entire editor buffer".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-U".to_string(),
                action: WarpAction::CopyAndClearCurrentLine,
                description: "Copy and Clear the current line".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-SHIFT-K".to_string(),
                action: WarpAction::ClearSelectedLines,
                description: "Clear selected lines".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-C, CTRL-X, CTRL-V".to_string(),
                action: WarpAction::Copy,
                description: "Copy, cut, paste".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-W / ALT-D".to_string(),
                action: WarpAction::CutWordLeft,
                description: "Cut the word to the left / right of the cursor".to_string(),
            },
            KeyboardShortcut {
                keys: "ALT-BACKSPACE / ALT-D".to_string(),
                action: WarpAction::DeleteWordLeft,
                description: "Delete the word to the left / right of the cursor".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-K".to_string(),
                action: WarpAction::DeleteToEndOfLine,
                description: "Delete everything to the right of the cursor".to_string(),
            },
            KeyboardShortcut {
                keys: "ALT-LEFT / ALT-RIGHT".to_string(),
                action: WarpAction::MoveToBeginningOfPreviousWord,
                description: "Move to the beginning of the previous / next word".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-LEFT / CTRL-RIGHT".to_string(),
                action: WarpAction::MoveBackwardBySubword,
                description: "Move backward / forward by one subword".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-A / CTRL-E".to_string(),
                action: WarpAction::MoveToStartOfLine,
                description: "Move the cursor to the start / end of the line".to_string(),
            },
            KeyboardShortcut {
                keys: "META-SHIFT-B / META-SHIFT-F".to_string(),
                action: WarpAction::SelectWordLeft,
                description: "Select the word to the left / right of the cursor".to_string(),
            },
            KeyboardShortcut {
                keys: "SHIFT-UP / SHIFT-DOWN".to_string(),
                action: WarpAction::SelectUp,
                description: "Select everything above / below the cursor".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-A".to_string(),
                action: WarpAction::SelectAll,
                description: "Select the entire editor buffer".to_string(),
            },
            KeyboardShortcut {
                keys: "SHIFT-ENTER, CTRL-ENTER, ALT-ENTER".to_string(),
                action: WarpAction::InsertNewline,
                description: "Insert newline".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-R".to_string(),
                action: WarpAction::CommandSearch,
                description: "Command Search".to_string(),
            },
            KeyboardShortcut {
                keys: "CTRL-SHIFT-D".to_string(),
                action: WarpAction::SplitPane,
                description: "Split pane".to_string(),
            },
        ]
    }

    /// Execute a Warp action
    pub fn execute_action(&self, action: WarpAction) -> ActionResult {
        match action {
            WarpAction::CloseInputSuggestionsOrHistory => ActionResult::CloseMenu,
            WarpAction::ClearTerminal => ActionResult::ClearScreen,
            WarpAction::Backspace => ActionResult::DeleteBackward,
            WarpAction::ClearEditorBuffer => ActionResult::ClearBuffer,
            WarpAction::CopyAndClearCurrentLine => ActionResult::CopyAndClear,
            WarpAction::ClearSelectedLines => ActionResult::ClearSelection,
            WarpAction::Copy => ActionResult::Copy,
            WarpAction::Cut => ActionResult::Cut,
            WarpAction::Paste => ActionResult::Paste,
            WarpAction::CutWordLeft => ActionResult::CutWord { direction: Direction::Left },
            WarpAction::CutWordRight => ActionResult::CutWord { direction: Direction::Right },
            WarpAction::DeleteWordLeft => ActionResult::DeleteWord { direction: Direction::Left },
            WarpAction::DeleteWordRight => ActionResult::DeleteWord { direction: Direction::Right },
            WarpAction::DeleteToEndOfLine => ActionResult::DeleteToEnd,
            WarpAction::MoveToBeginningOfPreviousWord => ActionResult::MoveWord { direction: Direction::Left },
            WarpAction::MoveToBeginningOfNextWord => ActionResult::MoveWord { direction: Direction::Right },
            WarpAction::MoveBackwardBySubword => ActionResult::MoveSubword { direction: Direction::Left },
            WarpAction::MoveForwardBySubword => ActionResult::MoveSubword { direction: Direction::Right },
            WarpAction::MoveToStartOfLine => ActionResult::MoveToStart,
            WarpAction::MoveToEndOfLine => ActionResult::MoveToEnd,
            WarpAction::SelectCharacterLeft => ActionResult::SelectChar { direction: Direction::Left },
            WarpAction::SelectCharacterRight => ActionResult::SelectChar { direction: Direction::Right },
            WarpAction::SelectWordLeft => ActionResult::SelectWord { direction: Direction::Left },
            WarpAction::SelectWordRight => ActionResult::SelectWord { direction: Direction::Right },
            WarpAction::SelectToStartOfLine => ActionResult::SelectToStart,
            WarpAction::SelectToEndOfLine => ActionResult::SelectToEnd,
            WarpAction::SelectUp => ActionResult::SelectLine { direction: Direction::Up },
            WarpAction::SelectDown => ActionResult::SelectLine { direction: Direction::Down },
            WarpAction::SelectAll => ActionResult::SelectAll,
            WarpAction::InsertNewline => ActionResult::InsertNewline,
            WarpAction::CommandSearch => ActionResult::OpenCommandSearch,
            WarpAction::SplitPane => ActionResult::SplitPane,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyboardShortcut {
    pub keys: String,
    pub action: WarpAction,
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum ActionResult {
    CloseMenu,
    ClearScreen,
    DeleteBackward,
    ClearBuffer,
    CopyAndClear,
    ClearSelection,
    Copy,
    Cut,
    Paste,
    CutWord { direction: Direction },
    DeleteWord { direction: Direction },
    DeleteToEnd,
    MoveWord { direction: Direction },
    MoveSubword { direction: Direction },
    MoveToStart,
    MoveToEnd,
    SelectChar { direction: Direction },
    SelectWord { direction: Direction },
    SelectToStart,
    SelectToEnd,
    SelectLine { direction: Direction },
    SelectAll,
    InsertNewline,
    OpenCommandSearch,
    SplitPane,
}

#[derive(Debug, Clone)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Default for WarpKeyboardShortcuts {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_shortcuts() {
        let shortcuts = WarpKeyboardShortcuts::new();
        
        // Test ESCAPE key
        let action = shortcuts.map_key_to_action(Key::Escape, ModifiersState::empty());
        assert_eq!(action, Some(WarpAction::CloseInputSuggestionsOrHistory));
        
        // Test CTRL-L
        let action = shortcuts.map_key_to_action(Key::L, ModifiersState::CTRL);
        assert_eq!(action, Some(WarpAction::ClearTerminal));
        
        // Test CTRL-C
        let action = shortcuts.map_key_to_action(Key::C, ModifiersState::CTRL);
        assert_eq!(action, Some(WarpAction::ClearEditorBuffer));
    }

    #[test]
    fn test_navigation_shortcuts() {
        let shortcuts = WarpKeyboardShortcuts::new();
        
        // Test ALT-LEFT
        let action = shortcuts.map_key_to_action(Key::Left, ModifiersState::ALT);
        assert_eq!(action, Some(WarpAction::MoveToBeginningOfPreviousWord));
        
        // Test CTRL-A
        let action = shortcuts.map_key_to_action(Key::A, ModifiersState::CTRL);
        assert_eq!(action, Some(WarpAction::MoveToStartOfLine));
    }

    #[test]
    fn test_selection_shortcuts() {
        let shortcuts = WarpKeyboardShortcuts::new();
        
        // Test SHIFT-LEFT
        let action = shortcuts.map_key_to_action(Key::Left, ModifiersState::SHIFT);
        assert_eq!(action, Some(WarpAction::SelectCharacterLeft));
        
        // Test SHIFT-UP
        let action = shortcuts.map_key_to_action(Key::Up, ModifiersState::SHIFT);
        assert_eq!(action, Some(WarpAction::SelectUp));
    }

    #[test]
    fn test_special_shortcuts() {
        let shortcuts = WarpKeyboardShortcuts::new();
        
        // Test CTRL-R for command search
        let action = shortcuts.map_key_to_action(Key::R, ModifiersState::CTRL);
        assert_eq!(action, Some(WarpAction::CommandSearch));
        
        // Test CTRL-SHIFT-D for split pane
        let action = shortcuts.map_key_to_action(Key::D, ModifiersState::CTRL | ModifiersState::SHIFT);
        assert_eq!(action, Some(WarpAction::SplitPane));
    }

    #[test]
    fn test_disabled_shortcuts() {
        let mut shortcuts = WarpKeyboardShortcuts::new();
        shortcuts.set_enabled(false);
        
        let action = shortcuts.map_key_to_action(Key::L, ModifiersState::CTRL);
        assert_eq!(action, None);
    }

    #[test]
    fn test_action_execution() {
        let shortcuts = WarpKeyboardShortcuts::new();
        
        let result = shortcuts.execute_action(WarpAction::ClearTerminal);
        matches!(result, ActionResult::ClearScreen);
        
        let result = shortcuts.execute_action(WarpAction::Copy);
        matches!(result, ActionResult::Copy);
    }

    #[test]
    fn test_get_all_shortcuts() {
        let shortcuts = WarpKeyboardShortcuts::new();
        let all_shortcuts = shortcuts.get_all_shortcuts();
        
        assert!(!all_shortcuts.is_empty());
        assert!(all_shortcuts.len() > 10); // Should have many shortcuts
        
        // Check that we have some key shortcuts
        assert!(all_shortcuts.iter().any(|s| matches!(s.action, WarpAction::ClearTerminal)));
        assert!(all_shortcuts.iter().any(|s| matches!(s.action, WarpAction::Copy)));
    }
}
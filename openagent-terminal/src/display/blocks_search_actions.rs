#![allow(dead_code)]
// Blocks search actions menu
// Provides contextual actions for selected blocks in advanced search mode

use crate::config::theme::ThemeTokens;
use crate::display::blocks_search_panel::BlocksSearchItem;
use crate::display::Display;
use openagent_terminal_core::index::{Column, Point};

/// Available actions for blocks in search results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockAction {
    /// Explain this error/output with AI
    ExplainError,
    /// Suggest a fix for this error with AI
    FixError,
    /// Copy command to clipboard
    CopyCommand,
    /// Copy output to clipboard
    CopyOutput,
    /// Copy both command and output
    CopyBoth,
    /// Insert command into prompt
    InsertCommand,
    /// Insert output as here-doc into prompt
    InsertAsHereDoc,
    /// Insert output as here-doc with custom command
    InsertAsHereDocCustom,
    /// Insert output as JSON here-doc for jq
    InsertAsJsonHereDoc,
    /// Insert output for shell-specific format
    InsertAsShellHereDoc,
    /// Rerun the command
    RerunCommand,
    /// Toggle star/bookmark
    ToggleStar,
    /// Add/edit tags
    EditTags,
    /// Export block to file
    ExportBlock,
    /// Share block (create permalink)
    ShareBlock,
    /// Delete block
    DeleteBlock,
    /// View full output
    ViewFullOutput,
    /// Create snippet from command
    CreateSnippet,
}

impl BlockAction {
    /// Get display name for the action
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ExplainError => "Explain Error",
            Self::FixError => "Suggest Fix",
            Self::CopyCommand => "Copy Command",
            Self::CopyOutput => "Copy Output",
            Self::CopyBoth => "Copy Both",
            Self::InsertCommand => "Insert Command",
            Self::InsertAsHereDoc => "Insert as Here-doc",
            Self::InsertAsHereDocCustom => "Custom Here-doc",
            Self::InsertAsJsonHereDoc => "JSON Here-doc",
            Self::InsertAsShellHereDoc => "Shell Here-doc",
            Self::RerunCommand => "Rerun Command",
            Self::ToggleStar => "Toggle Star",
            Self::EditTags => "Edit Tags",
            Self::ExportBlock => "Export to File",
            Self::ShareBlock => "Share Block",
            Self::DeleteBlock => "Delete Block",
            Self::ViewFullOutput => "View Full Output",
            Self::CreateSnippet => "Create Snippet",
        }
    }

    /// Get keyboard shortcut for the action
    pub fn shortcut(&self) -> &'static str {
        match self {
            Self::ExplainError => "X",
            Self::FixError => "F",
            Self::CopyCommand => "C",
            Self::CopyOutput => "O",
            Self::CopyBoth => "B",
            Self::InsertCommand => "I",
            Self::InsertAsHereDoc => "H",
            Self::InsertAsHereDocCustom => "Shift+H",
            Self::InsertAsJsonHereDoc => "J",
            Self::InsertAsShellHereDoc => "Shift+S",
            Self::RerunCommand => "R",
            Self::ToggleStar => "*",
            Self::EditTags => "T",
            Self::ExportBlock => "E",
            Self::ShareBlock => "S",
            Self::DeleteBlock => "Del",
            Self::ViewFullOutput => "V",
            Self::CreateSnippet => "N",
        }
    }

    /// Get icon for the action
    pub fn icon(&self) -> &'static str {
        match self {
            Self::ExplainError => "❓",
            Self::FixError => "🛠️",
            Self::CopyCommand => "📋",
            Self::CopyOutput => "📄",
            Self::CopyBoth => "📑",
            Self::InsertCommand => "↩️",
            Self::InsertAsHereDoc => "📝",
            Self::InsertAsHereDocCustom => "🔧",
            Self::InsertAsJsonHereDoc => "{ }",
            Self::InsertAsShellHereDoc => "🐚",
            Self::RerunCommand => "🔄",
            Self::ToggleStar => "⭐",
            Self::EditTags => "🏷️",
            Self::ExportBlock => "💾",
            Self::ShareBlock => "🔗",
            Self::DeleteBlock => "🗑️",
            Self::ViewFullOutput => "👁️",
            Self::CreateSnippet => "✂️",
        }
    }

    /// Check if action is destructive (requires confirmation)
    pub fn is_destructive(&self) -> bool {
        matches!(self, Self::DeleteBlock)
    }

    /// Check if action is available for the given block
    pub fn is_available_for(&self, block: &BlocksSearchItem) -> bool {
        match self {
            Self::ExplainError => !block.output.is_empty() || !block.command.is_empty(),
            Self::FixError => !block.output.is_empty() || !block.command.is_empty(),
            Self::CopyCommand => !block.command.is_empty(),
            Self::CopyOutput => !block.output.is_empty(),
            Self::CopyBoth => !block.command.is_empty() || !block.output.is_empty(),
            Self::InsertCommand => !block.command.is_empty(),
            Self::InsertAsHereDoc => !block.output.is_empty(),
            Self::InsertAsHereDocCustom => !block.output.is_empty(),
            Self::InsertAsJsonHereDoc => !block.output.is_empty() && is_likely_json(&block.output),
            Self::InsertAsShellHereDoc => !block.output.is_empty(),
            Self::RerunCommand => !block.command.is_empty(),
            Self::ToggleStar => true,
            Self::EditTags => true,
            Self::ExportBlock => !block.command.is_empty() || !block.output.is_empty(),
            Self::ShareBlock => !block.command.is_empty(),
            Self::DeleteBlock => true,
            Self::ViewFullOutput => !block.output.is_empty(),
            Self::CreateSnippet => !block.command.is_empty(),
        }
    }
}

/// Actions menu state
#[derive(Debug, Clone)]
pub struct ActionsMenuState {
    /// Whether the menu is active
    pub active: bool,
    /// Currently selected action index
    pub selected: usize,
    /// Available actions for the current block
    pub actions: Vec<BlockAction>,
    /// Position of the menu
    pub position: Point<usize>,
    /// Width of the menu
    pub width: usize,
}

impl Default for ActionsMenuState {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionsMenuState {
    /// Create new actions menu state
    pub fn new() -> Self {
        Self {
            active: false,
            selected: 0,
            actions: Vec::new(),
            position: Point::new(0, Column(0)),
            width: 25,
        }
    }

    /// Open actions menu for a specific block
    pub fn open_for_block(&mut self, block: &BlocksSearchItem, position: Point<usize>) {
        self.active = true;
        self.selected = 0;
        self.position = position;

        // Build list of available actions
        self.actions = vec![
            BlockAction::ExplainError,
            BlockAction::FixError,
            BlockAction::CopyCommand,
            BlockAction::CopyOutput,
            BlockAction::CopyBoth,
            BlockAction::InsertCommand,
            BlockAction::InsertAsHereDoc,
            BlockAction::InsertAsHereDocCustom,
            BlockAction::InsertAsJsonHereDoc,
            BlockAction::InsertAsShellHereDoc,
            BlockAction::RerunCommand,
            BlockAction::ToggleStar,
            BlockAction::EditTags,
            BlockAction::ExportBlock,
            BlockAction::ShareBlock,
            BlockAction::ViewFullOutput,
            BlockAction::CreateSnippet,
            BlockAction::DeleteBlock,
        ]
        .into_iter()
        .filter(|action| action.is_available_for(block))
        .collect();
    }

    /// Close the actions menu
    pub fn close(&mut self) {
        self.active = false;
        self.actions.clear();
    }

    /// Move selection in the menu
    pub fn move_selection(&mut self, delta: isize) {
        if self.actions.is_empty() {
            return;
        }

        let len = self.actions.len() as isize;
        let mut new_idx = self.selected as isize + delta;

        if new_idx < 0 {
            new_idx = len - 1;
        } else if new_idx >= len {
            new_idx = 0;
        }

        self.selected = new_idx as usize;
    }

    /// Get currently selected action
    pub fn get_selected_action(&self) -> Option<BlockAction> {
        self.actions.get(self.selected).copied()
    }

    /// Render the actions menu
    pub fn render(&self, display: &mut Display, tokens: &ThemeTokens) {
        if !self.active {
            return;
        }

        let size_info = display.size_info;
        let menu_width = self.width;
        let menu_height = self.actions.len() + 2; // +2 for header and border

        // Calculate menu position (near the selected item but within bounds)
        let menu_x = self
            .position
            .column
            .0
            .min(size_info.columns.saturating_sub(menu_width));
        let menu_y = self
            .position
            .line
            .min(size_info.screen_lines.saturating_sub(menu_height));

        // Draw header
        let header = "Actions";
        display.draw_ai_text(
            Point::new(menu_y, Column(menu_x + 1)),
            tokens.accent,
            tokens.surface,
            header,
            menu_width - 2,
        );

        // Draw separator
        let separator = "─".repeat(menu_width - 2);
        display.draw_ai_text(
            Point::new(menu_y + 1, Column(menu_x + 1)),
            tokens.text_muted,
            tokens.surface,
            &separator,
            menu_width - 2,
        );

        // Draw action items
        for (idx, action) in self.actions.iter().enumerate() {
            let line = menu_y + 2 + idx;
            let is_selected = idx == self.selected;

            let (fg_color, bg_color) = if is_selected {
                (tokens.surface, tokens.accent)
            } else {
                (tokens.text, tokens.surface)
            };

            let indicator = if is_selected { "▶ " } else { "  " };
            let action_text = format!(
                "{}{} {} {}",
                indicator,
                action.icon(),
                action.shortcut(),
                action.display_name()
            );

            display.draw_ai_text(
                Point::new(line, Column(menu_x)),
                fg_color,
                bg_color,
                &action_text,
                menu_width,
            );
        }
    }
}

/// Help overlay state
#[derive(Debug, Clone)]
pub struct HelpOverlayState {
    /// Whether the help is active
    pub active: bool,
    /// Current help section
    pub section: HelpSection,
    /// Scroll position in help content
    pub scroll: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpSection {
    Overview,
    BasicMode,
    AdvancedMode,
    Filters,
    Actions,
    Tips,
}

impl Default for HelpOverlayState {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpOverlayState {
    pub fn new() -> Self {
        Self {
            active: false,
            section: HelpSection::Overview,
            scroll: 0,
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.section = HelpSection::Overview;
        self.scroll = 0;
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn next_section(&mut self) {
        self.section = match self.section {
            HelpSection::Overview => HelpSection::BasicMode,
            HelpSection::BasicMode => HelpSection::AdvancedMode,
            HelpSection::AdvancedMode => HelpSection::Filters,
            HelpSection::Filters => HelpSection::Actions,
            HelpSection::Actions => HelpSection::Tips,
            HelpSection::Tips => HelpSection::Overview,
        };
        self.scroll = 0;
    }

    pub fn prev_section(&mut self) {
        self.section = match self.section {
            HelpSection::Overview => HelpSection::Tips,
            HelpSection::BasicMode => HelpSection::Overview,
            HelpSection::AdvancedMode => HelpSection::BasicMode,
            HelpSection::Filters => HelpSection::AdvancedMode,
            HelpSection::Actions => HelpSection::Filters,
            HelpSection::Tips => HelpSection::Actions,
        };
        self.scroll = 0;
    }

    /// Render the help overlay
    pub fn render(&self, display: &mut Display, tokens: &ThemeTokens) {
        if !self.active {
            return;
        }

        let size_info = display.size_info;
        let panel_width = size_info.columns.min(80);
        let panel_height = size_info.screen_lines.min(25);
        let panel_x = (size_info.columns.saturating_sub(panel_width)) / 2;
        let panel_y = (size_info.screen_lines.saturating_sub(panel_height)) / 2;

        // Draw content based on current section
        let content = get_help_content(self.section);
        self.draw_help_content(
            display,
            &content,
            panel_x,
            panel_y,
            panel_width,
            panel_height,
            tokens,
        );

        // Draw section navigation
        self.draw_help_navigation(display, panel_x, panel_y, panel_width, tokens);
    }

    /// Draw help content
    #[allow(clippy::too_many_arguments)]
    fn draw_help_content(
        &self,
        display: &mut Display,
        content: &[String],
        panel_x: usize,
        panel_y: usize,
        panel_width: usize,
        panel_height: usize,
        tokens: &ThemeTokens,
    ) {
        let content_height = panel_height.saturating_sub(4); // Reserve space for navigation
        let start_line = self.scroll;

        for (idx, line) in content
            .iter()
            .skip(start_line)
            .take(content_height)
            .enumerate()
        {
            let y = panel_y + 2 + idx;
            let color = if idx == 0 && !line.is_empty() {
                tokens.accent // Header color
            } else {
                tokens.text
            };

            display.draw_ai_text(
                Point::new(y, Column(panel_x + 2)),
                color,
                tokens.surface,
                line,
                panel_width.saturating_sub(4),
            );
        }
    }

    /// Draw help navigation
    fn draw_help_navigation(
        &self,
        display: &mut Display,
        panel_x: usize,
        panel_y: usize,
        panel_width: usize,
        tokens: &ThemeTokens,
    ) {
        let nav_y = panel_y + panel_width.saturating_sub(2);

        // Section tabs
        let sections = vec![
            (HelpSection::Overview, "Overview"),
            (HelpSection::BasicMode, "Basic"),
            (HelpSection::AdvancedMode, "Advanced"),
            (HelpSection::Filters, "Filters"),
            (HelpSection::Actions, "Actions"),
            (HelpSection::Tips, "Tips"),
        ];

        let mut x_offset = 2;
        for (section, name) in sections {
            let is_active = section == self.section;
            let (fg, bg) = if is_active {
                (tokens.surface, tokens.accent)
            } else {
                (tokens.text_muted, tokens.surface)
            };

            let tab_text = format!(" {} ", name);
            display.draw_ai_text(
                Point::new(nav_y, Column(panel_x + x_offset)),
                fg,
                bg,
                &tab_text,
                tab_text.len(),
            );

            x_offset += tab_text.len() + 1;
            if x_offset >= panel_width.saturating_sub(2) {
                break;
            }
        }

        // Navigation instructions
        let nav_help = "Tab/Shift+Tab: Navigate • Esc: Close";
        display.draw_ai_text(
            Point::new(nav_y + 1, Column(panel_x + 2)),
            tokens.text_muted,
            tokens.surface,
            nav_help,
            panel_width.saturating_sub(4),
        );
    }
}

/// Get help content for a specific section
fn get_help_content(section: HelpSection) -> Vec<String> {
    match section {
        HelpSection::Overview => vec![
            "Blocks Search Help".to_string(),
            "".to_string(),
            "The Blocks Search panel helps you find and work with".to_string(),
            "your command history efficiently.".to_string(),
            "".to_string(),
            "Enhanced Features:".to_string(),
            "• Search in commands, output, or both".to_string(),
            "• Advanced filtering by status, duration, tags".to_string(),
            "• Sorting and pagination with configurable sizes".to_string(),
            "• Actions menu with advanced operations".to_string(),
            "• Multiple here-doc insertion modes".to_string(),
            "• Configurable session persistence".to_string(),
            "• Keyboard help and discoverability hints".to_string(),
            "".to_string(),
            "Use Tab/Shift+Tab to navigate help sections.".to_string(),
        ],
        HelpSection::BasicMode => vec![
            "Basic Search Mode".to_string(),
            "".to_string(),
            "Key Bindings:".to_string(),
            "  Enter        Paste selected command".to_string(),
            "  Esc          Close search".to_string(),
            "  ↑/↓          Navigate results".to_string(),
            "  j/k          Navigate results (vi-style)".to_string(),
            "  PgUp/PgDn    Page navigation".to_string(),
            "  Tab          Cycle search mode".to_string(),
            "  ?            Show this help".to_string(),
            "".to_string(),
            "Search Modes:".to_string(),
            "  🔎 Basic     Search all text".to_string(),
            "  ⚡ Command   Search commands only".to_string(),
            "  📄 Output    Search output only".to_string(),
            "  🔧 Advanced  Full filter interface".to_string(),
        ],
        HelpSection::AdvancedMode => vec![
            "Advanced Search Mode".to_string(),
            "".to_string(),
            "All basic mode keys plus:".to_string(),
            "".to_string(),
            "Sorting & Filtering:".to_string(),
            "  Ctrl+S       Cycle sort field".to_string(),
            "  Ctrl+R       Toggle sort order".to_string(),
            "  Ctrl+F       Toggle starred filter".to_string(),
            "  Ctrl+C       Clear all filters".to_string(),
            "".to_string(),
            "Quick Actions:".to_string(),
            "  *            Toggle star on selected".to_string(),
            "  A            Open actions menu".to_string(),
            "  C            Copy command".to_string(),
            "  O            Copy output".to_string(),
            "  R            Rerun command".to_string(),
            "  H            Insert as here-doc".to_string(),
            "  E            Export block".to_string(),
            "  T            Edit tags".to_string(),
            "  D/Del        Delete block".to_string(),
        ],
        HelpSection::Filters => vec![
            "Filters".to_string(),
            "".to_string(),
            "Available Filters:".to_string(),
            "  ⭐ Starred    Only starred blocks".to_string(),
            "  ✓/✗ Status   Success/failure status".to_string(),
            "  Shell        Filter by shell type".to_string(),
            "  Directory    Filter by working directory".to_string(),
            "  Duration     Filter by execution time".to_string(),
            "  Tags         Filter by assigned tags".to_string(),
            "  Date Range   Filter by creation date".to_string(),
            "".to_string(),
            "Filter Examples:".to_string(),
            "  status:success    Only successful commands".to_string(),
            "  shell:zsh        Only zsh commands".to_string(),
            "  duration:>5s     Commands taking >5 seconds".to_string(),
            "  #important       Blocks tagged 'important'".to_string(),
            "".to_string(),
            "Filters persist within your session.".to_string(),
        ],
        HelpSection::Actions => vec![
            "Block Actions".to_string(),
            "".to_string(),
            "Copy Actions:".to_string(),
            "  C    Copy command to clipboard".to_string(),
            "  O    Copy output to clipboard".to_string(),
            "  B    Copy both command and output".to_string(),
            "".to_string(),
            "Insert Actions:".to_string(),
            "  I    Insert command into prompt".to_string(),
            "  H    Insert output as standard here-doc".to_string(),
            "  J    Insert JSON output as jq here-doc".to_string(),
            "".to_string(),
            "Advanced Here-doc Options (in Actions menu):".to_string(),
            "  • Custom command here-doc (editable)".to_string(),
            "  • Shell-specific here-doc format".to_string(),
            "  • JSON processing with jq".to_string(),
            "".to_string(),
            "Management Actions:".to_string(),
            "  R    Rerun command".to_string(),
            "  *    Toggle star/bookmark".to_string(),
            "  T    Edit tags".to_string(),
            "  E    Export to file".to_string(),
            "  S    Share block (create permalink)".to_string(),
            "  V    View full output".to_string(),
            "  N    Create reusable snippet".to_string(),
            "  D    Delete block (with confirmation)".to_string(),
        ],
        HelpSection::Tips => vec![
            "Tips & Tricks".to_string(),
            "".to_string(),
            "Performance:".to_string(),
            "• Use specific search modes (Command/Output)".to_string(),
            "• Apply filters to narrow results".to_string(),
            "• Configure persistence per your workflow".to_string(),
            "".to_string(),
            "Here-doc Workflow:".to_string(),
            "• Standard here-doc for general use (H)".to_string(),
            "• JSON here-doc for structured data (J)".to_string(),
            "• Custom here-doc for specialized processing".to_string(),
            "• Shell-specific syntax awareness".to_string(),
            "".to_string(),
            "Organization:".to_string(),
            "• Tag blocks with descriptive labels".to_string(),
            "• Star frequently used commands".to_string(),
            "• Create reusable snippets from patterns".to_string(),
            "• Export important blocks for sharing".to_string(),
            "".to_string(),
            "Keyboard Shortcuts:".to_string(),
            "• Most actions work without opening menu".to_string(),
            "• Vi-style navigation (j/k) supported".to_string(),
            "• Access advanced features via 'A' menu".to_string(),
        ],
    }
}

/// Here-doc insertion utilities
pub fn generate_heredoc(output: &str) -> String {
    let delimiter = generate_delimiter(output);
    format!("cat << '{}'\n{}\n{}", delimiter, output.trim(), delimiter)
}

/// Generate here-doc with custom command prefix
pub fn generate_heredoc_with_command(output: &str, command_prefix: &str) -> String {
    let delimiter = generate_delimiter(output);
    format!(
        "{} << '{}'\n{}\n{}",
        command_prefix,
        delimiter,
        output.trim(),
        delimiter
    )
}

/// Generate here-doc delimiter that doesn't conflict with content
fn generate_delimiter(content: &str) -> String {
    let base = "EOF";
    let mut counter = 0;
    let mut delimiter = base.to_string();

    // If the content contains the delimiter, add numbers until unique
    while content.contains(&delimiter) {
        counter += 1;
        delimiter = format!("{}{}", base, counter);
    }

    delimiter
}

/// Format block output as here-doc for insertion
pub fn format_as_heredoc(command: &str, output: &str) -> String {
    let delimiter = generate_delimiter(output);

    format!(
        "cat << '{}' # Output from: {}\n{}\n{}",
        delimiter,
        command.trim(),
        output.trim(),
        delimiter
    )
}

/// Format block output as here-doc with variable assignment
pub fn format_as_variable_heredoc(var_name: &str, command: &str, output: &str) -> String {
    let delimiter = generate_delimiter(output);

    format!(
        "{}=$(cat << '{}'  # Output from: {}\n{}\n{})",
        var_name,
        delimiter,
        command.trim(),
        output.trim(),
        delimiter
    )
}

/// Format as pipe input for next command
pub fn format_as_pipe_input(output: &str) -> String {
    // For multi-line output, use printf
    if output.contains('\n') {
        let escaped = output.replace('\'', "'\\''");
        format!("printf '%s\\n' '{}' | ", escaped)
    } else {
        // For single-line output, use echo
        format!("echo '{}' | ", output.trim())
    }
}

/// Format as JSON here-doc for jq processing
pub fn format_as_json_heredoc(output: &str) -> String {
    let delimiter = generate_delimiter(output);
    format!(
        "jq '.' << '{}'\n{}\n{}",
        delimiter,
        output.trim(),
        delimiter
    )
}

/// Format as base64 encoded here-doc
pub fn format_as_base64_heredoc(output: &str) -> String {
    let delimiter = generate_delimiter(output);
    format!(
        "base64 -d << '{}' | {}\n{}\n{}",
        delimiter,
        "# Decoded data follows",
        output.trim(),
        delimiter
    )
}

/// Check if output is likely JSON
fn is_likely_json(output: &str) -> bool {
    let trimmed = output.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

/// Generate here-doc for different shell types
pub fn format_heredoc_for_shell(output: &str, shell_type: &str) -> String {
    let delimiter = generate_delimiter(output);

    match shell_type {
        "fish" => {
            // Fish shell here-doc syntax
            format!("cat << '{}'\n{}\n{}", delimiter, output.trim(), delimiter)
        }
        "powershell" | "pwsh" => {
            // PowerShell here-string syntax
            format!("@'\n{}\n'@", output.trim())
        }
        "zsh" | "bash" => {
            // Standard POSIX here-doc
            format!("cat << '{}'\n{}\n{}", delimiter, output.trim(), delimiter)
        }
        _ => {
            // Default to POSIX here-doc
            format!("cat << '{}'\n{}\n{}", delimiter, output.trim(), delimiter)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_action_availability() {
        let block = BlocksSearchItem {
            id: "test".to_string(),
            command: "echo hello".to_string(),
            output: "hello\n".to_string(),
            directory: "/tmp".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-01-01T00:00:00Z".to_string(),
            exit_code: Some(0),
            duration_ms: Some(100),
            starred: false,
            tags: vec![],
            shell: "bash".to_string(),
            status: "Success".to_string(),
        };

        assert!(BlockAction::CopyCommand.is_available_for(&block));
        assert!(BlockAction::CopyOutput.is_available_for(&block));
        assert!(BlockAction::InsertAsHereDoc.is_available_for(&block));
        assert!(BlockAction::RerunCommand.is_available_for(&block));

        let empty_block = BlocksSearchItem {
            command: "".to_string(),
            output: "".to_string(),
            ..block
        };

        assert!(!BlockAction::CopyCommand.is_available_for(&empty_block));
        assert!(!BlockAction::CopyOutput.is_available_for(&empty_block));
        assert!(BlockAction::ToggleStar.is_available_for(&empty_block)); // Always available
    }

    #[test]
    fn test_heredoc_generation() {
        let command = "ls -la";
        let output = "total 4\ndrwxr-xr-x 2 user user 4096 Jan 1 00:00 .";

        let heredoc = format_as_heredoc(command, output);
        assert!(heredoc.contains("cat << 'EOF'"));
        assert!(heredoc.contains("# Output from: ls -la"));
        assert!(heredoc.contains(output));
        assert!(heredoc.ends_with("EOF"));
    }

    #[test]
    fn test_delimiter_generation() {
        let command_with_eof = "echo EOF && cat file";
        let delimiter = generate_delimiter(command_with_eof);
        assert_ne!(delimiter, "EOF");
        assert!(delimiter.starts_with("EOF"));
    }

    #[test]
    fn test_actions_menu_state() {
        let mut menu = ActionsMenuState::new();
        assert!(!menu.active);

        let block = BlocksSearchItem {
            id: "test".to_string(),
            command: "echo test".to_string(),
            output: "test\n".to_string(),
            directory: "/tmp".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-01-01T00:00:00Z".to_string(),
            exit_code: Some(0),
            duration_ms: Some(50),
            starred: false,
            tags: vec![],
            shell: "bash".to_string(),
            status: "Success".to_string(),
        };

        menu.open_for_block(&block, Point::new(5, Column(10)));
        assert!(menu.active);
        assert!(!menu.actions.is_empty());
        assert_eq!(menu.selected, 0);

        // Test navigation
        let initial_count = menu.actions.len();
        menu.move_selection(1);
        assert_eq!(menu.selected, 1);

        menu.move_selection(-1);
        assert_eq!(menu.selected, 0);

        // Test wrap-around
        menu.move_selection(-1);
        assert_eq!(menu.selected, initial_count - 1);
    }
}

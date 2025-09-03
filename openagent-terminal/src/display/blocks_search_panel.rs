// Blocks Search Panel UI: state and rendering
// Feature-gated under `blocks`

#![cfg(feature = "blocks")]

use unicode_width::UnicodeWidthStr;

use crate::blocks_v2::{ExitCodeFilter, DurationFilter, SortField, SortOrder, ExecutionStatus, ShellType};
use crate::config::UiConfig;
use crate::config::theme::ThemeTokens;
use crate::display::color::Rgb;
use crate::display::{Display, SizeInfo};
use crate::renderer::rects::RenderRect;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

// Re-export action types for convenience
pub use crate::display::blocks_search_actions::{
    BlockAction, ActionsMenuState, HelpOverlayState
};

/// One item in the search results list (UI summary)
#[derive(Clone, Debug)]
pub struct BlocksSearchItem {
    pub id: String,
    pub command: String,
    pub output: String,
    pub directory: String,
    pub created_at: String,
    pub modified_at: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub starred: bool,
    pub tags: Vec<String>,
    pub shell: String,
    pub status: String,
}

/// Search mode for different types of filtering
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchMode {
    Basic,      // Simple text search
    Command,    // Search in commands only
    Output,     // Search in output only
    Advanced,   // Full filter interface
}

/// Filter state for advanced search
#[derive(Clone, Debug, Default)]
pub struct FilterState {
    pub directory: Option<PathBuf>,
    pub shell: Option<ShellType>,
    pub status: Option<ExecutionStatus>,
    pub exit_code: Option<ExitCodeFilter>,
    pub duration: Option<DurationFilter>,
    pub starred_only: bool,
    pub tags: Vec<String>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
}

/// Enhanced blocks search state with advanced filtering
#[derive(Clone, Debug)]
pub struct BlocksSearchState {
    pub active: bool,
    pub mode: SearchMode,
    pub query: String,
    pub results: Vec<BlocksSearchItem>,
    pub selected: usize,
    pub filters: FilterState,
    pub sort_field: SortField,
    pub sort_order: SortOrder,
    pub current_page: usize,
    pub items_per_page: usize,
    pub total_results: usize,
    pub search_in_progress: bool,
    pub filter_input_active: bool,
    pub filter_input_field: String,
    pub available_tags: Vec<String>,
    
    // New enhanced features
    /// Actions menu state for advanced operations
    pub actions_menu: ActionsMenuState,
    /// Help overlay state
    pub help_overlay: HelpOverlayState,
    /// Search history for suggestions
    pub search_history: Vec<String>,
    /// Quick filter suggestions
    pub filter_suggestions: Vec<String>,
}

impl BlocksSearchState {
    pub fn new() -> Self {
        Self {
            active: false,
            mode: SearchMode::Basic,
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            filters: FilterState::default(),
            sort_field: SortField::CreatedAt,
            sort_order: SortOrder::Descending,
            current_page: 0,
            items_per_page: 20,
            total_results: 0,
            search_in_progress: false,
            filter_input_active: false,
            filter_input_field: String::new(),
            available_tags: Vec::new(),
            // Initialize new enhanced features
            actions_menu: ActionsMenuState::new(),
            help_overlay: HelpOverlayState::new(),
            search_history: Vec::new(),
            filter_suggestions: Vec::new(),
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.selected = 0;
        self.current_page = 0;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.filter_input_active = false;
    }

    pub fn clear_results(&mut self) {
        self.results.clear();
        self.selected = 0;
        self.total_results = 0;
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.results.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.results.len() as isize;
        let mut idx = self.selected as isize + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx >= len {
            idx = len - 1;
        }
        self.selected = idx as usize;
    }

    pub fn next_page(&mut self) -> bool {
        let max_page = (self.total_results.saturating_sub(1)) / self.items_per_page;
        if self.current_page < max_page {
            self.current_page += 1;
            self.selected = 0;
            return true;
        }
        false
    }

    pub fn prev_page(&mut self) -> bool {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.selected = 0;
            return true;
        }
        false
    }

    pub fn cycle_search_mode(&mut self) {
        self.mode = match self.mode {
            SearchMode::Basic => SearchMode::Command,
            SearchMode::Command => SearchMode::Output,
            SearchMode::Output => SearchMode::Advanced,
            SearchMode::Advanced => SearchMode::Basic,
        };
    }

    pub fn toggle_sort_order(&mut self) {
        self.sort_order = match self.sort_order {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        };
    }

    pub fn cycle_sort_field(&mut self) {
        self.sort_field = match self.sort_field {
            SortField::CreatedAt => SortField::ModifiedAt,
            SortField::ModifiedAt => SortField::Command,
            SortField::Command => SortField::Duration,
            SortField::Duration => SortField::ExitCode,
            SortField::ExitCode => SortField::Directory,
            SortField::Directory => SortField::CreatedAt,
        };
    }

    pub fn toggle_starred_filter(&mut self) {
        self.filters.starred_only = !self.filters.starred_only;
    }

    pub fn add_tag_filter(&mut self, tag: String) {
        if !self.filters.tags.contains(&tag) {
            self.filters.tags.push(tag);
        }
    }

    pub fn remove_tag_filter(&mut self, tag: &str) {
        self.filters.tags.retain(|t| t != tag);
    }

    pub fn clear_all_filters(&mut self) {
        self.filters = FilterState::default();
    }

    pub fn has_active_filters(&self) -> bool {
        self.filters.directory.is_some()
            || self.filters.shell.is_some()
            || self.filters.status.is_some()
            || self.filters.exit_code.is_some()
            || self.filters.duration.is_some()
            || self.filters.starred_only
            || !self.filters.tags.is_empty()
            || self.filters.date_from.is_some()
            || self.filters.date_to.is_some()
    }

    pub fn get_selected_item(&self) -> Option<&BlocksSearchItem> {
        self.results.get(self.selected)
    }
    
    /// Open actions menu for currently selected item
    pub fn open_actions_menu(&mut self) {
        if let Some(item) = self.get_selected_item().cloned() {
            let position = Point::new(
                self.selected.min(self.results.len().saturating_sub(1)),
                Column(0)
            );
            self.actions_menu.open_for_block(&item, position);
        }
    }
    
    /// Close actions menu
    pub fn close_actions_menu(&mut self) {
        self.actions_menu.close();
    }
    
    /// Check if actions menu is active
    pub fn actions_menu_active(&self) -> bool {
        self.actions_menu.active
    }
    
    /// Move selection in actions menu
    pub fn move_actions_selection(&mut self, delta: isize) {
        self.actions_menu.move_selection(delta);
    }
    
    /// Get selected action from actions menu
    pub fn get_selected_action(&self) -> Option<BlockAction> {
        self.actions_menu.get_selected_action()
    }
    
    /// Open help overlay
    pub fn open_help(&mut self) {
        self.help_overlay.open();
    }
    
    /// Close help overlay
    pub fn close_help(&mut self) {
        self.help_overlay.close();
    }
    
    /// Check if help overlay is active
    pub fn help_active(&self) -> bool {
        self.help_overlay.active
    }
    
    /// Navigate help sections
    pub fn navigate_help(&mut self, forward: bool) {
        if forward {
            self.help_overlay.next_section();
        } else {
            self.help_overlay.prev_section();
        }
    }
    
    /// Add to search history (for suggestions)
    pub fn add_to_history(&mut self, query: &str) {
        if query.is_empty() || self.search_history.contains(&query.to_string()) {
            return;
        }
        
        self.search_history.insert(0, query.to_string());
        
        // Limit history size
        if self.search_history.len() > 50 {
            self.search_history.truncate(50);
        }
    }
    
    /// Get search suggestions based on current query
    pub fn get_search_suggestions(&self, partial: &str) -> Vec<String> {
        if partial.is_empty() {
            return self.search_history.iter().take(10).cloned().collect();
        }
        
        self.search_history
            .iter()
            .filter(|entry| entry.starts_with(partial))
            .take(10)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_selection_bounds() {
        let mut st = BlocksSearchState::new();
        st.results = vec![
            BlocksSearchItem {
                id: "1".into(),
                command: "a".into(),
                output: "out1".into(),
                directory: "d".into(),
                created_at: "t".into(),
                modified_at: "t".into(),
                exit_code: None,
                duration_ms: None,
                starred: false,
                tags: vec![],
                shell: "bash".into(),
                status: "Success".into(),
            },
            BlocksSearchItem {
                id: "2".into(),
                command: "b".into(),
                output: "out2".into(),
                directory: "d".into(),
                created_at: "t".into(),
                modified_at: "t".into(),
                exit_code: None,
                duration_ms: None,
                starred: false,
                tags: vec![],
                shell: "bash".into(),
                status: "Success".into(),
            },
            BlocksSearchItem {
                id: "3".into(),
                command: "c".into(),
                output: "out3".into(),
                directory: "d".into(),
                created_at: "t".into(),
                modified_at: "t".into(),
                exit_code: None,
                duration_ms: None,
                starred: false,
                tags: vec![],
                shell: "bash".into(),
                status: "Success".into(),
            },
        ];
        st.selected = 1;
        st.move_selection(-10);
        assert_eq!(st.selected, 0);
        st.move_selection(100);
        assert_eq!(st.selected, 2);
    }

    #[test]
    fn open_resets_selection_and_active() {
        let mut st = BlocksSearchState::new();
        st.selected = 5;
        st.active = false;
        st.open();
        assert!(st.active);
        assert_eq!(st.selected, 0);
        st.close();
        assert!(!st.active);
    }
}

impl Display {
    /// Draw the Blocks Search panel (bottom overlay) with advanced filtering UI
    pub fn draw_blocks_search_overlay(
        &mut self,
        config: &UiConfig,
        state: &BlocksSearchState,
    ) {
        if !state.active {
            return;
        }
        let size_info = self.size_info;
        let theme = config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Panel sizing: 40% of viewport height for advanced UI, min 8 lines
        let num_lines = size_info.screen_lines();
        let target_lines = if state.mode == SearchMode::Advanced {
            ((num_lines as f32 * 0.45).round() as usize).clamp(10, num_lines)
        } else {
            ((num_lines as f32 * 0.35).round() as usize).clamp(6, num_lines)
        };
        let start_line = num_lines.saturating_sub(target_lines);
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_h = target_lines as f32 * size_info.cell_height();

        // Backdrop and panel background
        let backdrop = RenderRect::new(0.0, 0.0, size_info.width(), size_info.height(), tokens.overlay, 0.20);
        let panel_bg = RenderRect::new(0.0, panel_y, size_info.width(), panel_h, tokens.surface_muted, 0.95);

        let rects = vec![backdrop, panel_bg];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy: SizeInfo = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        let num_cols = size_info.columns();
        let fg = tokens.text;
        let bg = tokens.surface_muted;
        let accent_fg = tokens.accent;
        let muted_fg = tokens.text_muted;
        let success_fg = tokens.success;
        let error_fg = tokens.error;

        let mut line = start_line;

        // Header with search mode, result count, and status
        self.draw_search_header(state, line, num_cols, fg, accent_fg, bg);
        line += 1;

        // Search query input with mode indicator
        self.draw_search_input(state, line, num_cols, fg, accent_fg, bg);
        line += 1;

        // Filter bar (if advanced mode or active filters)
        if state.mode == SearchMode::Advanced || state.has_active_filters() {
            self.draw_filter_bar(state, line, num_cols, fg, muted_fg, accent_fg, bg);
            line += 1;
        }

        // Sort and pagination info
        self.draw_sort_pagination_info(state, line, num_cols, muted_fg, bg);
        line += 1;

        // Separator
        let sep = "─".repeat(num_cols);
        self.draw_ai_text(Point::new(line, Column(0)), muted_fg, bg, &sep, num_cols);
        line += 1;

        // Results list with enhanced display
        let footer_lines = if state.mode == SearchMode::Advanced { 2 } else { 1 };
        let footer_start = start_line + target_lines - footer_lines;
        let max_result_lines = footer_start.saturating_sub(1);
        
        line = self.draw_results_list(state, line, max_result_lines, num_cols, fg, muted_fg, success_fg, error_fg, accent_fg, bg);

        // Footer with keyboard shortcuts
        self.draw_footer(state, footer_start, footer_lines, num_cols, muted_fg, bg);
        
        // Draw overlays on top
        if state.actions_menu.active {
            self.draw_actions_menu(&state.actions_menu, &tokens);
        }
        
        if state.help_overlay.active {
            self.draw_help_overlay(&state.help_overlay, &tokens);
        }
    }

    fn draw_search_header(
        &mut self,
        state: &BlocksSearchState,
        line: usize,
        num_cols: usize,
        fg: Rgb,
        accent_fg: Rgb,
        bg: Rgb,
    ) {
        let mode_str = match state.mode {
            SearchMode::Basic => "Basic",
            SearchMode::Command => "Command",
            SearchMode::Output => "Output",
            SearchMode::Advanced => "Advanced",
        };
        
        let status = if state.search_in_progress {
            "Searching..."
        } else if state.total_results > state.results.len() {
            "(Paginated)"
        } else {
            ""
        };

        let header = if state.total_results == 1 {
            format!("Blocks Search [{}] — 1 result {}", mode_str, status)
        } else {
            format!("Blocks Search [{}] — {} results {}", mode_str, state.total_results, status)
        };
        
        self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &header, num_cols - 2);
        
        // Show filter indicator if active
        if state.has_active_filters() {
            let filter_indicator = "🔧";
            let indicator_col = num_cols.saturating_sub(4);
            self.draw_ai_text(Point::new(line, Column(indicator_col)), accent_fg, bg, filter_indicator, 2);
        }
    }

    fn draw_search_input(
        &mut self,
        state: &BlocksSearchState,
        line: usize,
        num_cols: usize,
        fg: Rgb,
        accent_fg: Rgb,
        bg: Rgb,
    ) {
        let mode_prefix = match state.mode {
            SearchMode::Basic => "🔎 ",
            SearchMode::Command => "⚡ ",
            SearchMode::Output => "📄 ",
            SearchMode::Advanced => "🔧 ",
        };
        
        let mut prompt = String::with_capacity(mode_prefix.len() + state.query.len());
        prompt.push_str(mode_prefix);
        prompt.push_str(&state.query);
        
        self.draw_ai_text(Point::new(line, Column(0)), accent_fg, bg, mode_prefix, mode_prefix.len());
        self.draw_ai_text(Point::new(line, Column(mode_prefix.width())), fg, bg, &state.query, num_cols - mode_prefix.width());
        
        // Cursor
        let cursor_col = mode_prefix.width() + state.query.width();
        if cursor_col < num_cols {
            self.draw_ai_text(Point::new(line, Column(cursor_col)), bg, fg, " ", 1);
        }
    }

    fn draw_filter_bar(
        &mut self,
        state: &BlocksSearchState,
        line: usize,
        num_cols: usize,
        fg: Rgb,
        muted_fg: Rgb,
        accent_fg: Rgb,
        bg: Rgb,
    ) {
        let mut filter_parts = Vec::new();
        
        // Active filters display
        if state.filters.starred_only {
            filter_parts.push("⭐".to_string());
        }
        
        if let Some(status) = state.filters.status {
            filter_parts.push(format!("status:{:?}", status));
        }
        
        if let Some(shell) = state.filters.shell {
            filter_parts.push(format!("shell:{}", shell.to_str()));
        }
        
        if let Some(exit_filter) = state.filters.exit_code {
            let exit_str = match exit_filter {
                ExitCodeFilter::Success => "✓".to_string(),
                ExitCodeFilter::Failure => "✗".to_string(),
                ExitCodeFilter::Specific(code) => format!("exit:{}", code),
                ExitCodeFilter::Range(min, max) => format!("exit:{}-{}", min, max),
            };
            filter_parts.push(exit_str);
        }
        
        if !state.filters.tags.is_empty() {
            for tag in &state.filters.tags {
                filter_parts.push(format!("#{}", tag));
            }
        }
        
        if let Some(duration_filter) = state.filters.duration {
            let duration_str = match duration_filter {
                DurationFilter::LessThan(ms) => format!("<{}ms", ms),
                DurationFilter::GreaterThan(ms) => format!(">{}ms", ms),
                DurationFilter::Range(min, max) => format!("{}ms-{}ms", min, max),
            };
            filter_parts.push(duration_str);
        }
        
        let filter_text = if filter_parts.is_empty() {
            "Filters: none".to_string()
        } else {
            format!("Filters: {}", filter_parts.join(" "))
        };
        
        self.draw_ai_text(Point::new(line, Column(2)), if filter_parts.is_empty() { muted_fg } else { accent_fg }, bg, &filter_text, num_cols - 2);
    }

    fn draw_sort_pagination_info(
        &mut self,
        state: &BlocksSearchState,
        line: usize,
        num_cols: usize,
        muted_fg: Rgb,
        bg: Rgb,
    ) {
        let sort_str = match state.sort_field {
            SortField::CreatedAt => "created",
            SortField::ModifiedAt => "modified",
            SortField::Command => "command",
            SortField::Duration => "duration",
            SortField::ExitCode => "exit",
            SortField::Directory => "directory",
        };
        
        let order_str = match state.sort_order {
            SortOrder::Ascending => "↑",
            SortOrder::Descending => "↓",
        };
        
        let total_pages = (state.total_results.saturating_sub(1) / state.items_per_page) + 1;
        let current_page = state.current_page + 1;
        
        let info = format!("Sort: {} {}  •  Page {} of {}  •  {} per page", 
                          sort_str, order_str, current_page, total_pages, state.items_per_page);
        
        self.draw_ai_text(Point::new(line, Column(2)), muted_fg, bg, &info, num_cols - 2);
    }

    fn draw_results_list(
        &mut self,
        state: &BlocksSearchState,
        mut line: usize,
        max_lines: usize,
        num_cols: usize,
        fg: Rgb,
        muted_fg: Rgb,
        success_fg: Rgb,
        error_fg: Rgb,
        accent_fg: Rgb,
        bg: Rgb,
    ) -> usize {
        for (idx, item) in state.results.iter().enumerate() {
            if line > max_lines {
                break;
            }
            
            let is_selected = idx == state.selected;
            let row_fg = if is_selected { accent_fg } else { fg };
            
            // Selection indicator
            let selection_indicator = if is_selected { "▶ " } else { "  " };
            let mut col = 0;
            self.draw_ai_text(Point::new(line, Column(col)), row_fg, bg, selection_indicator, 2);
            col += 2;
            
            // Status indicator
            let (status_icon, status_color) = match item.exit_code {
                Some(0) => ("✓", success_fg),
                Some(_) => ("✗", error_fg),
                None => ("…", muted_fg),
            };
            self.draw_ai_text(Point::new(line, Column(col)), status_color, bg, status_icon, 1);
            col += 1;
            
            // Star indicator
            if item.starred {
                self.draw_ai_text(Point::new(line, Column(col)), accent_fg, bg, "⭐", 2);
                col += 2;
            } else {
                col += 1; // Space for alignment
            }
            
            // Command (truncated if needed)
            let remaining_width = num_cols.saturating_sub(col + 15); // Reserve space for metadata
            let cmd = &item.command;
            let truncated_cmd = if cmd.width() > remaining_width {
                let mut truncated: String = cmd.chars().take(remaining_width.saturating_sub(3)).collect();
                truncated.push_str("...");
                truncated
            } else {
                cmd.clone()
            };
            
            self.draw_ai_text(Point::new(line, Column(col)), row_fg, bg, &truncated_cmd, remaining_width);
            col = num_cols.saturating_sub(15);
            
            // Metadata: duration, shell, directory
            let metadata = format!(
                "{} {} {}",
                self.format_duration(item.duration_ms),
                item.shell,
                self.format_directory(&item.directory)
            );
            self.draw_ai_text(Point::new(line, Column(col)), muted_fg, bg, &metadata, 15);
            
            line += 1;
        }
        line
    }

    fn draw_footer(
        &mut self,
        state: &BlocksSearchState,
        footer_start: usize,
        footer_lines: usize,
        num_cols: usize,
        muted_fg: Rgb,
        bg: Rgb,
    ) {
        let mut line = footer_start;
        
        if state.mode == SearchMode::Advanced {
            // Advanced mode shortcuts - first line
            let advanced_hint1 = "Tab: Mode • Ctrl+S: Sort • Ctrl+R: Reverse • Ctrl+F: ⭐Filter • *: Star • A: Actions";
            self.draw_ai_text(Point::new(line, Column(2)), muted_fg, bg, advanced_hint1, num_cols - 2);
            line += 1;
            
            // Advanced mode shortcuts - second line
            let advanced_hint2 = "C: Copy Cmd • O: Copy Out • B: Copy Both • I: Insert Cmd • H: Here-doc • Ctrl+C: Clear";
            self.draw_ai_text(Point::new(line, Column(2)), muted_fg, bg, advanced_hint2, num_cols - 2);
        } else {
            // Basic navigation shortcuts
            let basic_hint = "Enter: Paste • Esc: Close • ↑/↓/j/k: Navigate • PgUp/PgDn: Page • Tab: Mode • ?: Help";
            self.draw_ai_text(Point::new(line, Column(2)), muted_fg, bg, basic_hint, num_cols - 2);
        }
    }
    
    fn format_duration(&self, duration_ms: Option<u64>) -> String {
        match duration_ms {
            Some(ms) if ms < 1000 => format!("{}ms", ms),
            Some(ms) if ms < 60_000 => format!("{:.1}s", ms as f64 / 1000.0),
            Some(ms) => format!("{}m{}s", ms / 60_000, (ms % 60_000) / 1000),
            None => "-".to_string(),
        }
    }
    
    fn format_directory(&self, dir: &str) -> String {
        if dir.len() > 10 {
            format!("…{}", &dir[dir.len().saturating_sub(9)..])
        } else {
            dir.to_string()
        }
    }
    
    fn draw_actions_menu(
        &mut self,
        menu: &ActionsMenuState,
        tokens: &ThemeTokens,
    ) {
        if !menu.active {
            return;
        }
        
        let size_info = self.size_info;
        let menu_width = 30;
        let menu_height = 15;
        let menu_x = (menu.position.column.0 as f32).min(size_info.width() - menu_width as f32 * size_info.cell_width());
        let menu_y = (menu.position.line as f32 * size_info.cell_height()).min(size_info.height() - menu_height as f32 * size_info.cell_height());
        
        // Menu background
        let menu_bg = RenderRect::new(menu_x, menu_y, menu_width as f32 * size_info.cell_width(), menu_height as f32 * size_info.cell_height(), tokens.surface, 0.95);
        let border_rect = RenderRect::new(menu_x - 1.0, menu_y - 1.0, (menu_width as f32 + 2.0) * size_info.cell_width(), (menu_height as f32 + 2.0) * size_info.cell_height(), tokens.border, 1.0);
        
        let rects = vec![border_rect, menu_bg];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy: SizeInfo = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);
        
        // Render menu items
        menu.render(self, tokens);
    }
    
    fn draw_help_overlay(
        &mut self,
        help: &HelpOverlayState,
        tokens: &ThemeTokens,
    ) {
        if !help.active {
            return;
        }
        
        let size_info = self.size_info;
        let overlay_width = size_info.width() * 0.8;
        let overlay_height = size_info.height() * 0.8;
        let overlay_x = (size_info.width() - overlay_width) / 2.0;
        let overlay_y = (size_info.height() - overlay_height) / 2.0;
        
        // Help background
        let help_bg = RenderRect::new(overlay_x, overlay_y, overlay_width, overlay_height, tokens.surface, 0.98);
        let border_rect = RenderRect::new(overlay_x - 2.0, overlay_y - 2.0, overlay_width + 4.0, overlay_height + 4.0, tokens.border, 1.0);
        
        let rects = vec![border_rect, help_bg];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy: SizeInfo = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);
        
        // Render help content
        help.render(self, tokens);
    }

    /// Helper reused from AI panel for drawing text; exists on all builds.
    pub(crate) fn draw_ai_text(
        &mut self,
        point: Point<usize>,
        fg: Rgb,
        bg: Rgb,
        text: &str,
        max_width: usize,
    ) {
        let truncated_text: String = if text.width() > max_width {
            text.chars().take(max_width).collect()
        } else {
            text.to_string()
        };

        let size_info_copy = self.size_info;
        match &mut self.backend {
            crate::display::Backend::Gl { renderer, .. } => {
                renderer.draw_string(
                    point,
                    fg,
                    bg,
                    truncated_text.chars(),
                    &size_info_copy,
                    &mut self.glyph_cache,
                );
            },
            #[cfg(feature = "wgpu")]
            crate::display::Backend::Wgpu { renderer } => {
                renderer.draw_string(
                    point,
                    fg,
                    bg,
                    truncated_text.chars(),
                    &size_info_copy,
                    &mut self.glyph_cache,
                );
            },
        }
    }
}


// Plugins Panel UI: state and rendering
// Feature-gated under `plugins`

use unicode_width::UnicodeWidthStr;

use crate::config::UiConfig;
use crate::display::{Display, SizeInfo};
use crate::renderer::rects::RenderRect;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};

#[derive(Clone, Debug)]
pub struct PluginItem {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub loaded: bool,
    pub path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PluginPanelState {
    pub active: bool,
    pub query: String,
    pub results: Vec<PluginItem>,
    pub selected: usize,
}

impl Default for PluginPanelState {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginPanelState {
    pub fn new() -> Self {
        Self { active: false, query: String::new(), results: Vec::new(), selected: 0 }
    }
    #[allow(dead_code)]
    pub fn open(&mut self) {
        self.active = true;
        self.selected = 0;
    }
    #[cfg(feature = "plugins-ui")]
    pub fn close(&mut self) {
        self.active = false;
    }
    #[cfg(feature = "plugins-ui")]
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
}

impl Display {
    /// Draw the Plugins panel (bottom overlay). This draws both background rects and text.
    pub fn draw_plugins_panel_overlay(&mut self, config: &UiConfig, state: &PluginPanelState) {
        if !state.active {
            return;
        }
        let size_info = self.size_info;
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Panel sizing: 40% of viewport height, min 8 lines
        let num_lines = size_info.screen_lines();
        let target_lines = ((num_lines as f32 * 0.40).round() as usize).clamp(8, num_lines);
        let start_line = num_lines.saturating_sub(target_lines);
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_h = target_lines as f32 * size_info.cell_height();

        // Backdrop + panel
        let rects = vec![
            RenderRect::new(0.0, 0.0, size_info.width(), size_info.height(), tokens.overlay, 0.20),
            RenderRect::new(0.0, panel_y, size_info.width(), panel_h, tokens.surface_muted, 0.95),
        ];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy: SizeInfo = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Header and content
        let num_cols = size_info.columns();
        let fg = tokens.text;
        let bg = tokens.surface_muted;

        let mut line = start_line;
        // Header with result count
        let count = state.results.len();
        let header = if count == 1 {
            format!("Plugins — {} result", count)
        } else {
            format!("Plugins — {} results", count)
        };
        self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &header, num_cols - 2);
        line += 1;

        // Query prompt
        let prompt_prefix = "🔌 ";
        let mut prompt = String::with_capacity(prompt_prefix.len() + state.query.len());
        prompt.push_str(prompt_prefix);
        prompt.push_str(&state.query);
        self.draw_ai_text(Point::new(line, Column(0)), fg, bg, &prompt, num_cols);
        let mut cursor_col = prompt_prefix.width() + state.query.width();
        if cursor_col >= num_cols {
            cursor_col = num_cols.saturating_sub(1);
        }
        self.draw_ai_text(Point::new(line, Column(cursor_col)), bg, fg, " ", 1);
        line += 1;

        // Separator
        let sep = "─".repeat(num_cols);
        self.draw_ai_text(Point::new(line, Column(0)), fg, bg, &sep, num_cols);
        line += 1;

        // Compute footer line and results area
        let footer_line = start_line + target_lines - 1;
        // Results list (reserve one line for footer)
        let max_lines = footer_line.saturating_sub(1);
        for (idx, item) in state.results.iter().enumerate() {
            if line > max_lines {
                break;
            }
            let mut row = String::new();
            row.push_str(if idx == state.selected { "▶ " } else { "  " });
            // Name and version
            row.push_str(&item.name);
            if let Some(ver) = &item.version {
                if !ver.is_empty() {
                    row.push_str(&format!(" {}", ver));
                }
            }
            // Status
            row.push_str(if item.loaded { " — [Loaded]" } else { " — [Available]" });
            // Path or description
            if let Some(path) = &item.path {
                row.push_str("  ");
                row.push_str(path);
            } else if let Some(desc) = &item.description {
                if !desc.is_empty() {
                    row.push_str(" — ");
                    row.push_str(desc);
                }
            }
            // Truncate to width
            let remaining = num_cols;
            if row.width() > remaining {
                let truncated: String = row.chars().take(remaining.saturating_sub(3)).collect();
                row = truncated + "...";
            }
            self.draw_ai_text(Point::new(line, Column(0)), fg, bg, &row, num_cols);
            line += 1;
        }
        // Footer controls/hints
        let hint = "Enter: Load/Unload  •  Esc: Close  •  ↑/↓/PgUp/PgDn: Navigate  •  Ctrl+N/Ctrl+P: Navigate  •  Tip: type 'http(s)://…' then Enter to install from URL";
        let hint_fg = tokens.text_muted;
        self.draw_ai_text(Point::new(footer_line, Column(2)), hint_fg, bg, hint, num_cols - 2);
    }
}

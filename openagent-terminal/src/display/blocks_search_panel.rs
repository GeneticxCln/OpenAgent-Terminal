// Blocks Search Panel UI: state and rendering
// Feature-gated under `blocks`

#![cfg(feature = "blocks")]

use unicode_width::UnicodeWidthStr;

use crate::config::UiConfig;
use crate::display::color::Rgb;
use crate::display::{Display, SizeInfo};
use crate::renderer::rects::RenderRect;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};

/// One item in the search results list (UI summary)
#[derive(Clone, Debug)]
pub struct BlocksSearchItem {
    pub id: String,
    pub command: String,
    pub directory: String,
    pub created_at: String,
    pub exit_code: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct BlocksSearchState {
    pub active: bool,
    pub query: String,
    pub results: Vec<BlocksSearchItem>,
    pub selected: usize,
}

impl BlocksSearchState {
    pub fn new() -> Self {
        Self { active: false, query: String::new(), results: Vec::new(), selected: 0 }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn clear_results(&mut self) {
        self.results.clear();
        self.selected = 0;
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.results.is_empty() { self.selected = 0; return; }
        let len = self.results.len() as isize;
        let mut idx = self.selected as isize + delta;
        if idx < 0 { idx = 0; }
        if idx >= len { idx = len - 1; }
        self.selected = idx as usize;
    }
}

impl Display {
    /// Draw the Blocks Search panel (bottom overlay). This draws both background rects and text.
    pub fn draw_blocks_search_overlay(
        &mut self,
        config: &UiConfig,
        state: &BlocksSearchState,
    ) {
        if !state.active { return; }
        let size_info = self.size_info;
        let theme = config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Panel sizing: 35% of viewport height, min 6 lines
        let num_lines = size_info.screen_lines();
        let target_lines = ((num_lines as f32 * 0.35).round() as usize).clamp(6, num_lines);
        let start_line = num_lines.saturating_sub(target_lines);
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_h = target_lines as f32 * size_info.cell_height();

        // Backdrop dim
        let backdrop = RenderRect::new(0.0, 0.0, size_info.width(), size_info.height(), tokens.overlay, 0.20);
        // Panel background
        let panel_bg = RenderRect::new(0.0, panel_y, size_info.width(), panel_h, tokens.surface_muted, 0.95);

        // Stage rects then draw them
        let mut rects = vec![backdrop, panel_bg];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy: SizeInfo = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Header and content
        let num_cols = size_info.columns();
        let fg = tokens.text;
        let bg = tokens.surface_muted;

        let mut line = start_line;
        // Header
        let header = "Blocks Search";
        self.draw_ai_text(Point::new(line, Column(2)), fg, bg, header, num_cols - 2);
        line += 1;

        // Query prompt
        let prompt_prefix = "🔎 ";
        let mut prompt = String::with_capacity(prompt_prefix.len() + state.query.len());
        prompt.push_str(prompt_prefix);
        prompt.push_str(&state.query);
        self.draw_ai_text(Point::new(line, Column(0)), fg, bg, &prompt, num_cols);
        let mut cursor_col = prompt_prefix.width() + state.query.width();
        if cursor_col >= num_cols { cursor_col = num_cols.saturating_sub(1); }
        self.draw_ai_text(Point::new(line, Column(cursor_col)), bg, fg, " ", 1);
        line += 1;

        // Separator
        let sep = "─".repeat(num_cols);
        self.draw_ai_text(Point::new(line, Column(0)), fg, bg, &sep, num_cols);
        line += 1;

        // Results list
        let max_lines = start_line + target_lines - 1; // keep last for padding
        for (idx, item) in state.results.iter().enumerate() {
            if line > max_lines { break; }
            let mut row = String::new();
            if idx == state.selected { row.push_str("▶ "); } else { row.push_str("  "); }
            // Command truncated to remaining width
            let remaining = num_cols.saturating_sub(row.width());
            let cmd = item.command.as_str();
            if cmd.width() > remaining {
                let truncated: String = cmd.chars().take(remaining.saturating_sub(3)).collect();
                row.push_str(&truncated);
                row.push_str("...");
            } else {
                row.push_str(cmd);
            }
            self.draw_ai_text(Point::new(line, Column(0)), fg, bg, &row, num_cols);
            line += 1;
        }
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


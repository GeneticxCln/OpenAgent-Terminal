// Notebook Panel UI: visual overlay for listing notebooks and cells
// Feature-gated under `blocks` since notebooks depend on Blocks infra.

use crate::config::UiConfig;
use crate::display::Display;
use crate::renderer::rects::RenderRect;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};

#[derive(Clone, Debug)]
pub struct NotebookListItem {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct NotebookCellItem {
    pub id: String,
    pub idx: i64,
    pub cell_type: String,
    pub summary: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusArea {
    Notebooks,
    Cells,
}

#[derive(Clone, Debug)]
pub struct NotebookPanelState {
    pub active: bool,
    pub notebooks: Vec<NotebookListItem>,
    pub selected_notebook: Option<String>,
    pub cells: Vec<NotebookCellItem>,
    pub selected_cell: usize,
    pub focus: FocusArea,
}

impl Default for NotebookPanelState {
    fn default() -> Self {
        Self::new()
    }
}

impl NotebookPanelState {
    pub fn new() -> Self {
        Self {
            active: false,
            notebooks: Vec::new(),
            selected_notebook: None,
            cells: Vec::new(),
            selected_cell: 0,
            focus: FocusArea::Notebooks,
        }
    }

    pub fn open(&mut self) {
        self.active = true;
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    #[allow(dead_code)]
    pub fn focus_notebooks(&mut self) {
        self.focus = FocusArea::Notebooks;
    }

    #[allow(dead_code)]
    pub fn focus_cells(&mut self) {
        self.focus = FocusArea::Cells;
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            FocusArea::Notebooks => FocusArea::Cells,
            FocusArea::Cells => FocusArea::Notebooks,
        };
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct NotebookEditSession {
    pub cell_id: String,
    pub path: std::path::PathBuf,
}

impl Display {
    pub fn draw_notebooks_panel_overlay(&mut self, config: &UiConfig, state: &NotebookPanelState) {
        if !state.active {
            return;
        }
        let size_info = self.size_info;
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Panel sizing: 40% height
        let lines = size_info.screen_lines();
        if lines == 0 {
            return;
        }
        let target_lines = ((lines as f32 * 0.40).round() as usize).clamp(8, lines);
        let start_line = lines.saturating_sub(target_lines);
        let y = start_line as f32 * size_info.cell_height();
        let h = (target_lines as f32) * size_info.cell_height();

        // Backdrop and panel BG
        let rects = vec![
            RenderRect::new(0.0, 0.0, size_info.width(), size_info.height(), tokens.overlay, 0.15),
            RenderRect::new(0.0, y, size_info.width(), h, tokens.surface_muted, 0.95),
        ];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Columns: left = notebooks 30%, right = cells 70%
        let cols_total = size_info.columns();
        let left_cols = (cols_total as f32 * 0.3).max(20.0) as usize;
        let right_cols = cols_total.saturating_sub(left_cols + 3);
        let mut line = start_line;

        // Header
        self.draw_ai_text(
            Point::new(line, Column(2)),
            tokens.text,
            tokens.surface_muted,
            "Notebooks",
            cols_total - 4,
        );
        line += 1;

        // Draw notebooks list
        let mut nline = line;
        let notebooks_header =
            if matches!(state.focus, FocusArea::Notebooks) { "Notebooks ◉" } else { "Notebooks" };
        self.draw_ai_text(
            Point::new(nline, Column(2)),
            tokens.text_muted,
            tokens.surface_muted,
            notebooks_header,
            left_cols - 4,
        );
        nline += 1;
        let selected_nb_idx = state
            .selected_notebook
            .as_ref()
            .and_then(|id| state.notebooks.iter().position(|n| &n.id == id));
        for (i, nb) in state.notebooks.iter().enumerate() {
            let selected = Some(i) == selected_nb_idx;
            // Draw highlight background when focused and selected
            if selected && matches!(state.focus, FocusArea::Notebooks) {
                let line_y = y + ((nline - line) as f32) * size_info.cell_height();
                let rect = RenderRect::new(
                    0.0,
                    line_y,
                    (left_cols as f32) * size_info.cell_width(),
                    size_info.cell_height(),
                    tokens.surface,
                    0.18,
                );
                let metrics = self.glyph_cache.font_metrics();
                let size_copy = self.size_info;
                self.renderer_draw_rects(&size_copy, &metrics, vec![rect]);
            }
            let prefix = if selected { "▸ " } else { "  " };
            let name = format!("{}{}", prefix, nb.name);
            let color = tokens.text;
            self.draw_ai_text(
                Point::new(nline, Column(2)),
                color,
                tokens.surface_muted,
                &name,
                left_cols - 4,
            );
            nline += 1;
            if nline >= start_line + target_lines {
                break;
            }
        }

        // Draw selected notebook cells
        let mut cline = line;
        let header = match &state.selected_notebook {
            Some(id) => format!(
                "Cells — {}{}",
                id,
                if matches!(state.focus, FocusArea::Cells) { " ◉" } else { "" }
            ),
            None => "Cells".to_string(),
        };
        self.draw_ai_text(
            Point::new(cline, Column(left_cols + 4)),
            tokens.text_muted,
            tokens.surface_muted,
            &header,
            right_cols,
        );
        cline += 1;
        for (i, cell) in state.cells.iter().enumerate() {
            let status = match cell.exit_code {
                Some(0) => "✔",
                Some(_) => "✖",
                None => "…",
            };
            let selected = i == state.selected_cell && matches!(state.focus, FocusArea::Cells);
            if selected {
                let line_y = y + ((cline - line) as f32) * size_info.cell_height();
                let rect = RenderRect::new(
                    (left_cols as f32 + 4.0) * size_info.cell_width(),
                    line_y,
                    ((right_cols) as f32) * size_info.cell_width(),
                    size_info.cell_height(),
                    tokens.surface,
                    0.18,
                );
                let metrics = self.glyph_cache.font_metrics();
                let size_copy = self.size_info;
                self.renderer_draw_rects(&size_copy, &metrics, vec![rect]);
            }
            let label = format!("#{:>2} [{}] {}", cell.idx, cell.cell_type, cell.summary);
            let full = if selected {
                format!("▸ {}  {}", status, label)
            } else {
                format!("  {}  {}", status, label)
            };
            let color = match cell.exit_code {
                Some(0) => tokens.accent,
                Some(_) => tokens.warning,
                None => tokens.text_muted,
            };
            self.draw_ai_text(
                Point::new(cline, Column(left_cols + 4)),
                color,
                tokens.surface_muted,
                &full,
                right_cols,
            );
            cline += 1;
            if cline >= start_line + target_lines {
                break;
            }
        }

        // Footer controls hint
        let footer = start_line + target_lines - 1;
        self.draw_ai_text(
            Point::new(footer, Column(2)),
            tokens.text_muted,
            tokens.surface_muted,
            "Enter: Run cell   Shift+Enter: Run all   Esc: Close",
            cols_total - 4,
        );
    }
}

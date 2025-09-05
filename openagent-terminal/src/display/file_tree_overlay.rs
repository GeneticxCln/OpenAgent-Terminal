//! Native file tree overlay (Warp-like)

use std::path::{PathBuf};

use crate::config::UiConfig;
use crate::display::{Display};
use crate::renderer::rects::RenderRect;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Debug)]
pub struct FileTreeItem {
    pub path: PathBuf,
    pub is_dir: bool,
}

#[derive(Clone, Debug)]
pub struct FileTreeOverlayState {
    pub active: bool,
    pub root: PathBuf,
    pub items: Vec<FileTreeItem>,
    pub selected: usize,
    pub scroll: usize,
}

impl Default for FileTreeOverlayState {
    fn default() -> Self {
        Self { active: false, root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")), items: Vec::new(), selected: 0, scroll: 0 }
    }
}

impl FileTreeOverlayState {
    pub fn new() -> Self { Self::default() }
}

impl Display {
    pub fn file_tree_open(&mut self, root: Option<PathBuf>) {
        let rootp = root.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        // Simple scan: list immediate children; for nested browsing, user can select dirs to change root
        let mut items = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&rootp) {
            for e in rd.flatten() {
                let path = e.path();
                let is_dir = path.is_dir();
                items.push(FileTreeItem { path, is_dir });
            }
        }
        items.sort_by(|a,b| a.path.file_name().unwrap_or_default().cmp(b.path.file_name().unwrap_or_default()));
        self.file_tree.active = true;
        self.file_tree.root = rootp;
        self.file_tree.items = items;
        self.file_tree.selected = 0;
        self.file_tree.scroll = 0;
        self.pending_update.dirty = true;
    }

    pub fn file_tree_close(&mut self) {
        self.file_tree.active = false;
        self.pending_update.dirty = true;
    }

    pub fn file_tree_move_selection(&mut self, delta: isize) {
        if self.file_tree.items.is_empty() { return; }
        let len = self.file_tree.items.len() as isize;
        let mut idx = self.file_tree.selected as isize + delta;
        if idx < 0 { idx = 0; }
        if idx >= len { idx = len - 1; }
        self.file_tree.selected = idx as usize;
        // simple scroll follow
        let panel_lines = (self.size_info.screen_lines() as f32 * 0.80).round() as usize - 4;
        if self.file_tree.selected < self.file_tree.scroll { self.file_tree.scroll = self.file_tree.selected; }
        if self.file_tree.selected >= self.file_tree.scroll + panel_lines { self.file_tree.scroll = self.file_tree.selected.saturating_sub(panel_lines-1); }
        self.pending_update.dirty = true;
    }

    pub fn file_tree_confirm(&mut self) {
        if let Some(item) = self.file_tree.items.get(self.file_tree.selected).cloned() {
            if item.is_dir {
                self.file_tree_open(Some(item.path));
            } else {
                // Open in editor overlay (native)
                #[cfg(feature = "editor")]
                { self.editor_overlay_open(item.path); }
                self.file_tree_close();
            }
        }
    }

    pub fn draw_file_tree_overlay(&mut self, config: &UiConfig, state: &FileTreeOverlayState) {
        if !state.active { return; }
        let size = self.size_info;
        let theme = config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Backdrop
        let backdrop = RenderRect::new(0.0, 0.0, size.width(), size.height(), tokens.overlay, 0.20);
        // Left drawer: 30% width, 90% height
        let cols = size.columns();
        let lines = size.screen_lines();
        let panel_cols = (cols as f32 * 0.30).round() as usize;
        let panel_lines = (lines as f32 * 0.90).round() as usize;
        let start_col = 1usize;
        let start_line = (lines.saturating_sub(panel_lines))/2;
        let x = start_col as f32 * size.cell_width();
        let y = start_line as f32 * size.cell_height();
        let w = panel_cols as f32 * size.cell_width();
        let h = panel_lines as f32 * size.cell_height();
        let panel_bg = RenderRect::new(x, y, w, h, tokens.surface, 0.98);
        let rects = vec![backdrop, panel_bg];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Header
        let header = format!("File Tree — {}", state.root.display());
        self.draw_ai_text(Point::new(start_line, Column(start_col+1)), tokens.text, tokens.surface, &header, panel_cols.saturating_sub(2));

        // List items
        let visible = panel_lines.saturating_sub(2);
        for (i, it) in state.items.iter().enumerate().skip(state.scroll).take(visible) {
            let line = start_line + 1 + (i - state.scroll);
            let mut name = it.path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if it.is_dir { name.push('/'); }
            // truncate to width
            let maxw = panel_cols.saturating_sub(2);
            let mut txt = String::new();
            let mut wsum = 0;
            for ch in name.chars() { let w = ch.width().unwrap_or(1); if wsum + w > maxw { break; } txt.push(ch); wsum += w; }
            if i == state.selected {
                // selection bg line
                let sel_bg = RenderRect::new(x, line as f32 * size.cell_height(), w, size.cell_height(), tokens.surface_muted, 0.90);
                let rects = vec![sel_bg];
                let metrics = self.glyph_cache.font_metrics();
                let size_copy = self.size_info;
                self.renderer_draw_rects(&size_copy, &metrics, rects);
            }
            self.draw_ai_text(Point::new(line, Column(start_col+1)), tokens.text, tokens.surface, &txt, maxw);
        }
    }
}


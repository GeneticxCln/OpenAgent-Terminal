//! Multi-pane rendering for the Display module.
//!
//! Renders multiple terminal panes within a single frame. Each pane draws its own
//! terminal grid and cursor into its rectangle. Borders and the tab bar are drawn once.

use std::collections::HashMap;
use std::sync::Arc;

use openagent_terminal_core::sync::FairMutex;
use openagent_terminal_core::term::Term;

use crate::config::UiConfig;
use crate::display::content::RenderableContent;
use crate::display::cursor::IntoRects;
use crate::display::{Display, SizeInfo};
use crate::event::{EventProxy, SearchState};
use crate::message_bar::MessageBuffer;
use crate::renderer::rects::RenderRect;
use crate::scheduler::Scheduler;
use crate::workspace::split_manager::PaneRect;
use crate::workspace::PaneId;
use openagent_terminal_core::grid::Dimensions as TermDimensions;

impl Display {
    /// Draw a frame containing all panes and shared overlays.
    pub fn draw_multipane_frame(
        &mut self,
        terminals: &HashMap<PaneId, Arc<FairMutex<Term<EventProxy>>>>,
        rectangles: &HashMap<PaneId, PaneRect>,
        focused_pane: Option<PaneId>,
        config: &UiConfig,
        search_state: &mut SearchState,
        _scheduler: &mut Scheduler,
        message_buffer: &MessageBuffer,
        #[cfg(feature = "ai")] ai_state: Option<&crate::ai_runtime::AiUiState>,
        tab_manager: Option<&crate::workspace::TabManager>,
    ) {
        self.make_current();
        let bg = config.colors.primary.background;
        self.renderer_clear(bg, config.window_opacity());

        if terminals.is_empty() || rectangles.is_empty() {
            return;
        }

        // Save original size to restore projection/viewport later.
        let old_size = self.size_info;

        // Draw each pane grid.
        for (pane_id, rect) in rectangles.iter() {
            if let Some(term_arc) = terminals.get(pane_id) {
                let is_focused = Some(*pane_id) == focused_pane;
                self.draw_one_pane(term_arc, *rect, is_focused, config, search_state, old_size);
            }
        }

        // Restore full-window projection/viewport for overlays and text.
        match &mut self.backend {
            crate::display::Backend::Gl { renderer, .. } => {
                self.size_info = old_size;
                renderer.resize(&self.size_info);
            }
            #[cfg(feature = "wgpu")]
            crate::display::Backend::Wgpu { .. } => {
                self.size_info = old_size;
            }
        }

        // Draw pane borders.
        self.draw_pane_borders(rectangles, focused_pane);

        // Draw tab bar once if available.
        if let Some(tm) = tab_manager {
            let position = config.workspace.tab_bar.position;
            let _ = self.draw_tab_bar(config, tm, position);
        }

        // Visual bell overlay.
        let vb = self.visual_bell.intensity();
        if vb != 0.0 {
            let rect = RenderRect::new(0.0, 0.0, self.size_info.width(), self.size_info.height(), config.bell.color, vb as f32);
            let metrics = self.glyph_cache.font_metrics();
            let size_copy = self.size_info;
            self.renderer_draw_rects(&size_copy, &metrics, vec![rect]);
        }

        // AI ghost suggestion at cursor (optional).
        #[cfg(feature = "ai")]
        if let (Some(fid), Some(ai)) = (focused_pane, ai_state) {
            if !ai.active {
                if let Some(term_arc) = terminals.get(&fid) {
                    let term = term_arc.lock();
                    let cursor_point = term.grid().cursor.point;
                    let display_offset = term.grid().display_offset();
                    drop(term);
                    if let Some(vp) = openagent_terminal_core::term::point_to_viewport(display_offset, cursor_point) {
                        let start_col = vp.column.0;
                        let cols = self.size_info.columns();
                        if vp.line < self.size_info.screen_lines() && start_col < cols {
                            let avail = cols - start_col;
                            let theme = config
                                .resolved_theme
                                .as_ref()
                                .cloned()
                                .unwrap_or_else(|| config.theme.resolve());
                            let fg = theme.tokens.text_muted;
                            let bg = config.colors.primary.background;
                            let point = openagent_terminal_core::index::Point::new(
                                vp.line,
                                openagent_terminal_core::index::Column(start_col),
                            );
                            if let Some(suffix) = ai.inline_suggestion.as_ref() {
                                if !suffix.is_empty() {
                                    self.draw_ai_panel_text(point, fg, bg, suffix, avail);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Message bar overlay.
        if let Some(message) = message_buffer.message() {
            let metrics = self.glyph_cache.font_metrics();
            let size_info = self.size_info;
            let search_offset = usize::from(search_state.regex().is_some());
            let text = message.text(&size_info);
            let start_line = size_info.screen_lines() + search_offset;
            let y = size_info.cell_height().mul_add(start_line as f32, size_info.padding_y());
            let theme =
                config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
            let bg = match message.ty() {
                crate::message_bar::MessageType::Error => theme.tokens.error,
                crate::message_bar::MessageType::Warning => theme.tokens.warning,
            };
            let rect = RenderRect::new(0.0, y, size_info.width(), size_info.height() - y, bg, 1.0);
            self.renderer_draw_rects(&size_info, &metrics, vec![rect]);
            let fg = theme.tokens.surface;
            for (i, line) in text.iter().enumerate() {
                let point = openagent_terminal_core::index::Point::new(
                    start_line + i,
                    openagent_terminal_core::index::Column(0),
                );
                match &mut self.backend {
                    crate::display::Backend::Gl { renderer, .. } => {
                        renderer.draw_string(point, fg, bg, line.chars(), &size_info, &mut self.glyph_cache);
                    }
                    #[cfg(feature = "wgpu")]
                    crate::display::Backend::Wgpu { renderer } => {
                        renderer.draw_string(point, fg, bg, line.chars(), &size_info, &mut self.glyph_cache);
                    }
                }
            }
        }

        // Present for GL backend.
        self.window.pre_present_notify();
        if matches!(self.backend, crate::display::Backend::Gl { .. }) {
            self.swap_buffers();
        }
    }

    fn draw_one_pane(
        &mut self,
        terminal: &Arc<FairMutex<Term<EventProxy>>>,
        rect: PaneRect,
        is_focused: bool,
        config: &UiConfig,
        search_state: &mut SearchState,
        old_size: SizeInfo,
    ) {
        // Build pane-local SizeInfo and configure backend state.
        match &mut self.backend {
            crate::display::Backend::Gl { renderer, .. } => {
                // For GL: use viewport to position; projection uses pane size.
                let pane_size = SizeInfo::new(
                    rect.width + 2.0 * rect.x,
                    rect.height + 2.0 * rect.y,
                    old_size.cell_width(),
                    old_size.cell_height(),
                    rect.x,
                    rect.y,
                    false,
                );
                self.size_info = pane_size;
                renderer.resize(&self.size_info);
            }
            #[cfg(feature = "wgpu")]
            crate::display::Backend::Wgpu { .. } => {
                // For WGPU: positions are absolute via padding; projection stays window-based.
                let pane_size = SizeInfo::new(
                    rect.width,
                    rect.height,
                    old_size.cell_width(),
                    old_size.cell_height(),
                    rect.x,
                    rect.y,
                    false,
                );
                self.size_info = pane_size;
            }
        }

        // Build renderable content for this pane.
        let term = terminal.lock();
        let mut content = RenderableContent::new(config, self, &term, search_state);
        let mut cells = Vec::new();
        for c in &mut content {
            cells.push(c);
        }
        let cursor = content.cursor();
        drop(term);

        // Draw cell grid for this pane.
        match &mut self.backend {
            crate::display::Backend::Gl { renderer, .. } => {
                renderer.draw_cells(&self.size_info, &mut self.glyph_cache, cells.into_iter());
            }
            #[cfg(feature = "wgpu")]
            crate::display::Backend::Wgpu { renderer } => {
                renderer.draw_cells(&self.size_info, &mut self.glyph_cache, cells.into_iter());
            }
        }

        // Draw cursor if focused.
        if is_focused {
            let metrics = self.glyph_cache.font_metrics();
            let rects: Vec<RenderRect> = cursor.rects(&self.size_info, config.cursor.thickness()).collect();
            if !rects.is_empty() {
                let size_copy = self.size_info;
                self.renderer_draw_rects(&size_copy, &metrics, rects);
            }
        }
    }

    fn draw_pane_borders(
        &mut self,
        rectangles: &HashMap<PaneId, PaneRect>,
        focused: Option<PaneId>,
    ) {
        let mut rects = Vec::new();
        let bw = 1.0f32;
        let normal = crate::display::color::Rgb::new(100, 100, 100);
        let focus = crate::display::color::Rgb::new(200, 200, 255);

        for (pid, r) in rectangles.iter() {
            let color = if Some(*pid) == focused { focus } else { normal };
            // Right edge
            rects.push(RenderRect::new(r.x + r.width - bw, r.y, bw, r.height, color, 1.0));
            // Bottom edge
            rects.push(RenderRect::new(r.x, r.y + r.height - bw, r.width, bw, color, 1.0));
        }

        if !rects.is_empty() {
            let metrics = self.glyph_cache.font_metrics();
            let size_copy = self.size_info;
            self.renderer_draw_rects(&size_copy, &metrics, rects);
        }
    }
}

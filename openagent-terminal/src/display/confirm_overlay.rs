// Confirmation Overlay UI: state and rendering

use unicode_width::{UnicodeWidthStr, UnicodeWidthChar};

use crate::config::UiConfig;
use crate::display::{Display, SizeInfo};
use crate::renderer::rects::RenderRect;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};

#[derive(Clone, Debug, Default)]
pub struct ConfirmOverlayState {
    pub active: bool,
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub confirm_label: String,
    pub cancel_label: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_close_overlay_updates_state() {
        let mut st = ConfirmOverlayState::new();
        assert!(!st.active);
        assert!(st.id.is_none());
        st.open(
            "id-1".into(),
            "Title".into(),
            "Body".into(),
            Some("Confirm".into()),
            Some("Cancel".into()),
        );
        assert!(st.active);
        assert_eq!(st.id.as_deref(), Some("id-1"));
        assert_eq!(st.title, "Title");
        assert_eq!(st.body, "Body");
        st.close_if("id-1");
        assert!(!st.active);
        assert!(st.id.is_none());
        assert!(st.title.is_empty());
    }
}

impl ConfirmOverlayState {
    pub fn new() -> Self {
        Self { active: false, id: None, title: String::new(), body: String::new(), confirm_label: "Confirm".into(), cancel_label: "Cancel".into() }
    }

    pub fn open(&mut self, id: String, title: String, body: String, confirm_label: Option<String>, cancel_label: Option<String>) {
        self.active = true;
        self.id = Some(id);
        self.title = title;
        self.body = body;
        if let Some(c) = confirm_label { self.confirm_label = c; }
        if let Some(c) = cancel_label { self.cancel_label = c; }
    }

    pub fn close_if(&mut self, id: &str) {
        if self.id.as_deref() == Some(id) {
            self.active = false;
            self.id = None;
            self.title.clear();
            self.body.clear();
        }
    }
}

impl Display {
    /// Draw the confirmation overlay (centered modal with backdrop)
    pub fn draw_confirm_overlay(&mut self, config: &UiConfig, state: &ConfirmOverlayState) {
        if !state.active { return; }
        let size_info = self.size_info;
        let theme = config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Backdrop dim
        let backdrop = RenderRect::new(0.0, 0.0, size_info.width(), size_info.height(), tokens.overlay, 0.25);

        // Modal sizing: 60% width, min 40 cols; height fits content up to 40% of screen
        let cols = size_info.columns();
        let lines = size_info.screen_lines();
        let modal_cols = (cols as f32 * 0.60).round() as usize;
        let modal_cols = modal_cols.clamp(40, cols.saturating_sub(4));
        let modal_lines = (lines as f32 * 0.40).round() as usize;
        let modal_lines = modal_lines.clamp(6, lines.saturating_sub(4));

        let x_col = (cols.saturating_sub(modal_cols)) / 2;
        let y_line = (lines.saturating_sub(modal_lines)) / 2;

        let x = x_col as f32 * size_info.cell_width();
        let y = y_line as f32 * size_info.cell_height();
        let w = modal_cols as f32 * size_info.cell_width();
        let h = modal_lines as f32 * size_info.cell_height();

        let panel_bg = RenderRect::new(x, y, w, h, tokens.surface_muted, 0.98);

        // Stage rects
        let rects = vec![backdrop, panel_bg];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy: SizeInfo = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Draw title
        let mut line = y_line + 1;
        let title = format!("{}", state.title);
        self.draw_ai_text(Point::new(line, Column(x_col + 2)), tokens.text, tokens.surface_muted, &title, modal_cols.saturating_sub(4));
        line += 2;

        // Draw body wrapped
        let max_width = modal_cols.saturating_sub(4);
        for raw in state.body.lines() {
            let mut text = raw.to_string();
            while text.width() > 0 {
                let take = max_width.min(text.width());
                let mut slice = String::new();
                let mut count = 0;
                for ch in text.chars() {
                    let w = ch.width().unwrap_or(0);
                    if count + w > max_width { break; }
                    slice.push(ch);
                    count += w;
                    if count >= take { break; }
                }
                self.draw_ai_text(Point::new(line, Column(x_col + 2)), tokens.text, tokens.surface_muted, &slice, max_width);
                line += 1;
                text.replace_range(..slice.len(), "");
                if line >= y_line + modal_lines.saturating_sub(3) { break; }
            }
            if line >= y_line + modal_lines.saturating_sub(3) { break; }
        }

        // Draw footer with instructions
        let footer = format!("Enter = {}    Esc = {}    (Y/N)", state.confirm_label, state.cancel_label);
        self.draw_ai_text(Point::new(y_line + modal_lines - 2, Column(x_col + 2)), tokens.text_muted, tokens.surface_muted, &footer, modal_cols.saturating_sub(4));
    }
}

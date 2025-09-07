//! Native editor overlay state and rendering

#![allow(dead_code)]

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[cfg(feature = "editor")]
use openagent_terminal_ide_editor::EditorBuffer;

use std::path::PathBuf;

use crate::config::UiConfig;
use crate::display::Display;
use crate::renderer::rects::RenderRect;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};
use std::path::Path;

#[derive(Default)]
pub struct EditorOverlayState {
    pub active: bool,
    pub file_path: Option<PathBuf>,
    #[cfg(feature = "editor")]
    pub buffer: Option<EditorBuffer>,
    pub scroll_line: usize,
    // LSP integration (optional)
    #[cfg(feature = "lsp")]
    pub lsp: Option<openagent_terminal_ide_lsp::LspClient>,
    #[cfg(feature = "lsp")]
    pub lsp_uri: Option<lsp_types::Url>,
    #[cfg(feature = "lsp")]
    pub language_id: Option<String>,
    // Completion UI state
    pub completion_active: bool,
    pub completion_items: Vec<String>,
    pub completion_selected: usize,
    #[cfg(feature = "lsp")]
    pub completion_items_full: Vec<lsp_types::CompletionItem>,
    // Diagnostics for current file
    #[cfg(feature = "lsp")]
    pub diagnostics: Vec<lsp_types::Diagnostic>,
    // References UI
    #[cfg(feature = "lsp")]
    pub references_active: bool,
    #[cfg(feature = "lsp")]
    pub references: Vec<lsp_types::Location>,
    #[cfg(feature = "lsp")]
    pub references_selected: usize,
    // Rename UI
    #[cfg(feature = "lsp")]
    pub rename_active: bool,
    #[cfg(feature = "lsp")]
    pub rename_text: String,
    // Hover and signature help
    #[cfg(feature = "lsp")]
    pub hover_active: bool,
    #[cfg(feature = "lsp")]
    pub hover_text: String,
    #[cfg(feature = "lsp")]
    pub signature_active: bool,
    #[cfg(feature = "lsp")]
    pub signature_label: String,
}

#[cfg(feature = "lsp")]
fn lsp_server_config_for_language(lang: &str) -> Option<openagent_terminal_ide_lsp::ServerConfig> {
    match lang {
        "rust" => Some(openagent_terminal_ide_lsp::ServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            initialization_options: None,
        }),
        "typescript" | "javascript" => Some(openagent_terminal_ide_lsp::ServerConfig {
            command: "typescript-language-server".into(),
            args: vec!["--stdio".into()],
            initialization_options: None,
        }),
        "python" => Some(openagent_terminal_ide_lsp::ServerConfig {
            command: "pyright-langserver".into(),
            args: vec!["--stdio".into()],
            initialization_options: None,
        }),
        _ => None,
    }
}

fn guess_language_from_path(path: &Path) -> String {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "rs" => "rust",
        "ts" | "tsx" | "js" | "jsx" => "typescript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "cpp" | "cxx" | "cc" | "h" | "hpp" | "hh" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        _ => "plaintext",
    }
    .to_string()
}

impl Display {
    #[cfg(feature = "lsp")]
    pub fn editor_overlay_poll_lsp(&mut self) {
        if let Some(client) = self.editor_overlay.lsp.as_ref() {
            while let Some(note) = client.try_recv_notification() {
                match note {
                    openagent_terminal_ide_lsp::LspNotification::PublishDiagnostics(params) => {
                        if let Some(cur_uri) = self.editor_overlay.lsp_uri.clone() {
                            if params.uri == cur_uri {
                                self.editor_overlay.diagnostics = params.diagnostics;
                                self.pending_update.dirty = true;
                            }
                        }
                    },
                }
            }
        }
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_request_completion(&mut self) {
        if let (Some(client), Some(uri), Some(buf)) = (
            self.editor_overlay.lsp.as_ref(),
            self.editor_overlay.lsp_uri.clone(),
            self.editor_overlay.buffer.as_ref(),
        ) {
            let pos = lsp_types::Position {
                line: buf.cursor.read().line as u32,
                character: buf.cursor.read().column as u32,
            };
            let tdpp = lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: pos,
            };
            if let Ok(resp) = client.completion(tdpp) {
                #[cfg(feature = "lsp")]
                {
                    let items_full: Vec<lsp_types::CompletionItem> = match resp.clone() {
                        lsp_types::CompletionResponse::Array(v) => v,
                        lsp_types::CompletionResponse::List(l) => l.items,
                    };
                    self.editor_overlay.completion_items_full = items_full.clone();
                    let items: Vec<String> = items_full.into_iter().map(|i| i.label).collect();
                    self.editor_overlay.completion_items = items;
                }
                #[cfg(not(feature = "lsp"))]
                {
                    let items: Vec<String> = match resp {
                        lsp_types::CompletionResponse::Array(v) => {
                            v.into_iter().map(|i| i.label).collect()
                        },
                        lsp_types::CompletionResponse::List(l) => {
                            l.items.into_iter().map(|i| i.label).collect()
                        },
                    };
                    self.editor_overlay.completion_items = items;
                }
                self.editor_overlay.completion_selected = 0;
                self.editor_overlay.completion_active =
                    !self.editor_overlay.completion_items.is_empty();
                self.pending_update.dirty = true;
            }
        }
    }

    pub fn editor_overlay_completion_visible(&self) -> bool {
        self.editor_overlay.completion_active
    }

    pub fn editor_overlay_completion_move(&mut self, delta: isize) {
        if self.editor_overlay.completion_items.is_empty() {
            return;
        }
        let len = self.editor_overlay.completion_items.len() as isize;
        let mut idx = self.editor_overlay.completion_selected as isize + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx >= len {
            idx = len - 1;
        }
        self.editor_overlay.completion_selected = idx as usize;
        self.pending_update.dirty = true;
    }

    pub fn editor_overlay_completion_accept(&mut self) {
        if !self.editor_overlay.completion_active {
            return;
        }
        #[cfg(feature = "lsp")]
        {
            if let Some(item) = self
                .editor_overlay
                .completion_items_full
                .get(self.editor_overlay.completion_selected)
                .cloned()
            {
                self.apply_completion_item(item);
            }
        }
        #[cfg(not(feature = "lsp"))]
        {
            if let Some(text) = self
                .editor_overlay
                .completion_items
                .get(self.editor_overlay.completion_selected)
                .cloned()
            {
                #[cfg(feature = "editor")]
                if let Some(buf) = &self.editor_overlay.buffer {
                    for ch in text.chars() {
                        buf.insert(ch);
                    }
                }
            }
        }
        self.editor_overlay.completion_active = false;
        self.pending_update.dirty = true;
    }
}

impl Clone for EditorOverlayState {
    fn clone(&self) -> Self {
        Self {
            active: self.active,
            file_path: self.file_path.clone(),
            #[cfg(feature = "editor")]
            buffer: self.buffer.clone(),
            scroll_line: self.scroll_line,
            #[cfg(feature = "lsp")]
            lsp: None,
            #[cfg(feature = "lsp")]
            lsp_uri: self.lsp_uri.clone(),
            #[cfg(feature = "lsp")]
            language_id: self.language_id.clone(),
            completion_active: self.completion_active,
            completion_items: self.completion_items.clone(),
            completion_selected: self.completion_selected,
            #[cfg(feature = "lsp")]
            completion_items_full: self.completion_items_full.clone(),
            #[cfg(feature = "lsp")]
            diagnostics: self.diagnostics.clone(),
            #[cfg(feature = "lsp")]
            references_active: self.references_active,
            #[cfg(feature = "lsp")]
            references: self.references.clone(),
            #[cfg(feature = "lsp")]
            references_selected: self.references_selected,
            #[cfg(feature = "lsp")]
            rename_active: self.rename_active,
            #[cfg(feature = "lsp")]
            rename_text: self.rename_text.clone(),
            #[cfg(feature = "lsp")]
            hover_active: self.hover_active,
            #[cfg(feature = "lsp")]
            hover_text: self.hover_text.clone(),
            #[cfg(feature = "lsp")]
            signature_active: self.signature_active,
            #[cfg(feature = "lsp")]
            signature_label: self.signature_label.clone(),
        }
    }
}


impl EditorOverlayState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn close(&mut self) {
        self.active = false;
        self.file_path = None;
        #[cfg(feature = "editor")]
        {
            self.buffer = None;
        }
        self.scroll_line = 0;
    }

    pub fn open_path(&mut self, path: PathBuf) {
        #[cfg(feature = "editor")]
        {
            match EditorBuffer::open_file(path.clone()) {
                Ok(buf) => {
                    self.buffer = Some(buf);
                    self.file_path = Some(path.clone());
                    self.active = true;
                    self.scroll_line = 0;
                    self.completion_active = false;
                    self.completion_items.clear();
                    self.completion_selected = 0;
                    // LSP init if available
                    #[cfg(feature = "lsp")]
                    {
                        let lang = guess_language_from_path(&path);
                        self.language_id = Some(lang.clone());
                        if let Ok(uri) = lsp_types::Url::from_file_path(&path) {
                            self.lsp_uri = Some(uri.clone());
                            let cfg = lsp_server_config_for_language(&lang);
                            if let Some(cfg) = cfg {
                                if let Ok(client) = openagent_terminal_ide_lsp::LspClient::start(
                                    &cfg,
                                    Some(uri.clone()),
                                ) {
                                    let text =
                                        self.buffer.as_ref().map(|b| b.text()).unwrap_or_default();
                                    let _ = client.open_document(uri.clone(), &lang, &text);
                                    self.lsp = Some(client);
                                }
                            }
                        }
                    }
                },
                Err(_e) => {
                    self.active = false;
                },
            }
        }
        #[cfg(not(feature = "editor"))]
        {
            let _ = path; // feature disabled
            self.active = false;
        }
    }

    pub fn save(&self) {
        #[cfg(feature = "editor")]
        if let Some(buf) = &self.buffer {
            let _ = buf.save();
        }
    }
}

#[cfg(feature = "lsp")]
impl Display {
    fn sanitize_snippet(&self, s: &str) -> String {
        // Very basic: remove $0, ${1:...} -> ...
        let mut out = String::new();
        let mut i = 0;
        let bytes = s.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'$' {
                // Skip $0 or ${n:...}
                if i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                    i += 2;
                    continue;
                }
                if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                    // find closing '}'
                    i += 2; // skip ${
                    while i < bytes.len() && bytes[i] != b':' && bytes[i] != b'}' {
                        i += 1;
                    }
                    if i < bytes.len() && bytes[i] == b':' {
                        i += 1;
                    }
                    // copy placeholder content until '}'
                    while i < bytes.len() && bytes[i] != b'}' {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                    if i < bytes.len() && bytes[i] == b'}' {
                        i += 1;
                    }
                    continue;
                }
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        out
    }

    fn apply_completion_item(&mut self, item: lsp_types::CompletionItem) {
        if let Some(buf) = &self.editor_overlay.buffer {
            let mut inserted = false;
            // Apply text edit if present
            if let Some(edit) = item.text_edit.clone() {
                match edit {
                    lsp_types::CompletionTextEdit::Edit(e) => {
                        let s = self.lsp_pos_to_char_index_buf(buf, e.range.start);
                        let eidx = self.lsp_pos_to_char_index_buf(buf, e.range.end);
                        let mut rope_mut = buf.rope.write();
                        rope_mut.remove(s..eidx);
                        let text = if item.insert_text_format
                            == Some(lsp_types::InsertTextFormat::SNIPPET)
                        {
                            self.sanitize_snippet(&e.new_text)
                        } else {
                            e.new_text
                        };
                        rope_mut.insert(s, &text);
                        inserted = true;
                    },
                    lsp_types::CompletionTextEdit::InsertAndReplace(ir) => {
                        let s = self.lsp_pos_to_char_index_buf(buf, ir.replace.start);
                        let eidx = self.lsp_pos_to_char_index_buf(buf, ir.replace.end);
                        let mut rope_mut = buf.rope.write();
                        rope_mut.remove(s..eidx);
                        let text = if item.insert_text_format
                            == Some(lsp_types::InsertTextFormat::SNIPPET)
                        {
                            self.sanitize_snippet(&ir.new_text)
                        } else {
                            ir.new_text
                        };
                        rope_mut.insert(s, &text);
                        inserted = true;
                    },
                }
            }
            if !inserted {
                // Fallback to insertText or label
                let text = item.insert_text.clone().unwrap_or(item.label.clone());
                let text = if item.insert_text_format == Some(lsp_types::InsertTextFormat::SNIPPET)
                {
                    self.sanitize_snippet(&text)
                } else {
                    text
                };
                for ch in text.chars() {
                    buf.insert(ch);
                }
            }
            // Full sync
            if let (Some(client), Some(uri)) =
                (self.editor_overlay.lsp.as_ref(), self.editor_overlay.lsp_uri.clone())
            {
                let version = buf.meta.read().version;
                let change = lsp_types::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: buf.text(),
                };
                let _ = client.change_document(uri, version, vec![change]);
            }
        }
    }
}

impl Display {
    pub fn draw_editor_overlay(&mut self, config: &UiConfig, state: &EditorOverlayState) {
        if !state.active {
            return;
        }
        let size = self.size_info;
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Backdrop
        let backdrop = RenderRect::new(0.0, 0.0, size.width(), size.height(), tokens.overlay, 0.20);

        // Panel geometry (centered, 80% width x 80% height)
        let cols = size.columns();
        let lines = size.screen_lines();
        let panel_cols = (cols as f32 * 0.80).round() as usize;
        let panel_lines = (lines as f32 * 0.80).round() as usize;
        let start_col = (cols.saturating_sub(panel_cols)) / 2;
        let start_line = (lines.saturating_sub(panel_lines)) / 2;

        let x = start_col as f32 * size.cell_width();
        let y = start_line as f32 * size.cell_height();
        let w = panel_cols as f32 * size.cell_width();
        let h = panel_lines as f32 * size.cell_height();

        // Panel background
        let panel_bg = RenderRect::new(x, y, w, h, tokens.surface, 0.98);

        let rects = vec![backdrop, panel_bg];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Draw file path in header
        let header = if let Some(p) = state.file_path.as_ref() {
            p.display().to_string()
        } else {
            "Untitled".into()
        };
        self.draw_ai_text(
            Point::new(start_line, Column(start_col + 2)),
            tokens.text,
            tokens.surface,
            &header,
            panel_cols.saturating_sub(4),
        );

        // Content area: from line start_line+2 to end-2
        let content_top = start_line + 2;
        let content_lines = panel_lines.saturating_sub(3);

        #[cfg(feature = "editor")]
        if let Some(buf) = &state.buffer {
            // Render visible lines from rope
            let text_all = buf.text();
            for (yline, (i, raw)) in text_all
                .lines()
                .enumerate()
                .skip(state.scroll_line)
                .take(content_lines)
                .enumerate()
            {
                let line = content_top + yline;
                let mut text = raw.to_string();
                // Trim to width
                let maxw = panel_cols.saturating_sub(4);
                if text.width() > maxw {
                    // naive clipping by chars
                    let mut acc = String::new();
                    let mut wsum = 0;
                    for ch in text.chars() {
                        let w = ch.width().unwrap_or(1);
                        if wsum + w > maxw {
                            break;
                        }
                        acc.push(ch);
                        wsum += w;
                    }
                    text = acc;
                }
                self.draw_ai_text(
                    Point::new(line, Column(start_col + 2)),
                    tokens.text,
                    tokens.surface,
                    &text,
                    panel_cols.saturating_sub(4),
                );

                // Diagnostics underline for this line
                #[cfg(feature = "lsp")]
                {
                    let dl = i;
                    for d in &state.diagnostics {
                        let start = d.range.start;
                        let end = d.range.end;
                        if (start.line as usize) <= dl && (end.line as usize) >= dl {
                            let seg_start = if start.line as usize == dl {
                                start.character as usize
                            } else {
                                0
                            };
                            let seg_end = if end.line as usize == dl {
                                end.character as usize
                            } else {
                                panel_cols.saturating_sub(4)
                            };
                            if seg_start < seg_end {
                                let ux = (start_col + 2 + seg_start) as f32 * size.cell_width();
                                let uw = (seg_end - seg_start).max(1) as f32 * size.cell_width();
                                let uy = (line as f32 + 0.85) * size.cell_height();
                                let col = match d
                                    .severity
                                    .unwrap_or(lsp_types::DiagnosticSeverity::HINT)
                                {
                                    lsp_types::DiagnosticSeverity::ERROR => tokens.accent,
                                    lsp_types::DiagnosticSeverity::WARNING => tokens.warning,
                                    lsp_types::DiagnosticSeverity::INFORMATION => tokens.text_muted,
                                    lsp_types::DiagnosticSeverity::HINT => tokens.text_muted,
                                    _ => tokens.text_muted,
                                };
                                let rect = RenderRect::new(
                                    ux,
                                    uy,
                                    uw,
                                    size.cell_height() * 0.08,
                                    col,
                                    0.95,
                                );
                                let metrics = self.glyph_cache.font_metrics();
                                let size_copy = self.size_info;
                                self.renderer_draw_rects(&size_copy, &metrics, vec![rect]);
                            }
                        }
                    }
                }

            }

            // Caret drawing (vertical bar)
            let cur = buf.cursor.read().clone();
            let caret_line = cur.line;
            if caret_line >= state.scroll_line && caret_line < state.scroll_line + content_lines {
                let vis_line = content_top + (caret_line - state.scroll_line);
                let vis_col = start_col + 2 + cur.column;
                let cx = vis_col as f32 * size.cell_width();
                let cy = vis_line as f32 * size.cell_height();
                let rect = RenderRect::new(cx, cy, 2.0, size.cell_height(), tokens.text, 0.95);
                let metrics = self.glyph_cache.font_metrics();
                let size_copy = self.size_info;
                self.renderer_draw_rects(&size_copy, &metrics, vec![rect]);
            }
        }

        // Completion popup
        if state.completion_active && !state.completion_items.is_empty() {
            let popup_cols = panel_cols.saturating_sub(10).min(60);
            let popup_lines = state.completion_items.len().min(8);
            let px = (start_col + 2) as f32 * size.cell_width();
            let py = (content_top + 1) as f32 * size.cell_height();
            let pw = popup_cols as f32 * size.cell_width();
            let ph = popup_lines as f32 * size.cell_height();
            let bg = RenderRect::new(px, py, pw, ph, tokens.surface_muted, 0.98);
            let rects = vec![bg];
            let metrics = self.glyph_cache.font_metrics();
            let size_copy = self.size_info;
            self.renderer_draw_rects(&size_copy, &metrics, rects);
            for i in 0..popup_lines {
                let idx = i;
                let line = content_top + 1 + i;
                let label = state.completion_items.get(idx).cloned().unwrap_or_default();
                let mut text = label;
                if text.width() > popup_cols - 2 {
                    text.truncate((popup_cols - 2).saturating_sub(1));
                }
                if idx == state.completion_selected {
                    let selbg = RenderRect::new(
                        px,
                        line as f32 * size.cell_height(),
                        pw,
                        size.cell_height(),
                        tokens.surface,
                        0.92,
                    );
                    let rects = vec![selbg];
                    let metrics = self.glyph_cache.font_metrics();
                    let size_copy = self.size_info;
                    self.renderer_draw_rects(&size_copy, &metrics, rects);
                }
                self.draw_ai_text(
                    Point::new(line, Column(start_col + 3)),
                    tokens.text,
                    tokens.surface_muted,
                    &text,
                    popup_cols.saturating_sub(2),
                );
            }
        }

        // References popup
        #[cfg(feature = "lsp")]
        if state.references_active && !state.references.is_empty() {
            let popup_cols = panel_cols.saturating_sub(10).min(80);
            let popup_lines = state.references.len().min(8);
            let px = (start_col + panel_cols - popup_cols - 2) as f32 * size.cell_width();
            let py = (content_top + 1) as f32 * size.cell_height();
            let pw = popup_cols as f32 * size.cell_width();
            let ph = popup_lines as f32 * size.cell_height();
            let bg = RenderRect::new(px, py, pw, ph, tokens.surface_muted, 0.98);
            let rects = vec![bg];
            let metrics = self.glyph_cache.font_metrics();
            let size_copy = self.size_info;
            self.renderer_draw_rects(&size_copy, &metrics, rects);
            for i in 0..popup_lines {
                let idx = i;
                let line = content_top + 1 + i;
                let label = state
                    .references
                    .get(idx)
                    .map(|loc| {
                        format!(
                            "{}:{}:{}",
                            loc.uri.path().rsplit('/').next().unwrap_or(""),
                            loc.range.start.line + 1,
                            loc.range.start.character + 1
                        )
                    })
                    .unwrap_or_default();
                let mut text = label;
                if text.width() > popup_cols - 2 {
                    text.truncate((popup_cols - 2).saturating_sub(1));
                }
                if idx == state.references_selected {
                    let selbg = RenderRect::new(
                        px,
                        line as f32 * size.cell_height(),
                        pw,
                        size.cell_height(),
                        tokens.surface,
                        0.92,
                    );
                    let rects = vec![selbg];
                    let metrics = self.glyph_cache.font_metrics();
                    let size_copy = self.size_info;
                    self.renderer_draw_rects(&size_copy, &metrics, rects);
                }
                self.draw_ai_text(
                    Point::new(line, Column(start_col + panel_cols - popup_cols - 1)),
                    tokens.text,
                    tokens.surface_muted,
                    &text,
                    popup_cols.saturating_sub(2),
                );
            }
        }

        // Rename prompt
        #[cfg(feature = "lsp")]
        if state.rename_active {
            let prompt = format!("Rename to: {}", state.rename_text);
            let px = (start_col + 2) as f32 * size.cell_width();
            let py = (start_line + 1) as f32 * size.cell_height();
            let pw = (prompt.width().min(panel_cols.saturating_sub(4))) as f32 * size.cell_width();
            let ph = size.cell_height();
            let bg = RenderRect::new(px, py, pw.max(100.0), ph, tokens.surface_muted, 0.98);
            let rects = vec![bg];
            let metrics = self.glyph_cache.font_metrics();
            let size_copy = self.size_info;
            self.renderer_draw_rects(&size_copy, &metrics, rects);
            self.draw_ai_text(
                Point::new(start_line + 1, Column(start_col + 3)),
                tokens.text,
                tokens.surface_muted,
                &prompt,
                panel_cols.saturating_sub(6),
            );
        }

        // Hover panel
        #[cfg(feature = "lsp")]
        if state.hover_active && !state.hover_text.is_empty() {
            let lines_to_show = 3usize;
            let mut shown = Vec::new();
            for (i, l) in state.hover_text.lines().enumerate() {
                if i >= lines_to_show {
                    break;
                }
                shown.push(l);
            }
            let content = shown.join(" ");
            let popup_cols = content.width().min(panel_cols.saturating_sub(6));
            let px = (start_col + 2) as f32 * size.cell_width();
            let py = (start_line + panel_lines.saturating_sub(3)) as f32 * size.cell_height();
            let pw = (popup_cols as f32 * size.cell_width()).max(150.0);
            let ph = 3_f32 * size.cell_height();
            let bg = RenderRect::new(px, py, pw, ph, tokens.surface_muted, 0.98);
            let rects = vec![bg];
            let metrics = self.glyph_cache.font_metrics();
            let size_copy = self.size_info;
            self.renderer_draw_rects(&size_copy, &metrics, rects);
            self.draw_ai_text(
                Point::new(start_line + panel_lines.saturating_sub(3), Column(start_col + 3)),
                tokens.text,
                tokens.surface_muted,
                &content,
                panel_cols.saturating_sub(6),
            );
        }

        // Signature help (one-line)
        #[cfg(feature = "lsp")]
        if state.signature_active && !state.signature_label.is_empty() {
            let label = &state.signature_label;
            let px = (start_col + 2) as f32 * size.cell_width();
            let py = (start_line + 1) as f32 * size.cell_height();
            let pw = (label.width().min(panel_cols.saturating_sub(6)) as f32 * size.cell_width())
                .max(120.0);
            let ph = size.cell_height();
            let bg = RenderRect::new(px, py, pw, ph, tokens.surface_muted, 0.98);
            let rects = vec![bg];
            let metrics = self.glyph_cache.font_metrics();
            let size_copy = self.size_info;
            self.renderer_draw_rects(&size_copy, &metrics, rects);
            self.draw_ai_text(
                Point::new(start_line + 1, Column(start_col + 3)),
                tokens.text,
                tokens.surface_muted,
                label,
                panel_cols.saturating_sub(6),
            );
        }
    }

    pub fn editor_overlay_open(&mut self, path: PathBuf) {
        #[cfg(feature = "editor")]
        {
            self.editor_overlay.open_path(path);
        }
    }

    pub fn editor_overlay_close(&mut self) {
        self.editor_overlay.close();
    }

    pub fn editor_overlay_save(&mut self) {
        self.editor_overlay.save();
    }

    // Cursor navigation helpers
    #[cfg(feature = "editor")]
    pub fn editor_overlay_move_left(&mut self) {
        if let Some(buf) = &self.editor_overlay.buffer {
            buf.move_left();
        }
        self.ensure_cursor_visible();
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "editor")]
    pub fn editor_overlay_move_right(&mut self) {
        if let Some(buf) = &self.editor_overlay.buffer {
            buf.move_right();
        }
        self.ensure_cursor_visible();
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "editor")]
    pub fn editor_overlay_move_up(&mut self) {
        if let Some(buf) = &self.editor_overlay.buffer {
            buf.move_up();
        }
        self.ensure_cursor_visible();
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "editor")]
    pub fn editor_overlay_move_down(&mut self) {
        if let Some(buf) = &self.editor_overlay.buffer {
            buf.move_down();
        }
        self.ensure_cursor_visible();
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "editor")]
    pub fn editor_overlay_page_up(&mut self) {
        let lines = (self.size_info.screen_lines() as f32 * 0.80).round() as usize - 4;
        self.editor_overlay.scroll_line = self.editor_overlay.scroll_line.saturating_sub(lines);
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "editor")]
    pub fn editor_overlay_page_down(&mut self) {
        let lines = (self.size_info.screen_lines() as f32 * 0.80).round() as usize - 4;
        self.editor_overlay.scroll_line = self.editor_overlay.scroll_line.saturating_add(lines);
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "lsp")]
    fn lsp_tdpp_at_cursor(&self) -> Option<lsp_types::TextDocumentPositionParams> {
        let uri = self.editor_overlay.lsp_uri.clone()?;
        let buf = self.editor_overlay.buffer.as_ref()?;
        let cur = buf.cursor.read().clone();
        let pos = lsp_types::Position { line: cur.line as u32, character: cur.column as u32 };
        Some(lsp_types::TextDocumentPositionParams {
            text_document: lsp_types::TextDocumentIdentifier { uri },
            position: pos,
        })
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_goto_definition(&mut self) {
        if let (Some(client), Some(tdpp)) =
            (self.editor_overlay.lsp.as_ref(), self.lsp_tdpp_at_cursor())
        {
            if let Ok(resp) = client.definition(tdpp) {
                match resp {
                    lsp_types::GotoDefinitionResponse::Scalar(loc) => self.jump_to_location(loc),
                    lsp_types::GotoDefinitionResponse::Array(mut arr) => {
                        if let Some(loc) = arr.pop() {
                            self.jump_to_location(loc)
                        }
                    },
                    lsp_types::GotoDefinitionResponse::Link(mut links) => {
                        if let Some(link) = links.pop() {
                            self.jump_to_location(lsp_types::Location {
                                uri: link.target_uri,
                                range: link.target_range,
                            })
                        }
                    },
                }
            }
        }
    }

    #[cfg(feature = "lsp")]
    fn jump_to_location(&mut self, loc: lsp_types::Location) {
        // If same file, just move cursor; else open new file
        if let Some(cur_uri) = self.editor_overlay.lsp_uri.clone() {
            if loc.uri == cur_uri {
                if let Some(buf) = &self.editor_overlay.buffer {
                    let mut c = buf.cursor.write();
                    c.line = loc.range.start.line as usize;
                    c.column = loc.range.start.character as usize;
                }
                self.ensure_cursor_visible();
                self.pending_update.dirty = true;
                return;
            }
        }
        if let Ok(path) = loc.uri.to_file_path() {
            self.editor_overlay_open(path);
        }
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_show_references(&mut self) {
        if let (Some(client), Some(tdpp)) =
            (self.editor_overlay.lsp.as_ref(), self.lsp_tdpp_at_cursor())
        {
            let context = lsp_types::ReferenceContext { include_declaration: true };
            let params = lsp_types::ReferenceParams {
                text_document_position: tdpp,
                context,
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            };
            if let Ok(locs) = client.references(params) {
                self.editor_overlay.references = locs;
                self.editor_overlay.references_selected = 0;
                self.editor_overlay.references_active = !self.editor_overlay.references.is_empty();
                self.pending_update.dirty = true;
            }
        }
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_references_move(&mut self, delta: isize) {
        if !self.editor_overlay.references_active || self.editor_overlay.references.is_empty() {
            return;
        }
        let len = self.editor_overlay.references.len() as isize;
        let mut idx = self.editor_overlay.references_selected as isize + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx >= len {
            idx = len - 1;
        }
        self.editor_overlay.references_selected = idx as usize;
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_references_accept(&mut self) {
        if !self.editor_overlay.references_active {
            return;
        }
        if let Some(loc) =
            self.editor_overlay.references.get(self.editor_overlay.references_selected).cloned()
        {
            self.editor_overlay.references_active = false;
            self.jump_to_location(loc);
        }
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_toggle_rename(&mut self) {
        self.editor_overlay.rename_active = !self.editor_overlay.rename_active;
        if self.editor_overlay.rename_active {
            self.editor_overlay.rename_text.clear();
        }
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_rename_input(&mut self, ch: char) {
        self.editor_overlay.rename_text.push(ch);
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_rename_backspace(&mut self) {
        self.editor_overlay.rename_text.pop();
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_rename_commit(&mut self) {
        if !self.editor_overlay.rename_active {
            return;
        }
        let new_name = self.editor_overlay.rename_text.clone();
        if new_name.is_empty() {
            self.editor_overlay.rename_active = false;
            self.pending_update.dirty = true;
            return;
        }
        if let (Some(client), Some(tdpp)) =
            (self.editor_overlay.lsp.as_ref(), self.lsp_tdpp_at_cursor())
        {
            let params = lsp_types::RenameParams {
                text_document_position: tdpp,
                new_name,
                work_done_progress_params: Default::default(),
            };
            if let Ok(edit) = client.rename(params) {
                self.apply_workspace_edit(edit);
            }
        }
        self.editor_overlay.rename_active = false;
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "lsp")]
    fn apply_workspace_edit(&mut self, edit: lsp_types::WorkspaceEdit) {
        if let Some(changes) = edit.changes {
            for (uri, edits) in changes {
                if let Some(cur_uri) = self.editor_overlay.lsp_uri.clone() {
                    if uri != cur_uri {
                        continue;
                    }
                }
                if let Some(buf) = &self.editor_overlay.buffer {
                    let mut rope = buf.rope.write();
                    // Apply edits back-to-front
                    let mut ranges: Vec<(usize, usize, String)> = Vec::new();
                    for e in edits {
                        let s = self.lsp_pos_to_char_index_buf(buf, e.range.start);
                        let eidx = self.lsp_pos_to_char_index_buf(buf, e.range.end);
                        ranges.push((s, eidx, e.new_text));
                    }
                    ranges.sort_by(|a, b| b.0.cmp(&a.0));
                    for (s, eidx, txt) in ranges {
                        rope.remove(s..eidx);
                        rope.insert(s, &txt);
                    }
                }
            }
        }
    }

    #[cfg(feature = "lsp")]
    fn lsp_pos_to_char_index_buf(
        &self,
        buf: &openagent_terminal_ide_editor::EditorBuffer,
        pos: lsp_types::Position,
    ) -> usize {
        let rope = buf.rope.read();
        let line = pos.line as usize;
        let col = pos.character as usize;
        let line_start = rope.line_to_char(line);
        line_start + col
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_format_document(&mut self) {
        if let (Some(client), Some(uri), Some(buf)) = (
            self.editor_overlay.lsp.as_ref(),
            self.editor_overlay.lsp_uri.clone(),
            self.editor_overlay.buffer.as_ref(),
        ) {
            let options = lsp_types::FormattingOptions {
                tab_size: 4,
                insert_spaces: true,
                ..Default::default()
            };
            let params = lsp_types::DocumentFormattingParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                options,
                work_done_progress_params: Default::default(),
            };
            if let Ok(edits) = client.formatting(params) {
                let mut rope = buf.rope.write();
                // Apply edits back-to-front
                let mut ranges: Vec<(usize, usize, String)> = Vec::new();
                for e in edits {
                    let s = self.lsp_pos_to_char_index_buf(buf, e.range.start);
                    let eidx = self.lsp_pos_to_char_index_buf(buf, e.range.end);
                    ranges.push((s, eidx, e.new_text));
                }
                ranges.sort_by(|a, b| b.0.cmp(&a.0));
                for (s, eidx, txt) in ranges {
                    rope.remove(s..eidx);
                    rope.insert(s, &txt);
                }
                // Full sync after
                #[cfg(feature = "lsp")]
                {
                    let version = buf.meta.read().version;
                    let change = lsp_types::TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: rope.to_string(),
                    };
                    let _ = client.change_document(
                        self.editor_overlay.lsp_uri.clone().unwrap(),
                        version,
                        vec![change],
                    );
                }
                self.pending_update.dirty = true;
            }
        }
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_show_hover(&mut self) {
        if let (Some(client), Some(tdpp)) =
            (self.editor_overlay.lsp.as_ref(), self.lsp_tdpp_at_cursor())
        {
            if let Ok(Some(hover)) = client.hover(tdpp) {
                // Extract plain text
                let text = match hover.contents {
                    lsp_types::HoverContents::Scalar(ms) => match ms {
                        lsp_types::MarkedString::String(s) => s,
                        lsp_types::MarkedString::LanguageString(ls) => ls.value,
                    },
                    lsp_types::HoverContents::Array(arr) => arr
                        .into_iter()
                        .map(|m| match m {
                            lsp_types::MarkedString::String(s) => s,
                            lsp_types::MarkedString::LanguageString(ls) => ls.value,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    lsp_types::HoverContents::Markup(mark) => mark.value,
                };
                self.editor_overlay.hover_text = text;
                self.editor_overlay.hover_active = true;
                self.pending_update.dirty = true;
            }
        }
    }

    #[cfg(feature = "lsp")]
    pub fn editor_overlay_signature_help(&mut self) {
        if let (Some(client), Some(tdpp)) =
            (self.editor_overlay.lsp.as_ref(), self.lsp_tdpp_at_cursor())
        {
            if let Ok(Some(sig)) = client.signature_help(tdpp) {
                if let Some(item) = sig.signatures.first() {
                    self.editor_overlay.signature_label = item.label.clone();
                    self.editor_overlay.signature_active = true;
                    self.pending_update.dirty = true;
                }
            }
        }
    }

    fn ensure_cursor_visible(&mut self) {
        #[cfg(feature = "editor")]
        if let Some(buf) = &self.editor_overlay.buffer {
            let cur = buf.cursor.read().clone();
            let content_lines = (self.size_info.screen_lines() as f32 * 0.80).round() as usize - 3;
            if cur.line < self.editor_overlay.scroll_line {
                self.editor_overlay.scroll_line = cur.line;
            } else if cur.line >= self.editor_overlay.scroll_line + content_lines {
                self.editor_overlay.scroll_line = cur.line.saturating_sub(content_lines - 1);
            }
        }
    }

    #[cfg(feature = "editor")]
    pub fn editor_overlay_insert_char(&mut self, ch: char) {
        if let Some(buf) = &self.editor_overlay.buffer {
            buf.insert(ch);
        }
        // Send LSP didChange (full sync)
        #[cfg(feature = "lsp")]
        {
            if let (Some(client), Some(uri), Some(_lang), Some(buf)) = (
                self.editor_overlay.lsp.as_ref(),
                self.editor_overlay.lsp_uri.clone(),
                self.editor_overlay.language_id.clone(),
                self.editor_overlay.buffer.as_ref(),
            ) {
                let version = buf.meta.read().version;
                let change = lsp_types::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: buf.text(),
                };
                let _ = client.change_document(uri, version, vec![change]);
            }
            if ch == '(' {
                self.editor_overlay_signature_help();
            }
        }
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "editor")]
    pub fn editor_overlay_backspace(&mut self) {
        if let Some(buf) = &self.editor_overlay.buffer {
            buf.backspace();
        }
        #[cfg(feature = "lsp")]
        {
            if let (Some(client), Some(uri), Some(buf)) = (
                self.editor_overlay.lsp.as_ref(),
                self.editor_overlay.lsp_uri.clone(),
                self.editor_overlay.buffer.as_ref(),
            ) {
                let version = buf.meta.read().version;
                let change = lsp_types::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: buf.text(),
                };
                let _ = client.change_document(uri, version, vec![change]);
            }
        }
        self.pending_update.dirty = true;
    }
}

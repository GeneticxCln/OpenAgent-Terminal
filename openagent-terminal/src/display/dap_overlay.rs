#![allow(dead_code)]
//! Minimal DAP overlay to launch/continue a debug session via codelldb

#[cfg(feature = "dap")]
use openagent_terminal_ide_dap::{AdapterConfig, DapClient, DapEvent};

use std::path::PathBuf;

use crate::config::UiConfig;
use crate::display::Display;
use crate::renderer::rects::RenderRect;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};

#[derive(Default)]
pub struct DapOverlayState {
    pub active: bool,
    pub status: String,
    #[cfg(feature = "dap")]
    pub client: Option<DapClient>,
    pub program_path: Option<PathBuf>,
    #[cfg(feature = "dap")]
    pub breakpoints: std::collections::HashMap<String, Vec<i64>>, // source.path -> lines (1-based)
    #[cfg(feature = "dap")]
    pub current_thread_id: Option<i64>,
    #[cfg(feature = "dap")]
    pub stack_frames: Vec<(String, i64, i64)>, // (name, line, column)
    #[cfg(feature = "dap")]
    pub variables: Vec<(String, String)>,
}

impl Clone for DapOverlayState {
    fn clone(&self) -> Self {
        Self {
            active: self.active,
            status: self.status.clone(),
            #[cfg(feature = "dap")]
            client: None,
            program_path: self.program_path.clone(),
            #[cfg(feature = "dap")]
            breakpoints: self.breakpoints.clone(),
            #[cfg(feature = "dap")]
            current_thread_id: self.current_thread_id,
            #[cfg(feature = "dap")]
            stack_frames: self.stack_frames.clone(),
            #[cfg(feature = "dap")]
            variables: self.variables.clone(),
        }
    }
}

impl DapOverlayState {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Display {
    pub fn dap_open(&mut self, program: Option<PathBuf>) {
        let mut prog = program;
        if prog.is_none() {
            // Use editor overlay file if available
            #[cfg(feature = "editor")]
            {
                if let Some(p) = self.editor_overlay.file_path.as_ref() {
                    prog = Some(p.clone());
                }
            }
        }
        self.dap_overlay.active = true;
        self.dap_overlay.program_path = prog;
        self.pending_update.dirty = true;
    }

    pub fn dap_close(&mut self) {
        self.dap_overlay.active = false;
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "dap")]
    pub fn dap_launch(&mut self) {
        let program = match self.dap_overlay.program_path.clone() {
            Some(p) => p,
            None => return,
        };
        let cfg = AdapterConfig { command: "codelldb".into(), args: vec![] };
        match DapClient::start(&cfg) {
            Ok(client) => {
                let _ = client.initialize();
                let args = serde_json::json!({"program": program.to_string_lossy(), "cwd": std::env::current_dir().unwrap_or_default().to_string_lossy()});
                let _ = client.launch(args);
                let _ = client.configuration_done();
                self.dap_overlay.status = format!("Launched {:?}", program);
                self.dap_overlay.client = Some(client);
            }
            Err(e) => {
                self.dap_overlay.status = format!("DAP start error: {}", e);
            }
        }
        self.pending_update.dirty = true;
    }

    #[cfg(feature = "dap")]
    pub fn dap_toggle_breakpoint_here(&mut self) {
        // Use current editor overlay file and cursor line
        #[cfg(feature = "editor")]
        if let Some(path) = self.editor_overlay.file_path.as_ref() {
            let file = path.to_string_lossy().to_string();
            let line = self
                .editor_overlay
                .buffer
                .as_ref()
                .map(|b| b.cursor.read().line as i64 + 1)
                .unwrap_or(1);
            let lines = self.dap_overlay.breakpoints.entry(file.clone()).or_default();
            if let Some(idx) = lines.iter().position(|&l| l == line) {
                lines.remove(idx);
            } else {
                lines.push(line);
            }
            // Send setBreakpoints
            if let Some(client) = &self.dap_overlay.client {
                let src = serde_json::json!({"path": file});
                let bps: Vec<_> = lines.iter().map(|l| serde_json::json!({"line": l})).collect();
                let args = serde_json::json!({"source": src, "breakpoints": bps});
                let _ = client.set_breakpoints(args);
            }
            self.pending_update.dirty = true;
        }
    }

    #[cfg(feature = "dap")]
    pub fn dap_continue_current(&mut self) {
        if let (Some(client), Some(tid)) =
            (&self.dap_overlay.client, self.dap_overlay.current_thread_id)
        {
            let _ = client.continue_(serde_json::json!({"threadId": tid}));
        }
    }

    #[cfg(feature = "dap")]
    pub fn dap_step_over(&mut self) {
        if let (Some(client), Some(tid)) =
            (&self.dap_overlay.client, self.dap_overlay.current_thread_id)
        {
            let _ = client.next(tid);
        }
    }

    #[cfg(feature = "dap")]
    pub fn dap_step_in(&mut self) {
        if let (Some(client), Some(tid)) =
            (&self.dap_overlay.client, self.dap_overlay.current_thread_id)
        {
            let _ = client.step_in(tid);
        }
    }

    #[cfg(feature = "dap")]
    pub fn dap_step_out(&mut self) {
        if let (Some(client), Some(tid)) =
            (&self.dap_overlay.client, self.dap_overlay.current_thread_id)
        {
            let _ = client.step_out(tid);
        }
    }

    #[cfg(feature = "dap")]
    pub fn dap_poll_events(&mut self) {
        if let Some(client) = &self.dap_overlay.client {
            while let Some(ev) = client.try_recv_event() {
                match ev {
                    DapEvent::Stopped(_json) => {
                        // Query threads and stack trace
                        if let Ok(th) = client.threads() {
                            let tids = th
                                .get("body")
                                .and_then(|b| b.get("threads"))
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default();
                            let tid =
                                tids.first().and_then(|t| t.get("id")).and_then(|v| v.as_i64());
                            self.dap_overlay.current_thread_id = tid;
                            if let Some(tid) = tid {
                                if let Ok(st) = client.stack_trace(tid) {
                                    let frames = st
                                        .get("body")
                                        .and_then(|b| b.get("stackFrames"))
                                        .and_then(|v| v.as_array())
                                        .cloned()
                                        .unwrap_or_default();
                                    self.dap_overlay.stack_frames.clear();
                                    for f in frames.iter().take(10) {
                                        let name = f
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let line =
                                            f.get("line").and_then(|v| v.as_i64()).unwrap_or(0);
                                        let col =
                                            f.get("column").and_then(|v| v.as_i64()).unwrap_or(0);
                                        self.dap_overlay.stack_frames.push((name, line, col));
                                    }
                                    // Variables: scopes -> first scope -> variables
                                    let frame_id = frames
                                        .first()
                                        .and_then(|f| f.get("id"))
                                        .and_then(|v| v.as_i64());
                                    if let Some(fid) = frame_id {
                                        if let Ok(sc) = client.scopes(fid) {
                                            if let Some(var_ref) = sc
                                                .get("body")
                                                .and_then(|b| b.get("scopes"))
                                                .and_then(|arr| arr.as_array())
                                                .and_then(|a| a.first())
                                                .and_then(|s| s.get("variablesReference"))
                                                .and_then(|v| v.as_i64())
                                            {
                                                if let Ok(vars) = client.variables(var_ref) {
                                                    self.dap_overlay.variables.clear();
                                                    if let Some(vars_arr) = vars
                                                        .get("body")
                                                        .and_then(|b| b.get("variables"))
                                                        .and_then(|a| a.as_array())
                                                    {
                                                        for v in vars_arr.iter().take(10) {
                                                            let name = v
                                                                .get("name")
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("")
                                                                .to_string();
                                                            let val = v
                                                                .get("value")
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("")
                                                                .to_string();
                                                            self.dap_overlay
                                                                .variables
                                                                .push((name, val));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        self.dap_overlay.status = "Stopped".into();
                    }
                    DapEvent::Continued(_) => {
                        self.dap_overlay.status = "Running".into();
                    }
                    DapEvent::Output(s) => {
                        if !s.trim().is_empty() {
                            self.dap_overlay.status = format!("Output: {}", s.trim());
                        }
                    }
                    DapEvent::Initialized => {
                        self.dap_overlay.status = "Initialized".into();
                    }
                    DapEvent::Terminated => {
                        self.dap_overlay.status = "Terminated".into();
                    }
                    DapEvent::Thread(_) | DapEvent::Unknown(_) => {}
                }
                self.pending_update.dirty = true;
            }
        }
    }

    #[cfg(feature = "dap")]
    pub fn dap_continue(&mut self) {
        if let Some(client) = &self.dap_overlay.client {
            let _ = client.continue_(serde_json::json!({"threadId": 1}));
            self.dap_overlay.status = "Continue".into();
            self.pending_update.dirty = true;
        }
    }

    pub fn draw_dap_overlay(&mut self, config: &UiConfig, state: &DapOverlayState) {
        if !state.active {
            return;
        }
        let size = self.size_info;
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        // Panel center small
        let cols = size.columns();
        let lines = size.screen_lines();
        let panel_cols = (cols as f32 * 0.5).round() as usize;
        let panel_lines = 8usize;
        let start_col = (cols.saturating_sub(panel_cols)) / 2;
        let start_line = (lines.saturating_sub(panel_lines)) / 2;
        let x = start_col as f32 * size.cell_width();
        let y = start_line as f32 * size.cell_height();
        let w = panel_cols as f32 * size.cell_width();
        let h = panel_lines as f32 * size.cell_height();
        let backdrop = RenderRect::new(0.0, 0.0, size.width(), size.height(), tokens.overlay, 0.20);
        let panel_bg = RenderRect::new(x, y, w, h, tokens.surface, 0.98);
        let rects = vec![backdrop, panel_bg];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);
        let title = "Debug Panel (Ctrl+Shift+D to toggle)";
        self.draw_ai_text(
            Point::new(start_line, Column(start_col + 2)),
            tokens.text,
            tokens.surface,
            title,
            panel_cols.saturating_sub(4),
        );
        let status = format!("Status: {}", state.status);
        self.draw_ai_text(
            Point::new(start_line + 1, Column(start_col + 2)),
            tokens.text,
            tokens.surface,
            &status,
            panel_cols.saturating_sub(4),
        );
        let hint = "Keys: F9=Breakpoint, F5=Continue, F10=StepOver, F11=StepIn, \
                    Shift+F11=StepOut, L=Launch, C=Continue, Esc=Close";
        self.draw_ai_text(
            Point::new(start_line + 2, Column(start_col + 2)),
            tokens.text,
            tokens.surface,
            hint,
            panel_cols.saturating_sub(4),
        );

        // Stack frames
        #[cfg(feature = "dap")]
        {
            let mut row = start_line + 4;
            self.draw_ai_text(
                Point::new(row, Column(start_col + 2)),
                tokens.text,
                tokens.surface_muted,
                "Stack:",
                panel_cols.saturating_sub(4),
            );
            row += 1;
            for (name, line, _col) in state.stack_frames.iter().take(6) {
                let entry = format!("- {} @ {}", name, line);
                self.draw_ai_text(
                    Point::new(row, Column(start_col + 3)),
                    tokens.text,
                    tokens.surface,
                    &entry,
                    panel_cols.saturating_sub(6),
                );
                row += 1;
            }
            // Variables
            row += 1;
            self.draw_ai_text(
                Point::new(row, Column(start_col + 2)),
                tokens.text,
                tokens.surface_muted,
                "Variables:",
                panel_cols.saturating_sub(4),
            );
            row += 1;
            for (name, val) in state.variables.iter().take(6) {
                let entry = format!("{} = {}", name, val);
                self.draw_ai_text(
                    Point::new(row, Column(start_col + 3)),
                    tokens.text,
                    tokens.surface,
                    &entry,
                    panel_cols.saturating_sub(6),
                );
                row += 1;
            }
        }
    }
}

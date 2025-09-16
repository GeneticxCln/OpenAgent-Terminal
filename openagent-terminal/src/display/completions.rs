#![allow(dead_code)]
use openagent_terminal_core::index::{Column, Point};
use openagent_terminal_core::term;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[cfg(feature = "completions")]
use crate::completions_spec;
use crate::config::UiConfig;
use crate::renderer::rects::RenderRect;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    Flag,
    File,
    Dir,
    Branch,
    Command,
    Argument,
}

#[derive(Clone, Debug)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub details: Option<String>,
    pub icon: &'static str,
    pub score: f32,
}

#[derive(Clone, Debug)]
pub struct CompletionsState {
    pub items: Vec<CompletionItem>,
    pub last_prefix: String,
    pub last_compute: Instant,
    pub debounce: Duration,
}

impl CompletionsState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            last_prefix: String::new(),
            last_compute: Instant::now() - Duration::from_secs(10),
            debounce: Duration::from_millis(120),
        }
    }
}

impl Default for CompletionsState {
    fn default() -> Self {
        Self::new()
    }
}

impl super::Display {
    fn fuzzy_score(query: &str, candidate: &str) -> f32 {
        if query.is_empty() {
            return 0.1;
        }
        // Simple subsequence scoring: reward sequential matches and prefix
        let q = query.to_lowercase();
        let c = candidate.to_lowercase();
        let mut qi = 0usize;
        let mut score = 0f32;
        let mut streak = 0f32;
        for ch in c.chars() {
            if qi < q.len() && ch == q.as_bytes()[qi] as char {
                qi += 1;
                streak += 1.0;
                score += 1.0 + streak * 0.2;
            } else {
                streak = 0.0;
            }
            if qi == q.len() {
                break;
            }
        }
        if qi < q.len() {
            return 0.0;
        }
        // Prefix bonus
        if c.starts_with(&q) {
            score += 1.5;
        }
        // Normalize by candidate length
        score / (candidate.len().max(1) as f32)
    }

    fn compute_completions_for_prefix(prefix: &str, cwd: Option<PathBuf>) -> Vec<CompletionItem> {
        let mut out: Vec<CompletionItem> = Vec::new();

        // Tokenize to get current token and first word (command)
        let tokens: Vec<&str> = prefix.split_whitespace().collect();
        let first = tokens.first().copied().unwrap_or("");
        let cur_token = if prefix.ends_with(' ') {
            ""
        } else {
            tokens.last().copied().unwrap_or("")
        };
        let is_flag_context = cur_token.starts_with('-');

        // 1) Flags: minimal spec for a few common commands, else generic
        if is_flag_context {
            let cmd = first;
            // Prefer structured spec when available
            if let Some(spec) = completions_spec::get_spec_for(cmd) {
                for fs in spec.flags {
                    let score = Self::fuzzy_score(
                        cur_token.trim_start_matches('-'),
                        fs.flag.trim_start_matches('-'),
                    );
                    if score > 0.0 {
                        out.push(CompletionItem {
                            label: fs.flag.to_string(),
                            kind: CompletionKind::Flag,
                            details: Some(fs.desc.to_string()),
                            icon: "🚩",
                            score,
                        });
                    }
                }
            } else {
                let known = Self::known_flags_for_command(cmd);
                for (flag, desc) in known {
                    let score = Self::fuzzy_score(
                        cur_token.trim_start_matches('-'),
                        flag.trim_start_matches('-'),
                    );
                    if score > 0.0 {
                        out.push(CompletionItem {
                            label: flag.to_string(),
                            kind: CompletionKind::Flag,
                            details: Some(desc.to_string()),
                            icon: "🚩",
                            score,
                        });
                    }
                }
            }
        }

        // 2) Files/dirs in cwd
        if !is_flag_context {
            if let Some(dir) = cwd.or_else(|| std::env::current_dir().ok()) {
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    for entry in rd.flatten() {
                        let path = entry.path();
                        let name = match path.file_name().and_then(|s| s.to_str()) {
                            Some(s) => s.to_string(),
                            None => continue,
                        };
                        let is_dir = path.is_dir();
                        let label = if is_dir {
                            format!("{}/", name)
                        } else {
                            name.clone()
                        };
                        let score = Self::fuzzy_score(cur_token, &name);
                        if score > 0.0 {
                            out.push(CompletionItem {
                                label,
                                kind: if is_dir {
                                    CompletionKind::Dir
                                } else {
                                    CompletionKind::File
                                },
                                details: None,
                                icon: if is_dir { "📁" } else { "📄" },
                                score,
                            });
                        }
                    }
                }
            }
        }

        // 3) Subcommands from spec
        if let Some(spec) = completions_spec::get_spec_for(first) {
            // Offer subcommands if current token is the second token and not a flag
            if tokens.len() <= 2 && !is_flag_context {
                for &sc in spec.subcommands.iter() {
                    let score = Self::fuzzy_score(cur_token, sc);
                    if score > 0.0 {
                        out.push(CompletionItem {
                            label: sc.to_string(),
                            kind: CompletionKind::Command,
                            details: Some(format!("{} subcommand", first)),
                            icon: "⌘",
                            score,
                        });
                    }
                }
            }
        }

        // 4) Git branches (very naive default suggestions if looks like git checkout)
        if first == "git"
            && (prefix.contains(" checkout")
                || prefix.ends_with(" switch")
                || prefix.contains(" switch "))
        {
            for b in ["main", "master", "develop", "release", "feature/"] {
                let score = Self::fuzzy_score(cur_token, b);
                if score > 0.0 {
                    out.push(CompletionItem {
                        label: b.to_string(),
                        kind: CompletionKind::Branch,
                        details: Some("Git branch (suggested)".to_string()),
                        icon: "🌿",
                        score,
                    });
                }
            }
        }

        // Sort by score desc and truncate
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out.truncate(12);
        out
    }

    fn known_flags_for_command(cmd: &str) -> Vec<(&'static str, &'static str)> {
        match cmd {
            "git" => vec![
                ("--help", "Show help for git or a subcommand"),
                ("-C", "Run as if git was started in <path>"),
                ("-c", "Pass a configuration parameter"),
                ("--version", "Show version information"),
                ("--no-pager", "Do not pipe git output into a pager"),
            ],
            "ls" => vec![
                ("-l", "Use a long listing format"),
                ("-a", "Do not ignore entries starting with ."),
                ("-h", "With -l, print sizes in human readable format"),
                ("-R", "List subdirectories recursively"),
            ],
            "cargo" => vec![
                (
                    "--help",
                    "Print this message or the help of the given subcommand(s)",
                ),
                ("-v", "Use verbose output (-vv very verbose)"),
                ("-q", "No output printed to stdout"),
            ],
            _ => vec![
                ("--help", "Show help"),
                ("-h", "Show help"),
                ("--version", "Show version"),
                ("-v", "Verbose output"),
            ],
        }
    }

    pub fn draw_completions_overlay_with_context(
        &mut self,
        config: &UiConfig,
        prefix: &str,
        cursor_point: Point<usize>,
        display_offset: usize,
        alt_screen: bool,
    ) {
        // Do not draw in alt-screen or when other modal overlays likely active
        if alt_screen {
            return;
        }
        // Skip when palette or confirm overlay is active — drawn elsewhere
        if self.palette.active() || self.confirm_overlay.active {
            return;
        }
        // Debounce recompute
        let now = Instant::now();
        if prefix != self.completions.last_prefix
            || now.duration_since(self.completions.last_compute) > self.completions.debounce
        {
            let cwd = None::<PathBuf>; // Future: track via shell integration/OSC
            self.completions.items = Self::compute_completions_for_prefix(prefix, cwd);
            self.completions.last_prefix = prefix.to_string();
            self.completions.last_compute = now;
        }
        if self.completions.items.is_empty() {
            return;
        }

        // Compute anchor near cursor
        let cursor_point_line = openagent_terminal_core::index::Point::new(
            openagent_terminal_core::index::Line(cursor_point.line.try_into().unwrap()),
            cursor_point.column,
        );
        let vp = match term::point_to_viewport(display_offset, cursor_point_line) {
            Some(p) => p,
            None => return,
        };

        // Theme tokens
        let theme = config
            .resolved_theme
            .as_ref()
            .cloned()
            .unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        let fg = tokens.text;
        let muted = tokens.text_muted;
        let accent = tokens.accent;

        // Layout box under the cursor (or shifted if close to bottom)
        let cols = self.size_info.columns;
        let lines = self.size_info.screen_lines;
        let box_width_cols = cols.min(48);
        let max_rows = 8usize;
        let needed_rows = self.completions.items.len().min(max_rows);
        let start_line = if vp.line + 2 + needed_rows >= lines {
            vp.line.saturating_sub(needed_rows + 1)
        } else {
            vp.line + 1
        };
        let start_col = vp.column.0.min(cols.saturating_sub(box_width_cols));
        let x = start_col as f32 * self.size_info.cell_width();
        let y = start_line as f32 * self.size_info.cell_height();
        let w = box_width_cols as f32 * self.size_info.cell_width();
        let h = (needed_rows as f32 + 1.0) * self.size_info.cell_height();

        // Background
        let rects = vec![RenderRect::new(x, y, w, h, tokens.surface, 0.96)];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Header row: shows context icon and prefix
        let mut header = String::new();
        header.push('→');
        header.push(' ');
        header.push_str(prefix.trim());
        self.draw_ai_text(
            Point::new(start_line, Column(start_col)),
            muted,
            tokens.surface,
            &header,
            box_width_cols,
        );

        // Items
        let mut line = start_line + 1;
        // If AI inline suggestion is available in future, we could show it as a top row here.
        let items_to_draw: Vec<_> = self
            .completions
            .items
            .iter()
            .take(max_rows)
            .cloned()
            .collect();
        for item in items_to_draw {
            let icon = item.icon;
            let mut row = String::new();
            row.push_str(icon);
            row.push(' ');
            row.push_str(&item.label);
            // Reserve space for details preview tail
            let avail = box_width_cols;
            self.draw_ai_text(
                Point::new(line, Column(start_col)),
                fg,
                tokens.surface,
                &row,
                avail,
            );
            line += 1;
        }

        // Simple flag inspector: if current token matches a known flag, show a tooltip to the right
        if let Some(tok) = prefix.split_whitespace().last() {
            if tok.starts_with('-') {
                let cmd = prefix.split_whitespace().next().unwrap_or("");
                let spec = Self::known_flags_for_command(cmd);
                if let Some((_, desc)) = spec.into_iter().find(|(f, _)| *f == tok) {
                    let tooltip_cols =
                        40usize.min(cols.saturating_sub(start_col + box_width_cols + 1));
                    if tooltip_cols > 10 {
                        let tx =
                            (start_col + box_width_cols + 1) as f32 * self.size_info.cell_width();
                        let ty = y;
                        let tw = tooltip_cols as f32 * self.size_info.cell_width();
                        let th = 2.0 * self.size_info.cell_height();
                        let rects =
                            vec![RenderRect::new(tx, ty, tw, th, tokens.surface_muted, 0.98)];
                        let metrics = self.glyph_cache.font_metrics();
                        let size_copy = self.size_info;
                        self.renderer_draw_rects(&size_copy, &metrics, rects);
                        let text = desc.to_string();
                        self.draw_ai_text(
                            Point::new(start_line, Column(start_col + box_width_cols + 2)),
                            accent,
                            tokens.surface_muted,
                            &text,
                            tooltip_cols - 2,
                        );
                    }
                }
            }
        }
    }
}

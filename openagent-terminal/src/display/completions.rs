#![allow(dead_code)]
use openagent_terminal_core::index::{Column, Point};
use openagent_terminal_core::term;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[cfg(feature = "completions")]
use crate::completions_spec;
use crate::config::UiConfig;
use crate::renderer::rects::RenderRect;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
    pub external: Vec<CompletionItem>,
    pub last_prefix: String,
    pub last_compute: Instant,
    pub debounce: Duration,
    pub selected_index: usize,
}

impl CompletionsState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            external: Vec::new(),
            last_prefix: String::new(),
            last_compute: Instant::now() - Duration::from_secs(10),
            // Slightly faster feedback while typing
            debounce: Duration::from_millis(80),
            selected_index: 0,
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

    fn compute_completions_for_prefix(&self, prefix: &str, cwd: Option<PathBuf>) -> Vec<CompletionItem> {
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
                        let label = if is_dir { format!("{}/", name) } else { name.clone() };
                        let score = Self::fuzzy_score(cur_token, &name);
                        if score > 0.0 {
                            out.push(CompletionItem {
                                label,
                                kind: if is_dir { CompletionKind::Dir } else { CompletionKind::File },
                                details: None,
                                icon: if is_dir { "📁" } else { "📄" },
                                score,
                            });
                        }
                    }
                }
            }
        }

        // 2b) Commands from PATH when at the first token (and not a flag)
        if !is_flag_context && tokens.len() <= 1 {
            let cmds = path_commands();
            for cmd in cmds {
                let score = Self::fuzzy_score(cur_token, cmd);
                if score > 0.0 {
                    out.push(CompletionItem {
                        label: cmd.clone(),
                        kind: CompletionKind::Command,
                        details: Some("$PATH command".to_string()),
                        icon: "⌘",
                        score,
                    });
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

        // 4b) History-aware ranking and suggestions (from in-memory composer history)
        // Only applies in first-token, non-flag context
        if !is_flag_context && tokens.len() <= 1 {
            use std::collections::{HashMap, HashSet};

            // Build MRU/frequency maps from recent composer history (most-recent-first)
            let mut freq: HashMap<String, usize> = HashMap::new();
            let mut best_recency: HashMap<String, usize> = HashMap::new();
            let mut seen_cmds: HashSet<String> = HashSet::new();
            let consider = 200usize; // cap work per keystroke
            for (idx, entry) in self.composer_history.iter().take(consider).enumerate() {
                let first_tok = entry.split_whitespace().next().unwrap_or("");
                if first_tok.is_empty() { continue; }
                // Track only first token commands
                let key = first_tok.to_string();
                *freq.entry(key.clone()).or_insert(0) += 1;
                best_recency
                    .entry(key.clone())
                    .and_modify(|r| { if idx < *r { *r = idx } })
                    .or_insert(idx);
                seen_cmds.insert(key);
            }
            let max_freq = freq.values().copied().max().unwrap_or(1) as f32;

            // Boost scores of existing PATH/subcommand command items based on history
            if !freq.is_empty() {
                for it in &mut out {
                    if matches!(it.kind, CompletionKind::Command) {
                        if let Some(f) = freq.get(&it.label) {
                            let rec = *best_recency.get(&it.label).unwrap_or(&usize::MAX);
                            let recency_score = if rec == usize::MAX { 0.0 } else { 1.0 / (1.0 + rec as f32) };
                            let freq_score = (*f as f32) / max_freq.max(1.0);
                            // Keep boost modest so fuzzy/type context still dominates
                            let boost = 1.0 + (0.35 * recency_score + 0.35 * freq_score);
                            it.score *= boost;
                        }
                    }
                }

                // Add a few top MRU first-token commands that match the current prefix but
                // aren't already present. These appear as "Recently used" commands.
                let mut existing: HashSet<String> = out.iter().map(|i| i.label.clone()).collect();
                let mut mru: Vec<(String, usize, usize)> = freq
                    .iter()
                    .map(|(k, f)| (k.clone(), *best_recency.get(k).unwrap_or(&usize::MAX), *f))
                    .collect();
                // Sort by recency first (smaller idx => more recent), then by freq desc
                mru.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2)));

                let mut added = 0usize;
                let max_add = 4usize;
                for (name, rec, f) in mru.into_iter() {
                    if added >= max_add { break; }
                    if existing.contains(&name) { continue; }
                    let base = Self::fuzzy_score(cur_token, &name);
                    if base <= 0.0 { continue; }
                    let recency_score = if rec == usize::MAX { 0.0 } else { 1.0 / (1.0 + rec as f32) };
                    let freq_score = (f as f32) / max_freq.max(1.0);
                    let score = base * (1.0 + 0.45 * recency_score + 0.45 * freq_score);
                    out.push(CompletionItem {
                        label: name.clone(),
                        kind: CompletionKind::Command,
                        details: Some("Recently used".to_string()),
                        icon: "🕘",
                        score,
                    });
                    existing.insert(name);
                    added += 1;
                }
            }
        }

        // Type-aware weighting to improve ranking by context
        let type_weight = |kind: &CompletionKind| -> f32 {
            if is_flag_context {
                return match kind {
                    CompletionKind::Flag => 1.25,
                    CompletionKind::Command => 0.85,
                    CompletionKind::File | CompletionKind::Dir => 0.8,
                    _ => 0.9,
                };
            }
            if tokens.len() <= 1 {
                match kind {
                    CompletionKind::Command => 1.20,
                    CompletionKind::File | CompletionKind::Dir => 1.0,
                    CompletionKind::Flag => 0.85,
                    _ => 0.95,
                }
            } else {
                match kind {
                    CompletionKind::File | CompletionKind::Dir => 1.15,
                    CompletionKind::Command => 1.0,
                    CompletionKind::Flag => 0.95,
                    _ => 1.0,
                }
            }
        };
        for it in &mut out {
            it.score *= type_weight(&it.kind);
            // Tiny boost for exact prefix
            if !cur_token.is_empty() && it.label.starts_with(cur_token) {
                it.score *= 1.08;
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
            // Overlay not drawn in alt-screen; ensure we reset active state for animation and hit testing
            self.completions_last_active = false;
            self.completions_overlay_item_lines.clear();
            self.completions_overlay_bounds = None;
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
            self.completions.items = self.compute_completions_for_prefix(prefix, cwd);
            // Reset selection when prefix changes
            if self.completions.selected_index >= self.completions.items.len() {
                self.completions.selected_index = 0;
            }
            self.completions.last_prefix = prefix.to_string();
            self.completions.last_compute = now;
        }
        // Merge in external completions (from IDE) with Warp-like interleave
        if !self.completions.external.is_empty() {
            use std::collections::{HashMap, HashSet};
            // Deduplicate within each source by highest score per label
            let mut local_by_label: HashMap<String, CompletionItem> = HashMap::new();
            for it in self.completions.items.drain(..) {
                local_by_label
                    .entry(it.label.clone())
                    .and_modify(|e| {
                        if it.score > e.score {
                            *e = it.clone();
                        }
                    })
                    .or_insert(it);
            }
            let mut ext_by_label: HashMap<String, CompletionItem> = HashMap::new();
            for it in self.completions.external.iter().cloned() {
                ext_by_label
                    .entry(it.label.clone())
                    .and_modify(|e| {
                        if it.score > e.score {
                            *e = it.clone();
                        }
                    })
                    .or_insert(it);
            }
            // Remove duplicates across sources; prefer external items
            let mut seen: HashSet<String> = HashSet::new();
            let mut ext_sorted: Vec<_> = ext_by_label.into_values().collect();
            ext_sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            for it in &ext_sorted { seen.insert(it.label.clone()); }
            let mut loc_sorted: Vec<_> = local_by_label
                .into_iter()
                .filter(|(k, _)| !seen.contains(k))
                .map(|(_, v)| v)
                .collect();
            loc_sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            // Interleave starting with external (Warp-like bias for smarter suggestions)
            let mut interleaved: Vec<CompletionItem> = Vec::new();
            let mut i = 0usize;
            let mut j = 0usize;
            while i < ext_sorted.len() || j < loc_sorted.len() {
                if i < ext_sorted.len() {
                    interleaved.push(ext_sorted[i].clone());
                    i += 1;
                }
                if j < loc_sorted.len() {
                    interleaved.push(loc_sorted[j].clone());
                    j += 1;
                }
            }
            self.completions.items = interleaved;
        }
        if self.completions.items.is_empty() {
            // Reset hit testing and animation state when no items
            self.completions_last_active = false;
            self.completions_overlay_item_lines.clear();
            self.completions_overlay_bounds = None;
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
        // Rough estimate: we may add up to one header per category shown; cap by max_rows
        let distinct_kinds = {
            use std::collections::HashSet;
            let mut s = HashSet::new();
            for it in &self.completions.items { s.insert(it.kind.clone()); }
            s.len()
        };
        let needed_rows = (self.completions.items.len() + distinct_kinds).min(max_rows);
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

        // Simple fade-in animation on open
        if !self.completions_last_active {
            self.completions_last_active = true;
            self.completions_anim_start = Some(now);
        }
        let alpha = if let Some(ts) = self.completions_anim_start {
            let ms = now.saturating_duration_since(ts).as_millis() as f32;
            (ms / 120.0).clamp(0.0, 1.0) * 0.96
        } else {
            0.96
        };
        // Background
        let rects = vec![RenderRect::new(x, y, w, h, tokens.surface, alpha)];
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

        // Items with Warp-like section headers per category as they first appear
        // Cache overlay bounds for hit testing
        let end_line = start_line + needed_rows;
        self.completions_overlay_bounds = Some((start_line, end_line, start_col, start_col + box_width_cols));

        let mut line = start_line + 1;
        use std::collections::HashSet;
        let mut seen_kinds: HashSet<CompletionKind> = HashSet::new();
        let mut rows_used = 0usize;
        let mut item_idx = 0usize; // index into items only (exclude headers)
        self.completions_overlay_item_lines.clear();
        let items_snapshot = self.completions.items.clone();
        for item in items_snapshot.into_iter() {
            // Stop when reaching max rows (accounting for headers)
            if rows_used >= max_rows { break; }
            // Determine section header if first time we see this kind
            let header_opt = if !seen_kinds.contains(&item.kind) {
                seen_kinds.insert(item.kind.clone());
                match item.kind {
                    CompletionKind::Command => Some("Commands".to_string()),
                    CompletionKind::File | CompletionKind::Dir => Some("Files".to_string()),
                    CompletionKind::Branch => Some("Branches".to_string()),
                    CompletionKind::Flag => Some("Flags".to_string()),
                    CompletionKind::Argument => Some("Arguments".to_string()),
                }
            } else { None };

            if let Some(header) = header_opt {
                if rows_used >= max_rows { break; }
                // Draw header row in muted color
                self.draw_ai_text(
                    Point::new(line, Column(start_col)),
                    muted,
                    tokens.surface,
                    &format!("{}", header),
                    box_width_cols,
                );
                line += 1;
                rows_used += 1;
                if rows_used >= max_rows { break; }
            }

            // Draw item row
            let icon = item.icon;
            let mut row = String::new();
            row.push_str(icon);
            row.push(' ');
            row.push_str(&item.label);
            let avail = box_width_cols;
            // Highlight selected item using item index (headers excluded)
            let color = if self.completions.selected_index == item_idx { accent } else { fg };
            self.draw_ai_text(
                Point::new(line, Column(start_col)),
                color,
                tokens.surface,
                &row,
                avail,
            );
            // Record mapping for hover/click hit-testing (viewport line -> item index)
            self.completions_overlay_item_lines.push((line, item_idx));
            line += 1;
            rows_used += 1;
            item_idx += 1;
            if rows_used >= max_rows { break; }
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

    // Public helpers used by ActionContext for navigation/acceptance
    pub fn completions_active(&self) -> bool {
        !self.completions.external.is_empty() || !self.completions.items.is_empty()
    }

    pub fn completions_move_selection(&mut self, delta: isize) {
        if self.completions.items.is_empty() {
            self.completions.selected_index = 0;
            return;
        }
        let len = self.completions.items.len();
        let mut idx = self.completions.selected_index as isize + delta;
        if idx < 0 {
            idx = 0;
        } else if idx as usize >= len {
            idx = len.saturating_sub(1) as isize;
        }
        self.completions.selected_index = idx as usize;
    }

    pub fn completions_selected_label(&self) -> Option<String> {
        self.completions
            .items
            .get(self.completions.selected_index)
            .map(|it| it.label.clone())
    }

    pub fn completions_clear(&mut self) {
        self.completions.external.clear();
        self.completions.items.clear();
        self.completions.selected_index = 0;
    }
}

// Lazily compute list of commands available in $PATH. Returns cached vector.
fn path_commands() -> &'static Vec<String> {
    use std::collections::HashSet;
    use std::sync::OnceLock;
    static CMDS: OnceLock<Vec<String>> = OnceLock::new();
    CMDS.get_or_init(|| {
        let mut out: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        if let Ok(path_env) = std::env::var("PATH") {
            for dir in path_env.split(':') {
                let p = std::path::Path::new(dir);
                if let Ok(rd) = std::fs::read_dir(p) {
                    for ent in rd.flatten() {
                        if let Some(name) = ent.file_name().to_str() {
                            // Skip names with path separators or obvious non-commands
                            if name.is_empty() || name.contains('/') { continue; }
                            // De-duplicate across PATH entries
                            if seen.insert(name.to_string()) {
                                out.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        out
    })
}

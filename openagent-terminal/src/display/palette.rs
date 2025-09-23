use crate::config::Action;

/// Entry that can be executed from the palette.
#[derive(Clone, Debug)]
pub enum PaletteEntry {
    Action(Action),
    Workflow(String),
    File(String), // absolute or relative path
    /// Recent/suggested command entry (with optional cwd and last exit status)
    Command {
        cmd: String,
        cwd: Option<String>,
        #[allow(dead_code)]
        exit: Option<i32>,
    },
    #[cfg(feature = "plugins")]
    PluginCommand {
        plugin: String,
        command: String,
    },
}

#[derive(Clone, Debug)]
pub struct PaletteItem {
    pub key: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub entry: PaletteEntry,
}

#[derive(Default, Debug, Clone)]
pub struct PaletteState {
    active: bool,
    filter: String,
    items: Vec<PaletteItem>,
    filtered_indices: Vec<usize>,
    selected: usize,
    // Simple MRU counts to boost ranking for recently used entries
    mru_counts: std::collections::HashMap<String, u32>,
}

impl PaletteState {
    pub fn new() -> Self {
        Self {
            active: false,
            filter: String::new(),
            items: Vec::new(),
            filtered_indices: Vec::new(),
            selected: 0,
            mru_counts: std::collections::HashMap::new(),
        }
    }

    /// Append new items, deduplicating by key; then re-run filtering
    pub fn add_items_unique(&mut self, mut new_items: Vec<PaletteItem>) {
        if new_items.is_empty() {
            return;
        }
        use std::collections::HashSet;
        let mut existing: HashSet<&str> = HashSet::new();
        for it in &self.items {
            existing.insert(it.key.as_str());
        }
        new_items.retain(|it| !existing.contains(it.key.as_str()));
        if new_items.is_empty() {
            return;
        }
        self.items.extend(new_items);
        self.refilter();
    }

    /// Return up to `max` recent file paths from MRU counts
    pub fn recent_file_paths(&self, max: usize) -> Vec<String> {
        let mut pairs: Vec<(&String, &u32)> =
            self.mru_counts.iter().filter(|(k, _)| k.starts_with("file:")).collect();
        // Sort by count desc
        pairs.sort_by(|a, b| b.1.cmp(a.1));
        let mut out = Vec::new();
        for (k, _) in pairs.into_iter().take(max) {
            if let Some(path) = k.strip_prefix("file:") {
                out.push(path.to_string());
            }
        }
        out
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn open(&mut self, items: Vec<PaletteItem>) {
        self.items = items;
        self.filter.clear();
        self.selected = 0;
        self.active = true;
        self.refilter();
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    #[allow(dead_code)]
    pub fn items(&self) -> &Vec<PaletteItem> {
        &self.items
    }

    #[allow(dead_code)]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    #[allow(dead_code)]
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.refilter();
    }

    pub fn push_filter_char(&mut self, ch: char) {
        self.filter.push(ch);
        self.refilter();
    }

    pub fn pop_filter_char(&mut self) {
        self.filter.pop();
        self.refilter();
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.filtered_indices.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.filtered_indices.len() as isize;
        let mut idx = self.selected as isize + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx >= len {
            idx = len - 1;
        }
        self.selected = idx as usize;
    }

    pub fn selected_entry(&self) -> Option<&PaletteEntry> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|&i| self.items.get(i))
            .map(|it| &it.entry)
    }

    pub fn selected_item_key(&self) -> Option<&str> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|&i| self.items.get(i))
            .map(|it| it.key.as_str())
    }

    pub fn note_used(&mut self, key: &str) {
        let e = self.mru_counts.entry(key.to_string()).or_insert(0);
        *e = e.saturating_add(1);
    }

    pub fn load_mru_from_config(&mut self, _config: &UiConfig) {
        if let Some(dir) = dirs::config_dir() {
            let path = dir.join("openagent-terminal").join("palette_state.json");
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(state) = serde_json::from_str::<PalettePersistentState>(&text) {
                    self.mru_counts = state.mru_counts;
                }
            }
        }
    }

    pub fn save_mru_to_config(&self, _config: &UiConfig) {
        let state = PalettePersistentState { mru_counts: self.mru_counts.clone() };
        if let Some(dir) = dirs::config_dir() {
            let base = dir.join("openagent-terminal");
            let _ = std::fs::create_dir_all(&base);
            let path = base.join("palette_state.json");
            if let Ok(json) = serde_json::to_string_pretty(&state) {
                let _ = std::fs::write(&path, json);
            }
        }
    }

    /// Returns (filter, selected_visible_index, visible_items)
    pub fn view(&self) -> (String, usize, Vec<PaletteItemView>) {
        let visible = self
            .filtered_indices
            .iter()
            .filter_map(|&i| self.items.get(i))
            .map(|it| PaletteItemView { title: it.title.clone(), subtitle: it.subtitle.clone() })
            .collect::<Vec<_>>();
        (self.filter.clone(), self.selected.min(visible.len().saturating_sub(1)), visible)
    }

    fn refilter(&mut self) {
        let q = self.filter.to_lowercase();
        // Prefix filters
        let (filter_type, term) = if q.starts_with("w:") || q.starts_with("workflows:") {
            (Some("workflow"), q.split_once(':').map(|(_, t)| t).unwrap_or("").trim().to_string())
        } else if q.starts_with("a:") || q.starts_with("actions:") {
            (Some("action"), q.split_once(':').map(|(_, t)| t).unwrap_or("").trim().to_string())
        } else if q.starts_with("f:") || q.starts_with("files:") {
            (Some("file"), q.split_once(':').map(|(_, t)| t).unwrap_or("").trim().to_string())
        } else if q.starts_with("p:") || q.starts_with("plugins:") {
            (Some("plugin"), q.split_once(':').map(|(_, t)| t).unwrap_or("").trim().to_string())
        } else {
            (None, q)
        };

        // Fuzzy rank items by query term (simple subsequence scoring with bonuses)
        let mut ranked: Vec<(usize, i32)> = Vec::new();
        for (i, it) in self.items.iter().enumerate() {
            if let Some(ft) = filter_type {
                match (&it.entry, ft) {
                    (PaletteEntry::Action(_), "action") => {}
                    (PaletteEntry::Workflow(_), "workflow") => {}
                    (PaletteEntry::File(_), "file") => {}
                    #[cfg(feature = "plugins")]
                    (PaletteEntry::PluginCommand { .. }, "plugin") => {}
                    #[cfg(not(feature = "plugins"))]
                    _ => {}
                    #[cfg(feature = "plugins")]
                    _ => {}
                }
            }

            // Base score (0 if no term)
            let mut base_score: i32 = 0;
            if !term.is_empty() {
                let title_lower = it.title.to_lowercase();
                let hay = format!(
                    "{} {}",
                    title_lower,
                    it.subtitle.as_deref().unwrap_or("").to_lowercase()
                );
                if let Some(score) = fuzzy_score(&term, &hay) {
                    base_score = score;
                    // Small boosts for title-preferring matches
                    if title_lower == term {
                        // Exact title match
                        base_score += 120;
                    } else {
                        // Exact prefix on title
                        if title_lower.starts_with(&term) {
                            base_score += 45;
                        } else if title_lower.contains(&term) {
                            // Contiguous substring in title (not necessarily at start)
                            base_score += 15;
                        }
                        // Any word in title starts with term (split on non-alnum)
                        let word_prefix = title_lower
                            .split(|c: char| !c.is_alphanumeric())
                            .any(|w| !w.is_empty() && w.starts_with(&term));
                        if word_prefix {
                            base_score += 20;
                        }
                    }
                } else {
                    // Skip items that don't match at all when term is provided
                    continue;
                }
            }

            // MRU boost based on per-key usage
            let mru_boost = self.mru_counts.get(&it.key).copied().unwrap_or(0) as i32 * 50;
            ranked.push((i, base_score + mru_boost));
        }
        // Sort by score desc, then by title length asc for stability
        ranked.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| {
                let la = self.items[a.0].title.len();
                let lb = self.items[b.0].title.len();
                la.cmp(&lb)
            })
        });
        self.filtered_indices.clear();
        self.filtered_indices.extend(ranked.into_iter().map(|(i, _)| i));
        if self.selected >= self.filtered_indices.len() {
            self.selected = self.filtered_indices.len().saturating_sub(1);
        }
    }
}

fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    // Very small queries: prefer contains to avoid odd ranks
    if query.len() <= 1 {
        if text.contains(query) {
            return Some(5);
        } else {
            return None;
        }
    }
    // Subsequence match with bonuses
    let mut score: i32 = 0;
    let mut qi = 0usize;
    let qbytes = query.as_bytes();
    let tbytes = text.as_bytes();
    let mut last_match_pos: i32 = -10;
    for (i, &tb) in tbytes.iter().enumerate() {
        if qi >= qbytes.len() {
            break;
        }
        let qb = qbytes[qi];
        if qb == tb {
            // Base match
            score += 10;
            // Bonus for consecutive
            if (i as i32) == last_match_pos + 1 {
                score += 8;
            }
            // Bonus for start-of-word or early match
            if i == 0
                || tbytes
                    .get(i - 1)
                    .map(|c| *c == b' ' || *c == b'_' || *c == b'-')
                    .unwrap_or(false)
            {
                score += 6;
            }
            last_match_pos = i as i32;
            qi += 1;
        }
    }
    if qi < qbytes.len() {
        return None;
    }
    // Penalize distance between first and last match lightly (prefer tighter groups)
    // And slightly favor shorter texts
    Some(score - (text.len() as i32 / 16))
}

#[derive(Clone, Debug)]
pub struct PaletteItemView {
    pub title: String,
    pub subtitle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PalettePersistentState {
    mru_counts: std::collections::HashMap<String, u32>,
}

/// Compute indices in `text` that match the subsequence `query` (lowercased inputs).
fn fuzzy_highlight_positions(query: &str, text: &str) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }
    // Compute indices for the first matching subsequence run
    let mut out = Vec::new();
    let mut qi = 0usize;
    for (i, ch) in text.chars().enumerate() {
        if qi >= query.len() {
            break;
        }
        if ch == query.as_bytes()[qi] as char {
            out.push(i);
            qi += 1;
        }
    }
    if qi < query.len() {
        return Vec::new();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_items_unique_dedupes() {
        let mut ps = PaletteState::new();
        ps.add_items_unique(vec![PaletteItem {
            key: "k1".into(),
            title: "T1".into(),
            subtitle: None,
            entry: PaletteEntry::File("/tmp/a".into()),
        }]);
        // Attempt to add duplicate key and a new key
        ps.add_items_unique(vec![
            PaletteItem {
                key: "k1".into(),
                title: "T1 dup".into(),
                subtitle: None,
                entry: PaletteEntry::File("/tmp/a2".into()),
            },
            PaletteItem {
                key: "k2".into(),
                title: "T2".into(),
                subtitle: Some("S".into()),
                entry: PaletteEntry::Workflow("w1".into()),
            },
        ]);
        assert_eq!(ps.items.len(), 2, "should keep only unique keys");
        let keys: Vec<_> = ps.items.iter().map(|i| i.key.as_str()).collect();
        assert!(keys.contains(&"k1") && keys.contains(&"k2"));
    }

    #[test]
    fn recent_file_paths_respects_mru() {
        let mut ps = PaletteState::new();
        // Simulate usage
        ps.note_used("file:/a");
        ps.note_used("file:/b");
        ps.note_used("file:/b");
        ps.note_used("file:/c");
        ps.note_used("file:/c");
        ps.note_used("file:/c");
        let out = ps.recent_file_paths(2);
        assert_eq!(out, vec!["/c".to_string(), "/b".to_string()]);
    }

    #[test]
    fn fuzzy_helpers_basic() {
        // Contains for 1-char query
        assert!(fuzzy_score("a", "abc").is_some());
        assert!(fuzzy_score("z", "abc").is_none());

        // Subsequence scoring should exist for ordered letters
        assert!(fuzzy_score("abc", "a_b_c").is_some());
        // Highlight positions are indices in the lowercased text
        let pos = fuzzy_highlight_positions("abc", "a_b_c");
        assert_eq!(pos, vec![0, 2, 4]);
}
}

use crate::config::{Action as BindingAction, BindingKey, KeyBinding, UiConfig};
use crate::display::{Display, SizeInfo};
use crate::renderer::rects::RenderRect;
use crate::renderer::ui::{UiRoundedRect, UiSprite};
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};
use serde::{Deserialize, Serialize};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use winit::keyboard::{Key, ModifiersState, NamedKey};

fn binding_to_hint(binding: &KeyBinding) -> Option<String> {
    // Compose modifier names in a consistent order and style per platform
    let mods = binding.mods;
    let mut parts: Vec<String> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if mods.contains(ModifiersState::CONTROL) {
            parts.push("⌃".to_string());
        }
        if mods.contains(ModifiersState::SHIFT) {
            parts.push("⇧".to_string());
        }
        if mods.contains(ModifiersState::ALT) {
            parts.push("⌥".to_string());
        }
        if mods.contains(ModifiersState::SUPER) {
            parts.push("⌘".to_string());
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if mods.contains(ModifiersState::CONTROL) {
            parts.push("Ctrl".to_string());
        }
        if mods.contains(ModifiersState::SHIFT) {
            parts.push("Shift".to_string());
        }
        if mods.contains(ModifiersState::ALT) {
            parts.push("Alt".to_string());
        }
        if mods.contains(ModifiersState::SUPER) {
            parts.push("Super".to_string());
        }
    }

    let key_str = match &binding.trigger {
        BindingKey::Keycode { key, .. } => match key {
            Key::Named(named) => match named {
                #[cfg(target_os = "macos")]
                NamedKey::Enter => "↩".to_string(),
                #[cfg(not(target_os = "macos"))]
                NamedKey::Enter => "Enter".to_string(),
                NamedKey::Tab => "Tab".to_string(),
                NamedKey::Backspace => "Backspace".to_string(),
                NamedKey::Escape => "Esc".to_string(),
                NamedKey::ArrowUp => "↑".to_string(),
                NamedKey::ArrowDown => "↓".to_string(),
                NamedKey::ArrowLeft => "←".to_string(),
                NamedKey::ArrowRight => "→".to_string(),
                NamedKey::Home => "Home".to_string(),
                NamedKey::End => "End".to_string(),
                NamedKey::PageUp => "PgUp".to_string(),
                NamedKey::PageDown => "PgDn".to_string(),
                other => format!("{:?}", other),
            },
            Key::Character(s) => {
                let ss = s.to_string();
                if ss.chars().count() == 1 {
                    ss.to_uppercase()
                } else {
                    ss
                }
            }
            _ => return None,
        },
        _ => return None,
    };

    parts.push(key_str);

    // Joiner: macOS uses no joiner (⌘⇧P), others use '+' (Ctrl+Shift+P)
    #[cfg(target_os = "macos")]
    {
        Some(parts.join(""))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Some(parts.join("+"))
    }
}

fn action_hints_for(config: &UiConfig, action: &BindingAction, max: usize) -> Vec<String> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for kb in config.key_bindings() {
        if &kb.action == action {
            if let Some(s) = binding_to_hint(kb) {
                if seen.insert(s.clone()) {
                    out.push(s);
                    if out.len() >= max {
                        break;
                    }
                }
            }
        }
    }
    out
}

impl Display {
    /// Draw the Command Palette overlay as a centered modal with input and a list
    pub fn draw_palette_overlay(&mut self, config: &UiConfig) {
        if !self.palette.active() {
            return;
        }
        let size_info: SizeInfo = self.size_info;
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;
        let ui = theme.ui.clone();

        // Animation progress (open/close)
        let mut progress: f32 = 1.0;
        if theme.ui.reduce_motion {
            self.palette_anim_start = None;
        } else if let Some(start) = self.palette_anim_start {
            let elapsed = start.elapsed().as_millis() as u32;
            let dur = self.palette_anim_duration_ms.max(1);
            let t = (elapsed as f32 / dur as f32).clamp(0.0, 1.0);
            // ease-out cubic
            let eased = 1.0 - (1.0 - t).powi(3);
            progress = if self.palette_anim_opening { eased } else { 1.0 - eased };
            if t >= 1.0 {
                self.palette_anim_start = None;
            }
        }

        // Geometry: centered panel, ~70% width, ~45% height (min 8 lines, max 16 lines)
        let num_lines = size_info.screen_lines();
        if num_lines == 0 {
            return;
        }
        let panel_lines = ((num_lines as f32 * 0.45).round() as usize).clamp(8, 16).min(num_lines);
        let cols = size_info.columns();
        let panel_cols = ((cols as f32 * 0.7).round() as usize).clamp(40, cols.saturating_sub(2));
        let panel_start_line = (num_lines.saturating_sub(panel_lines)) / 2;
        let panel_start_col = (cols.saturating_sub(panel_cols)) / 2;

        // Pixel coordinates for rectangles
        let panel_x = panel_start_col as f32 * size_info.cell_width();
        let panel_y = panel_start_line as f32 * size_info.cell_height();
        let panel_w = panel_cols as f32 * size_info.cell_width();
        let panel_h = panel_lines as f32 * size_info.cell_height();

        // Apply scale based on progress (simulate scale from 0.98 -> 1.0 on open)
        let scale = 0.98 + 0.02 * progress;
        let cx = panel_x + panel_w * 0.5;
        let cy = panel_y + panel_h * 0.5;
        let scaled_w = panel_w * scale;
        let scaled_h = panel_h * scale;
        let sx = cx - scaled_w * 0.5;
        let sy = cy - scaled_h * 0.5;

        // Backdrop, shadow, and panel background
        let rects = vec![
            // Fade backdrop with progress
            RenderRect::new(
                0.0,
                0.0,
                size_info.width(),
                size_info.height(),
                tokens.overlay,
                0.22 * progress,
            ),
            // Soft shadow
            RenderRect::new(
                sx + 3.0,
                sy + 5.0,
                scaled_w,
                scaled_h,
                tokens.surface,
                0.12 * progress,
            ),
            // Panel background
            RenderRect::new(sx, sy, scaled_w, scaled_h, tokens.surface_muted, 0.97),
        ];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy: SizeInfo = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        // Colors with fade based on animation progress (approximate by blending toward bg)
        let bg = tokens.surface_muted;
        fn lerp_rgb(
            a: crate::display::color::Rgb,
            b: crate::display::color::Rgb,
            t: f32,
        ) -> crate::display::color::Rgb {
            let t = t.clamp(0.0, 1.0);
            let r = (a.r as f32 + (b.r as f32 - a.r as f32) * t).round().clamp(0.0, 255.0) as u8;
            let g = (a.g as f32 + (b.g as f32 - a.g as f32) * t).round().clamp(0.0, 255.0) as u8;
            let bb = (a.b as f32 + (b.b as f32 - a.b as f32) * t).round().clamp(0.0, 255.0) as u8;
            crate::display::color::Rgb::new(r, g, bb)
        }
        let fade_t = progress; // 0..1
        let fg = lerp_rgb(bg, tokens.text, fade_t);
        let muted_fg = lerp_rgb(bg, tokens.text_muted, fade_t);
        let accent_fg = lerp_rgb(bg, tokens.accent, fade_t);

        // Gather visible items and selection
        let (filter, selected_visible, views) = self.palette.view();
        let count = views.len();

        // Update selection animation if changed
        if self.palette_sel_last_index != Some(selected_visible) {
            self.palette_sel_last_index = Some(selected_visible);
            self.palette_sel_anim_start = Some(std::time::Instant::now());
        }
        let sel_anim_t = if let Some(start) = self.palette_sel_anim_start {
            let elapsed = start.elapsed().as_millis() as f32;
            let dur = 120.0_f32;
            (elapsed / dur).clamp(0.0, 1.0)
        } else {
            1.0
        };
        // Ease-out cubic
        let sel_anim_eased = 1.0 - (1.0 - sel_anim_t).powi(3);

        // Row cursor in grid units (start drawing inside the panel)
        let mut line = panel_start_line;

        // Header (inside panel, inset by 2 cols)
        let header = if count == 1 {
            "Command Palette — 1 item".to_string()
        } else {
            format!("Command Palette — {} items", count)
        };
        self.draw_ai_text(
            Point::new(line, Column(panel_start_col + 2)),
            fg,
            bg,
            &header,
            panel_cols.saturating_sub(2),
        );
        line += 1;

        // Input row background
        let input_cw = size_info.cell_width();
        let input_ch = size_info.cell_height();
        let input_x = panel_x + input_cw;
        let input_y = (line as f32) * input_ch;
        let input_w = (panel_cols.saturating_sub(2)) as f32 * input_cw;
        let input_radius = ui.palette_pill_radius_px;
        let input_bg = UiRoundedRect::new(
            input_x,
            input_y,
            input_w,
            input_ch,
            input_radius,
            tokens.surface,
            0.12 * progress,
        );
        self.stage_ui_rounded_rect(input_bg);

        // Leading search icon (magnifier)
        let icon_px = ui.palette_icon_px.max(1.0);
        let ix = input_x + 4.0;
        let iy = input_y + ((input_ch - icon_px) * 0.5).max(0.0);
        // Atlas slot 0 = magnifier
        const UI_ATLAS_SLOTS: usize = 9;
        let step: f32 = 1.0 / UI_ATLAS_SLOTS as f32;
        let uv_x = 0.0f32;
        let uv_y = 0.0f32;
        let uv_w = step;
        let uv_h = 1.0f32;
        let tint = if ui.palette_icon_tint {
            accent_fg
        } else {
            crate::display::color::Rgb::new(255, 255, 255)
        };
        let icon_filter_nearest = (icon_px - 16.0).abs() < 0.5;
        self.stage_ui_sprite(UiSprite::new(
            ix,
            iy,
            icon_px,
            icon_px,
            uv_x,
            uv_y,
            uv_w,
            uv_h,
            tint,
            1.0,
            Some(icon_filter_nearest),
        ));
        let icon_cols = ((icon_px / input_cw).ceil() as usize).max(1);

        // Optional scope chip from query prefix (a:/w:)
        let lower = filter.to_lowercase();
        let scope_label: Option<&str> =
            if lower.starts_with("w:") || lower.starts_with("workflows:") {
                Some("Workflows")
            } else if lower.starts_with("a:") || lower.starts_with("actions:") {
                Some("Actions")
            } else if lower.starts_with("p:") || lower.starts_with("plugins:") {
                Some("Plugins")
            } else {
                None
            };

        // Compose prompt and draw, reserving space for icon and optional chip
        let prompt_prefix = ""; // no text prefix when icon is present
        let mut prompt = String::with_capacity(prompt_prefix.len() + filter.len());
        prompt.push_str(prompt_prefix);
        prompt.push_str(&filter);
        let mut prompt_col = panel_start_col + 1 + icon_cols + 1; // gap after icon

        if let Some(label) = scope_label {
            // Draw a small chip before the text input showing scope
            let cw = size_info.cell_width();
            let ch = size_info.cell_height();
            let pad = ui.palette_chip_pad_px;
            let chip_text = format!("[{}]", label);
            let chip_w_px = (chip_text.width() as f32) * cw + pad * 2.0;
            let chip_x = (prompt_col as f32) * cw - pad;
            let chip_y = (line as f32) * ch;
            let radius = ui.palette_pill_radius_px;
            let border_px = ui.palette_hint_border_px.max(0.0);
            if border_px > 0.0 {
                let stroke_alpha = ui.palette_hint_border_alpha;
                let outer = UiRoundedRect::new(
                    chip_x - border_px,
                    chip_y,
                    chip_w_px + border_px * 2.0,
                    ch,
                    radius,
                    tokens.border,
                    stroke_alpha,
                );
                self.stage_ui_rounded_rect(outer);
            }
            let pill = UiRoundedRect::new(
                chip_x,
                chip_y,
                chip_w_px,
                ch,
                radius,
                tokens.surface_muted,
                0.16 * progress,
            );
            self.stage_ui_rounded_rect(pill);
            let chip_fg = lerp_rgb(bg, accent_fg, 0.8);
            self.draw_ai_text(
                Point::new(line, Column(prompt_col)),
                chip_fg,
                bg,
                &chip_text,
                chip_text.width(),
            );
            prompt_col += chip_text.width() + 1; // gap after chip
        }

        // Draw the input text
        self.draw_ai_text(
            Point::new(line, Column(prompt_col)),
            fg,
            bg,
            &prompt,
            panel_cols.saturating_sub(prompt_col - panel_start_col),
        );
        let mut cursor_col = prompt_col + prompt.width();
        if cursor_col >= panel_start_col + panel_cols {
            cursor_col = panel_start_col + panel_cols - 1;
        }
        // Draw cursor block (invert)
        self.draw_ai_text(Point::new(line, Column(cursor_col)), bg, fg, " ", 1);
        line += 1;

        // Separator
        let sep = "─".repeat(panel_cols.saturating_sub(2));
        self.draw_ai_text(
            Point::new(line, Column(panel_start_col + 1)),
            muted_fg,
            bg,
            &sep,
            panel_cols.saturating_sub(2),
        );
        line += 1;

        // Results list (reserve one line for footer)
        let footer_line = panel_start_line + panel_lines - 1;

        // Empty state when no results
        if count == 0 {
            let msg = "No results";
            let sub = "Try a: for actions    w: for workflows";
            let center_line = (panel_start_line + footer_line) / 2;
            let start_col = panel_start_col + (panel_cols.saturating_sub(msg.len())) / 2;
            self.draw_ai_text(
                Point::new(center_line.saturating_sub(1), Column(start_col)),
                muted_fg,
                bg,
                msg,
                panel_cols,
            );
            let start_col2 = panel_start_col + (panel_cols.saturating_sub(sub.len())) / 2;
            self.draw_ai_text(
                Point::new(center_line.saturating_add(1), Column(start_col2)),
                muted_fg,
                bg,
                sub,
                panel_cols,
            );
            return;
        }

        let max_lines = footer_line.saturating_sub(1);
        for (idx, item) in views.iter().enumerate() {
            let row_sel_p = if idx == selected_visible { sel_anim_eased } else { 0.0 };
            // Compute hints for actions (right-aligned), allow up to 3 and render as pills
            let mut hints: Vec<String> = Vec::new();
            if let Some(&orig_i) = self.palette.filtered_indices.get(idx) {
                if let Some(orig_item) = self.palette.items.get(orig_i) {
                    if let PaletteEntry::Action(a) = &orig_item.entry {
                        hints = action_hints_for(config, a, 3);
                    }
                }
            }
            // Budget content width based on hints (approximate: each hint plus 2 spaces)
            let hints_cols_approx: usize = if hints.is_empty() {
                0
            } else {
                hints.iter().map(|h| h.width() + 2).sum::<usize>() + 1
            };
            let content_max_cols = panel_cols.saturating_sub(2).saturating_sub(hints_cols_approx);
            if line > max_lines {
                break;
            }

            // Selected row background highlight
            if idx == selected_visible {
                let y = line as f32 * size_info.cell_height();
                let x = panel_x + size_info.cell_width();
                let w = (panel_cols.saturating_sub(2)) as f32 * size_info.cell_width();
                let h = size_info.cell_height();
                let rects = vec![RenderRect::new(x, y, w, h, tokens.surface, 0.26)];
                let metrics = self.glyph_cache.font_metrics();
                let size_copy: SizeInfo = self.size_info;
                self.renderer_draw_rects(&size_copy, &metrics, rects);
            }

            // Build row: marker + title + subtitle
            let mut col_cursor = panel_start_col + 1;
            let marker = if idx == selected_visible { "▶ " } else { "  " };
            self.draw_ai_text(Point::new(line, Column(col_cursor)), fg, bg, marker, 2);
            col_cursor += 2;

            // Optional icon before title based on entry type
            if let Some(&orig_i) = self.palette.filtered_indices.get(idx) {
                if let Some(orig_item) = self.palette.items.get(orig_i) {
                    // Use sprite atlas where possible; fallback to text if needed.
                    // Atlas: 9 slots horizontally; step = 1/9
                    const UI_ATLAS_SLOTS: usize = 9;
                    const STEP: f32 = 1.0 / UI_ATLAS_SLOTS as f32;
                    fn uv_for_slot(slot: usize) -> (f32, f32, f32, f32) {
                        let x = (slot as f32) * STEP;
                        (x, 0.0, STEP, 1.0)
                    }
                    let (uv_x, uv_y, uv_w, uv_h) = match &orig_item.entry {
                        PaletteEntry::Workflow(_) => uv_for_slot(1),
                        PaletteEntry::File(_) => uv_for_slot(2),
                        PaletteEntry::Command { .. } => uv_for_slot(0),
                        #[cfg(feature = "plugins")]
                        PaletteEntry::PluginCommand { .. } => uv_for_slot(8),
                        PaletteEntry::Action(a) => {
                            use BindingAction as BA;
                            let slot = match a {
                                BA::CreateTab => 3,
                                BA::SplitVertical => 4,
                                BA::SplitHorizontal => 5,
                                BA::FocusNextPane | BA::FocusPreviousPane => 6,
                                BA::ToggleZoom => 7,
                                BA::OpenBlocksSearchPanel => 0,
                                BA::OpenWorkflowsPanel => 1,
                                _ => 0,
                            };
                            uv_for_slot(slot)
                        }
                    };
                    let cw = size_info.cell_width();
                    let ch = size_info.cell_height();
                    // Use theme-configurable icon px (Warp baseline 16px), centered vertically in
                    // row
                    let icon_px: f32 = ui.palette_icon_px.max(1.0);
                    let x = (col_cursor as f32) * cw;
                    let y = (line as f32) * ch + ((ch - icon_px) * 0.5).max(0.0);
                    let w = icon_px;
                    let h = icon_px;
                    let tint = if ui.palette_icon_tint {
                        if idx == selected_visible {
                            accent_fg
                        } else {
                            lerp_rgb(bg, accent_fg, 0.6)
                        }
                    } else {
                        crate::display::color::Rgb::new(255, 255, 255)
                    };
                    // Determine filter: honor config override if present, otherwise auto (NEAREST
                    // at 1x icon size)
                    let forced = config.theme.palette_icon_filter_nearest;
                    let auto_nearest = (w - 16.0).abs() < 0.5 && (h - 16.0).abs() < 0.5;
                    let filter_nearest = forced.unwrap_or(auto_nearest);
                    // Stage sprite; fallback to text if the atlas/icon set is unavailable.
                    self.stage_ui_sprite(UiSprite::new(
                        x,
                        y,
                        w,
                        h,
                        uv_x,
                        uv_y,
                        uv_w,
                        uv_h,
                        tint,
                        1.0,
                        Some(filter_nearest),
                    ));
                    // Reserve text columns equivalent to icon width
                    let icon_cols = ((w / cw).ceil() as usize).max(1);
                    col_cursor += icon_cols;
                }
            }

            let row_start_col = col_cursor;

            // Title with fuzzy highlights (and pill background behind visible title)
            let q = filter.trim().to_lowercase();
            let title = &item.title;
            let title_lower = title.to_lowercase();
            let hl_positions =
                if q.is_empty() { Vec::new() } else { fuzzy_highlight_positions(&q, &title_lower) };
            // Compute visible title and width under budget
            let mut visible = String::new();
            let mut visible_w = 0usize;
            for ch in title.chars() {
                let cw = ch.width().unwrap_or(1);
                if visible_w + cw >= content_max_cols.saturating_sub(2) {
                    break;
                }
                visible.push(ch);
                visible_w += cw;
            }
            // Draw pill behind title with inner padding and selection scale
            if visible_w > 0 {
                let cw = size_info.cell_width();
                let ch = size_info.cell_height();
                let pad = ui.palette_title_pad_px; // px
                let x = (row_start_col as f32) * cw - pad;
                let y = (line as f32) * ch;
                let w = (visible_w as f32) * cw + pad * 2.0;
                let h = ch;
                let scale = 1.0 + ui.palette_selection_scale * row_sel_p;
                let cx = x + w * 0.5;
                let scaled_w = w * scale;
                let sx = cx - scaled_w * 0.5;
                let radius = ui.palette_pill_radius_px;
                let alpha = 0.12 * progress + 0.10 * row_sel_p;
                let pill = UiRoundedRect::new(sx, y, scaled_w, h, radius, tokens.surface, alpha);
                self.stage_ui_rounded_rect(pill);
            }
            // Accent background behind matching segments (Warp-like)
            if visible_w > 0 {
                let cwpx = size_info.cell_width();
                let chpx = size_info.cell_height();
                let mut col_offset = 0usize; // columns advanced within visible
                let mut run_active = false;
                let mut run_start_cols = 0usize;
                let mut run_width_cols = 0usize;
                for (i, ch_) in visible.chars().enumerate() {
                    let ch_cols = ch_.width().unwrap_or(1);
                    let is_hl = hl_positions.binary_search(&i).is_ok();
                    if is_hl && !run_active {
                        run_active = true;
                        run_start_cols = col_offset;
                        run_width_cols = ch_cols;
                    } else if is_hl && run_active {
                        run_width_cols += ch_cols;
                    } else if !is_hl && run_active {
                        // Flush run
                        let x = ((row_start_col + run_start_cols) as f32) * cwpx;
                        let y = (line as f32) * chpx;
                        let w = (run_width_cols as f32) * cwpx;
                        let pill = UiRoundedRect::new(
                            x,
                            y,
                            w,
                            chpx,
                            ui.palette_pill_radius_px * 0.6,
                            accent_fg,
                            0.17,
                        );
                        self.stage_ui_rounded_rect(pill);
                        run_active = false;
                        run_width_cols = 0;
                    }
                    col_offset += ch_cols;
                }
                if run_active && run_width_cols > 0 {
                    let x = ((row_start_col + run_start_cols) as f32) * cwpx;
                    let y = (line as f32) * chpx;
                    let w = (run_width_cols as f32) * cwpx;
                    let pill = UiRoundedRect::new(
                        x,
                        y,
                        w,
                        chpx,
                        ui.palette_pill_radius_px * 0.6,
                        accent_fg,
                        0.17,
                    );
                    self.stage_ui_rounded_rect(pill);
                }
            }

            // Row-level color dimming + selection brightness animation
            let white = crate::display::color::Rgb::new(255, 255, 255);
            let row_fg_base = if idx == selected_visible { fg } else { lerp_rgb(bg, fg, 0.7) };
            let row_acc_base =
                if idx == selected_visible { accent_fg } else { lerp_rgb(bg, accent_fg, 0.7) };
            let row_mut_base =
                if idx == selected_visible { muted_fg } else { lerp_rgb(bg, muted_fg, 0.7) };
            let row_fg = if idx == selected_visible {
                lerp_rgb(row_fg_base, white, 0.10 * row_sel_p)
            } else {
                row_fg_base
            };
            let row_acc = if idx == selected_visible {
                lerp_rgb(row_acc_base, white, 0.12 * row_sel_p)
            } else {
                row_acc_base
            };
            let row_mut = if idx == selected_visible {
                lerp_rgb(row_mut_base, white, 0.06 * row_sel_p)
            } else {
                row_mut_base
            };
            // Draw title segments with highlights
            let mut seg = String::new();
            let mut seg_color = row_fg;
            let mut col_used = 0usize;
            for (i, ch) in visible.chars().enumerate() {
                let is_hl = hl_positions.binary_search(&i).is_ok();
                let ch_color = if is_hl { row_acc } else { row_fg };
                if ch_color != seg_color && !seg.is_empty() {
                    // flush segment
                    self.draw_ai_text(
                        Point::new(line, Column(col_cursor)),
                        seg_color,
                        bg,
                        &seg,
                        content_max_cols.saturating_sub(col_used),
                    );
                    col_cursor += seg.width();
                    col_used += seg.width();
                    seg.clear();
                }
                seg.push(ch);
                seg_color = ch_color;
            }
            if !seg.is_empty() {
                self.draw_ai_text(
                    Point::new(line, Column(col_cursor)),
                    seg_color,
                    bg,
                    &seg,
                    content_max_cols.saturating_sub(col_used),
                );
                col_cursor += seg.width();
            }

            // Category chip (Action/Workflow)
            if let Some(&orig_i) = self.palette.filtered_indices.get(idx) {
                if let Some(orig_item) = self.palette.items.get(orig_i) {
                    let chip = match orig_item.entry {
                        PaletteEntry::Action(_) => " [⚙ Action]",
                        PaletteEntry::Workflow(_) => " [⚡ Workflow]",
                        PaletteEntry::File(_) => " [📄 File]",
                        PaletteEntry::Command { .. } => " [› Command]",
                        #[cfg(feature = "plugins")]
                        PaletteEntry::PluginCommand { .. } => " [🔌 Plugin]",
                    };
                    if col_cursor < row_start_col + content_max_cols - chip.width() {
                        // Draw chip pill background with inner padding and selection scale
                        let cw = size_info.cell_width();
                        let ch = size_info.cell_height();
                        let pad = ui.palette_chip_pad_px;
                        let x = (col_cursor as f32) * cw - pad;
                        let y = (line as f32) * ch;
                        let w = (chip.width() as f32) * cw + pad * 2.0;
                        let h = ch;
                        let scale = 1.0 + ui.palette_selection_scale * row_sel_p;
                        let cx = x + w * 0.5;
                        let scaled_w = w * scale;
                        let sx = cx - scaled_w * 0.5;
                        let radius = ui.palette_pill_radius_px;
                        // Border then background to match hint pill style
                        let border_px = ui.palette_hint_border_px.max(0.0);
                        if border_px > 0.0 {
                            let stroke_alpha = ui.palette_hint_border_alpha;
                            let outer = UiRoundedRect::new(
                                sx - border_px,
                                y,
                                scaled_w + border_px * 2.0,
                                h,
                                radius,
                                tokens.border,
                                stroke_alpha,
                            );
                            self.stage_ui_rounded_rect(outer);
                        }
                        let alpha = 0.18 * progress + 0.10 * row_sel_p;
                        let pill = UiRoundedRect::new(
                            sx,
                            y,
                            scaled_w,
                            h,
                            radius,
                            tokens.surface_muted,
                            alpha,
                        );
                        self.stage_ui_rounded_rect(pill);
                        let row_chip_fg = if idx == selected_visible {
                            accent_fg
                        } else {
                            lerp_rgb(bg, accent_fg, 0.7)
                        };
                        self.draw_ai_text(
                            Point::new(line, Column(col_cursor)),
                            row_chip_fg,
                            bg,
                            chip,
                            chip.width(),
                        );
                        col_cursor += chip.width();
                    }
                }
            }

            // Subtitle with fuzzy highlights as well
            if let Some(sub) = &item.subtitle {
                if !sub.is_empty() && col_cursor < row_start_col + content_max_cols - 3 {
                    let sep = " — ";
                    self.draw_ai_text(
                        Point::new(line, Column(col_cursor)),
                        row_mut,
                        bg,
                        sep,
                        content_max_cols.saturating_sub(col_cursor - row_start_col),
                    );
                    col_cursor += sep.width();
                    let avail = (row_start_col + content_max_cols).saturating_sub(col_cursor + 1);
                    if avail > 3 {
                        let q = filter.trim().to_lowercase();
                        let sub_lower = sub.to_lowercase();
                        let hl_positions = if q.is_empty() {
                            Vec::new()
                        } else {
                            fuzzy_highlight_positions(&q, &sub_lower)
                        };

                        // Accent background runs behind subtitle matches
                        let cwpx = size_info.cell_width();
                        let chpx = size_info.cell_height();
                        let mut cols_before = 0usize;
                        let mut run_active = false;
                        let mut run_start_cols = 0usize;
                        let mut run_width_cols = 0usize;
                        for (i, ch) in sub.chars().enumerate() {
                            let ccols = ch.width().unwrap_or(1);
                            if cols_before + ccols >= avail.saturating_sub(3) {
                                break;
                            }
                            let is_hl = hl_positions.binary_search(&i).is_ok();
                            if is_hl && !run_active {
                                run_active = true;
                                run_start_cols = cols_before;
                                run_width_cols = ccols;
                            } else if is_hl && run_active {
                                run_width_cols += ccols;
                            } else if !is_hl && run_active {
                                let x = ((row_start_col
                                    + (col_cursor - row_start_col)
                                    + run_start_cols)
                                    as f32)
                                    * cwpx;
                                let y = (line as f32) * chpx;
                                let w = (run_width_cols as f32) * cwpx;
                                let pill = UiRoundedRect::new(
                                    x,
                                    y,
                                    w,
                                    chpx,
                                    ui.palette_pill_radius_px * 0.6,
                                    accent_fg,
                                    0.14,
                                );
                                self.stage_ui_rounded_rect(pill);
                                run_active = false;
                                run_width_cols = 0;
                            }
                            cols_before += ccols;
                        }
                        if run_active && run_width_cols > 0 {
                            let x = ((row_start_col + (col_cursor - row_start_col) + run_start_cols)
                                as f32)
                                * cwpx;
                            let y = (line as f32) * chpx;
                            let w = (run_width_cols as f32) * cwpx;
                            let pill = UiRoundedRect::new(
                                x,
                                y,
                                w,
                                chpx,
                                ui.palette_pill_radius_px * 0.6,
                                accent_fg,
                                0.14,
                            );
                            self.stage_ui_rounded_rect(pill);
                        }

                        let mut used = 0usize;
                        let mut seg = String::new();
                        let mut seg_color = row_mut;
                        for (i, ch) in sub.chars().enumerate() {
                            let cw = ch.width().unwrap_or(1);
                            if used + cw >= avail.saturating_sub(3) {
                                // leave space for ellipsis
                                break;
                            }
                            let is_hl = hl_positions.binary_search(&i).is_ok();
                            let ch_color = if is_hl { row_acc } else { row_mut };
                            if ch_color != seg_color && !seg.is_empty() {
                                self.draw_ai_text(
                                    Point::new(line, Column(col_cursor)),
                                    seg_color,
                                    bg,
                                    &seg,
                                    avail.saturating_sub(used),
                                );
                                col_cursor += seg.width();
                                used += seg.width();
                                seg.clear();
                            }
                            seg.push(ch);
                            seg_color = ch_color;
                        }
                        if !seg.is_empty() && used < avail {
                            self.draw_ai_text(
                                Point::new(line, Column(col_cursor)),
                                seg_color,
                                bg,
                                &seg,
                                avail.saturating_sub(used),
                            );
                            col_cursor += seg.width();
                            used += seg.width();
                        }
                        // Ellipsis if truncated
                        if used < sub.width() && avail >= 3 {
                            self.draw_ai_text(
                                Point::new(line, Column(col_cursor)),
                                muted_fg,
                                bg,
                                "...",
                                3,
                            );
                        }
                    }
                }
            }

            // Draw hint(s) at right edge as individual pill badges
            if !hints.is_empty() {
                let cw = size_info.cell_width();
                let ch = size_info.cell_height();
                let pad_px = ui.palette_hint_pad_px;
                let gap_px = ui.palette_hint_gap_px;
                let radius = ui.palette_pill_radius_px;
                let border_px = ui.palette_hint_border_px.max(0.0);
                let mut right_px = ((panel_start_col + panel_cols - 2) as f32) * cw;
                for h in hints.iter().rev() {
                    let wcols = h.width();
                    let text_w_px = (wcols as f32) * cw;
                    let full_w_px = text_w_px + pad_px * 2.0;
                    let start_px = right_px - full_w_px;
                    // Border pill (simulate stroke)
                    if border_px > 0.0 {
                        let stroke_alpha = ui.palette_hint_border_alpha;
                        let outer = UiRoundedRect::new(
                            start_px - border_px,
                            (line as f32) * ch,
                            full_w_px + border_px * 2.0,
                            ch,
                            radius,
                            tokens.border,
                            stroke_alpha,
                        );
                        self.stage_ui_rounded_rect(outer);
                    }
                    // Background pill (use surface_muted to differentiate)
                    let alpha = 0.18 * progress + 0.10 * row_sel_p;
                    let pill = UiRoundedRect::new(
                        start_px,
                        (line as f32) * ch,
                        full_w_px,
                        ch,
                        radius,
                        tokens.surface_muted,
                        alpha,
                    );
                    self.stage_ui_rounded_rect(pill);
                    // Text offset by pad columns
                    let pad_cols = (pad_px / cw).floor() as usize;
                    let start_col = ((start_px / cw).floor() as usize).saturating_add(pad_cols);
                    let row_hint_fg = if idx == selected_visible {
                        muted_fg
                    } else {
                        lerp_rgb(bg, muted_fg, 0.6)
                    };
                    self.draw_ai_text(
                        Point::new(line, Column(start_col)),
                        row_hint_fg,
                        bg,
                        h,
                        wcols,
                    );
                    right_px = start_px - gap_px;
                }
            }

            line += 1;
        }

        // Footer hints
        if line <= footer_line {
            let hints = "Enter • Run    Shift+Enter • Edit    Alt+Enter • cd dir    Esc • Close    ↑/↓ • Navigate";
            self.draw_ai_text(
                Point::new(footer_line, Column(panel_start_col + 2)),
                muted_fg,
                bg,
                hints,
                panel_cols.saturating_sub(4),
            );
        }
    }
}

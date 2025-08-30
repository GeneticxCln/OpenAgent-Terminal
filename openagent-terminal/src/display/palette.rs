use crate::config::bindings::Action;

/// Entry that can be executed from the palette.
#[derive(Clone, Debug)]
pub enum PaletteEntry {
    Action(Action),
    Workflow(String),
}

#[derive(Clone, Debug)]
pub struct PaletteItem {
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
}

impl PaletteState {
    pub fn new() -> Self {
        Self { active: false, filter: String::new(), items: Vec::new(), filtered_indices: Vec::new(), selected: 0 }
    }

    pub fn active(&self) -> bool { self.active }

    pub fn open(&mut self, items: Vec<PaletteItem>) {
        self.items = items;
        self.filter.clear();
        self.selected = 0;
        self.active = true;
        self.refilter();
    }

    pub fn close(&mut self) { self.active = false; }

    pub fn items(&self) -> &Vec<PaletteItem> { &self.items }

    pub fn filter(&self) -> &str { &self.filter }

    pub fn set_filter(&mut self, filter: String) { self.filter = filter; self.refilter(); }

    pub fn push_filter_char(&mut self, ch: char) { self.filter.push(ch); self.refilter(); }

    pub fn pop_filter_char(&mut self) { self.filter.pop(); self.refilter(); }

    pub fn move_selection(&mut self, delta: isize) {
        if self.filtered_indices.is_empty() { self.selected = 0; return; }
        let len = self.filtered_indices.len() as isize;
        let mut idx = self.selected as isize + delta;
        if idx < 0 { idx = 0; }
        if idx >= len { idx = len - 1; }
        self.selected = idx as usize;
    }

    pub fn selected_entry(&self) -> Option<&PaletteEntry> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|&i| self.items.get(i))
            .map(|it| &it.entry)
    }

    /// Returns (filter, selected_visible_index, visible_items)
    pub fn view(&self) -> (String, usize, Vec<PaletteItemView>) {
        let visible = self.filtered_indices
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
            (Some("workflow"), q.splitn(2, ':').nth(1).unwrap_or("").trim().to_string())
        } else if q.starts_with("a:") || q.starts_with("actions:") {
            (Some("action"), q.splitn(2, ':').nth(1).unwrap_or("").trim().to_string())
        } else {
            (None, q)
        };

        self.filtered_indices.clear();
        for (i, it) in self.items.iter().enumerate() {
            if let Some(ft) = filter_type {
                match (&it.entry, ft) {
                    (PaletteEntry::Action(_), "action") => {},
                    (PaletteEntry::Workflow(_), "workflow") => {},
                    _ => continue,
                }
            }

            if term.is_empty() {
                self.filtered_indices.push(i);
                continue;
            }

            let hay = format!("{} {}", it.title.to_lowercase(), it.subtitle.as_deref().unwrap_or("").to_lowercase());
            if hay.contains(&term) {
                self.filtered_indices.push(i);
            }
        }
        if self.selected >= self.filtered_indices.len() { self.selected = self.filtered_indices.len().saturating_sub(1); }
    }
}

#[derive(Clone, Debug)]
pub struct PaletteItemView {
    pub title: String,
    pub subtitle: Option<String>,
}


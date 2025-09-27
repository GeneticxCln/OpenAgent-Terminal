// Minimal Blocks Search panel stubs (feature="never")
#![allow(dead_code)]

#[derive(Default, Clone, Debug)]
pub struct BlocksSearchFilters {
    pub starred_only: bool,
    pub tags: Vec<String>,
    pub directory: Option<String>,
    pub shell: Option<&'static str>,
    pub status: Option<&'static str>,
    pub exit_code: Option<i32>,
    pub duration: Option<u64>,
    pub date_from: Option<&'static str>,
    pub date_to: Option<&'static str>,
}

#[derive(Default, Clone, Copy, Debug)]
pub enum SearchMode {
    #[default]
    Basic,
    Command,
    Output,
    Advanced,
}

#[derive(Default, Clone, Debug)]
pub struct BlocksSearchItem {
    pub id: String,
    pub command: String,
    pub output: String,
    pub directory: String,
    pub created_at: String,
    pub modified_at: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub starred: bool,
    pub tags: Vec<String>,
    pub shell: String,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct BlocksSearchState {
    pub active: bool,
    pub query: String,
    pub selected: usize,
    pub results: Vec<BlocksSearchItem>,
    pub items_per_page: usize,
    pub current_page: usize,
    pub mode: SearchMode,
    pub filters: BlocksSearchFilters,
    pub sort_field: Option<&'static str>,
    pub sort_order: Option<&'static str>,
}

impl Default for BlocksSearchState {
    fn default() -> Self {
        Self {
            active: false,
            query: String::new(),
            selected: 0,
            results: Vec::new(),
            items_per_page: 50,
            current_page: 0,
            mode: SearchMode::Basic,
            filters: BlocksSearchFilters::default(),
            sort_field: None,
            sort_order: None,
        }
    }
}

impl BlocksSearchState {
    pub fn new() -> Self { Self::default() }
    pub fn open(&mut self) { self.active = true; }
    pub fn close(&mut self) { self.active = false; }
    pub fn move_selection(&mut self, delta: isize) {
        let len = self.results.len();
        if len == 0 { return; }
        let cur = self.selected as isize;
        let mut idx = cur + delta;
        if idx < 0 { idx = 0; }
        if idx as usize >= len { idx = (len - 1) as isize; }
        self.selected = idx as usize;
    }

    // --- Additional no-op helpers to satisfy feature="never" event paths ---
    pub fn toggle_starred_filter(&mut self) { self.filters.starred_only = !self.filters.starred_only; }
    pub fn clear_all_filters(&mut self) { self.filters = BlocksSearchFilters::default(); }
    pub fn next_page(&mut self) { self.current_page = self.current_page.saturating_add(1); }
    pub fn prev_page(&mut self) { self.current_page = self.current_page.saturating_sub(1); }
    pub fn get_selected_item(&self) -> Option<BlocksSearchItem> { self.results.get(self.selected).cloned() }

    // Actions menu stubs
    pub fn open_actions_menu(&mut self) {}
    pub fn close_actions_menu(&mut self) {}
    pub fn actions_menu_active(&self) -> bool { false }
    pub fn get_selected_action(&self) -> Option<crate::display::blocks_search_actions::BlockAction> { None }
    pub fn move_actions_selection(&mut self, _delta: isize) {}

    // Help overlay stubs
    pub fn open_help(&mut self) {}
    pub fn close_help(&mut self) {}
    pub fn help_active(&self) -> bool { false }
    pub fn navigate_help(&mut self, _forward: bool) {}

    // Sorting / mode
    pub fn cycle_search_mode(&mut self) { self.mode = match self.mode { SearchMode::Basic => SearchMode::Command, SearchMode::Command => SearchMode::Output, SearchMode::Output => SearchMode::Advanced, SearchMode::Advanced => SearchMode::Basic } }
    pub fn cycle_sort_field(&mut self) {
        self.sort_field = match self.sort_field {
            None => Some("date"),
            Some("date") => Some("duration"),
            Some("duration") => Some("exit_code"),
            _ => None,
        };
    }
    pub fn toggle_sort_order(&mut self) {
        self.sort_order = match self.sort_order {
            None => Some("desc"),
            Some("desc") => Some("asc"),
            _ => None,
        };
    }
}

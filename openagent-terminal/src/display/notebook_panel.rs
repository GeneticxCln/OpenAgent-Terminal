// Minimal stubs for a legacy Notebooks panel (feature="never").
#![allow(dead_code)]

#[derive(Clone, Debug, Default)]
pub struct NotebookListItem {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct NotebookCellItem {
    pub id: String,
    pub idx: usize,
    pub cell_type: String, // "md" | "cmd"
    pub summary: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Default)]
pub struct NotebookEditSession {
    pub path: NotebookPath,
    pub cell_id: String,
}

#[derive(Clone, Debug, Default)]
pub struct NotebookPath(pub Option<std::path::PathBuf>);

impl AsRef<std::path::Path> for NotebookPath {
    fn as_ref(&self) -> &std::path::Path {
        self.0.as_deref().unwrap_or(std::path::Path::new(""))
    }
}

#[derive(Clone, Debug, Default)]
pub enum FocusArea {
    #[default]
    Notebooks,
    Cells,
}

#[derive(Clone, Debug, Default)]
pub struct NotebookPanelState {
    pub active: bool,
    pub notebooks: Vec<NotebookListItem>,
    pub selected_notebook: Option<String>,
    pub cells: Vec<NotebookCellItem>,
    pub selected_cell: usize,
    pub focus: FocusArea,
}

impl NotebookPanelState {
    pub fn new() -> Self { Self::default() }
    pub fn open(&mut self) { self.active = true; }
    pub fn close(&mut self) { self.active = false; }
    pub fn toggle_focus(&mut self) { self.focus = match self.focus { FocusArea::Notebooks => FocusArea::Cells, FocusArea::Cells => FocusArea::Notebooks }; }
}

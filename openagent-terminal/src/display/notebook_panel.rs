// Production-ready Notebooks panel system
// Supports Jupyter-style notebooks with cells, execution, and persistence

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotebookListItem {
    pub id: String,
    pub name: String,
    pub path: Option<PathBuf>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub modified_at: chrono::DateTime<chrono::Utc>,
    pub cell_count: usize,
    pub tags: Vec<String>,
}

impl Default for NotebookListItem {
    fn default() -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: "Untitled Notebook".to_string(),
            path: None,
            created_at: now,
            modified_at: now,
            cell_count: 0,
            tags: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotebookCellItem {
    pub id: String,
    pub idx: usize,
    pub cell_type: CellType,
    pub content: String,
    pub summary: String,
    pub output: Option<CellOutput>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub execution_count: Option<u64>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CellType {
    Markdown,
    Code { language: String },
    Raw,
}

impl Default for CellType {
    fn default() -> Self {
        CellType::Code { language: "bash".to_string() }
    }
}

impl std::fmt::Display for CellType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CellType::Markdown => write!(f, "markdown"),
            CellType::Code { language } => write!(f, "code:{}", language),
            CellType::Raw => write!(f, "raw"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CellOutput {
    pub stdout: String,
    pub stderr: String,
    pub display_data: HashMap<String, serde_json::Value>,
    pub execution_time: chrono::DateTime<chrono::Utc>,
}

impl Default for NotebookCellItem {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            idx: 0,
            cell_type: CellType::default(),
            content: String::new(),
            summary: String::new(),
            output: None,
            exit_code: None,
            duration_ms: 0,
            execution_count: None,
            metadata: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NotebookEditSession {
    pub path: NotebookPath,
    pub cell_id: String,
    pub cursor_position: usize,
    pub is_editing: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NotebookPath(pub Option<PathBuf>);

impl AsRef<std::path::Path> for NotebookPath {
    fn as_ref(&self) -> &std::path::Path {
        self.0.as_deref().unwrap_or(std::path::Path::new(""))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum FocusArea {
    #[default]
    Notebooks,
    Cells,
    Editor,
}

#[derive(Clone, Debug)]
pub struct NotebookPanelState {
    pub active: bool,
    pub notebooks: Vec<NotebookListItem>,
    pub selected_notebook: Option<String>,
    pub cells: Vec<NotebookCellItem>,
    pub selected_cell: usize,
    pub focus: FocusArea,
    pub search_query: String,
    pub edit_session: Option<NotebookEditSession>,
    pub kernel_status: KernelStatus,
    pub execution_queue: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum KernelStatus {
    Idle,
    Busy,
    Starting,
    Error(String),
    Disconnected,
}

impl Default for NotebookPanelState {
    fn default() -> Self {
        Self::new()
    }
}

impl NotebookPanelState {
    pub fn new() -> Self {
        Self {
            active: false,
            notebooks: Vec::new(),
            selected_notebook: None,
            cells: Vec::new(),
            selected_cell: 0,
            focus: FocusArea::default(),
            search_query: String::new(),
            edit_session: None,
            kernel_status: KernelStatus::Idle,
            execution_queue: Vec::new(),
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.focus = FocusArea::Notebooks;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.edit_session = None;
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            FocusArea::Notebooks => FocusArea::Cells,
            FocusArea::Cells => FocusArea::Notebooks,
            FocusArea::Editor => FocusArea::Cells,
        };
    }

    pub fn move_notebook_selection(&mut self, delta: isize) {
        if self.notebooks.is_empty() {
            return;
        }
        let len = self.notebooks.len() as isize;
        let current = self.notebooks.iter().position(|n| {
            Some(&n.id) == self.selected_notebook.as_ref()
        }).unwrap_or(0) as isize;
        
        let new_idx = ((current + delta).max(0).min(len - 1)) as usize;
        self.selected_notebook = Some(self.notebooks[new_idx].id.clone());
    }

    pub fn move_cell_selection(&mut self, delta: isize) {
        if self.cells.is_empty() {
            self.selected_cell = 0;
            return;
        }
        let len = self.cells.len() as isize;
        let new_idx = ((self.selected_cell as isize + delta).max(0).min(len - 1)) as usize;
        self.selected_cell = new_idx;
    }

    pub fn start_edit_session(&mut self, cell_id: String) {
        if let Some(cell) = self.cells.iter().find(|c| c.id == cell_id) {
            self.edit_session = Some(NotebookEditSession {
                path: NotebookPath(None),
                cell_id: cell_id.clone(),
                cursor_position: cell.content.len(),
                is_editing: true,
            });
            self.focus = FocusArea::Editor;
        }
    }

    pub fn end_edit_session(&mut self) {
        self.edit_session = None;
        self.focus = FocusArea::Cells;
    }

    pub fn add_cell(&mut self, cell_type: CellType) -> String {
        let cell_id = Uuid::new_v4().to_string();
        let idx = self.cells.len();
        
        let mut cell = NotebookCellItem::default();
        cell.id = cell_id.clone();
        cell.idx = idx;
        cell.cell_type = cell_type;
        
        self.cells.push(cell);
        self.selected_cell = idx;
        
        cell_id
    }

    pub fn delete_cell(&mut self, cell_id: &str) -> bool {
        if let Some(pos) = self.cells.iter().position(|c| c.id == cell_id) {
            self.cells.remove(pos);
            // Reindex remaining cells
            for (idx, cell) in self.cells.iter_mut().enumerate() {
                cell.idx = idx;
            }
            // Adjust selection
            if self.selected_cell >= self.cells.len() && !self.cells.is_empty() {
                self.selected_cell = self.cells.len() - 1;
            }
            true
        } else {
            false
        }
    }

    pub fn get_selected_cell(&self) -> Option<&NotebookCellItem> {
        self.cells.get(self.selected_cell)
    }

    pub fn get_selected_cell_mut(&mut self) -> Option<&mut NotebookCellItem> {
        self.cells.get_mut(self.selected_cell)
    }

    pub fn execute_cell(&mut self, cell_id: String) {
        if !self.execution_queue.contains(&cell_id) {
            self.execution_queue.push(cell_id);
            self.kernel_status = KernelStatus::Busy;
        }
    }

    pub fn clear_execution_queue(&mut self) {
        self.execution_queue.clear();
        self.kernel_status = KernelStatus::Idle;
    }
}

//! Notebook-style terminal experience for OpenAgent Terminal
//!
//! Provides Jupyter-like cell-based command execution and visualization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

/// Type alias for notebook IDs
pub type NotebookId = String;

/// Type alias for cell IDs  
pub type CellId = String;

impl FromStr for NotebookId {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.to_string())
    }
}

impl FromStr for CellId {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.to_string())
    }
}

/// Notebook cell types supported
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellType {
    /// Shell command cell
    Command,
    /// Markdown documentation cell
    Markdown,
    /// Raw text cell
    Raw,
}

/// Individual notebook cell
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookCell {
    pub id: String,
    pub cell_type: CellType,
    pub content: String,
    pub output: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub execution_count: Option<u32>,
}

impl NotebookCell {
    pub fn new(cell_type: CellType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            cell_type,
            content: String::new(),
            output: None,
            metadata: HashMap::new(),
            execution_count: None,
        }
    }
}

/// Notebook list item for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookListItem {
    pub id: String,
    pub title: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub modified_at: chrono::DateTime<chrono::Utc>,
    pub cell_count: usize,
}

/// Notebook cell item for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookCellItem {
    pub id: String,
    pub idx: usize,
    pub cell_type: String,
    pub summary: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub has_output: bool,
    pub execution_count: Option<u32>,
}

/// Complete notebook structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    pub id: String,
    pub title: String,
    pub cells: Vec<NotebookCell>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub modified_at: chrono::DateTime<chrono::Utc>,
}

impl Notebook {
    pub fn new(title: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            title,
            cells: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            modified_at: now,
        }
    }

    pub fn add_cell(&mut self, cell_type: CellType) -> String {
        let cell = NotebookCell::new(cell_type);
        let id = cell.id.clone();
        self.cells.push(cell);
        self.modified_at = chrono::Utc::now();
        id
    }

    pub fn remove_cell(&mut self, cell_id: &str) -> bool {
        if let Some(pos) = self.cells.iter().position(|c| c.id == cell_id) {
            self.cells.remove(pos);
            self.modified_at = chrono::Utc::now();
            true
        } else {
            false
        }
    }

    pub fn get_cell_mut(&mut self, cell_id: &str) -> Option<&mut NotebookCell> {
        self.cells.iter_mut().find(|c| c.id == cell_id)
    }

    pub fn execute_cell(&mut self, cell_id: &str, output: String, execution_count: u32) {
        if let Some(cell) = self.get_cell_mut(cell_id) {
            cell.output = Some(output);
            cell.execution_count = Some(execution_count);
            self.modified_at = chrono::Utc::now();
        }
    }
}

/// Notebook manager for terminal integration
#[derive(Debug)]
pub struct NotebookManager {
    notebooks: HashMap<String, Notebook>,
    current_notebook: Option<String>,
}

impl NotebookManager {
    pub fn new() -> Self {
        Self {
            notebooks: HashMap::new(),
            current_notebook: None,
        }
    }

    pub fn create_notebook(&mut self, title: String) -> String {
        let notebook = Notebook::new(title);
        let id = notebook.id.clone();
        self.notebooks.insert(id.clone(), notebook);
        self.current_notebook = Some(id.clone());
        id
    }

    pub fn get_notebook(&self, id: &str) -> Option<&Notebook> {
        self.notebooks.get(id)
    }

    pub fn get_notebook_mut(&mut self, id: &str) -> Option<&mut Notebook> {
        self.notebooks.get_mut(id)
    }

    pub fn list_notebooks(&self) -> Vec<NotebookListItem> {
        self.notebooks
            .values()
            .map(|nb| NotebookListItem {
                id: nb.id.clone(),
                title: nb.title.clone(),
                created_at: nb.created_at,
                modified_at: nb.modified_at,
                cell_count: nb.cells.len(),
            })
            .collect()
    }

    pub fn delete_notebook(&mut self, id: &str) -> bool {
        if self.notebooks.remove(id).is_some() {
            if self.current_notebook.as_ref() == Some(id) {
                self.current_notebook = None;
            }
            true
        } else {
            false
        }
    }

    pub fn set_current_notebook(&mut self, id: String) -> bool {
        if self.notebooks.contains_key(&id) {
            self.current_notebook = Some(id);
            true
        } else {
            false
        }
    }

    pub fn current_notebook_id(&self) -> Option<&String> {
        self.current_notebook.as_ref()
    }

    pub fn current_notebook(&self) -> Option<&Notebook> {
        self.current_notebook
            .as_ref()
            .and_then(|id| self.notebooks.get(id))
    }

    pub fn current_notebook_mut(&mut self) -> Option<&mut Notebook> {
        let id = self.current_notebook.clone()?;
        self.notebooks.get_mut(&id)
    }
}

impl Default for NotebookManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notebook_creation() {
        let mut manager = NotebookManager::new();
        let id = manager.create_notebook("Test Notebook".to_string());
        
        assert!(manager.get_notebook(&id).is_some());
        assert_eq!(manager.current_notebook_id(), Some(&id));
    }

    #[test]
    fn test_cell_operations() {
        let mut notebook = Notebook::new("Test".to_string());
        let cell_id = notebook.add_cell(CellType::Code { language: "bash".to_string() });
        
        assert_eq!(notebook.cells.len(), 1);
        assert!(notebook.get_cell_mut(&cell_id).is_some());
        
        let removed = notebook.remove_cell(&cell_id);
        assert!(removed);
        assert_eq!(notebook.cells.len(), 0);
    }

    #[test]
    fn test_cell_execution() {
        let mut notebook = Notebook::new("Test".to_string());
        let cell_id = notebook.add_cell(CellType::Code { language: "bash".to_string() });
        
        notebook.execute_cell(&cell_id, "Hello, world!".to_string(), 1);
        
        let cell = notebook.get_cell_mut(&cell_id).unwrap();
        assert_eq!(cell.output, Some("Hello, world!".to_string()));
        assert_eq!(cell.execution_count, Some(1));
    }
}
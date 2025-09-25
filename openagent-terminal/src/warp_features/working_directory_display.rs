use std::env;
use std::path::{Path, PathBuf};
use std::collections::VecDeque;
use std::time::SystemTime;

/// Warp-style working directory display with visual indicators
pub struct WorkingDirectoryDisplay {
    current_dir: PathBuf,
    directory_history: VecDeque<PathBuf>,
    max_history: usize,
    show_git_info: bool,
    show_relative_paths: bool,
    compact_home: bool,
}

#[derive(Debug, Clone)]
pub struct DirectoryInfo {
    pub path: PathBuf,
    pub display_name: String,
    pub is_git_repo: bool,
    pub git_branch: Option<String>,
    pub git_dirty: bool,
    pub last_modified: Option<SystemTime>,
    pub size_indicator: SizeIndicator,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SizeIndicator {
    Small,    // < 10 files
    Medium,   // 10-100 files  
    Large,    // 100-1000 files
    VeryLarge, // > 1000 files
}

impl WorkingDirectoryDisplay {
    pub fn new() -> Self {
        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        
        Self {
            current_dir,
            directory_history: VecDeque::new(),
            max_history: 20,
            show_git_info: true,
            show_relative_paths: true,
            compact_home: true,
        }
    }

    /// Update the current working directory
    pub fn set_current_dir(&mut self, path: PathBuf) {
        if path != self.current_dir {
            self.directory_history.push_front(self.current_dir.clone());
            if self.directory_history.len() > self.max_history {
                self.directory_history.pop_back();
            }
            self.current_dir = path;
        }
    }

    /// Get current directory information with visual indicators
    pub fn get_current_info(&self) -> DirectoryInfo {
        self.analyze_directory(&self.current_dir)
    }

    /// Get formatted display string for current directory
    pub fn get_display_string(&self) -> String {
        let info = self.get_current_info();
        self.format_directory_display(&info)
    }

    /// Get compact display suitable for prompt integration
    pub fn get_compact_display(&self) -> String {
        let info = self.get_current_info();
        let path_display = self.format_path_compact(&info.path);
        
        if info.is_git_repo {
            if let Some(branch) = &info.git_branch {
                let status_icon = if info.git_dirty { "●" } else { "○" };
                format!("{} {} {}", path_display, branch, status_icon)
            } else {
                format!("{} git", path_display)
            }
        } else {
            path_display
        }
    }

    /// Get clickable breadcrumb navigation
    pub fn get_breadcrumbs(&self) -> Vec<BreadcrumbItem> {
        let mut breadcrumbs = Vec::new();
        let mut current_path = PathBuf::new();
        
        for component in self.current_dir.components() {
            current_path.push(component);
            breadcrumbs.push(BreadcrumbItem {
                name: component.as_os_str().to_string_lossy().to_string(),
                path: current_path.clone(),
                is_current: current_path == self.current_dir,
            });
        }
        
        breadcrumbs
    }

    /// Navigate to a directory from breadcrumb click
    pub fn navigate_to(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        if path.exists() && path.is_dir() {
            self.set_current_dir(path.canonicalize()?);
            env::set_current_dir(&self.current_dir)?;
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Directory does not exist"
            ))
        }
    }

    /// Get directory history for quick navigation
    pub fn get_history(&self) -> &VecDeque<PathBuf> {
        &self.directory_history
    }

    /// Go back to previous directory
    pub fn go_back(&mut self) -> Result<(), std::io::Error> {
        if let Some(prev_dir) = self.directory_history.pop_front() {
            if prev_dir.exists() {
                let current = self.current_dir.clone();
                self.current_dir = prev_dir;
                env::set_current_dir(&self.current_dir)?;
                
                // Don't add the current directory back to history to avoid loops
                return Ok(());
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No previous directory available"
        ))
    }

    fn analyze_directory(&self, path: &Path) -> DirectoryInfo {
        let display_name = self.format_path_display(path);
        let is_git_repo = self.is_git_repository(path);
        let (git_branch, git_dirty) = if is_git_repo {
            self.get_git_info(path)
        } else {
            (None, false)
        };
        
        let last_modified = path.metadata()
            .and_then(|m| m.modified())
            .ok();
            
        let size_indicator = self.calculate_size_indicator(path);

        DirectoryInfo {
            path: path.to_path_buf(),
            display_name,
            is_git_repo,
            git_branch,
            git_dirty,
            last_modified,
            size_indicator,
        }
    }

    fn format_directory_display(&self, info: &DirectoryInfo) -> String {
        let mut display = String::new();
        
        // Add size indicator
        let size_icon = match info.size_indicator {
            SizeIndicator::Small => "📂",
            SizeIndicator::Medium => "📁", 
            SizeIndicator::Large => "🗂️",
            SizeIndicator::VeryLarge => "🗄️",
        };
        
        display.push_str(&format!("{} {}", size_icon, info.display_name));
        
        // Add git info if available
        if info.is_git_repo {
            if let Some(branch) = &info.git_branch {
                let git_status = if info.git_dirty { "●" } else { "○" };
                display.push_str(&format!(" (git:{} {})", branch, git_status));
            }
        }
        
        display
    }

    fn format_path_display(&self, path: &Path) -> String {
        if self.compact_home {
            if let Some(home) = dirs::home_dir() {
                if let Ok(relative) = path.strip_prefix(&home) {
                    return format!("~/{}", relative.display());
                }
            }
        }
        path.display().to_string()
    }

    fn format_path_compact(&self, path: &Path) -> String {
        let display = self.format_path_display(path);
        
        // If path is too long, show just the last few components
        if display.len() > 40 {
            if let Some(file_name) = path.file_name() {
                if let Some(parent) = path.parent() {
                    if let Some(grandparent) = parent.file_name() {
                        return format!(".../{}/{}", grandparent.to_string_lossy(), file_name.to_string_lossy());
                    }
                }
                return format!(".../{}", file_name.to_string_lossy());
            }
        }
        
        display
    }

    fn is_git_repository(&self, path: &Path) -> bool {
        let mut current = Some(path);
        while let Some(dir) = current {
            if dir.join(".git").exists() {
                return true;
            }
            current = dir.parent();
        }
        false
    }

    fn get_git_info(&self, path: &Path) -> (Option<String>, bool) {
        // Simple git status check - in a real implementation, 
        // you'd use a git library like git2
        use std::process::Command;
        
        let branch_result = Command::new("git")
            .args(&["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(path)
            .output();
            
        let branch = if let Ok(output) = branch_result {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        } else {
            None
        };

        let status_result = Command::new("git")
            .args(&["status", "--porcelain"])
            .current_dir(path)
            .output();
            
        let is_dirty = if let Ok(output) = status_result {
            output.status.success() && !output.stdout.is_empty()
        } else {
            false
        };

        (branch, is_dirty)
    }

    fn calculate_size_indicator(&self, path: &Path) -> SizeIndicator {
        if let Ok(entries) = std::fs::read_dir(path) {
            let count = entries.count();
            match count {
                0..=10 => SizeIndicator::Small,
                11..=100 => SizeIndicator::Medium,
                101..=1000 => SizeIndicator::Large,
                _ => SizeIndicator::VeryLarge,
            }
        } else {
            SizeIndicator::Small
        }
    }

    /// Configuration options
    pub fn set_show_git_info(&mut self, show: bool) {
        self.show_git_info = show;
    }

    pub fn set_compact_home(&mut self, compact: bool) {
        self.compact_home = compact;
    }

    pub fn set_show_relative_paths(&mut self, show: bool) {
        self.show_relative_paths = show;
    }
}

#[derive(Debug, Clone)]
pub struct BreadcrumbItem {
    pub name: String,
    pub path: PathBuf,
    pub is_current: bool,
}

impl Default for WorkingDirectoryDisplay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_current_directory() {
        let display = WorkingDirectoryDisplay::new();
        assert!(!display.get_display_string().is_empty());
    }

    #[test]
    fn test_directory_history() {
        let mut display = WorkingDirectoryDisplay::new();
        let original = display.current_dir.clone();
        
        // Change to a different directory
        if let Some(parent) = original.parent() {
            display.set_current_dir(parent.to_path_buf());
            assert_eq!(display.get_history().len(), 1);
            assert_eq!(display.get_history()[0], original);
        }
    }

    #[test]
    fn test_breadcrumbs() {
        let display = WorkingDirectoryDisplay::new();
        let breadcrumbs = display.get_breadcrumbs();
        assert!(!breadcrumbs.is_empty());
        
        // Last breadcrumb should be current
        if let Some(last) = breadcrumbs.last() {
            assert!(last.is_current);
        }
    }

    #[test]
    fn test_size_indicator() {
        let display = WorkingDirectoryDisplay::new();
        let info = display.get_current_info();
        
        // Should have some size indicator
        matches!(info.size_indicator, 
            SizeIndicator::Small | SizeIndicator::Medium | 
            SizeIndicator::Large | SizeIndicator::VeryLarge);
    }

    #[test]
    fn test_compact_display() {
        let display = WorkingDirectoryDisplay::new();
        let compact = display.get_compact_display();
        assert!(!compact.is_empty());
        
        // Should be shorter than full display
        let full = display.get_display_string();
        assert!(compact.len() <= full.len());
    }
}
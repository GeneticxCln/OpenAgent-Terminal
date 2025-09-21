//! Warp-style Inline IDE Integration
//!
//! Provides Warp-inspired terminal-integrated IDE features:
//! - Inline code completion in terminal
//! - Error hints and suggestions
//! - File navigation from terminal
//! - Quick fixes and refactoring suggestions

use std::collections::HashMap;
use std::path::PathBuf;

/// Warp-style inline IDE manager
pub struct WarpIde {
    /// Configuration
    config: WarpIdeConfig,
}

/// Individual completion item
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// Completion text
    pub text: String,

    /// Type of completion
    pub kind: CompletionKind,

    /// Confidence score
    pub confidence: f32,

    /// Description
    pub description: Option<String>,

    /// Icon/symbol
    pub icon: &'static str,
}

/// Types of completions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionKind {
    /// File path
    FilePath,

    /// Command name
    Command,

    /// Git branch/ref
    GitRef,
}

/// Completion triggers
#[derive(Debug, Clone)]
pub enum CompletionTrigger {
    /// File path context
    FilePath(String),

    /// Command context
    Command(String),

    /// Git context
    Git(String),
}

/// Error suggestions (minimal, used for inline messaging)
#[derive(Debug, Clone)]
pub struct ErrorSuggestion {
    /// Suggestion text
    pub text: String,
}

/// Warp IDE configuration
#[derive(Debug, Clone)]
pub struct WarpIdeConfig {
    /// Enable inline completions
    pub completions_enabled: bool,

    /// Enable error detection
    pub error_detection_enabled: bool,

    /// Completion settings
    pub completion_settings: CompletionSettings,
}

/// Completion settings
#[derive(Debug, Clone)]
pub struct CompletionSettings {
    /// Maximum suggestions
    pub max_suggestions: usize,

    /// Minimum confidence
    pub min_confidence: f32,
}

impl WarpIde {
    /// Create new Warp IDE
    pub fn new(config: WarpIdeConfig) -> Self {
        Self { config }
    }

    /// Handle terminal input for completions
    pub fn handle_input(&mut self, input: &str, cursor_pos: usize) -> Vec<CompletionItem> {
        if !self.config.completions_enabled {
            return Vec::new();
        }

        // Detect multiple completion contexts and aggregate suggestions
        let triggers = self.detect_completion_triggers(input, cursor_pos);

        let mut all: Vec<CompletionItem> = Vec::new();
        for t in triggers {
            let mut items = match t {
                CompletionTrigger::FilePath(partial) => self.get_file_completions(&partial),
                CompletionTrigger::Command(partial) => self.get_command_completions(&partial),
                CompletionTrigger::Git(partial) => self.get_git_completions(&partial),
            };
            all.append(&mut items);
        }

        // Filter by confidence and apply interleaving by category
        let min_conf = self.config.completion_settings.min_confidence;
        let max_suggestions = self.config.completion_settings.max_suggestions;

        let filtered: Vec<CompletionItem> =
            all.into_iter().filter(|i| i.confidence >= min_conf).collect();

        let interleaved = self.interleave_suggestions(filtered);

        interleaved.into_iter().take(max_suggestions).collect()
    }

    /// Handle command errors
    pub fn handle_error(
        &mut self,
        command: &str,
        exit_code: i32,
        output: &str,
    ) -> Vec<ErrorSuggestion> {
        if !self.config.error_detection_enabled {
            return Vec::new();
        }

        self.analyze_error(command, exit_code, output)
    }

    // Private helper methods

    // Interleave suggestions by category (CompletionKind) in a round-robin fashion
    fn interleave_suggestions(&self, items: Vec<CompletionItem>) -> Vec<CompletionItem> {
        use std::collections::VecDeque;
        let mut by_kind: HashMap<CompletionKind, VecDeque<CompletionItem>> = HashMap::new();

        // Group by kind
        for item in items.into_iter() {
            by_kind.entry(item.kind).or_default().push_back(item);
        }

        // Stable category order similar to Warp sections
        let order = [CompletionKind::Command, CompletionKind::FilePath, CompletionKind::GitRef];

        let mut out = Vec::new();
        let mut total_remaining: usize = by_kind.values().map(|q| q.len()).sum();

        while total_remaining > 0 {
            let mut progressed = false;

            // Prefer known order first
            for kind in order.iter() {
                if let Some(queue) = by_kind.get_mut(kind) {
                    if let Some(item) = queue.pop_front() {
                        out.push(item);
                        progressed = true;
                        total_remaining -= 1;
                    }
                }
            }

            // Drain any other unexpected kinds if present
            for (_kind, queue) in by_kind.iter_mut() {
                if let Some(item) = queue.pop_front() {
                    out.push(item);
                    progressed = true;
                    total_remaining -= 1;
                }
            }

            if !progressed {
                break;
            }
        }

        out
    }

    fn detect_completion_triggers(&self, input: &str, cursor_pos: usize) -> Vec<CompletionTrigger> {
        // Ensure we slice at a valid UTF-8 boundary; cursor_pos may be a byte index inside a multi-byte char.
        let safe_idx = if cursor_pos >= input.len() {
            input.len()
        } else {
            let mut i = cursor_pos;
            while i > 0 && !input.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        let before_cursor = &input[..safe_idx];
        let parts: Vec<&str> = before_cursor.split_whitespace().collect();

        let mut triggers = Vec::new();

        if let Some(last_part) = parts.last() {
            // File path completion
            if last_part.contains('/') || last_part.starts_with('.') || last_part.starts_with('~') {
                triggers.push(CompletionTrigger::FilePath(last_part.to_string()));
            }

            // Command completion (first word)
            if parts.len() == 1 {
                triggers.push(CompletionTrigger::Command(last_part.to_string()));
            }

            // Git command completion
            if parts.first() == Some(&"git") && parts.len() >= 2 {
                triggers.push(CompletionTrigger::Git(last_part.to_string()));
            }
        }

        triggers
    }

    fn analyze_error(&self, command: &str, exit_code: i32, output: &str) -> Vec<ErrorSuggestion> {
        // Common error patterns
        if exit_code == 127 {
            let first = command.split_whitespace().next().unwrap_or(command);
            return vec![ErrorSuggestion { text: format!("Install {}", first) }];
        }

        if output.contains("No such file or directory") {
            return vec![ErrorSuggestion { text: "Check file path and spelling".to_string() }];
        }

        Vec::new()
    }

    fn get_file_completions(&self, partial: &str) -> Vec<CompletionItem> {
        let mut completions = Vec::new();

        // Expand path
        let path = if partial.starts_with('~') {
            dirs::home_dir().unwrap_or_default().join(&partial[2..])
        } else if partial.starts_with('.') {
            std::env::current_dir().unwrap_or_default().join(partial)
        } else {
            PathBuf::from(partial)
        };

        let parent = path.parent().unwrap_or(&path);
        let filename =
            path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();

        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&filename) {
                    let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    completions.push(CompletionItem {
                        text: name,
                        kind: CompletionKind::FilePath,
                        confidence: 0.9,
                        description: Some(if is_dir { "Directory" } else { "File" }.to_string()),
                        icon: if is_dir { "📁" } else { "📄" },
                    });
                }
            }
        }

        completions
    }

    fn get_command_completions(&self, partial: &str) -> Vec<CompletionItem> {
        let mut completions = Vec::new();

        // Common commands
        let common_commands = [
            ("git", "Version control"),
            ("ls", "List directory contents"),
            ("cd", "Change directory"),
            ("mkdir", "Create directory"),
            ("rm", "Remove files"),
            ("cp", "Copy files"),
            ("mv", "Move files"),
            ("cat", "Display file contents"),
            ("grep", "Search text"),
            ("find", "Find files"),
            ("curl", "Transfer data"),
            ("docker", "Container management"),
            ("npm", "Node package manager"),
            ("cargo", "Rust package manager"),
            ("python", "Python interpreter"),
            ("node", "Node.js runtime"),
        ];

        for (cmd, desc) in common_commands.iter() {
            if cmd.starts_with(partial) {
                completions.push(CompletionItem {
                    text: cmd.to_string(),
                    kind: CompletionKind::Command,
                    confidence: 0.8,
                    description: Some(desc.to_string()),
                    icon: "⚡",
                });
            }
        }

        completions
    }

    fn get_git_completions(&self, partial: &str) -> Vec<CompletionItem> {
        let mut completions = Vec::new();

        let git_commands = [
            ("add", "Stage changes"),
            ("commit", "Create commit"),
            ("push", "Push to remote"),
            ("pull", "Pull from remote"),
            ("status", "Show status"),
            ("log", "Show commit history"),
            ("diff", "Show differences"),
            ("branch", "Manage branches"),
            ("checkout", "Switch branches/files"),
            ("merge", "Merge branches"),
            ("rebase", "Rebase commits"),
            ("stash", "Stash changes"),
        ];

        for (cmd, desc) in git_commands.iter() {
            if cmd.starts_with(partial) {
                completions.push(CompletionItem {
                    text: cmd.to_string(),
                    kind: CompletionKind::GitRef,
                    confidence: 0.9,
                    description: Some(desc.to_string()),
                    icon: "🌿",
                });
            }
        }

        completions
    }
}

impl Default for WarpIdeConfig {
    fn default() -> Self {
        Self {
            completions_enabled: true,
            error_detection_enabled: true,
            completion_settings: CompletionSettings { max_suggestions: 10, min_confidence: 0.5 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_completion_basic() {
        let mut ide = WarpIde::new(WarpIdeConfig::default());
        let input = "gi";
        let items = ide.handle_input(input, input.len());
        assert!(items.iter().any(|i| i.text == "git" && matches!(i.kind, CompletionKind::Command)));
    }

    #[test]
    fn git_subcommand_completion() {
        let mut ide = WarpIde::new(WarpIdeConfig::default());
        let input = "git ch";
        let items = ide.handle_input(input, input.len());
        assert!(items
            .iter()
            .any(|i| i.text == "checkout" && matches!(i.kind, CompletionKind::GitRef)));
    }

    #[test]
    fn completions_disabled_returns_empty() {
        let cfg = WarpIdeConfig { completions_enabled: false, ..Default::default() };
        let mut ide = WarpIde::new(cfg);
        let items = ide.handle_input("gi", 2);
        assert!(items.is_empty());
    }

    #[test]
    fn error_127_suggests_install() {
        let mut ide = WarpIde::new(WarpIdeConfig::default());
        let suggestions = ide.handle_error("git", 127, "");
        assert!(suggestions.iter().any(|s| s.text.starts_with("Install ")));
    }

    #[test]
    fn error_file_not_found_hint() {
        let mut ide = WarpIde::new(WarpIdeConfig::default());
        let suggestions = ide.handle_error("cat missing.file", 1, "No such file or directory");
        assert!(suggestions.iter().any(|s| s.text.contains("Check file path")));
    }

    #[test]
    fn error_detection_disabled_returns_empty() {
        let cfg = WarpIdeConfig { error_detection_enabled: false, ..Default::default() };
        let mut ide = WarpIde::new(cfg);
        let suggestions = ide.handle_error("git", 127, "");
        assert!(suggestions.is_empty());
    }
}

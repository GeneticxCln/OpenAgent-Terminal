//! Advanced snippet and macro system for OpenAgent Terminal
//!
//! This crate provides:
//! - Text expansion snippets
//! - Template-based command generation
//! - Integration with existing workflow system
//! - Context-aware snippet suggestions
//! - Shell-agnostic snippet execution

pub mod config;
pub mod engine;
pub mod manager;
pub mod expander;
pub mod templates;
pub mod integration;

pub use config::{Snippet, SnippetCollection, SnippetTrigger, SnippetContext};
pub use engine::SnippetEngine;
pub use manager::SnippetManager;
pub use expander::SnippetExpander;

use anyhow::Result;
use std::collections::HashMap;

/// Main snippet system interface
#[derive(Debug)]
pub struct SnippetSystem {
    manager: SnippetManager,
    engine: SnippetEngine,
    context: SnippetContext,
}

impl SnippetSystem {
    /// Create a new snippet system
    pub fn new() -> Result<Self> {
        let manager = SnippetManager::new()?;
        let engine = SnippetEngine::new();
        let context = SnippetContext::new()?;

        Ok(Self {
            manager,
            engine,
            context,
        })
    }

    /// Register a new snippet
    pub fn register_snippet(&mut self, snippet: Snippet) -> Result<()> {
        self.manager.add_snippet(snippet)
    }

    /// Find matching snippets for a trigger
    pub fn find_snippets(&self, trigger: &str) -> Result<Vec<&Snippet>> {
        self.manager.find_by_trigger(trigger)
    }

    /// Expand a snippet with the given context
    pub fn expand_snippet(&self, snippet: &Snippet, variables: HashMap<String, String>) -> Result<String> {
        self.engine.expand(snippet, variables, &self.context)
    }

    /// Get snippet suggestions based on current context
    pub fn get_suggestions(&self, input: &str, limit: Option<usize>) -> Result<Vec<SnippetSuggestion>> {
        self.manager.get_suggestions(input, limit.unwrap_or(10))
    }

    /// Load snippets from a directory
    pub fn load_snippets_from_dir(&mut self, dir: &std::path::Path) -> Result<()> {
        self.manager.load_from_directory(dir)
    }

    /// Save current snippets to a file
    pub fn save_snippets(&self, path: &std::path::Path) -> Result<()> {
        self.manager.save_to_file(path)
    }

    /// Update the current context (working directory, environment, etc.)
    pub fn update_context(&mut self, new_context: SnippetContext) {
        self.context = new_context;
    }

    /// Import snippets from other formats (VSCode, TextExpander, etc.)
    pub fn import_snippets(&mut self, format: ImportFormat, path: &std::path::Path) -> Result<()> {
        integration::import_snippets(&mut self.manager, format, path)
    }

    /// Export snippets to different formats
    pub fn export_snippets(&self, format: ExportFormat, path: &std::path::Path) -> Result<()> {
        integration::export_snippets(&self.manager, format, path)
    }

    /// Enable snippet completion mode
    pub fn enable_completion_mode(&mut self) -> Result<()> {
        // This would integrate with the terminal's input handling
        Ok(())
    }

    /// Convert a workflow to snippets
    pub fn workflow_to_snippets(&mut self, workflow_path: &std::path::Path) -> Result<Vec<Snippet>> {
        integration::convert_workflow_to_snippets(workflow_path)
    }
}

/// Snippet suggestion with match quality
#[derive(Debug, Clone)]
pub struct SnippetSuggestion {
    pub snippet: Snippet,
    pub trigger_match: String,
    pub score: f64,
    pub context_relevance: f64,
}

/// Supported import formats
#[derive(Debug, Clone)]
pub enum ImportFormat {
    VSCode,
    TextExpander,
    Alfred,
    Espanso,
    Autokey,
    OpenAgentWorkflow,
}

/// Supported export formats
#[derive(Debug, Clone)]
pub enum ExportFormat {
    Json,
    Yaml,
    VSCode,
    TextExpander,
    OpenAgentWorkflow,
}

impl Default for SnippetSystem {
    fn default() -> Self {
        Self::new().expect("Failed to create snippet system")
    }
}

/// Utility functions for snippet operations
pub mod utils {
    use super::*;

    /// Extract variables from a template string
    pub fn extract_variables(template: &str) -> Vec<String> {
        let mut variables = Vec::new();
        let re = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();
        
        for cap in re.captures_iter(template) {
            if let Some(var) = cap.get(1) {
                let var_name = var.as_str().trim();
                if !variables.contains(&var_name.to_string()) {
                    variables.push(var_name.to_string());
                }
            }
        }
        
        variables
    }

    /// Validate a snippet template
    pub fn validate_template(template: &str) -> Result<()> {
        match tera::Tera::one_off(template, &tera::Context::new(), false) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Invalid template: {}", e)),
        }
    }

    /// Get snippet statistics
    pub fn get_snippet_stats(snippets: &[Snippet]) -> SnippetStats {
        SnippetStats {
            total_count: snippets.len(),
            trigger_count: snippets.iter().map(|s| s.triggers.len()).sum(),
            template_count: snippets.iter().filter(|s| s.is_template).count(),
            shell_specific_count: snippets.iter().filter(|s| s.shell_specific.is_some()).count(),
        }
    }
}

/// Statistics about a snippet collection
#[derive(Debug, Clone)]
pub struct SnippetStats {
    pub total_count: usize,
    pub trigger_count: usize,
    pub template_count: usize,
    pub shell_specific_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snippet_system_creation() {
        let system = SnippetSystem::new();
        assert!(system.is_ok());
    }

    #[test]
    fn test_extract_variables() {
        let template = "Hello {{name}}, your {{item}} is ready!";
        let vars = utils::extract_variables(template);
        assert_eq!(vars, vec!["name", "item"]);
    }

    #[test]
    fn test_validate_template() {
        assert!(utils::validate_template("Hello {{name}}").is_ok());
        assert!(utils::validate_template("Hello {{").is_err());
    }

    #[tokio::test]
    #[cfg(feature = "async")]
    async fn test_async_snippet_expansion() {
        let _system = SnippetSystem::new().unwrap();
        // Test async snippet expansion
        // This would test actual async template processing
    }
}

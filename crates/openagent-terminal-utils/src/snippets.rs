//! Snippets functionality for OpenAgent Terminal
//! 
//! This module provides code snippet and template management capabilities.

use crate::{UtilsError, UtilsResult};
use std::path::Path;
use std::collections::HashMap;

/// Code snippet definition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Snippet {
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub language: Option<String>,
    pub tags: Vec<String>,
}

/// Template variable for snippet expansion
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    pub description: Option<String>,
    pub default_value: Option<String>,
}

/// Template definition with variables
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Template {
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub variables: Vec<TemplateVariable>,
}

/// Snippet manager
#[derive(Debug, Default)]
pub struct SnippetsManager {
    snippets: HashMap<String, Snippet>,
    templates: HashMap<String, Template>,
}

impl SnippetsManager {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn initialize(&mut self) -> UtilsResult<()> {
        tracing::info!("Initializing snippets manager");
        self.load_builtin_snippets()?;
        Ok(())
    }
    
    pub fn load_from_directory(&mut self, path: &Path) -> UtilsResult<()> {
        tracing::info!("Loading snippets from directory: {:?}", path);
        // TODO: Scan directory for .toml snippet files and load them
        Ok(())
    }
    
    pub fn get_snippet(&self, name: &str) -> Option<&Snippet> {
        self.snippets.get(name)
    }
    
    pub fn list_snippets(&self) -> Vec<&str> {
        self.snippets.keys().map(|s| s.as_str()).collect()
    }
    
    pub fn search_snippets(&self, query: &str) -> Vec<&Snippet> {
        self.snippets
            .values()
            .filter(|snippet| {
                snippet.name.to_lowercase().contains(&query.to_lowercase())
                    || snippet.description
                        .as_ref()
                        .map_or(false, |desc| desc.to_lowercase().contains(&query.to_lowercase()))
                    || snippet.tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query.to_lowercase()))
            })
            .collect()
    }
    
    pub fn add_snippet(&mut self, snippet: Snippet) -> UtilsResult<()> {
        let name = snippet.name.clone();
        self.snippets.insert(name, snippet);
        Ok(())
    }
    
    pub fn get_template(&self, name: &str) -> Option<&Template> {
        self.templates.get(name)
    }
    
    pub fn expand_template(&self, name: &str, variables: &HashMap<String, String>) -> UtilsResult<String> {
        if let Some(template) = self.templates.get(name) {
            let mut content = template.content.clone();
            
            // Simple variable substitution {{variable_name}}
            for (key, value) in variables {
                let placeholder = format!("{{{{{}}}}}", key);
                content = content.replace(&placeholder, value);
            }
            
            // Fill in default values for unset variables
            for template_var in &template.variables {
                let placeholder = format!("{{{{{}}}}}", template_var.name);
                if content.contains(&placeholder) {
                    if let Some(default) = &template_var.default_value {
                        content = content.replace(&placeholder, default);
                    } else {
                        content = content.replace(&placeholder, "");
                    }
                }
            }
            
            Ok(content)
        } else {
            Err(UtilsError::Snippet(format!("Template '{}' not found", name)))
        }
    }
    
    fn load_builtin_snippets(&mut self) -> UtilsResult<()> {
        // Add some basic snippets
        let git_status = Snippet {
            name: "git-status".to_string(),
            description: Some("Check git repository status".to_string()),
            content: "git status --porcelain".to_string(),
            language: Some("bash".to_string()),
            tags: vec!["git".to_string(), "status".to_string()],
        };
        
        let git_log = Snippet {
            name: "git-log".to_string(),
            description: Some("Show git commit history".to_string()),
            content: "git log --oneline -10".to_string(),
            language: Some("bash".to_string()),
            tags: vec!["git".to_string(), "history".to_string()],
        };
        
        let cargo_build = Snippet {
            name: "cargo-build".to_string(),
            description: Some("Build Rust project".to_string()),
            content: "cargo build --release".to_string(),
            language: Some("bash".to_string()),
            tags: vec!["rust".to_string(), "cargo".to_string(), "build".to_string()],
        };
        
        self.snippets.insert("git-status".to_string(), git_status);
        self.snippets.insert("git-log".to_string(), git_log);
        self.snippets.insert("cargo-build".to_string(), cargo_build);
        
        // Add a basic template
        let rust_module = Template {
            name: "rust-module".to_string(),
            description: Some("Basic Rust module template".to_string()),
            content: r#"//! {{description}}

use std::{{imports}};

/// {{struct_description}}
#[derive(Debug)]
pub struct {{struct_name}} {
    // TODO: Add fields
}

impl {{struct_name}} {
    pub fn new() -> Self {
        Self {
            // TODO: Initialize fields
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_{{test_name}}() {
        let instance = {{struct_name}}::new();
        // TODO: Add test assertions
    }
}
"#.to_string(),
            variables: vec![
                TemplateVariable {
                    name: "description".to_string(),
                    description: Some("Module description".to_string()),
                    default_value: Some("Module documentation".to_string()),
                },
                TemplateVariable {
                    name: "imports".to_string(),
                    description: Some("Standard library imports".to_string()),
                    default_value: Some("collections::HashMap".to_string()),
                },
                TemplateVariable {
                    name: "struct_name".to_string(),
                    description: Some("Name of the main struct".to_string()),
                    default_value: Some("MyStruct".to_string()),
                },
                TemplateVariable {
                    name: "struct_description".to_string(),
                    description: Some("Struct documentation".to_string()),
                    default_value: Some("Main struct for this module".to_string()),
                },
                TemplateVariable {
                    name: "test_name".to_string(),
                    description: Some("Test function name".to_string()),
                    default_value: Some("basic_functionality".to_string()),
                },
            ],
        };
        
        self.templates.insert("rust-module".to_string(), rust_module);
        Ok(())
    }
}
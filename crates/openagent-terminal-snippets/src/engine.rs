use crate::config::{Snippet, SnippetContext};
use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug)]
pub struct SnippetEngine {
    template_engine: tera::Tera,
}

impl SnippetEngine {
    pub fn new() -> Self {
        Self { template_engine: tera::Tera::new("templates/**/*").unwrap_or_default() }
    }

    pub fn expand(
        &self,
        snippet: &Snippet,
        variables: HashMap<String, String>,
        context: &SnippetContext,
    ) -> Result<String> {
        if snippet.is_template {
            self.expand_template(snippet, variables, context)
        } else {
            Ok(snippet.content.clone())
        }
    }

    fn expand_template(
        &self,
        snippet: &Snippet,
        mut variables: HashMap<String, String>,
        context: &SnippetContext,
    ) -> Result<String> {
        // Add context variables
        variables.insert(
            "current_dir".to_string(),
            context.working_directory.to_string_lossy().to_string(),
        );
        variables.insert("shell".to_string(), context.shell_type.clone());
        variables.insert("date".to_string(), context.current_time.format("%Y-%m-%d").to_string());
        variables.insert("time".to_string(), context.current_time.format("%H:%M:%S").to_string());

        if let Some(git_info) = &context.git_info {
            variables.insert("git_branch".to_string(), git_info.branch.clone());
        }

        // Create Tera context
        let mut tera_context = tera::Context::new();
        for (key, value) in variables {
            tera_context.insert(&key, &value);
        }

        // Render template
        tera::Tera::one_off(&snippet.content, &tera_context, false)
            .map_err(|e| anyhow::anyhow!("Template rendering failed: {}", e))
    }
}

use chrono::{Datelike, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single snippet with triggers and expansion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<SnippetTrigger>,
    pub content: String,
    pub is_template: bool,
    pub variables: Option<Vec<SnippetVariable>>,
    pub shell_specific: Option<String>,
    pub context_requirements: Option<ContextRequirements>,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub usage_count: u64,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
}

/// Collection of snippets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetCollection {
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub snippets: Vec<Snippet>,
    pub metadata: HashMap<String, String>,
}

/// Snippet trigger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetTrigger {
    pub pattern: String,
    pub trigger_type: TriggerType,
    pub case_sensitive: bool,
    pub word_boundary: bool,
}

/// Types of snippet triggers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerType {
    /// Simple text replacement
    Text,
    /// Regular expression
    Regex,
    /// Tab completion
    Tab,
    /// Keyword at start of line
    Keyword,
    /// Custom trigger logic
    Custom(String),
}

/// Variable definition for templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetVariable {
    pub name: String,
    pub description: Option<String>,
    pub default_value: Option<String>,
    pub variable_type: VariableType,
    pub options: Option<Vec<String>>,
    pub validation: Option<String>,
}

/// Variable types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableType {
    String,
    Number,
    Boolean,
    Choice,
    Date,
    Time,
    Path,
    Command,
    Environment,
}

/// Context requirements for snippet activation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRequirements {
    pub working_directory: Option<String>,
    pub file_extensions: Option<Vec<String>>,
    pub git_repository: Option<bool>,
    pub environment_variables: Option<HashMap<String, String>>,
    pub shell_type: Option<Vec<String>>,
    pub time_range: Option<TimeRange>,
}

/// Time-based activation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_hour: u8,
    pub end_hour: u8,
    pub days_of_week: Option<Vec<u8>>,
}

/// Current context for snippet evaluation
#[derive(Debug, Clone)]
pub struct SnippetContext {
    pub working_directory: std::path::PathBuf,
    pub shell_type: String,
    pub environment_variables: HashMap<String, String>,
    pub current_time: chrono::DateTime<chrono::Utc>,
    pub git_info: Option<GitInfo>,
    pub recent_commands: Vec<String>,
}

/// Git repository information
#[derive(Debug, Clone)]
pub struct GitInfo {
    pub branch: String,
    pub has_changes: bool,
    pub remote_url: Option<String>,
}

/// Snippet expansion settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpansionSettings {
    pub auto_expand: bool,
    pub expand_on_space: bool,
    pub expand_on_tab: bool,
    pub show_preview: bool,
    pub confirm_before_expand: bool,
    pub max_suggestions: usize,
}

impl Snippet {
    pub fn new(id: String, name: String, content: String) -> Self {
        Self {
            id,
            name,
            description: None,
            triggers: vec![],
            content,
            is_template: false,
            variables: None,
            shell_specific: None,
            context_requirements: None,
            tags: vec![],
            created_at: chrono::Utc::now(),
            usage_count: 0,
            last_used: None,
        }
    }

    pub fn with_trigger(mut self, trigger: SnippetTrigger) -> Self {
        self.triggers.push(trigger);
        self
    }

    pub fn with_template_variables(mut self, variables: Vec<SnippetVariable>) -> Self {
        self.is_template = true;
        self.variables = Some(variables);
        self
    }

    pub fn for_shell(mut self, shell: String) -> Self {
        self.shell_specific = Some(shell);
        self
    }

    pub fn with_context_requirements(mut self, requirements: ContextRequirements) -> Self {
        self.context_requirements = Some(requirements);
        self
    }

    pub fn matches_context(&self, context: &SnippetContext) -> bool {
        if let Some(requirements) = &self.context_requirements {
            // Check shell type
            if let Some(allowed_shells) = &requirements.shell_type {
                if !allowed_shells.contains(&context.shell_type) {
                    return false;
                }
            }

            // Check working directory
            if let Some(required_dir) = &requirements.working_directory {
                if !context.working_directory.to_string_lossy().contains(required_dir) {
                    return false;
                }
            }

            // Check git repository
            if let Some(needs_git) = requirements.git_repository {
                if needs_git && context.git_info.is_none() {
                    return false;
                }
                if !needs_git && context.git_info.is_some() {
                    return false;
                }
            }

            // Check environment variables
            if let Some(required_env) = &requirements.environment_variables {
                for (key, value) in required_env {
                    if context.environment_variables.get(key) != Some(value) {
                        return false;
                    }
                }
            }

            // Check time range
            if let Some(time_range) = &requirements.time_range {
                let current_hour = context.current_time.hour();
                if current_hour < time_range.start_hour as u32
                    || current_hour > time_range.end_hour as u32
                {
                    return false;
                }

                if let Some(allowed_days) = &time_range.days_of_week {
                    let current_day = context.current_time.weekday().number_from_monday() as u8;
                    if !allowed_days.contains(&current_day) {
                        return false;
                    }
                }
            }
        }

        true
    }

    pub fn record_usage(&mut self) {
        self.usage_count += 1;
        self.last_used = Some(chrono::Utc::now());
    }
}

impl SnippetTrigger {
    pub fn simple_text(pattern: String) -> Self {
        Self {
            pattern,
            trigger_type: TriggerType::Text,
            case_sensitive: false,
            word_boundary: true,
        }
    }

    pub fn regex(pattern: String) -> Self {
        Self {
            pattern,
            trigger_type: TriggerType::Regex,
            case_sensitive: false,
            word_boundary: false,
        }
    }

    pub fn tab_completion(pattern: String) -> Self {
        Self { pattern, trigger_type: TriggerType::Tab, case_sensitive: false, word_boundary: true }
    }

    pub fn matches(&self, input: &str) -> bool {
        match self.trigger_type {
            TriggerType::Text => {
                // Apply case-sensitivity consistently, then respect word_boundary
                if self.case_sensitive {
                    if self.word_boundary {
                        input.split_whitespace().any(|word| word == self.pattern)
                    } else {
                        input.contains(&self.pattern)
                    }
                } else {
                    let input_ci = input.to_lowercase();
                    let pattern_ci = self.pattern.to_lowercase();
                    if self.word_boundary {
                        input_ci.split_whitespace().any(|word| word == pattern_ci)
                    } else {
                        input_ci.contains(&pattern_ci)
                    }
                }
            }
            TriggerType::Regex => {
                if let Ok(re) = regex::Regex::new(&self.pattern) {
                    re.is_match(input)
                } else {
                    false
                }
            }
            TriggerType::Tab => input.starts_with(&self.pattern),
            TriggerType::Keyword => input.trim_start().starts_with(&self.pattern),
            TriggerType::Custom(_) => {
                // Custom trigger logic would be implemented here
                false
            }
        }
    }
}

impl SnippetContext {
    pub fn new() -> anyhow::Result<Self> {
        let working_directory = std::env::current_dir()?;
        let shell_type = detect_shell();
        let environment_variables = std::env::vars().collect();
        let current_time = chrono::Utc::now();
        let git_info = detect_git_info(&working_directory);

        Ok(Self {
            working_directory,
            shell_type,
            environment_variables,
            current_time,
            git_info,
            recent_commands: vec![],
        })
    }

    pub fn update_working_directory(&mut self, path: std::path::PathBuf) {
        self.working_directory = path;
        self.git_info = detect_git_info(&self.working_directory);
    }

    pub fn add_recent_command(&mut self, command: String) {
        self.recent_commands.insert(0, command);
        self.recent_commands.truncate(100); // Keep last 100 commands
    }
}

fn detect_shell() -> String {
    std::env::var("SHELL")
        .unwrap_or_else(|_| "bash".to_string())
        .split('/')
        .next_back()
        .unwrap_or("bash")
        .to_string()
}

fn detect_git_info(path: &std::path::Path) -> Option<GitInfo> {
    // Simple git detection - in a real implementation this would use libgit2
    let git_dir = path.join(".git");
    if git_dir.exists() {
        Some(GitInfo {
            branch: "main".to_string(), // Placeholder
            has_changes: false,
            remote_url: None,
        })
    } else {
        None
    }
}

impl Default for ExpansionSettings {
    fn default() -> Self {
        Self {
            auto_expand: true,
            expand_on_space: false,
            expand_on_tab: true,
            show_preview: true,
            confirm_before_expand: false,
            max_suggestions: 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snippet_creation() {
        let snippet = Snippet::new(
            "test".to_string(),
            "Test Snippet".to_string(),
            "echo 'hello world'".to_string(),
        );

        assert_eq!(snippet.id, "test");
        assert_eq!(snippet.name, "Test Snippet");
        assert!(!snippet.is_template);
    }

    #[test]
    fn test_trigger_matching() {
        let trigger = SnippetTrigger::simple_text("hello".to_string());
        assert!(trigger.matches("hello world"));
        assert!(trigger.matches("say hello"));
        assert!(!trigger.matches("helloworld"));
    }

    #[test]
    fn test_context_creation() {
        let context = SnippetContext::new();
        assert!(context.is_ok());
    }

    #[test]
    fn test_snippet_with_context_requirements() {
        let requirements = ContextRequirements {
            shell_type: Some(vec!["bash".to_string(), "zsh".to_string()]),
            git_repository: Some(true),
            working_directory: None,
            file_extensions: None,
            environment_variables: None,
            time_range: None,
        };

        let snippet = Snippet::new(
            "git-snippet".to_string(),
            "Git Snippet".to_string(),
            "git status".to_string(),
        )
        .with_context_requirements(requirements);

        let mut context = SnippetContext::new().unwrap();
        context.shell_type = "bash".to_string();
        context.git_info =
            Some(GitInfo { branch: "main".to_string(), has_changes: false, remote_url: None });

        assert!(snippet.matches_context(&context));
    }
}

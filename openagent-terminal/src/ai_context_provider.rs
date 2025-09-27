//! AI Context Provider
//!
//! This module provides context from the terminal environment to AI agents,
//! including current working directory, recent commands, file contents, and
//! other contextual information that helps AI provide better assistance.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Context information extracted from terminal environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalContext {
    /// Current working directory
    pub working_directory: PathBuf,
    
    /// Recent command history (limited for privacy)
    pub recent_commands: Vec<String>,
    
    /// Current git branch if in a git repository
    pub git_branch: Option<String>,
    
    /// Git status if available
    pub git_status: Option<String>,
    
    /// Environment variables (filtered for security)
    pub environment: HashMap<String, String>,
    
    /// Currently open/visible files
    pub visible_files: Vec<FileContext>,
    
    /// Project metadata if detected
    pub project_info: Option<ProjectInfo>,
    
    /// Shell type and version
    pub shell_info: Option<ShellInfo>,
    
    /// System information
    pub system_info: SystemInfo,
}

/// Information about a file in the context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContext {
    pub path: PathBuf,
    pub file_type: String,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
    /// Only include content for small text files
    pub content: Option<String>,
    /// Summary for large files
    pub summary: Option<String>,
}

/// Project information detected from directory structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub project_type: ProjectType,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub package_files: Vec<PathBuf>,
    pub entry_points: Vec<PathBuf>,
}

/// Type of project detected
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProjectType {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Go,
    Java,
    C,
    Cpp,
    Unknown,
}

/// Shell information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellInfo {
    pub name: String,
    pub version: Option<String>,
    pub config_files: Vec<PathBuf>,
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub hostname: Option<String>,
    pub username: Option<String>,
}

/// AI parameter structure for passing context to AI providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiContextParams {
    pub working_directory: String,
    pub shell_keywords: Vec<String>,
}

/// PTY-specific AI context used by the terminal
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PtyAiContext {
    /// Terminal context information
    pub terminal_context: TerminalContext,
    /// Current command being typed
    pub current_input: Option<String>,
    /// Last command output
    pub last_output: Option<String>,
    /// Error context if available
    pub error_context: Option<String>,
}

impl Default for TerminalContext {
    fn default() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            recent_commands: Vec::new(),
            git_branch: None,
            git_status: None,
            environment: HashMap::new(),
            visible_files: Vec::new(),
            project_info: None,
            shell_info: None,
            system_info: SystemInfo {
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
                hostname: None,
                username: None,
            },
        }
    }
}

/// Context provider for extracting terminal context
pub struct ContextProvider {
    /// Maximum number of recent commands to include
    max_commands: usize,
    
    /// Maximum file size to include content for (in bytes)
    max_file_size: usize,
    
    /// Filtered environment variables (for security)
    env_whitelist: Vec<String>,
}

impl ContextProvider {
    /// Create a new context provider with default settings
    pub fn new() -> Self {
        Self {
            max_commands: 10,
            max_file_size: 10_000, // 10KB
            env_whitelist: vec![
                "PATH".to_string(),
                "HOME".to_string(),
                "USER".to_string(),
                "SHELL".to_string(),
                "PWD".to_string(),
                "LANG".to_string(),
                "TERM".to_string(),
            ],
        }
    }

    /// Extract complete terminal context
    pub fn extract_context(&self) -> Result<TerminalContext> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut context = TerminalContext { working_directory: cwd, ..TerminalContext::default() };
        
        // Extract git information
        self.extract_git_info(&mut context);
        
        // Extract environment variables
        self.extract_environment(&mut context);
        
        // Detect project information
        self.detect_project_info(&mut context);
        
        // Extract shell information
        self.extract_shell_info(&mut context);
        
        // Extract system information
        self.extract_system_info(&mut context);
        
        Ok(context)
    }

    /// Add recent commands to context (called externally)
    pub fn add_recent_commands(&self, context: &mut TerminalContext, commands: Vec<String>) {
        context.recent_commands = commands
            .into_iter()
            .rev() // Most recent first
            .take(self.max_commands)
            .collect();
    }

    /// Add visible files to context
    pub fn add_visible_files(&self, context: &mut TerminalContext, file_paths: &[PathBuf]) -> Result<()> {
        context.visible_files.clear();
        
        for path in file_paths {
            if let Ok(metadata) = std::fs::metadata(path) {
                let mut file_context = FileContext {
                    path: path.clone(),
                    file_type: self.detect_file_type(path),
                    size: metadata.len(),
                    modified: metadata.modified().ok(),
                    content: None,
                    summary: None,
                };
                
                // Include content for small text files
                if metadata.len() <= self.max_file_size as u64 && self.is_text_file(path) {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        file_context.content = Some(content);
                    }
                } else if metadata.len() > self.max_file_size as u64 {
                    file_context.summary = Some(format!(
                        "Large {} file ({} bytes)",
                        file_context.file_type,
                        metadata.len()
                    ));
                }
                
                context.visible_files.push(file_context);
            }
        }
        
        Ok(())
    }

    /// Extract git repository information
    fn extract_git_info(&self, context: &mut TerminalContext) {
        // Check if we're in a git repository
        if !context.working_directory.join(".git").exists() {
            // Check parent directories
            let mut current = context.working_directory.clone();
            let mut found = false;
            
            while let Some(parent) = current.parent() {
                if parent.join(".git").exists() {
                    found = true;
                    break;
                }
                current = parent.to_path_buf();
                
                // Don't go above user's home directory
                if let Ok(home) = std::env::var("HOME") {
                    if current == PathBuf::from(home) {
                        break;
                    }
                }
            }
            
            if !found {
                return;
            }
        }
        
        // Get current branch
        if let Ok(output) = Command::new("git")
            .arg("branch")
            .arg("--show-current")
            .current_dir(&context.working_directory)
            .output()
        {
            if output.status.success() {
                if let Ok(branch) = String::from_utf8(output.stdout) {
                    context.git_branch = Some(branch.trim().to_string());
                }
            }
        }
        
        // Get git status (short form)
        if let Ok(output) = Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(&context.working_directory)
            .output()
        {
            if output.status.success() {
                if let Ok(status) = String::from_utf8(output.stdout) {
                    context.git_status = Some(status);
                }
            }
        }
    }

    /// Extract filtered environment variables
    fn extract_environment(&self, context: &mut TerminalContext) {
        for (key, value) in std::env::vars() {
            if self.env_whitelist.contains(&key) {
                context.environment.insert(key, value);
            }
        }
    }

    /// Detect project type and information
    fn detect_project_info(&self, context: &mut TerminalContext) {
        let dir = &context.working_directory;
        
        // Check for common project files
        let mut project_info = ProjectInfo {
            name: dir.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            project_type: ProjectType::Unknown,
            language: None,
            framework: None,
            package_files: Vec::new(),
            entry_points: Vec::new(),
        };
        
        // Rust project
        if dir.join("Cargo.toml").exists() {
            project_info.project_type = ProjectType::Rust;
            project_info.language = Some("Rust".to_string());
            project_info.package_files.push(dir.join("Cargo.toml"));
            
            if dir.join("src/main.rs").exists() {
                project_info.entry_points.push(dir.join("src/main.rs"));
            }
            if dir.join("src/lib.rs").exists() {
                project_info.entry_points.push(dir.join("src/lib.rs"));
            }
        }
        // JavaScript/TypeScript project
        else if dir.join("package.json").exists() {
            project_info.package_files.push(dir.join("package.json"));
            
            if dir.join("tsconfig.json").exists() {
                project_info.project_type = ProjectType::TypeScript;
                project_info.language = Some("TypeScript".to_string());
            } else {
                project_info.project_type = ProjectType::JavaScript;
                project_info.language = Some("JavaScript".to_string());
            }
            
            // Common entry points
            for entry in &["index.js", "index.ts", "app.js", "app.ts", "main.js", "main.ts"] {
                if dir.join(entry).exists() {
                    project_info.entry_points.push(dir.join(entry));
                }
            }
            
            // Detect framework
            if let Ok(content) = std::fs::read_to_string(dir.join("package.json")) {
                if content.contains("\"react\"") {
                    project_info.framework = Some("React".to_string());
                } else if content.contains("\"vue\"") {
                    project_info.framework = Some("Vue.js".to_string());
                } else if content.contains("\"angular\"") {
                    project_info.framework = Some("Angular".to_string());
                } else if content.contains("\"express\"") {
                    project_info.framework = Some("Express.js".to_string());
                }
            }
        }
        // Python project
        else if dir.join("requirements.txt").exists() || 
                 dir.join("pyproject.toml").exists() ||
                 dir.join("setup.py").exists() {
            project_info.project_type = ProjectType::Python;
            project_info.language = Some("Python".to_string());
            
            for file in &["requirements.txt", "pyproject.toml", "setup.py"] {
                if dir.join(file).exists() {
                    project_info.package_files.push(dir.join(file));
                }
            }
            
            for entry in &["main.py", "app.py", "__init__.py"] {
                if dir.join(entry).exists() {
                    project_info.entry_points.push(dir.join(entry));
                }
            }
        }
        // Go project
        else if dir.join("go.mod").exists() {
            project_info.project_type = ProjectType::Go;
            project_info.language = Some("Go".to_string());
            project_info.package_files.push(dir.join("go.mod"));
            
            if dir.join("main.go").exists() {
                project_info.entry_points.push(dir.join("main.go"));
            }
        }
        
        // Only set project info if we detected something
        if project_info.project_type != ProjectType::Unknown {
            context.project_info = Some(project_info);
        }
    }

    /// Extract shell information
    fn extract_shell_info(&self, context: &mut TerminalContext) {
        if let Ok(shell_path) = std::env::var("SHELL") {
            let shell_name = Path::new(&shell_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            
            let mut shell_info = ShellInfo {
                name: shell_name.clone(),
                version: None,
                config_files: Vec::new(),
            };
            
            // Try to get version
            if let Ok(output) = Command::new(&shell_path).arg("--version").output() {
                if let Ok(version_str) = String::from_utf8(output.stdout) {
                    shell_info.version = Some(version_str.lines().next().unwrap_or("").to_string());
                }
            }
            
            // Common shell config files
            if let Ok(home) = std::env::var("HOME") {
                let home_path = PathBuf::from(home);
                let config_files = match shell_name.as_str() {
                    "bash" => vec![".bashrc", ".bash_profile", ".profile"],
                    "zsh" => vec![".zshrc", ".zprofile", ".zshenv"],
                    "fish" => vec![".config/fish/config.fish"],
                    _ => vec![".profile"],
                };
                
                for config in config_files {
                    let config_path = home_path.join(config);
                    if config_path.exists() {
                        shell_info.config_files.push(config_path);
                    }
                }
            }
            
            context.shell_info = Some(shell_info);
        }
    }

    /// Extract system information
    fn extract_system_info(&self, context: &mut TerminalContext) {
        context.system_info.hostname = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .ok();
        
        context.system_info.username = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .ok();
    }

    /// Detect file type from extension
    fn detect_file_type(&self, path: &Path) -> String {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("rs") => "Rust".to_string(),
            Some("js") => "JavaScript".to_string(),
            Some("ts") => "TypeScript".to_string(),
            Some("py") => "Python".to_string(),
            Some("go") => "Go".to_string(),
            Some("java") => "Java".to_string(),
            Some("c") => "C".to_string(),
            Some("cpp") | Some("cc") | Some("cxx") => "C++".to_string(),
            Some("h") | Some("hpp") => "Header".to_string(),
            Some("json") => "JSON".to_string(),
            Some("toml") => "TOML".to_string(),
            Some("yaml") | Some("yml") => "YAML".to_string(),
            Some("md") => "Markdown".to_string(),
            Some("txt") => "Text".to_string(),
            Some(ext) => ext.to_uppercase(),
            None => "Unknown".to_string(),
        }
    }

    /// Check if file is likely a text file
    fn is_text_file(&self, path: &Path) -> bool {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) => matches!(
                ext,
                "rs" | "js" | "ts" | "py" | "go" | "java" | "c" | "cpp" | "cc" | "cxx" |
                "h" | "hpp" | "json" | "toml" | "yaml" | "yml" | "md" | "txt" | 
                "sh" | "bash" | "zsh" | "fish" | "xml" | "html" | "css" | "scss" |
                "sql" | "dockerfile" | "gitignore" | "env"
            ),
            None => {
                // Check common files without extensions
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    matches!(
                        filename,
                        "Dockerfile" | "Makefile" | "README" | "LICENSE" | "CHANGELOG" |
                        ".gitignore" | ".env" | ".dockerignore"
                    )
                } else {
                    false
                }
            }
        }
    }
}

impl Default for ContextProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert terminal context to AI parameters
pub fn context_to_ai_params(context: &Option<TerminalContext>) -> AiContextParams {
    match context {
        Some(ctx) => {
            let mut shell_keywords = Vec::new();
            
            // Add working directory
            shell_keywords.push(format!("cd {}", ctx.working_directory.display()));
            
            // Add git branch if available
            if let Some(branch) = &ctx.git_branch {
                shell_keywords.push(format!("git branch: {}", branch));
            }
            
            // Add project type if detected
            if let Some(project) = &ctx.project_info {
                shell_keywords.push(format!("project: {} ({:?})", project.name, project.project_type));
                if let Some(lang) = &project.language {
                    shell_keywords.push(format!("language: {}", lang));
                }
                if let Some(framework) = &project.framework {
                    shell_keywords.push(format!("framework: {}", framework));
                }
            }
            
            // Add shell info
            if let Some(shell) = &ctx.shell_info {
                shell_keywords.push(format!("shell: {}", shell.name));
            }
            
            AiContextParams {
                working_directory: ctx.working_directory.to_string_lossy().to_string(),
                shell_keywords,
            }
        }
        None => AiContextParams {
            working_directory: std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            shell_keywords: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_context_provider_creation() {
        let provider = ContextProvider::new();
        assert_eq!(provider.max_commands, 10);
        assert_eq!(provider.max_file_size, 10_000);
        assert!(provider.env_whitelist.contains(&"PATH".to_string()));
    }

    #[test]
    fn test_file_type_detection() {
        let provider = ContextProvider::new();
        
        assert_eq!(provider.detect_file_type(Path::new("test.rs")), "Rust");
        assert_eq!(provider.detect_file_type(Path::new("test.js")), "JavaScript");
        assert_eq!(provider.detect_file_type(Path::new("test.py")), "Python");
        assert_eq!(provider.detect_file_type(Path::new("test.unknown")), "UNKNOWN");
    }

    #[test]
    fn test_text_file_detection() {
        let provider = ContextProvider::new();
        
        assert!(provider.is_text_file(Path::new("test.rs")));
        assert!(provider.is_text_file(Path::new("test.json")));
        assert!(provider.is_text_file(Path::new("Dockerfile")));
        assert!(!provider.is_text_file(Path::new("test.bin")));
    }

    #[test]
    fn test_context_extraction() {
        let provider = ContextProvider::new();
        let context = provider.extract_context();
        
        assert!(context.is_ok());
        let ctx = context.unwrap();
        assert!(ctx.working_directory.exists());
        assert_eq!(ctx.system_info.os, std::env::consts::OS);
    }

    #[test]
    fn test_rust_project_detection() {
        let temp_dir = TempDir::new().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        std::fs::write(&cargo_toml, "[package]\nname = \"test\"\nversion = \"0.1.0\"\n").unwrap();
        
        let provider = ContextProvider::new();
        let mut context = TerminalContext { working_directory: temp_dir.path().to_path_buf(), ..TerminalContext::default() };
        
        provider.detect_project_info(&mut context);
        
        assert!(context.project_info.is_some());
        let project = context.project_info.unwrap();
        assert!(matches!(project.project_type, ProjectType::Rust));
        assert_eq!(project.language, Some("Rust".to_string()));
    }

    #[test]
    fn test_context_to_ai_params() {
        let context = TerminalContext { 
            working_directory: PathBuf::from("/test/project"), 
            git_branch: Some("main".to_string()), 
            ..TerminalContext::default() 
        };
        
        let params = context_to_ai_params(&Some(context));
        assert_eq!(params.working_directory, "/test/project");
        assert!(params.shell_keywords.iter().any(|k| k.contains("git branch: main")));
    }
}
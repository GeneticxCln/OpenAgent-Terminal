use async_trait::async_trait;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use super::*;
use super::conversation_manager::*;

/// Enhanced project context agent with deep project understanding
pub struct BlitzyProjectContextAgent {
    id: String,
    config: ProjectContextConfig,
    project_cache: std::sync::Arc<tokio::sync::RwLock<ProjectCache>>,
    conversation_manager: Option<std::sync::Arc<ConversationManager>>,
    is_initialized: bool,
}

/// Configuration for project context analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContextConfig {
    pub auto_detect_project_root: bool,
    pub track_file_changes: bool,
    pub analyze_git_history: bool,
    pub scan_dependencies: bool,
    pub generate_file_summaries: bool,
    pub max_file_size_bytes: u64,
    pub excluded_dirs: Vec<String>,
    pub included_extensions: Vec<String>,
}

/// Cached project information
pub struct ProjectCache {
    projects: HashMap<String, ProjectInfo>,
    file_summaries: HashMap<String, FileSummary>,
    last_updated: DateTime<Utc>,
}

impl ProjectCache {
    pub fn upsert_summary(&mut self, summary: FileSummary) {
        self.file_summaries.insert(summary.path.clone(), summary);
        self.last_updated = Utc::now();
    }
}

/// Comprehensive project information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub root_path: String,
    pub name: String,
    pub project_type: ProjectType,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub git_info: Option<GitInfo>,
    pub dependencies: Vec<Dependency>,
    pub files: Vec<ProjectFile>,
    pub structure: ProjectStructure,
    pub metadata: ProjectMetadata,
    pub created_at: DateTime<Utc>,
    pub last_analyzed: DateTime<Utc>,
}

/// Types of projects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectType {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Go,
    Java,
    Cpp,
    Web,
    Mobile,
    Library,
    Application,
    Microservice,
    Monorepo,
    Unknown,
}

/// Git repository information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    pub current_branch: String,
    pub remote_url: Option<String>,
    pub status: GitStatus,
    pub recent_commits: Vec<GitCommit>,
    pub tags: Vec<String>,
    pub stash_count: u32,
}

/// Git status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub staged_files: Vec<String>,
    pub modified_files: Vec<String>,
    pub untracked_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub is_clean: bool,
}

/// Git commit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommit {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub date: DateTime<Utc>,
    pub files_changed: Vec<String>,
}

/// Project dependency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub dependency_type: DependencyType,
    pub source: String, // package.json, Cargo.toml, etc.
}

/// Types of dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType {
    Runtime,
    Development,
    Build,
    Test,
    Optional,
    Peer,
}

/// Project file information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub path: String,
    pub relative_path: String,
    pub file_type: String,
    pub size: u64,
    pub lines: Option<u32>,
    pub last_modified: DateTime<Utc>,
    pub importance: FileImportance,
    pub summary: Option<String>,
}

/// File importance levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileImportance {
    Critical,  // main.rs, package.json, etc.
    High,      // Core modules, important configs
    Medium,    // Regular source files
    Low,       // Tests, docs
    Ignore,    // Build artifacts, temp files
}

/// Project structure information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStructure {
    pub directories: Vec<DirectoryInfo>,
    pub entry_points: Vec<String>,
    pub config_files: Vec<String>,
    pub documentation: Vec<String>,
    pub tests: Vec<String>,
}

/// Directory information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryInfo {
    pub path: String,
    pub purpose: DirectoryPurpose,
    pub file_count: u32,
    pub size_bytes: u64,
}

/// Directory purposes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DirectoryPurpose {
    Source,
    Tests,
    Documentation,
    Configuration,
    Build,
    Assets,
    Dependencies,
    Scripts,
    Unknown,
}

/// Project metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub description: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub keywords: Vec<String>,
    pub build_system: Option<String>,
}

/// File summary generated by AI or analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSummary {
    pub path: String,
    pub summary: String,
    pub purpose: String,
    pub key_functions: Vec<String>,
    pub dependencies: Vec<String>,
    pub complexity_score: f32,
    pub generated_at: DateTime<Utc>,
}

/// Request for project context analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContextRequest {
    pub path: Option<String>,
    pub include_git: bool,
    pub include_dependencies: bool,
    pub include_file_summaries: bool,
    pub max_files: Option<usize>,
}

/// Response with project context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContextResponse {
    pub project: Option<ProjectInfo>,
    pub context_summary: String,
    pub relevant_files: Vec<ProjectFile>,
    pub suggestions: Vec<String>,
    pub confidence_score: f32,
}

impl Default for ProjectContextConfig {
    fn default() -> Self {
        Self {
            auto_detect_project_root: true,
            track_file_changes: true,
            analyze_git_history: true,
            scan_dependencies: true,
            generate_file_summaries: false, // Disabled by default due to cost
            max_file_size_bytes: 1024 * 1024, // 1MB
            excluded_dirs: vec![
                "target".to_string(),
                "node_modules".to_string(),
                ".git".to_string(),
                "build".to_string(),
                "dist".to_string(),
                ".next".to_string(),
                "__pycache__".to_string(),
            ],
            included_extensions: vec![
                "rs".to_string(),
                "js".to_string(),
                "ts".to_string(),
                "py".to_string(),
                "go".to_string(),
                "java".to_string(),
                "cpp".to_string(),
                "c".to_string(),
                "h".to_string(),
                "md".to_string(),
                "json".to_string(),
                "yaml".to_string(),
                "yml".to_string(),
                "toml".to_string(),
            ],
        }
    }
}

impl BlitzyProjectContextAgent {
    pub fn new() -> Self {
        Self {
            id: "blitzy-project-context".to_string(),
            config: ProjectContextConfig::default(),
            project_cache: std::sync::Arc::new(tokio::sync::RwLock::new(ProjectCache::new())),
            conversation_manager: None,
            is_initialized: false,
        }
    }

    pub fn with_config(mut self, config: ProjectContextConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_conversation_manager(mut self, conversation_manager: std::sync::Arc<ConversationManager>) -> Self {
        self.conversation_manager = Some(conversation_manager);
        self
    }

    /// Analyze project at given path
    pub async fn analyze_project(&self, path: &str) -> Result<ProjectInfo> {
        let root_path = if self.config.auto_detect_project_root {
            self.detect_project_root(path).await?
        } else {
            PathBuf::from(path)
        };

        let project_name = root_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let mut project = ProjectInfo {
            root_path: root_path.to_string_lossy().to_string(),
            name: project_name,
            project_type: ProjectType::Unknown,
            language: None,
            framework: None,
            git_info: None,
            dependencies: Vec::new(),
            files: Vec::new(),
            structure: ProjectStructure {
                directories: Vec::new(),
                entry_points: Vec::new(),
                config_files: Vec::new(),
                documentation: Vec::new(),
                tests: Vec::new(),
            },
            metadata: ProjectMetadata {
                description: None,
                version: None,
                author: None,
                license: None,
                homepage: None,
                keywords: Vec::new(),
                build_system: None,
            },
            created_at: Utc::now(),
            last_analyzed: Utc::now(),
        };

        // Detect project type and language
        self.detect_project_type(&root_path, &mut project).await?;

        // Analyze git repository if enabled
        if self.config.analyze_git_history {
            project.git_info = self.analyze_git_repository(&root_path).await.ok();
        }

        // Scan dependencies if enabled
        if self.config.scan_dependencies {
            project.dependencies = self.scan_dependencies(&root_path).await?;
        }

        // Analyze project structure
        project.structure = self.analyze_project_structure(&root_path).await?;

        // Scan files
        project.files = self.scan_project_files(&root_path).await?;

        // Optionally generate summaries
        if self.config.generate_file_summaries {
            let mut summaries: Vec<FileSummary> = Vec::new();
            for f in project.files.iter().filter(|f| f.size <= self.config.max_file_size_bytes) {
                if let Ok(content) = std::fs::read_to_string(&f.path) {
                    let summary = self.generate_file_summary(&f.path, &content);
                    summaries.push(summary);
                }
            }
            let mut cache = self.project_cache.write().await;
            for s in summaries {
                cache.upsert_summary(s.clone());
            }
            // Attach summaries into project files
            for pf in project.files.iter_mut() {
                if let Some(s) = cache.file_summaries.get(&pf.path) {
                    pf.summary = Some(s.summary.clone());
                }
            }
        }

        // Cache the project
        {
            let mut cache = self.project_cache.write().await;
            cache.projects.insert(project.root_path.clone(), project.clone());
            cache.last_updated = Utc::now();
        }

        // Update conversation context if available
        if let Some(conv_manager) = &self.conversation_manager {
            if let Ok(session_id) = conv_manager.get_default_session().await {
                let files: Vec<String> = project.files.iter()
                    .take(10) // Limit to avoid overwhelming context
                    .map(|f| f.path.clone())
                    .collect();
                let _ = conv_manager.update_file_context(session_id, files).await;
            }
        }

        Ok(project)
    }

    fn generate_file_summary(&self, path: &str, content: &str) -> FileSummary {
        // Simple heuristic summary: first non-empty 5 lines and keyword-based purpose
        let mut lines = content.lines().filter(|l| !l.trim().is_empty());
        let preview: String = lines.by_ref().take(5).collect::<Vec<_>>().join("\n");
        let purpose = if content.contains("fn main") || content.contains("pub fn") {
            "Source code"
        } else if path.ends_with("Cargo.toml") || path.ends_with("package.json") {
            "Manifest/Config"
        } else if path.ends_with(".md") {
            "Documentation"
        } else {
            "File"
        }.to_string();
        FileSummary {
            path: path.to_string(),
            summary: if preview.is_empty() { "(empty/undetermined)".to_string() } else { preview },
            purpose,
            key_functions: Vec::new(),
            dependencies: Vec::new(),
            complexity_score: 0.2,
            generated_at: Utc::now(),
        }
    }

    /// Detect project root by looking for marker files
    async fn detect_project_root(&self, start_path: &str) -> Result<PathBuf> {
        let mut current = PathBuf::from(start_path);
        
        // Marker files that indicate project roots
        let markers = [
            "Cargo.toml", "package.json", "go.mod", "pom.xml", "setup.py",
            "pyproject.toml", "CMakeLists.txt", "Makefile", ".git",
        ];

        loop {
            for marker in &markers {
                if current.join(marker).exists() {
                    return Ok(current);
                }
            }

            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                break;
            }
        }

        // Fallback to start path
        Ok(PathBuf::from(start_path))
    }

    /// Detect project type and language from files and structure
    async fn detect_project_type(&self, root_path: &Path, project: &mut ProjectInfo) -> Result<()> {
        // Check for specific project files
        if root_path.join("Cargo.toml").exists() {
            project.project_type = ProjectType::Rust;
            project.language = Some("Rust".to_string());
            project.metadata.build_system = Some("Cargo".to_string());
            
            // Parse Cargo.toml for metadata
            if let Ok(content) = std::fs::read_to_string(root_path.join("Cargo.toml")) {
                if let Ok(cargo_toml) = content.parse::<toml::Value>() {
                    if let Some(package) = cargo_toml.get("package") {
                        project.metadata.description = package.get("description")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        project.metadata.version = package.get("version")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        project.metadata.author = package.get("authors")
                            .and_then(|v| v.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        project.metadata.license = package.get("license")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
            }
        } else if root_path.join("package.json").exists() {
            project.language = Some("JavaScript".to_string());
            project.metadata.build_system = Some("npm".to_string());
            
            // Check for TypeScript
            if root_path.join("tsconfig.json").exists() || 
               std::fs::read_dir(root_path)
                   .unwrap_or_else(|_| std::fs::read_dir(".").unwrap())
                   .any(|entry| entry.unwrap().path().extension().and_then(|ext| ext.to_str()) == Some("ts")) {
                project.project_type = ProjectType::TypeScript;
                project.language = Some("TypeScript".to_string());
            } else {
                project.project_type = ProjectType::JavaScript;
            }

            // Parse package.json for metadata
            if let Ok(content) = std::fs::read_to_string(root_path.join("package.json")) {
                if let Ok(package_json) = serde_json::from_str::<serde_json::Value>(&content) {
                    project.metadata.description = package_json.get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    project.metadata.version = package_json.get("version")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    project.metadata.author = package_json.get("author")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    project.metadata.license = package_json.get("license")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    project.metadata.homepage = package_json.get("homepage")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
        } else if root_path.join("go.mod").exists() {
            project.project_type = ProjectType::Go;
            project.language = Some("Go".to_string());
            project.metadata.build_system = Some("Go Modules".to_string());
        } else if root_path.join("setup.py").exists() || root_path.join("pyproject.toml").exists() {
            project.project_type = ProjectType::Python;
            project.language = Some("Python".to_string());
        }

        Ok(())
    }

    /// Analyze Git repository information
    async fn analyze_git_repository(&self, root_path: &Path) -> Result<GitInfo> {
        // Check if it's a Git repository
        if !root_path.join(".git").exists() {
            return Err(anyhow!("Not a Git repository"));
        }

        let mut git_info = GitInfo {
            current_branch: String::new(),
            remote_url: None,
            status: GitStatus {
                staged_files: Vec::new(),
                modified_files: Vec::new(),
                untracked_files: Vec::new(),
                deleted_files: Vec::new(),
                is_clean: true,
            },
            recent_commits: Vec::new(),
            tags: Vec::new(),
            stash_count: 0,
        };

        // Get current branch
        if let Ok(output) = Command::new("git")
            .arg("branch")
            .arg("--show-current")
            .current_dir(root_path)
            .output() {
            git_info.current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }

        // Get remote URL
        if let Ok(output) = Command::new("git")
            .arg("remote")
            .arg("get-url")
            .arg("origin")
            .current_dir(root_path)
            .output() {
            if output.status.success() {
                git_info.remote_url = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }

        // Get status
        if let Ok(output) = Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(root_path)
            .output() {
            let status_lines = String::from_utf8_lossy(&output.stdout);
            
            git_info.status.is_clean = status_lines.trim().is_empty();
            
            for line in status_lines.lines() {
                let chars: Vec<char> = line.chars().collect();
                if chars.len() >= 3 {
                    let file_path = line[3..].to_string();
                    
                    match chars[0] {
                        'A' | 'M' | 'D' | 'R' | 'C' => git_info.status.staged_files.push(file_path.clone()),
                        _ => {}
                    }
                    
                    match chars[1] {
                        'M' => git_info.status.modified_files.push(file_path.clone()),
                        'D' => git_info.status.deleted_files.push(file_path.clone()),
                        _ => {}
                    }
                    
                    if chars[0] == '?' && chars[1] == '?' {
                        git_info.status.untracked_files.push(file_path);
                    }
                }
            }
        }

        // Get recent commits
        if let Ok(output) = Command::new("git")
            .arg("log")
            .arg("--oneline")
            .arg("-10")
            .arg("--pretty=format:%H|%s|%an|%ad")
            .arg("--date=iso")
            .current_dir(root_path)
            .output() {
            
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 4 {
                    if let Ok(date) = chrono::DateTime::parse_from_str(parts[3], "%Y-%m-%d %H:%M:%S %z") {
                        git_info.recent_commits.push(GitCommit {
                            hash: parts[0].to_string(),
                            message: parts[1].to_string(),
                            author: parts[2].to_string(),
                            date: date.with_timezone(&Utc),
                            files_changed: Vec::new(), // Could be populated separately
                        });
                    }
                }
            }
        }

        // Get tags
        if let Ok(output) = Command::new("git")
            .arg("tag")
            .arg("-l")
            .current_dir(root_path)
            .output() {
            git_info.tags = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect();
        }

        Ok(git_info)
    }

    /// Scan project dependencies
    async fn scan_dependencies(&self, root_path: &Path) -> Result<Vec<Dependency>> {
        let mut dependencies = Vec::new();

        // Scan Cargo.toml
        if let Ok(content) = std::fs::read_to_string(root_path.join("Cargo.toml")) {
            if let Ok(cargo_toml) = content.parse::<toml::Value>() {
                if let Some(deps) = cargo_toml.get("dependencies").and_then(|v| v.as_table()) {
                    for (name, version) in deps {
                        dependencies.push(Dependency {
                            name: name.clone(),
                            version: version.as_str().map(|s| s.to_string()),
                            dependency_type: DependencyType::Runtime,
                            source: "Cargo.toml".to_string(),
                        });
                    }
                }

                if let Some(dev_deps) = cargo_toml.get("dev-dependencies").and_then(|v| v.as_table()) {
                    for (name, version) in dev_deps {
                        dependencies.push(Dependency {
                            name: name.clone(),
                            version: version.as_str().map(|s| s.to_string()),
                            dependency_type: DependencyType::Development,
                            source: "Cargo.toml".to_string(),
                        });
                    }
                }
            }
        }

        // Scan package.json
        if let Ok(content) = std::fs::read_to_string(root_path.join("package.json")) {
            if let Ok(package_json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(deps) = package_json.get("dependencies").and_then(|v| v.as_object()) {
                    for (name, version) in deps {
                        dependencies.push(Dependency {
                            name: name.clone(),
                            version: version.as_str().map(|s| s.to_string()),
                            dependency_type: DependencyType::Runtime,
                            source: "package.json".to_string(),
                        });
                    }
                }

                if let Some(dev_deps) = package_json.get("devDependencies").and_then(|v| v.as_object()) {
                    for (name, version) in dev_deps {
                        dependencies.push(Dependency {
                            name: name.clone(),
                            version: version.as_str().map(|s| s.to_string()),
                            dependency_type: DependencyType::Development,
                            source: "package.json".to_string(),
                        });
                    }
                }
            }
        }

        Ok(dependencies)
    }

    /// Analyze project structure
    async fn analyze_project_structure(&self, root_path: &Path) -> Result<ProjectStructure> {
        let mut structure = ProjectStructure {
            directories: Vec::new(),
            entry_points: Vec::new(),
            config_files: Vec::new(),
            documentation: Vec::new(),
            tests: Vec::new(),
        };

        // Walk directory structure
        if let Ok(entries) = std::fs::read_dir(root_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                let relative_path = path.strip_prefix(root_path).unwrap_or(&path);
                
                if path.is_dir() {
                    let dir_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    
                    // Skip excluded directories
                    if self.config.excluded_dirs.contains(&dir_name) {
                        continue;
                    }

                    let purpose = self.determine_directory_purpose(&dir_name);
                    let (file_count, size_bytes) = self.calculate_directory_stats(&path).await.unwrap_or((0, 0));
                    
                    structure.directories.push(DirectoryInfo {
                        path: relative_path.to_string_lossy().to_string(),
                        purpose,
                        file_count,
                        size_bytes,
                    });
                } else if path.is_file() {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let path_str = relative_path.to_string_lossy().to_string();
                    
                    // Categorize files
                    if self.is_entry_point(&file_name) {
                        structure.entry_points.push(path_str.clone());
                    }
                    
                    if self.is_config_file(&file_name) {
                        structure.config_files.push(path_str.clone());
                    }
                    
                    if self.is_documentation(&file_name) {
                        structure.documentation.push(path_str.clone());
                    }
                    
                    if self.is_test_file(&file_name) {
                        structure.tests.push(path_str);
                    }
                }
            }
        }

        Ok(structure)
    }

    /// Scan project files
    async fn scan_project_files(&self, root_path: &Path) -> Result<Vec<ProjectFile>> {
        let mut files = Vec::new();

        self.scan_files_recursive(root_path, root_path, &mut files).await?;

        // Sort by importance and limit count
        files.sort_by(|a, b| {
            use FileImportance::*;
            let importance_order = |imp: &FileImportance| match imp {
                Critical => 0,
                High => 1,
                Medium => 2,
                Low => 3,
                Ignore => 4,
            };
            importance_order(&a.importance).cmp(&importance_order(&b.importance))
        });

        Ok(files)
    }

    /// Recursively scan files
    async fn scan_files_recursive(&self, current_path: &Path, root_path: &Path, files: &mut Vec<ProjectFile>) -> Result<()> {
        if let Ok(entries) = std::fs::read_dir(current_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                
                if path.is_dir() {
                    let dir_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    
                    // Skip excluded directories
                    if !self.config.excluded_dirs.contains(&dir_name) {
                        Box::pin(self.scan_files_recursive(&path, root_path, files)).await?;
                    }
                } else if path.is_file() {
                    if let Some(project_file) = self.analyze_file(&path, root_path).await? {
                        files.push(project_file);
                    }
                }
            }
        }

        Ok(())
    }

    /// Analyze individual file
    async fn analyze_file(&self, file_path: &Path, root_path: &Path) -> Result<Option<ProjectFile>> {
        let metadata = std::fs::metadata(file_path)?;
        
        // Skip large files
        if metadata.len() > self.config.max_file_size_bytes {
            return Ok(None);
        }

        let file_name = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let extension = file_path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        
        // Check if extension is included
        if !extension.is_empty() && !self.config.included_extensions.contains(&extension.to_string()) {
            return Ok(None);
        }

        let relative_path = file_path.strip_prefix(root_path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        let importance = self.determine_file_importance(&file_name, &relative_path);
        
        if matches!(importance, FileImportance::Ignore) {
            return Ok(None);
        }

        let lines = if self.is_text_file(extension) {
            self.count_lines(file_path).await.ok()
        } else {
            None
        };

        Ok(Some(ProjectFile {
            path: file_path.to_string_lossy().to_string(),
            relative_path,
            file_type: extension.to_string(),
            size: metadata.len(),
            lines,
            last_modified: DateTime::from(metadata.modified().unwrap_or(std::time::SystemTime::now())),
            importance,
            summary: None, // Could be generated by AI
        }))
    }

    // Helper methods
    fn determine_directory_purpose(&self, dir_name: &str) -> DirectoryPurpose {
        match dir_name.to_lowercase().as_str() {
            "src" | "source" | "app" | "lib" => DirectoryPurpose::Source,
            "test" | "tests" | "__tests__" | "spec" => DirectoryPurpose::Tests,
            "doc" | "docs" | "documentation" => DirectoryPurpose::Documentation,
            "config" | "configs" | "configuration" | "settings" => DirectoryPurpose::Configuration,
            "build" | "dist" | "target" | "out" | "output" => DirectoryPurpose::Build,
            "assets" | "static" | "public" | "resources" => DirectoryPurpose::Assets,
            "node_modules" | "vendor" | "deps" | "dependencies" => DirectoryPurpose::Dependencies,
            "scripts" | "bin" | "tools" => DirectoryPurpose::Scripts,
            _ => DirectoryPurpose::Unknown,
        }
    }

    async fn calculate_directory_stats(&self, dir_path: &Path) -> Result<(u32, u64)> {
        let mut file_count = 0u32;
        let mut size_bytes = 0u64;

        if let Ok(entries) = std::fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    file_count += 1;
                    if let Ok(metadata) = entry.metadata() {
                        size_bytes += metadata.len();
                    }
                }
            }
        }

        Ok((file_count, size_bytes))
    }

    fn is_entry_point(&self, file_name: &str) -> bool {
        matches!(file_name.to_lowercase().as_str(), 
            "main.rs" | "lib.rs" | "index.js" | "index.ts" | "main.go" | 
            "main.py" | "__main__.py" | "app.py" | "main.java" | "Main.java")
    }

    fn is_config_file(&self, file_name: &str) -> bool {
        matches!(file_name.to_lowercase().as_str(),
            "cargo.toml" | "package.json" | "tsconfig.json" | "go.mod" | 
            "setup.py" | "pyproject.toml" | "pom.xml" | "build.gradle" |
            "makefile" | "dockerfile" | ".gitignore" | ".env" | "config.yaml" |
            "config.yml" | "config.json" | "settings.json")
    }

    fn is_documentation(&self, file_name: &str) -> bool {
        let lower = file_name.to_lowercase();
        lower.ends_with(".md") || 
        matches!(lower.as_str(), "readme" | "readme.txt" | "changelog" | "license" | "contributing")
    }

    fn is_test_file(&self, file_name: &str) -> bool {
        let lower = file_name.to_lowercase();
        lower.contains("test") || lower.contains("spec") || lower.starts_with("test_")
    }

    fn determine_file_importance(&self, file_name: &str, relative_path: &str) -> FileImportance {
        // Critical files
        if self.is_entry_point(file_name) || self.is_config_file(file_name) {
            return FileImportance::Critical;
        }

        // High importance
        if relative_path.starts_with("src/") && !self.is_test_file(file_name) {
            return FileImportance::High;
        }

        // Low importance
        if self.is_test_file(file_name) || self.is_documentation(file_name) {
            return FileImportance::Low;
        }

        // Ignore build artifacts and temporary files
        if relative_path.contains("/target/") || 
           relative_path.contains("/build/") ||
           relative_path.contains("/dist/") ||
           file_name.starts_with('.') ||
           file_name.ends_with(".tmp") ||
           file_name.ends_with(".bak") {
            return FileImportance::Ignore;
        }

        FileImportance::Medium
    }

    fn is_text_file(&self, extension: &str) -> bool {
        matches!(extension.to_lowercase().as_str(),
            "rs" | "js" | "ts" | "py" | "go" | "java" | "cpp" | "c" | "h" | 
            "md" | "txt" | "json" | "yaml" | "yml" | "toml" | "xml" | "html" | "css")
    }

    async fn count_lines(&self, file_path: &Path) -> Result<u32> {
        let content = std::fs::read_to_string(file_path)?;
        Ok(content.lines().count() as u32)
    }
}

#[async_trait]
impl Agent for BlitzyProjectContextAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Blitzy Project Context Agent"
    }

    fn description(&self) -> &str {
        "Enhanced project context agent with deep project understanding, Git integration, and conversation awareness"
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::ProjectManagement,
            AgentCapability::ContextManagement,
            AgentCapability::GitIntegration,
            AgentCapability::FileSystem,
            AgentCapability::Custom("ProjectAnalysis".to_string()),
            AgentCapability::Custom("DependencyScanning".to_string()),
        ]
    }

    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        let mut response = AgentResponse {
            request_id: request.id,
            agent_id: self.id.clone(),
            success: false,
            payload: serde_json::json!({}),
            artifacts: Vec::new(),
            next_actions: Vec::new(),
            metadata: HashMap::new(),
        };

        match request.request_type {
            AgentRequestType::ManageProject => {
                if let Ok(project_request) = serde_json::from_value::<ProjectContextRequest>(request.payload.clone()) {
                    let path = project_request.path.unwrap_or_else(|| request.context.current_directory.clone());
                    
                    match self.analyze_project(&path).await {
                        Ok(project) => {
                            let context_summary = format!(
                                "Project: {} ({})\nFiles: {}\nLanguage: {}\nBranch: {}",
                                project.name,
                                project.project_type,
                                project.files.len(),
                                project.language.as_deref().unwrap_or("Unknown"),
                                project.git_info.as_ref().map(|g| g.current_branch.as_str()).unwrap_or("No Git")
                            );

                            let file_count = project.files.len();
                            let relevant_files = project.files.clone().into_iter().take(20).collect();
                            
                            let project_response = ProjectContextResponse {
                                project: Some(project.clone()),
                                context_summary,
                                relevant_files,
                                suggestions: vec![
                                    "Files analyzed and project structure understood".to_string(),
                                    "Git status and dependencies scanned".to_string(),
                                ],
                                confidence_score: 0.9,
                            };

                            response.success = true;
                            response.payload = serde_json::to_value(project_response)?;
                            
                            response.artifacts.push(AgentArtifact {
                                id: Uuid::new_v4(),
                                artifact_type: ArtifactType::Report,
                                content: format!("Project Analysis: {}", project.name),
                                metadata: {
                                    let mut meta = HashMap::new();
                                    meta.insert("project_type".to_string(), format!("{:?}", project.project_type));
                                    meta.insert("file_count".to_string(), file_count.to_string());
                                    meta
                                },
                            });
                        }
                        Err(e) => {
                            response.payload = serde_json::json!({
                                "error": e.to_string()
                            });
                        }
                    }
                }
            }
            _ => {
                return Err(anyhow!("Blitzy Project Context Agent cannot handle request type: {:?}", request.request_type));
            }
        }

        Ok(response)
    }

    fn can_handle(&self, request_type: &AgentRequestType) -> bool {
        matches!(request_type, AgentRequestType::ManageProject)
    }

    async fn status(&self) -> AgentStatus {
        let cache = self.project_cache.read().await;
        let cached_projects = cache.projects.len();

        AgentStatus {
            is_healthy: self.is_initialized,
            is_busy: false,
            last_activity: Utc::now(),
            current_task: if cached_projects > 0 {
                Some(format!("Tracking {} projects", cached_projects))
            } else {
                None
            },
            error_message: None,
        }
    }

    async fn initialize(&mut self, _config: AgentConfig) -> Result<()> {
        self.is_initialized = true;
        tracing::info!("Blitzy Project Context Agent initialized");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.is_initialized = false;
        tracing::info!("Blitzy Project Context Agent shut down");
        Ok(())
    }
}

impl ProjectCache {
    pub fn new() -> Self {
        Self {
            projects: HashMap::new(),
            file_summaries: HashMap::new(),
            last_updated: Utc::now(),
        }
    }
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_blitzy_project_context_agent_creation() {
        let agent = BlitzyProjectContextAgent::new();
        assert_eq!(agent.id(), "blitzy-project-context");
        assert_eq!(agent.name(), "Blitzy Project Context Agent");
    }

    #[test]
    fn test_file_importance_determination() {
        let agent = BlitzyProjectContextAgent::new();
        
        assert!(matches!(agent.determine_file_importance("main.rs", "src/main.rs"), FileImportance::Critical));
        assert!(matches!(agent.determine_file_importance("lib.rs", "src/lib.rs"), FileImportance::Critical));
        assert!(matches!(agent.determine_file_importance("module.rs", "src/module.rs"), FileImportance::High));
        assert!(matches!(agent.determine_file_importance("test_module.rs", "tests/test_module.rs"), FileImportance::Low));
        assert!(matches!(agent.determine_file_importance("temp.tmp", "target/temp.tmp"), FileImportance::Ignore));
    }
}
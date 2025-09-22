//! Project context agent for deriving and managing project information.
//! Provides working directory detection, shell identification, repository info, and project analysis.

use crate::agents::types::{
    BuildSystem, ConcurrencyState, LanguageInfo, ProjectContextInfo, ProjectType, RepoStatus,
    RepositoryInfo, ShellKind, VcsType,
};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Project context manager for analyzing and caching project information
#[derive(Debug)]
pub struct ProjectContextAgent {
    /// Cache of project context information by path
    context_cache: Arc<RwLock<HashMap<String, CachedContext>>>,
    /// Configuration for context detection
    config: ContextConfig,
    /// Concurrency state for managing overlapping operations
    concurrency_state: ConcurrencyState,
}

/// Cached project context with TTL
#[derive(Debug, Clone)]
struct CachedContext {
    info: ProjectContextInfo,
    cached_at: DateTime<Utc>,
    ttl_seconds: u64,
}

/// Configuration for project context detection
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Default cache TTL in seconds
    pub default_cache_ttl: u64,
    /// Whether to enable Git analysis
    pub enable_git_analysis: bool,
    /// Whether to analyze project dependencies
    pub analyze_dependencies: bool,
    /// Whether to detect language frameworks
    pub detect_frameworks: bool,
    /// Maximum depth for project tree analysis
    pub max_analysis_depth: usize,
    /// File patterns to ignore during analysis
    pub ignore_patterns: Vec<String>,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            default_cache_ttl: 300, // 5 minutes
            enable_git_analysis: true,
            analyze_dependencies: true,
            detect_frameworks: true,
            max_analysis_depth: 3,
            ignore_patterns: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".vscode".to_string(),
                ".idea".to_string(),
                "*.log".to_string(),
                "*.tmp".to_string(),
            ],
        }
    }
}

/// Language detection result
#[derive(Debug, Clone)]
struct LanguageDetectionResult {
    languages: HashMap<String, f64>,
    primary_language: String,
    confidence: f64,
}

/// Framework detection result
#[derive(Debug, Clone)]
struct FrameworkDetectionResult {
    frameworks: Vec<String>,
    package_managers: Vec<String>,
    build_system: Option<BuildSystem>,
}

impl ProjectContextAgent {
    /// Create a new project context agent
    pub fn new(config: ContextConfig) -> Self {
        Self {
            context_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            concurrency_state: ConcurrencyState::default(),
        }
    }

    /// Get or derive project context for a given directory
    pub async fn get_project_context<P: AsRef<Path>>(&self, path: P) -> Result<ProjectContextInfo> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        // Check cache first
        if let Some(cached) = self.get_cached_context(&path_str).await {
            if !self.is_cache_expired(&cached) {
                debug!(path = %path_str, "Using cached project context");
                return Ok(cached.info);
            }
        }

        // Derive fresh context
        info!(path = %path_str, "Deriving project context");
        let context = self.derive_project_context(path.as_ref()).await?;

        // Cache the result
        self.cache_context(&path_str, &context).await;

        Ok(context)
    }

    /// Derive complete project context information
    async fn derive_project_context(&self, path: &Path) -> Result<ProjectContextInfo> {
        let working_directory = path
            .canonicalize()
            .with_context(|| format!("Failed to canonicalize path: {:?}", path))?
            .to_string_lossy()
            .to_string();

        // Detect shell kind
        let shell_kind = self.detect_shell_kind();

        // Analyze repository information
        let repository_info = if self.config.enable_git_analysis {
            self.analyze_repository(&working_directory).await.ok()
        } else {
            None
        };

        // Detect project type and languages
        let language_result = self.detect_languages(&working_directory).await?;
        let framework_result = if self.config.detect_frameworks {
            self.detect_frameworks(&working_directory).await?
        } else {
            FrameworkDetectionResult {
                frameworks: Vec::new(),
                package_managers: Vec::new(),
                build_system: None,
            }
        };

        // Determine project type
        let project_type = self.determine_project_type(&language_result, &framework_result);

        // Collect environment variables
        let environment_vars = self.collect_relevant_env_vars();

        let now = Utc::now();

        Ok(ProjectContextInfo {
            working_directory,
            shell_kind,
            repository_info,
            project_type: Some(project_type),
            language_info: LanguageInfo {
                primary_language: language_result.primary_language,
                detected_languages: language_result.languages,
                frameworks: framework_result.frameworks,
                package_managers: framework_result.package_managers,
            },
            build_system: framework_result.build_system,
            environment_vars,
            cached_at: now,
            cache_ttl_seconds: self.config.default_cache_ttl,
        })
    }

    /// Detect the current shell kind
    fn detect_shell_kind(&self) -> ShellKind {
        // Try multiple methods to detect shell

        // Method 1: Check SHELL environment variable
        if let Ok(shell_path) = std::env::var("SHELL") {
            if shell_path.contains("bash") {
                return ShellKind::Bash;
            } else if shell_path.contains("zsh") {
                return ShellKind::Zsh;
            } else if shell_path.contains("fish") {
                return ShellKind::Fish;
            } else if shell_path.contains("pwsh") || shell_path.contains("powershell") {
                return ShellKind::PowerShell;
            }
        }

        // Method 2: Check parent process name (if available)
        // This would require platform-specific code

        // Method 3: Platform defaults
        if cfg!(windows) {
            ShellKind::PowerShell
        } else {
            // Default to bash on Unix-like systems
            ShellKind::Bash
        }
    }

    /// Analyze repository information (Git focus)
    async fn analyze_repository(&self, path: &str) -> Result<RepositoryInfo> {
        let path_buf = PathBuf::from(path);

        // Check if we're in a Git repository
        if self.is_git_repository(&path_buf) {
            self.analyze_git_repository(&path_buf).await
        } else {
            Err(anyhow!("Not a Git repository"))
        }
    }

    /// Check if directory is a Git repository
    fn is_git_repository(&self, path: &Path) -> bool {
        let mut current = path;
        loop {
            if current.join(".git").exists() {
                return true;
            }
            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }
        false
    }

    /// Analyze Git repository details
    async fn analyze_git_repository(&self, path: &Path) -> Result<RepositoryInfo> {
        let repo_root = self
            .find_git_root(path)
            .ok_or_else(|| anyhow!("Could not find Git root"))?
            .to_string_lossy()
            .to_string();

        // Get current branch
        let current_branch = self
            .get_git_output(path, &["branch", "--show-current"])
            .map(|s| s.trim().to_string())
            .ok();

        // Get current commit
        let current_commit =
            self.get_git_output(path, &["rev-parse", "HEAD"]).map(|s| s.trim().to_string()).ok();

        // Get remote URL
        let remote_url = self
            .get_git_output(path, &["remote", "get-url", "origin"])
            .map(|s| s.trim().to_string())
            .ok();

        // Analyze repository status
        let status = self.analyze_git_status(path).await?;

        Ok(RepositoryInfo {
            vcs_type: VcsType::Git,
            remote_url,
            current_branch,
            current_commit,
            status,
            root_path: repo_root,
        })
    }

    /// Find Git repository root
    fn find_git_root(&self, path: &Path) -> Option<PathBuf> {
        let mut current = path;
        loop {
            if current.join(".git").exists() {
                return Some(current.to_path_buf());
            }
            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }
        None
    }

    /// Execute Git command and get output
    fn get_git_output(&self, path: &Path, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .current_dir(path)
            .args(args)
            .output()
            .with_context(|| format!("Failed to execute git {:?}", args))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!("Git command failed: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }

    /// Analyze Git repository status
    async fn analyze_git_status(&self, path: &Path) -> Result<RepoStatus> {
        // Get porcelain status
        let status_output = self.get_git_output(path, &["status", "--porcelain"])?;

        let mut modified_files = Vec::new();
        let mut untracked_files = Vec::new();
        let mut staged_files = Vec::new();

        for line in status_output.lines() {
            if line.len() < 3 {
                continue;
            }

            let index_status = line.chars().nth(0).unwrap();
            let worktree_status = line.chars().nth(1).unwrap();
            let filename = &line[3..];

            match (index_status, worktree_status) {
                ('?', '?') => untracked_files.push(filename.to_string()),
                (_, 'M') | (_, 'D') => modified_files.push(filename.to_string()),
                ('M', _) | ('A', _) | ('D', _) => staged_files.push(filename.to_string()),
                _ => {}
            }
        }

        // Check if repository is clean
        let is_clean =
            modified_files.is_empty() && untracked_files.is_empty() && staged_files.is_empty();

        // Get ahead/behind information
        let (ahead, behind) = self.get_ahead_behind_count(path).unwrap_or((0, 0));

        Ok(RepoStatus { is_clean, modified_files, untracked_files, staged_files, ahead, behind })
    }

    /// Get ahead/behind count relative to upstream
    fn get_ahead_behind_count(&self, path: &Path) -> Option<(i32, i32)> {
        let output = self
            .get_git_output(path, &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
            .ok()?;

        let counts: Vec<&str> = output.trim().split('\t').collect();
        if counts.len() == 2 {
            let ahead = counts[0].parse().unwrap_or(0);
            let behind = counts[1].parse().unwrap_or(0);
            Some((ahead, behind))
        } else {
            None
        }
    }

    /// Detect programming languages in the project
    async fn detect_languages(&self, path: &str) -> Result<LanguageDetectionResult> {
        let path_buf = PathBuf::from(path);
        let mut language_counts = HashMap::new();
        let mut total_files = 0;

        self.analyze_directory_languages(&path_buf, &mut language_counts, &mut total_files, 0)?;

        // Convert counts to percentages
        let mut languages = HashMap::new();
        for (lang, count) in language_counts {
            let percentage = count as f64 / total_files as f64;
            languages.insert(lang, percentage);
        }

        // Determine primary language
        let primary_language = languages
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(lang, _)| lang.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let confidence = languages.get(&primary_language).cloned().unwrap_or(0.0);

        Ok(LanguageDetectionResult { languages, primary_language, confidence })
    }

    /// Recursively analyze directory for language files
    fn analyze_directory_languages(
        &self,
        dir: &Path,
        language_counts: &mut HashMap<String, u32>,
        total_files: &mut u32,
        depth: usize,
    ) -> Result<()> {
        if depth > self.config.max_analysis_depth {
            return Ok(());
        }

        let entries =
            fs::read_dir(dir).with_context(|| format!("Failed to read directory: {:?}", dir))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();

            // Skip ignored patterns
            if self.should_ignore_file(&file_name) {
                continue;
            }

            if path.is_file() {
                if let Some(language) = self.detect_language_from_file(&path) {
                    *language_counts.entry(language).or_insert(0) += 1;
                    *total_files += 1;
                }
            } else if path.is_dir() {
                self.analyze_directory_languages(&path, language_counts, total_files, depth + 1)?;
            }
        }

        Ok(())
    }

    /// Detect language from file extension and content
    fn detect_language_from_file(&self, path: &Path) -> Option<String> {
        // First handle well-known filenames without extensions (e.g., Dockerfile, Makefile)
        if let Some(name_os) = path.file_name() {
            let filename = name_os.to_string_lossy().to_lowercase();
            match filename.as_str() {
                "dockerfile" => return Some("Docker".to_string()),
                "makefile" => return Some("Makefile".to_string()),
                "cmakelists.txt" => return Some("CMake".to_string()),
                _ => {}
            }
        }

        // Then detect by extension if present
        if let Some(ext_os) = path.extension() {
            let extension = ext_os.to_string_lossy().to_lowercase();
            match extension.as_str() {
                "rs" => Some("Rust".to_string()),
                "js" | "mjs" => Some("JavaScript".to_string()),
                "ts" => Some("TypeScript".to_string()),
                "py" => Some("Python".to_string()),
                "go" => Some("Go".to_string()),
                "java" => Some("Java".to_string()),
                "cpp" | "cxx" | "cc" => Some("C++".to_string()),
                "c" => Some("C".to_string()),
                "cs" => Some("C#".to_string()),
                "rb" => Some("Ruby".to_string()),
                "php" => Some("PHP".to_string()),
                "swift" => Some("Swift".to_string()),
                "kt" => Some("Kotlin".to_string()),
                "scala" => Some("Scala".to_string()),
                "sh" | "bash" => Some("Shell".to_string()),
                "ps1" => Some("PowerShell".to_string()),
                "html" => Some("HTML".to_string()),
                "css" => Some("CSS".to_string()),
                "scss" | "sass" => Some("SCSS".to_string()),
                "json" => Some("JSON".to_string()),
                "yaml" | "yml" => Some("YAML".to_string()),
                "toml" => Some("TOML".to_string()),
                "xml" => Some("XML".to_string()),
                "md" => Some("Markdown".to_string()),
                "sql" => Some("SQL".to_string()),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Check if file should be ignored based on patterns
    fn should_ignore_file(&self, filename: &str) -> bool {
        for pattern in &self.config.ignore_patterns {
            if pattern.contains('*') {
                // Simple glob pattern matching
                let pattern_parts: Vec<&str> = pattern.split('*').collect();
                if pattern_parts.len() == 2 {
                    let starts_with = pattern_parts[0];
                    let ends_with = pattern_parts[1];
                    if filename.starts_with(starts_with) && filename.ends_with(ends_with) {
                        return true;
                    }
                }
            } else if filename == pattern {
                return true;
            }
        }
        false
    }

    /// Detect frameworks and build systems (plus package managers) by reading project manifests
    async fn detect_frameworks(&self, path: &str) -> Result<FrameworkDetectionResult> {
        let path_buf = PathBuf::from(path);
        let mut frameworks: Vec<String> = Vec::new();
        let mut package_managers: Vec<String> = Vec::new();
        let mut build_system: Option<BuildSystem> = None;

        // Rust: parse Cargo.toml for dependency names (direct). Detect common frameworks.
        let cargo_toml = path_buf.join("Cargo.toml");
        if cargo_toml.exists() {
            package_managers.push("cargo".to_string());
            build_system = Some(BuildSystem::Cargo);
            if let Ok(s) = fs::read_to_string(&cargo_toml) {
                if let Ok(value) = s.parse::<toml::Value>() {
                    let mut names: Vec<String> = Vec::new();
                    if let Some(table) = value.get("dependencies").and_then(|v| v.as_table()) {
                        names.extend(table.keys().cloned());
                    }
                    if let Some(table) = value.get("dev-dependencies").and_then(|v| v.as_table()) {
                        names.extend(table.keys().cloned());
                    }
                    let lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
                    if lower.iter().any(|n| n.starts_with("actix")) {
                        frameworks.push("actix".to_string());
                    }
                    if lower.iter().any(|n| n == "axum") {
                        frameworks.push("axum".to_string());
                    }
                    if lower.iter().any(|n| n == "tokio") {
                        frameworks.push("tokio".to_string());
                    }
                }
            }
        }

        // Node: detect manager by lockfile and analyze package.json
        let pkg_json = path_buf.join("package.json");
        if pkg_json.exists() {
            let has_pnpm = path_buf.join("pnpm-lock.yaml").exists();
            let has_yarn = path_buf.join("yarn.lock").exists();
            let has_npm = path_buf.join("package-lock.json").exists();
            if has_pnpm {
                package_managers.push("pnpm".to_string());
                build_system = Some(BuildSystem::Pnpm);
            } else if has_yarn {
                package_managers.push("yarn".to_string());
                build_system = Some(BuildSystem::Yarn);
            } else if has_npm {
                package_managers.push("npm".to_string());
                build_system = Some(BuildSystem::Npm);
            } else {
                package_managers.push("npm".to_string());
                build_system.get_or_insert(BuildSystem::Npm);
            }
            if let Ok(s) = fs::read_to_string(&pkg_json) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                    // Aggregate dependency names
                    let mut names: Vec<String> = Vec::new();
                    if let Some(deps) = v.get("dependencies").and_then(|d| d.as_object()) {
                        names.extend(deps.keys().cloned().collect::<Vec<_>>());
                    }
                    if let Some(dev) = v.get("devDependencies").and_then(|d| d.as_object()) {
                        names.extend(dev.keys().cloned().collect::<Vec<_>>());
                    }
                    let lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
                    if lower.iter().any(|n| n == "react") {
                        frameworks.push("react".to_string());
                    }
                    if lower.iter().any(|n| n == "next" || n == "next.js") {
                        frameworks.push("next".to_string());
                    }
                    if lower.iter().any(|n| n == "express") {
                        frameworks.push("express".to_string());
                    }
                } else {
                    // Fallback to substring scanning
                    self.detect_js_frameworks(&s, &mut frameworks);
                }
            }
        }

        // Python: pyproject.toml and requirements.txt
        let pyproject = path_buf.join("pyproject.toml");
        let requirements = path_buf.join("requirements.txt");
        if pyproject.exists() {
            if let Ok(s) = fs::read_to_string(&pyproject) {
                if let Ok(value) = s.parse::<toml::Value>() {
                    if value.get("tool").and_then(|t| t.get("poetry")).is_some() {
                        package_managers.push("poetry".to_string());
                        build_system = Some(BuildSystem::Custom("poetry".to_string()));
                    } else {
                        package_managers.push("pip".to_string());
                        build_system.get_or_insert(BuildSystem::Custom("pip".to_string()));
                    }
                    let mut deps: Vec<String> = Vec::new();
                    if let Some(arr) = value
                        .get("project")
                        .and_then(|p| p.get("dependencies"))
                        .and_then(|d| d.as_array())
                    {
                        for it in arr {
                            if let Some(s) = it.as_str() {
                                deps.push(s.to_string());
                            }
                        }
                    }
                    if let Some(table) = value
                        .get("tool")
                        .and_then(|t| t.get("poetry"))
                        .and_then(|p| p.get("dependencies"))
                        .and_then(|d| d.as_table())
                    {
                        deps.extend(table.keys().cloned());
                    }
                    let lower: Vec<String> = deps.iter().map(|n| n.to_lowercase()).collect();
                    if lower.iter().any(|n| n == "django") {
                        frameworks.push("django".to_string());
                    }
                    if lower.iter().any(|n| n == "flask") {
                        frameworks.push("flask".to_string());
                    }
                }
            }
        }
        if requirements.exists() {
            if let Ok(s) = fs::read_to_string(&requirements) {
                let names: Vec<String> = s
                    .lines()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .map(|l| l.split(&['=', '>', '<', ' '][..]).next().unwrap_or("").to_string())
                    .collect();
                let lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
                if lower.iter().any(|n| n == "django") && !frameworks.iter().any(|f| f == "django")
                {
                    frameworks.push("django".to_string());
                }
                if lower.iter().any(|n| n == "flask") && !frameworks.iter().any(|f| f == "flask") {
                    frameworks.push("flask".to_string());
                }
                if !package_managers.iter().any(|m| m == "poetry") {
                    package_managers.push("pip".to_string());
                    build_system.get_or_insert(BuildSystem::Custom("pip".to_string()));
                }
            }
        }

        // Go: go.mod (naive parse)
        let go_mod = path_buf.join("go.mod");
        if go_mod.exists() {
            package_managers.push("go".to_string());
            build_system = Some(BuildSystem::Custom("go".to_string()));
            if let Ok(s) = fs::read_to_string(&go_mod) {
                let lower_s = s.to_lowercase();
                if lower_s.contains("gin-gonic") && !frameworks.iter().any(|f| f == "gin") {
                    frameworks.push("gin".to_string());
                }
            }
        }

        // De-dup
        frameworks.sort();
        frameworks.dedup();
        package_managers.sort();
        package_managers.dedup();

        Ok(FrameworkDetectionResult { frameworks, package_managers, build_system })
    }

    /// Detect JavaScript/TypeScript frameworks from package.json
    fn detect_js_frameworks(&self, package_json: &str, frameworks: &mut Vec<String>) {
        let js_frameworks = [
            ("react", "React"),
            ("vue", "Vue.js"),
            ("@angular/core", "Angular"),
            ("svelte", "Svelte"),
            ("next", "Next.js"),
            ("nuxt", "Nuxt.js"),
            ("gatsby", "Gatsby"),
            ("express", "Express.js"),
            ("fastify", "Fastify"),
            ("koa", "Koa.js"),
            ("nestjs", "NestJS"),
            ("electron", "Electron"),
            ("webpack", "Webpack"),
            ("vite", "Vite"),
            ("rollup", "Rollup"),
            ("parcel", "Parcel"),
        ];

        for (dependency, framework) in &js_frameworks {
            if package_json.contains(dependency) {
                frameworks.push(framework.to_string());
            }
        }
    }

    /// Determine project type from language and framework analysis
    fn determine_project_type(
        &self,
        language_result: &LanguageDetectionResult,
        framework_result: &FrameworkDetectionResult,
    ) -> ProjectType {
        // Check build systems first
        if let Some(ref build_system) = framework_result.build_system {
            match build_system {
                BuildSystem::Cargo => return ProjectType::RustCargo,
                BuildSystem::Npm | BuildSystem::Yarn | BuildSystem::Pnpm => {
                    return ProjectType::NodeJs;
                }
                BuildSystem::Maven | BuildSystem::Gradle => return ProjectType::Java,
                _ => {}
            }
        }

        // Check primary language
        match language_result.primary_language.as_str() {
            "Rust" => ProjectType::RustCargo,
            "JavaScript" | "TypeScript" => ProjectType::NodeJs,
            "Python" => ProjectType::Python,
            "Go" => ProjectType::Go,
            "Java" => ProjectType::Java,
            "C#" => ProjectType::CSharp,
            "C++" | "C" => ProjectType::Cpp,
            _ => ProjectType::Generic,
        }
    }

    /// Collect relevant environment variables
    fn collect_relevant_env_vars(&self) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();

        let relevant_vars = [
            "PATH",
            "HOME",
            "USER",
            "SHELL",
            "PWD",
            "TERM",
            "NODE_ENV",
            "RUST_LOG",
            "PYTHONPATH",
            "GOPATH",
            "JAVA_HOME",
            "CC",
            "CXX",
            "CFLAGS",
            "CXXFLAGS",
            "LDFLAGS",
            "npm_config_registry",
            "CARGO_HOME",
            "RUSTUP_HOME",
        ];

        for var_name in &relevant_vars {
            if let Ok(value) = std::env::var(var_name) {
                env_vars.insert(var_name.to_string(), value);
            }
        }

        env_vars
    }

    /// Get cached context if available and not expired
    async fn get_cached_context(&self, path: &str) -> Option<CachedContext> {
        let cache = self.context_cache.read().await;
        cache.get(path).cloned()
    }

    /// Check if cached context is expired
    fn is_cache_expired(&self, cached: &CachedContext) -> bool {
        let now = Utc::now();
        let elapsed = (now - cached.cached_at).num_seconds() as u64;
        elapsed > cached.ttl_seconds
    }

    /// Cache project context information
    async fn cache_context(&self, path: &str, info: &ProjectContextInfo) {
        let cached = CachedContext {
            info: info.clone(),
            cached_at: info.cached_at,
            ttl_seconds: info.cache_ttl_seconds,
        };

        let mut cache = self.context_cache.write().await;
        cache.insert(path.to_string(), cached);
    }

    /// Clear expired cache entries
    pub async fn cleanup_cache(&self) {
        let mut cache = self.context_cache.write().await;
        let now = Utc::now();

        cache.retain(|_, cached| {
            let elapsed = (now - cached.cached_at).num_seconds() as u64;
            elapsed <= cached.ttl_seconds
        });
    }

    /// Invalidate cache for a specific path
    pub async fn invalidate_cache<P: AsRef<Path>>(&self, path: P) {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let mut cache = self.context_cache.write().await;
        cache.remove(&path_str);
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> (usize, usize) {
        let cache = self.context_cache.read().await;
        let total = cache.len();
        let now = Utc::now();

        let expired = cache
            .values()
            .filter(|cached| {
                let elapsed = (now - cached.cached_at).num_seconds() as u64;
                elapsed > cached.ttl_seconds
            })
            .count();

        (total, expired)
    }

    /// Force refresh of project context (bypass cache)
    pub async fn refresh_context<P: AsRef<Path>>(&self, path: P) -> Result<ProjectContextInfo> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        // Invalidate existing cache
        self.invalidate_cache(&path_str).await;

        // Get fresh context
        self.get_project_context(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_project_context_agent_creation() {
        let config = ContextConfig::default();
        let agent = ProjectContextAgent::new(config);

        let (total, expired) = agent.cache_stats().await;
        assert_eq!(total, 0);
        assert_eq!(expired, 0);
    }

    #[tokio::test]
    async fn test_language_detection() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create some test files
        let mut rust_file = File::create(temp_path.join("main.rs")).unwrap();
        writeln!(rust_file, "fn main() {{ println!(\"Hello, world!\"); }}").unwrap();

        let mut js_file = File::create(temp_path.join("app.js")).unwrap();
        writeln!(js_file, "console.log('Hello, world!');").unwrap();

        let agent = ProjectContextAgent::new(ContextConfig::default());
        let result = agent.detect_languages(temp_path.to_str().unwrap()).await.unwrap();

        assert!(result.languages.contains_key("Rust"));
        assert!(result.languages.contains_key("JavaScript"));
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_shell_detection() {
        let agent = ProjectContextAgent::new(ContextConfig::default());
        let shell_kind = agent.detect_shell_kind();

        // Should detect some shell type
        match shell_kind {
            ShellKind::Bash
            | ShellKind::Zsh
            | ShellKind::Fish
            | ShellKind::PowerShell
            | ShellKind::Cmd
            | ShellKind::Unknown(_) => {
                // All valid shells
            }
        }
    }

    #[test]
    fn test_language_from_file() {
        let agent = ProjectContextAgent::new(ContextConfig::default());

        assert_eq!(agent.detect_language_from_file(Path::new("test.rs")), Some("Rust".to_string()));
        assert_eq!(
            agent.detect_language_from_file(Path::new("test.js")),
            Some("JavaScript".to_string())
        );
        assert_eq!(
            agent.detect_language_from_file(Path::new("test.py")),
            Some("Python".to_string())
        );
        assert_eq!(
            agent.detect_language_from_file(Path::new("Dockerfile")),
            Some("Docker".to_string())
        );
    }

    #[test]
    fn test_ignore_patterns() {
        let agent = ProjectContextAgent::new(ContextConfig::default());

        assert!(agent.should_ignore_file(".git"));
        assert!(agent.should_ignore_file("node_modules"));
        assert!(agent.should_ignore_file("test.log"));
        assert!(agent.should_ignore_file("temp.tmp"));
        assert!(!agent.should_ignore_file("src"));
        assert!(!agent.should_ignore_file("main.rs"));
    }
}

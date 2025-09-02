// Git Context Plugin - Provides git repository information and context

use plugin_api::*;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub struct GitContextPlugin {
    config: Option<PluginConfig>,
    git_binary: String,
    cache: GitCache,
}

struct GitCache {
    current_branch: Option<String>,
    last_update: std::time::Instant,
    cache_duration: std::time::Duration,
}

impl Default for GitContextPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl GitContextPlugin {
    pub fn new() -> Self {
        Self {
            config: None,
            git_binary: "git".to_string(),
            cache: GitCache {
                current_branch: None,
                last_update: std::time::Instant::now(),
                cache_duration: std::time::Duration::from_secs(2),
            },
        }
    }

    fn is_git_repo(&self, path: &Path) -> bool {
        path.join(".git").exists()
            || self.run_git_command(&["rev-parse", "--git-dir"], path).is_ok()
    }

    fn run_git_command(&self, args: &[&str], cwd: &Path) -> Result<String, PluginError> {
        let output = Command::new(&self.git_binary)
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|e| PluginError::CommandError(format!("Failed to run git: {}", e)))?;

        if !output.status.success() {
            return Err(PluginError::CommandError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn get_branch(&mut self, path: &Path) -> Option<String> {
        if self.cache.last_update.elapsed() < self.cache.cache_duration {
            if let Some(ref branch) = self.cache.current_branch {
                return Some(branch.clone());
            }
        }

        let branch = self
            .run_git_command(&["branch", "--show-current"], path)
            .ok()
            .filter(|b| !b.is_empty())
            .or_else(|| {
                // Fallback for detached HEAD
                self.run_git_command(&["describe", "--tags", "--always"], path).ok()
            });

        self.cache.current_branch = branch.clone();
        self.cache.last_update = std::time::Instant::now();

        branch
    }

    fn get_status_summary(&self, path: &Path) -> HashMap<String, usize> {
        let mut summary = HashMap::new();

        if let Ok(status) = self.run_git_command(&["status", "--porcelain=v1"], path) {
            let mut staged = 0;
            let mut modified = 0;
            let mut untracked = 0;
            let mut conflicts = 0;

            for line in status.lines() {
                if line.len() < 2 {
                    continue;
                }

                let status_chars = &line[..2];
                match status_chars {
                    "??" => untracked += 1,
                    "UU" | "AA" | "DD" => conflicts += 1,
                    _ => {
                        if status_chars.chars().nth(0) != Some(' ') {
                            staged += 1;
                        }
                        if status_chars.chars().nth(1) != Some(' ') {
                            modified += 1;
                        }
                    },
                }
            }

            summary.insert("staged".to_string(), staged);
            summary.insert("modified".to_string(), modified);
            summary.insert("untracked".to_string(), untracked);
            summary.insert("conflicts".to_string(), conflicts);
        }

        summary
    }

    fn get_recent_commits(&self, path: &Path, count: usize) -> Vec<HashMap<String, String>> {
        let mut commits = Vec::new();

        let format = "--pretty=format:%H|%h|%an|%ae|%at|%s";
        let count_arg = format!("-{}", count);
        let args = vec!["log", format, &count_arg];

        if let Ok(output) = self.run_git_command(&args, path) {
            for line in output.lines() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 6 {
                    let mut commit = HashMap::new();
                    commit.insert("hash".to_string(), parts[0].to_string());
                    commit.insert("short_hash".to_string(), parts[1].to_string());
                    commit.insert("author".to_string(), parts[2].to_string());
                    commit.insert("email".to_string(), parts[3].to_string());
                    commit.insert("timestamp".to_string(), parts[4].to_string());
                    commit.insert("message".to_string(), parts[5].to_string());
                    commits.push(commit);
                }
            }
        }

        commits
    }

    fn get_remotes(&self, path: &Path) -> Vec<HashMap<String, String>> {
        let mut remotes = Vec::new();

        if let Ok(output) = self.run_git_command(&["remote", "-v"], path) {
            for line in output.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let mut remote = HashMap::new();
                    remote.insert("name".to_string(), parts[0].to_string());
                    remote.insert("url".to_string(), parts[1].to_string());
                    if parts.len() >= 3 {
                        remote.insert(
                            "type".to_string(),
                            parts[2].trim_matches('(').trim_matches(')').to_string(),
                        );
                    }
                    remotes.push(remote);
                }
            }
        }

        remotes
    }
}

impl Plugin for GitContextPlugin {
    fn metadata(&self) -> PluginMetadata {
        plugin_metadata! {
            name: "git-context",
            version: "1.0.0",
            author: "OpenAgent Team",
            description: "Provides git repository context and information",
            capabilities: {
                completions: true,
                context_provider: true,
                commands: vec!["git-status".to_string(), "git-info".to_string()],
                hooks: vec![HookType::DirectoryChange, HookType::PrePrompt]
            },
            permissions: {
                read_files: vec![".git/**".to_string(), ".gitignore".to_string()],
                execute_commands: true,
                environment_variables: vec!["GIT_*".to_string()]
            }
        }
    }

    fn init(&mut self, config: PluginConfig) -> Result<(), PluginError> {
        self.config = Some(config.clone());

        // Check if custom git binary is specified
        if let Some(git_path) = config.settings.get("git_binary") {
            if let Some(path) = git_path.as_str() {
                self.git_binary = path.to_string();
            }
        }

        // Verify git is available
        Command::new(&self.git_binary)
            .arg("--version")
            .output()
            .map_err(|e| PluginError::InitError(format!("Git not found: {}", e)))?;

        Ok(())
    }

    fn provide_completions(&self, context: CompletionContext) -> Vec<Completion> {
        let mut completions = Vec::new();

        // Parse the input to see if we're completing git commands
        let parts: Vec<&str> = context.input.split_whitespace().collect();

        if parts.is_empty() {
            return completions;
        }

        if parts[0] == "git" {
            let path = Path::new(&context.current_dir);

            if parts.len() == 1 {
                // Complete git subcommands
                let subcommands = vec![
                    ("add", "Add file contents to the index"),
                    ("commit", "Record changes to the repository"),
                    ("push", "Update remote refs"),
                    ("pull", "Fetch and integrate changes"),
                    ("status", "Show working tree status"),
                    ("log", "Show commit logs"),
                    ("diff", "Show changes"),
                    ("branch", "List, create, or delete branches"),
                    ("checkout", "Switch branches or restore files"),
                    ("merge", "Join development histories"),
                    ("rebase", "Reapply commits on top of another base"),
                    ("stash", "Stash changes in a dirty working directory"),
                ];

                for (cmd, desc) in subcommands {
                    completions.push(Completion {
                        value: format!("git {}", cmd),
                        display: cmd.to_string(),
                        description: Some(desc.to_string()),
                        kind: CompletionKind::Command,
                        score: 1.0,
                        icon: Some("🔀".to_string()),
                    });
                }
            } else if parts.len() == 2 && parts[1] == "checkout" {
                // Complete branch names for checkout
                if let Ok(branches) = self.run_git_command(&["branch", "-a"], path) {
                    for branch in branches.lines() {
                        let branch = branch.trim().trim_start_matches("* ");
                        if !branch.is_empty() {
                            completions.push(Completion {
                                value: format!("git checkout {}", branch),
                                display: branch.to_string(),
                                description: Some("Branch".to_string()),
                                kind: CompletionKind::Argument,
                                score: 0.9,
                                icon: Some("🌿".to_string()),
                            });
                        }
                    }
                }
            }
        }

        completions
    }

    fn collect_context(&self, request: ContextRequest) -> Option<Context> {
        let config = self.config.as_ref()?;
        let path = Path::new(&config.terminal_info.current_dir);

        if !self.is_git_repo(path) {
            return None;
        }

        let mut context_data = HashMap::new();

        // Collect git information based on request
        if request.purpose.contains("status") || request.purpose.contains("all") {
            // Current branch
            if let Ok(branch) = self.run_git_command(&["branch", "--show-current"], path) {
                context_data.insert("branch", json!(branch));
            }

            // Status summary
            let status = self.get_status_summary(path);
            context_data.insert("status", json!(status));

            // Remote info
            let remotes = self.get_remotes(path);
            context_data.insert("remotes", json!(remotes));
        }

        if request.purpose.contains("history") || request.purpose.contains("all") {
            // Recent commits
            let commits = self.get_recent_commits(path, 10);
            context_data.insert("recent_commits", json!(commits));
        }

        if request.purpose.contains("diff") && !request.include_sensitive {
            // Get diff stats (not the actual diff to avoid sensitive data)
            if let Ok(diff_stat) = self.run_git_command(&["diff", "--stat"], path) {
                context_data.insert("diff_stats", json!(diff_stat));
            }
        }

        let content = serde_json::to_string_pretty(&context_data).ok()?;
        let size = content.len();

        // Check size limit
        if size > request.max_size_bytes {
            return None;
        }

        Some(Context {
            name: "Git Repository Context".to_string(),
            content,
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("plugin".to_string(), "git-context".to_string());
                meta.insert("repo_path".to_string(), path.display().to_string());
                meta
            },
            sensitivity: if request.include_sensitive {
                SensitivityLevel::Internal
            } else {
                SensitivityLevel::Public
            },
            size_bytes: size,
        })
    }

    fn execute_command(&self, cmd: &str, _args: &[String]) -> Result<CommandOutput, PluginError> {
        let start = std::time::Instant::now();
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| PluginError::InitError("Plugin not initialized".to_string()))?;

        let path = Path::new(&config.terminal_info.current_dir);

        match cmd {
            "git-status" => {
                if !self.is_git_repo(path) {
                    return Ok(CommandOutput {
                        stdout: "Not in a git repository".to_string(),
                        stderr: String::new(),
                        exit_code: 1,
                        execution_time_ms: start.elapsed().as_millis() as u64,
                    });
                }

                let mut output = String::new();

                // Branch info
                if let Ok(branch) = self.run_git_command(&["branch", "--show-current"], path) {
                    output.push_str(&format!("📍 Branch: {}\n", branch));
                }

                // Status
                let status = self.get_status_summary(path);
                output.push_str("📊 Status:\n");
                output.push_str(&format!("  ✅ Staged: {}\n", status.get("staged").unwrap_or(&0)));
                output.push_str(&format!(
                    "  ✏️  Modified: {}\n",
                    status.get("modified").unwrap_or(&0)
                ));
                output.push_str(&format!(
                    "  ❓ Untracked: {}\n",
                    status.get("untracked").unwrap_or(&0)
                ));
                output.push_str(&format!(
                    "  ⚠️  Conflicts: {}\n",
                    status.get("conflicts").unwrap_or(&0)
                ));

                // Last commit
                if let Ok(last_commit) = self.run_git_command(&["log", "-1", "--oneline"], path) {
                    output.push_str(&format!("\n📝 Last commit: {}\n", last_commit));
                }

                Ok(CommandOutput {
                    stdout: output,
                    stderr: String::new(),
                    exit_code: 0,
                    execution_time_ms: start.elapsed().as_millis() as u64,
                })
            },

            "git-info" => {
                if !self.is_git_repo(path) {
                    return Ok(CommandOutput {
                        stdout: "Not in a git repository".to_string(),
                        stderr: String::new(),
                        exit_code: 1,
                        execution_time_ms: start.elapsed().as_millis() as u64,
                    });
                }

                let mut output = String::new();
                output.push_str("🔍 Git Repository Information\n");
                output.push_str("=".repeat(40).as_str());
                output.push_str("\n\n");

                // Repository root
                if let Ok(root) = self.run_git_command(&["rev-parse", "--show-toplevel"], path) {
                    output.push_str(&format!("📁 Repository root: {}\n", root));
                }

                // Current branch
                if let Ok(branch) = self.run_git_command(&["branch", "--show-current"], path) {
                    output.push_str(&format!("🌿 Current branch: {}\n", branch));
                }

                // Remotes
                output.push_str("\n🌐 Remotes:\n");
                for remote in self.get_remotes(path) {
                    output.push_str(&format!(
                        "  - {} ({})\n",
                        remote.get("name").unwrap_or(&String::new()),
                        remote.get("url").unwrap_or(&String::new())
                    ));
                }

                // Recent commits
                output.push_str("\n📜 Recent commits:\n");
                for commit in self.get_recent_commits(path, 5) {
                    output.push_str(&format!(
                        "  {} - {} ({})\n",
                        commit.get("short_hash").unwrap_or(&String::new()),
                        commit.get("message").unwrap_or(&String::new()),
                        commit.get("author").unwrap_or(&String::new())
                    ));
                }

                Ok(CommandOutput {
                    stdout: output,
                    stderr: String::new(),
                    exit_code: 0,
                    execution_time_ms: start.elapsed().as_millis() as u64,
                })
            },

            _ => Err(PluginError::CommandError(format!("Unknown command: {}", cmd))),
        }
    }

    fn handle_hook(&mut self, hook: HookEvent) -> Result<HookResponse, PluginError> {
        match hook.hook_type {
            HookType::DirectoryChange => {
                // Clear cache when directory changes
                self.cache.current_branch = None;
                Ok(HookResponse {
                    modified_command: None,
                    prevent_execution: false,
                    messages: vec![],
                })
            },

            HookType::PrePrompt => {
                // Update git info for prompt
                if let Some(config) = &self.config {
                    let current_dir = config.terminal_info.current_dir.clone();
                    let path = Path::new(&current_dir);
                    if self.is_git_repo(path) {
                        let _ = self.get_branch(path);
                    }
                }
                Ok(HookResponse {
                    modified_command: None,
                    prevent_execution: false,
                    messages: vec![],
                })
            },

            _ => Ok(HookResponse {
                modified_command: None,
                prevent_execution: false,
                messages: vec![],
            }),
        }
    }

    fn cleanup(&mut self) -> Result<(), PluginError> {
        self.cache.current_branch = None;
        Ok(())
    }
}

// Register the plugin
register_plugin!(GitContextPlugin);

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tera::{Context, Tera};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
    pub remote: Option<String>,
    pub upstream: Option<String>,
    pub ahead: i32,
    pub behind: i32,
    pub last_commit: GitCommit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCommit {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub email: String,
    pub date: DateTime<Utc>,
    pub message: String,
    pub signed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConflict {
    pub file: PathBuf,
    pub conflict_type: ConflictType,
    pub our_changes: Vec<String>,
    pub their_changes: Vec<String>,
    pub base_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType {
    ContentConflict,
    DeleteModify,
    ModifyDelete,
    AddAdd,
    BinaryConflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepository {
    pub path: PathBuf,
    pub branches: Vec<GitBranch>,
    pub current_branch: Option<String>,
    pub conflicts: Vec<GitConflict>,
    pub status: GitStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub staged: Vec<String>,
    pub modified: Vec<String>,
    pub untracked: Vec<String>,
    pub deleted: Vec<String>,
    pub renamed: Vec<String>,
}

pub struct GitIntegration {
    repo_path: PathBuf,
    signing_key: Option<String>,
    templates: Tera,
}

impl GitIntegration {
    pub fn new(repo_path: PathBuf) -> Result<Self> {
        let mut templates = Tera::new("templates/git/*").unwrap_or_else(|_| Tera::new("").unwrap());

        // Add built-in templates for conflict resolution
        templates.add_raw_template(
            "conflict_resolution",
            r#"
╭─────────────────────────────────────────────────────────────────╮
│ Git Conflict Resolution: {{ file }}
├─────────────────────────────────────────────────────────────────┤
│ [1] Accept Ours    [2] Accept Theirs    [3] Manual Edit
│ [4] Show Diff      [5] Skip File        [q] Quit
╰─────────────────────────────────────────────────────────────────╯

Conflict Type: {{ conflict_type }}
Base Content: {{ base_content | length }} lines

Our Changes ({{ our_changes | length }} lines):
{% for line in our_changes -%}
  + {{ line }}
{% endfor %}

Their Changes ({{ their_changes | length }} lines):
{% for line in their_changes -%}
  - {{ line }}
{% endfor %}
            "#,
        )?;

        templates.add_raw_template(
            "branch_visualization",
            r#"
╭─ Git Branch Visualization ─────────────────────────────────────────╮
│                                                                    │
{% for branch in branches -%}
│ {% if branch.current %}●{% else %}○{% endif %} {{ branch.name | truncate(length=20) }}{% if branch.current %} (current){% endif %}
│   └─ {{ branch.last_commit.short_hash }} {{ branch.last_commit.message | truncate(length=40) }}
│      {{ branch.last_commit.author }} • {{ branch.last_commit.date | date(format="%Y-%m-%d %H:%M") }}
│      {% if branch.ahead > 0 %}↑{{ branch.ahead }}{% endif %}{% if branch.behind > 0 %}↓{{ branch.behind }}{% endif %}{% if branch.last_commit.signed %}🔒{% endif %}
│
{% endfor -%}
╰────────────────────────────────────────────────────────────────────╯
            "#,
        )?;

        Ok(Self { repo_path, signing_key: Self::detect_signing_key()?, templates })
    }

    fn detect_signing_key() -> Result<Option<String>> {
        let output = Command::new("git")
            .args(["config", "--get", "user.signingkey"])
            .output()
            .map_err(|e| anyhow!("Failed to get git signing key: {}", e))?;

        if output.status.success() {
            let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !key.is_empty() {
                return Ok(Some(key));
            }
        }

        Ok(None)
    }

    pub async fn get_repository_info(&self) -> Result<GitRepository> {
        let branches = self.get_branches().await?;
        let current_branch = self.get_current_branch().await?;
        let conflicts = self.get_conflicts().await?;
        let status = self.get_status().await?;

        Ok(GitRepository {
            path: self.repo_path.clone(),
            branches,
            current_branch,
            conflicts,
            status,
        })
    }

    async fn get_branches(&self) -> Result<Vec<GitBranch>> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args([
                "for-each-ref",
                "--format=%(refname:short)|%(upstream:short)|%(HEAD)|%(committerdate:iso8601-strict)|%\
                 (objectname)|%(objectname:short)|%(authorname)|%(authoremail)|%(subject)",
                "refs/heads/",
            ])
            .output()
            .map_err(|e| anyhow!("Failed to get branches: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Git command failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        let mut branches = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(branch) = self.parse_branch_line(line).await? {
                branches.push(branch);
            }
        }

        // Get ahead/behind info
        for branch in &mut branches {
            if let Some(upstream) = &branch.upstream {
                let (ahead, behind) = self.get_ahead_behind(&branch.name, upstream).await?;
                branch.ahead = ahead;
                branch.behind = behind;
            }
        }

        Ok(branches)
    }

    async fn parse_branch_line(&self, line: &str) -> Result<Option<GitBranch>> {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 9 {
            return Ok(None);
        }

        let name = parts[0].to_string();
        let upstream = if parts[1].is_empty() { None } else { Some(parts[1].to_string()) };
        let current = parts[2] == "*";
        let date_str = parts[3];
        let hash = parts[4].to_string();
        let short_hash = parts[5].to_string();
        let author = parts[6].to_string();
        let email = parts[7].to_string();
        let message = parts[8].to_string();

        let date = chrono::DateTime::parse_from_rfc3339(date_str)
            .map_err(|e| anyhow!("Failed to parse date: {}", e))?
            .with_timezone(&Utc);

        let signed = self.is_commit_signed(&hash).await?;

        let last_commit = GitCommit { hash, short_hash, author, email, date, message, signed };

        Ok(Some(GitBranch {
            name,
            current,
            remote: None, // Will be populated separately if needed
            upstream,
            ahead: 0,
            behind: 0,
            last_commit,
        }))
    }

    async fn get_ahead_behind(&self, branch: &str, upstream: &str) -> Result<(i32, i32)> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-list", "--left-right", "--count", &format!("{}...{}", branch, upstream)])
            .output()
            .map_err(|e| anyhow!("Failed to get ahead/behind: {}", e))?;

        if !output.status.success() {
            return Ok((0, 0));
        }

        let result = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = result.trim().split('\t').collect();

        if parts.len() == 2 {
            let ahead = parts[0].parse().unwrap_or(0);
            let behind = parts[1].parse().unwrap_or(0);
            Ok((ahead, behind))
        } else {
            Ok((0, 0))
        }
    }

    async fn is_commit_signed(&self, hash: &str) -> Result<bool> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["verify-commit", hash])
            .output()
            .map_err(|e| anyhow!("Failed to verify commit signature: {}", e))?;

        Ok(output.status.success())
    }

    async fn get_current_branch(&self) -> Result<Option<String>> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .map_err(|e| anyhow!("Failed to get current branch: {}", e))?;

        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if branch != "HEAD" {
                return Ok(Some(branch));
            }
        }

        Ok(None)
    }

    async fn get_conflicts(&self) -> Result<Vec<GitConflict>> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["diff", "--name-only", "--diff-filter=U"])
            .output()
            .map_err(|e| anyhow!("Failed to get conflicts: {}", e))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let mut conflicts = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if !line.trim().is_empty() {
                let file = self.repo_path.join(line.trim());
                if let Ok(conflict) = self.parse_conflict_file(&file).await {
                    conflicts.push(conflict);
                }
            }
        }

        Ok(conflicts)
    }

    async fn parse_conflict_file(&self, file: &Path) -> Result<GitConflict> {
        let content = fs::read_to_string(file).await?;

        let conflict_marker_re = Regex::new(r"<<<<<<< (.+?)\n(.*?)\n======= ?(.*?)\n>>>>>>> (.+)")?;

        let mut our_changes = Vec::new();
        let mut their_changes = Vec::new();

        for cap in conflict_marker_re.captures_iter(&content) {
            our_changes.extend(cap[2].lines().map(|s| s.to_string()));
            their_changes.extend(cap[3].lines().map(|s| s.to_string()));
        }

        Ok(GitConflict {
            file: file.to_path_buf(),
            conflict_type: ConflictType::ContentConflict,
            our_changes,
            their_changes,
            base_content: None,
        })
    }

    async fn get_status(&self) -> Result<GitStatus> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["status", "--porcelain=v1"])
            .output()
            .map_err(|e| anyhow!("Failed to get git status: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Git status failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        let mut staged = Vec::new();
        let mut modified = Vec::new();
        let mut untracked = Vec::new();
        let mut deleted = Vec::new();
        let mut renamed = Vec::new();

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.len() < 3 {
                continue;
            }

            let status_char = &line[0..2];
            let filename = &line[3..];

            match status_char {
                "A " | "AM" | "AD" => staged.push(filename.to_string()),
                " M" | "MM" => modified.push(filename.to_string()),
                "??" => untracked.push(filename.to_string()),
                " D" | "MD" => deleted.push(filename.to_string()),
                "R " | "RM" => renamed.push(filename.to_string()),
                _ => {},
            }
        }

        Ok(GitStatus { staged, modified, untracked, deleted, renamed })
    }

    pub async fn resolve_conflict(
        &self,
        file: &Path,
        resolution: ConflictResolution,
    ) -> Result<()> {
        match resolution {
            ConflictResolution::AcceptOurs => {
                Command::new("git")
                    .current_dir(&self.repo_path)
                    .args(["checkout", "--ours", file.to_str().unwrap()])
                    .output()?;
            },
            ConflictResolution::AcceptTheirs => {
                Command::new("git")
                    .current_dir(&self.repo_path)
                    .args(["checkout", "--theirs", file.to_str().unwrap()])
                    .output()?;
            },
            ConflictResolution::Manual(content) => {
                fs::write(file, content).await?;
            },
        }

        Command::new("git")
            .current_dir(&self.repo_path)
            .args(["add", file.to_str().unwrap()])
            .output()?;

        Ok(())
    }

    pub async fn create_signed_commit(&self, message: &str, files: &[String]) -> Result<String> {
        // Add files
        for file in files {
            Command::new("git").current_dir(&self.repo_path).args(["add", file]).output()?;
        }

        // Create signed commit
        let mut args = vec!["commit", "-m", message];
        if self.signing_key.is_some() {
            args.push("-S");
        }

        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(&args)
            .output()
            .map_err(|e| anyhow!("Failed to create commit: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Commit failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        // Get the commit hash
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "HEAD"])
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn render_branch_visualization(&self, branches: &[GitBranch]) -> Result<String> {
        let mut context = Context::new();
        context.insert("branches", branches);

        self.templates
            .render("branch_visualization", &context)
            .map_err(|e| anyhow!("Failed to render branch visualization: {}", e))
    }

    pub fn render_conflict_resolution(&self, conflict: &GitConflict) -> Result<String> {
        let mut context = Context::new();
        context.insert("file", &conflict.file.to_string_lossy());
        context.insert("conflict_type", &format!("{:?}", conflict.conflict_type));
        context.insert("our_changes", &conflict.our_changes);
        context.insert("their_changes", &conflict.their_changes);
        context.insert("base_content", &conflict.base_content.as_deref().unwrap_or(""));

        self.templates
            .render("conflict_resolution", &context)
            .map_err(|e| anyhow!("Failed to render conflict resolution: {}", e))
    }

    pub async fn get_commit_graph(&self, max_commits: usize) -> Result<Vec<GitCommit>> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args([
                "log",
                &format!("--max-count={}", max_commits),
                "--pretty=format:%H|%h|%an|%ae|%ci|%s|%G?",
            ])
            .output()
            .map_err(|e| anyhow!("Failed to get commit graph: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("Git log failed: {}", String::from_utf8_lossy(&output.stderr)));
        }

        let mut commits = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(commit) = self.parse_commit_line(line)? {
                commits.push(commit);
            }
        }

        Ok(commits)
    }

    fn parse_commit_line(&self, line: &str) -> Result<Option<GitCommit>> {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 7 {
            return Ok(None);
        }

        let hash = parts[0].to_string();
        let short_hash = parts[1].to_string();
        let author = parts[2].to_string();
        let email = parts[3].to_string();
        let date_str = parts[4];
        let message = parts[5].to_string();
        let signature_status = parts[6];

        let date = chrono::DateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S %z")
            .map_err(|e| anyhow!("Failed to parse commit date: {}", e))?
            .with_timezone(&Utc);

        let signed = signature_status == "G" || signature_status == "U";

        Ok(Some(GitCommit { hash, short_hash, author, email, date, message, signed }))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictResolution {
    AcceptOurs,
    AcceptTheirs,
    Manual(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_test_repo() -> Result<(TempDir, GitIntegration)> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git").current_dir(&repo_path).args(["init"]).output()?;

        Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.name", "Test User"])
            .output()?;

        Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.email", "test@example.com"])
            .output()?;

        let git_integration = GitIntegration::new(repo_path)?;
        Ok((temp_dir, git_integration))
    }

    #[tokio::test]
    async fn test_get_repository_info() -> Result<()> {
        let (_temp_dir, git_integration) = setup_test_repo().await?;

        // Create initial commit
        std::fs::write(git_integration.repo_path.join("test.txt"), "Hello, world!")?;
        Command::new("git")
            .current_dir(&git_integration.repo_path)
            .args(["add", "test.txt"])
            .output()?;
        Command::new("git")
            .current_dir(&git_integration.repo_path)
            .args(["commit", "-m", "Initial commit"])
            .output()?;

        let repo_info = git_integration.get_repository_info().await?;

        assert_eq!(repo_info.current_branch, Some("master".to_string()));
        assert_eq!(repo_info.branches.len(), 1);
        assert!(repo_info.conflicts.is_empty());

        Ok(())
    }
}

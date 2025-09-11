//! Context collection for AI requests (Phase 5 MVP)
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitivityLevel {
    Safe,
    Caution,
    Warning,
    Critical,
}

/// Collected context payload
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub items: Vec<(String, String)>,
    pub estimated_size: usize,
}

impl Context {
    pub fn push(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let k = key.into();
        let v = value.into();
        self.estimated_size += k.len() + v.len();
        self.items.push((k, v));
    }
}

/// Provider interface for terminal/host context.
pub trait ContextProvider: Send + Sync {
    fn name(&self) -> &str;
    fn collect(&self) -> anyhow::Result<Context>;
    fn sensitivity_level(&self) -> SensitivityLevel {
        SensitivityLevel::Safe
    }
}

/// Manager for composing multiple providers and enforcing size limits.
#[derive(Default)]
pub struct ContextManager {
    providers: Vec<ProviderEntry>,
    per_provider_timeout_ms: Option<u64>,
    overall_deadline_ms: Option<u64>,
}

#[derive(Clone)]
struct ProviderEntry {
    provider: std::sync::Arc<dyn ContextProvider>,
    timeout_ms: Option<u64>,
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            per_provider_timeout_ms: None,
            overall_deadline_ms: None,
        }
    }

    pub fn with_provider(mut self, provider: Box<dyn ContextProvider>) -> Self {
        self.providers.push(ProviderEntry { provider: std::sync::Arc::from(provider), timeout_ms: None });
        self
    }

    pub fn add_provider(&mut self, provider: Box<dyn ContextProvider>) {
        self.providers.push(ProviderEntry { provider: std::sync::Arc::from(provider), timeout_ms: None });
    }

    pub fn add_provider_with_timeout(&mut self, provider: Box<dyn ContextProvider>, timeout_ms: Option<u64>) {
        self.providers.push(ProviderEntry { provider: std::sync::Arc::from(provider), timeout_ms });
    }

    /// Configure default timeouts for provider collection. Per-provider overrides take precedence.
    pub fn set_timeouts(&mut self, per_provider_timeout_ms: Option<u64>, overall_deadline_ms: Option<u64>) {
        self.per_provider_timeout_ms = per_provider_timeout_ms;
        self.overall_deadline_ms = overall_deadline_ms;
    }

    /// Collect context from all providers, truncated to `max_size_kb` (best-effort, simple sum of lengths).
    pub fn collect_all(&self, max_size_kb: usize) -> Vec<(String, String)> {
        let budget = max_size_kb * 1024;
        // Sequential path when no timeouts configured.
        if self.per_provider_timeout_ms.is_none() && self.overall_deadline_ms.is_none() {
            let mut items: Vec<(String, String)> = Vec::new();
            let mut total: usize = 0;
        for entry in &self.providers {
            if let Ok(mut ctx) = entry.provider.collect() {
                    for (k, v) in ctx.items.drain(..) {
                        let add = k.len() + v.len();
                        if total + add > budget { return items; }
                        total += add;
                        items.push((k, v));
                    }
                }
            }
            return items;
        }

        // Concurrent path with soft deadlines.
        use std::sync::mpsc;
        use std::thread;
        use std::time::{Duration, Instant};

        let (tx, rx) = mpsc::channel::<Vec<(String, String)>>();
        for entry in &self.providers {
            let tx2 = tx.clone();
            let p = entry.provider.clone();
            thread::spawn(move || {
                let res = p.collect();
                let mut send_items: Vec<(String, String)> = Vec::new();
                if let Ok(mut ctx) = res { send_items.append(&mut ctx.items); }
                // Best effort; ignore send errors if receiver dropped early.
                let _ = tx2.send(send_items);
            });
        }
        drop(tx); // We will only receive from now on

        let start = Instant::now();
        let overall_deadline = self.overall_deadline_ms.map(|ms| start + Duration::from_millis(ms));
        // Build individual deadlines for each provider
        let mut deadlines: Vec<Option<Instant>> = self
            .providers
            .iter()
            .map(|e| match e.timeout_ms.or(self.per_provider_timeout_ms) {
                Some(ms) if ms > 0 => Some(start + Duration::from_millis(ms)),
                _ => None,
            })
            .collect();

        let mut items: Vec<(String, String)> = Vec::new();
        let mut total: usize = 0;
        let mut pending = self.providers.len();

        while pending > 0 {
            // Compute next wait deadline: min of non-expired per-provider deadlines and overall
            let now = Instant::now();
            let mut next_deadline: Option<Instant> = None;
            for dl in &deadlines {
                if let Some(d) = dl {
                    if *d <= now { continue; }
                    next_deadline = Some(match next_deadline {
                        Some(cur_min) => cur_min.min(*d),
                        None => *d,
                    });
                }
            }
            if let Some(ov) = overall_deadline {
                next_deadline = Some(match next_deadline { Some(nd) => nd.min(ov), None => ov });
            }

            let wait_duration = next_deadline.and_then(|nd| nd.checked_duration_since(now));
            if let Some(dur) = wait_duration {
                match rx.recv_timeout(dur) {
                    Ok(mut batch) => {
                        // Received one provider result
                        pending -= 1;
                        // Remove any one deadline (prefer an active one)
                        if let Some(pos) = deadlines.iter().position(|d| d.is_some()) {
                            deadlines.remove(pos);
                        } else if !deadlines.is_empty() {
                            deadlines.remove(0);
                        }
                        for (k, v) in batch.drain(..) {
                            let add = k.len() + v.len();
                            if total + add > budget { return items; }
                            total += add;
                            items.push((k, v));
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // The earliest deadline hit; expire one provider (earliest)
                        if let Some(idx) = earliest_deadline_index(&deadlines, now) {
                            deadlines.remove(idx);
                            pending -= 1;
                            continue;
                        } else {
                            // No per-provider deadlines; must be overall timeout
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => { break; }
                }
            } else {
                // No deadlines remaining -> non-blocking drain then break
                match rx.try_recv() {
                    Ok(mut batch) => {
                        pending -= 1;
                        if let Some(pos) = deadlines.iter().position(|d| d.is_some()) {
                            deadlines.remove(pos);
                        } else if !deadlines.is_empty() {
                            deadlines.remove(0);
                        }
                        for (k, v) in batch.drain(..) {
                            let add = k.len() + v.len();
                            if total + add > budget { return items; }
                            total += add;
                            items.push((k, v));
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
        }

        items
    }
}

fn earliest_deadline_index(deadlines: &[Option<Instant>], now: Instant) -> Option<usize> {
    let mut idx: Option<usize> = None;
    let mut min_deadline: Option<Instant> = None;
    for (i, dl) in deadlines.iter().enumerate() {
        if let Some(d) = dl {
            if *d <= now {
                // expired now, remove immediately
                return Some(i);
            }
            match min_deadline {
                None => { min_deadline = Some(*d); idx = Some(i); }
                Some(cur) if *d < cur => { min_deadline = Some(*d); idx = Some(i); }
                _ => {}
            }
        }
    }
    idx
}

/// Built-in minimal provider: basic environment and cwd, with conservative filtering.
pub struct BasicEnvProvider;

/// Git repository status provider (branch and summary status).
pub struct GitProvider {
    pub include_branch: bool,
    pub include_status: bool,
}

impl Default for GitProvider { fn default() -> Self { Self { include_branch: true, include_status: true } } }
impl GitProvider { pub fn new(include_branch: bool, include_status: bool) -> Self { Self { include_branch, include_status } } }

/// File tree provider that respects .gitignore and lists files relative to repo root or cwd
pub struct FileTreeProvider {
    pub max_entries: usize,
    pub root_strategy: FileTreeRootStrategy,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FileTreeRootStrategy { RepoRoot, Cwd }

impl Default for FileTreeProvider { fn default() -> Self { Self { max_entries: 500, root_strategy: FileTreeRootStrategy::RepoRoot } } }
impl FileTreeProvider { pub fn new(max_entries: usize, root_strategy: FileTreeRootStrategy) -> Self { Self { max_entries, root_strategy } } }

impl BasicEnvProvider {
    fn env_is_sensitive(key: &str) -> bool {
        let lower = key.to_ascii_lowercase();
        [
            "key", "token", "secret", "password", "apikey", "api_key", "auth", "credential",
        ]
        .iter()
        .any(|kw| lower.contains(kw))
    }
}

impl ContextProvider for BasicEnvProvider {
    fn name(&self) -> &str {
        "basic-env"
    }

    fn collect(&self) -> anyhow::Result<Context> {
        let mut ctx = Context::default();
        // Current working directory
        if let Ok(cwd) = std::env::current_dir() {
            ctx.push("cwd", cwd.to_string_lossy().to_string());
        }
        // Shell (best-effort)
        if let Ok(shell) = std::env::var("SHELL") {
            if !shell.is_empty() {
                ctx.push("shell", shell);
            }
        }
        // A few safe environment variables (filtered)
        for (k, v) in std::env::vars() {
            if Self::env_is_sensitive(&k) { continue; }
            if matches!(k.as_str(), "PATH" | "LANG" | "HOME" | "TERM") {
                // HOME will be redacted later by privacy if enabled
                ctx.push(format!("env.{}", k), v);
            }
        }
        Ok(ctx)
    }

    fn sensitivity_level(&self) -> SensitivityLevel {
        SensitivityLevel::Caution // contains HOME; privacy layer will redact if configured
    }
}

impl ContextProvider for GitProvider {
    fn name(&self) -> &str { "git" }
    fn collect(&self) -> anyhow::Result<Context> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let repo = match git2::Repository::discover(&cwd) {
            Ok(r) => r,
            Err(_) => return Ok(Context::default()), // No repo; return empty
        };
        let mut ctx = Context::default();
        if self.include_branch {
            if let Ok(head) = repo.head() {
                if let Some(name) = head.shorthand() {
                    ctx.push("git.branch", name.to_string());
                }
            }
        }
        if let Some(root) = repo.workdir() {
            ctx.push("git.root", root.to_string_lossy().to_string());
        }
        if self.include_status {
            // Summarize status
            if let Ok(statuses) = repo.statuses(None) {
                let mut modified = 0usize;
                let mut added = 0usize;
                let mut deleted = 0usize;
                let mut untracked = 0usize;
                for entry in statuses.iter() {
                    let s = entry.status();
                    if s.is_wt_new() { untracked += 1; }
                    if s.is_wt_modified() || s.is_index_modified() { modified += 1; }
                    if s.is_index_new() { added += 1; }
                    if s.is_wt_deleted() || s.is_index_deleted() { deleted += 1; }
                }
                ctx.push("git.modified", modified.to_string());
                ctx.push("git.added", added.to_string());
                ctx.push("git.deleted", deleted.to_string());
                ctx.push("git.untracked", untracked.to_string());
            }
        }
        Ok(ctx)
    }
    fn sensitivity_level(&self) -> SensitivityLevel { SensitivityLevel::Safe }
}

impl ContextProvider for FileTreeProvider {
    fn name(&self) -> &str { "file-tree" }
    fn collect(&self) -> anyhow::Result<Context> {
        use ignore::WalkBuilder;
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        // Determine root based on strategy
        let root = match self.root_strategy {
            FileTreeRootStrategy::RepoRoot => {
                git2::Repository::discover(&cwd)
                    .ok()
                    .and_then(|r| r.workdir().map(|p| p.to_path_buf()))
                    .unwrap_or_else(|| cwd.clone())
            }
            FileTreeRootStrategy::Cwd => cwd.clone(),
        };
        let mut ctx = Context::default();
        let mut walker = WalkBuilder::new(&root);
        walker.hidden(true).git_ignore(true).git_global(true).git_exclude(true);
        // Limit number of files scanned to avoid worst-case walks
        let mut count = 0usize;
        let max_entries = self.max_entries.min(5000); // Safety cap
        for result in walker.build() {
            if count >= max_entries { break; }
            let dent = match result { Ok(d) => d, Err(_) => continue };
            if !dent.file_type().map(|ft| ft.is_file()).unwrap_or(false) { continue; }
            let p = dent.path();
            // Create relative path from root
            let rel = pathdiff::diff_paths(p, &root).unwrap_or_else(|| p.to_path_buf());
            let rel_str = rel.to_string_lossy().to_string();
            ctx.push("file", rel_str);
            count += 1;
        }
        Ok(ctx)
    }
    fn sensitivity_level(&self) -> SensitivityLevel { SensitivityLevel::Safe }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_limits_size_budget() {
        struct SmallProv;
        impl ContextProvider for SmallProv {
            fn name(&self) -> &str { "small" }
            fn collect(&self) -> anyhow::Result<Context> {
                let mut c = Context::default();
                c.push("a", "1");
                c.push("b", "2");
                Ok(c)
            }
        }
        let mut mgr = ContextManager::new();
        mgr.add_provider(Box::new(SmallProv));
        // 0KB budget should yield no items
        let items = mgr.collect_all(0);
        assert!(items.is_empty(), "tiny budget should yield no items");

        // Larger budget should include both pairs
        let items = mgr.collect_all(4); // 4KB
        assert!(items.len() >= 2);
    }

    #[test]
    fn concurrent_collection_respects_timeouts() {
        struct SlowProv;
        impl ContextProvider for SlowProv {
            fn name(&self) -> &str { "slow" }
            fn collect(&self) -> anyhow::Result<Context> {
                std::thread::sleep(std::time::Duration::from_millis(100));
                let mut c = Context::default();
                c.push("x", "y");
                Ok(c)
            }
        }
        let mut mgr = ContextManager::new();
        mgr.add_provider(Box::new(SlowProv));
        mgr.set_timeouts(Some(10), Some(20)); // 10ms per provider, 20ms overall
        let items = mgr.collect_all(4);
        assert!(items.is_empty(), "slow provider should time out and yield no items");
    }
}

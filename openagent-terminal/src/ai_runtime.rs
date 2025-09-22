//! AI runtime: UI state and provider wiring (optional feature)

use crate::ai_context_provider::AiContextProvider;
use log::{debug, error, info};
use std::collections::VecDeque;

use crate::security::{SecurityLens, SecurityPolicy};
use openagent_terminal_ai::build_request_with_context;
use openagent_terminal_ai::context::{
    BasicEnvProvider, ContextManager, FileTreeProvider, FileTreeRootStrategy, GitProvider,
};
// Privacy sanitization is applied via build_request_with_context
use crate::config::ai::ProviderConfig as ProviderCfg;
use openagent_terminal_ai::providers::{
    AnthropicProvider, OllamaProvider, OpenAiProvider, OpenRouterProvider,
};
use openagent_terminal_ai::{create_provider, AiProposal, AiProvider, AiRequest};

/// Default UI history capacity for initial allocation (pruning is configurable)
const DEFAULT_UI_HISTORY_CAPACITY: usize = 128;

#[derive(Debug, Clone)]
pub struct AiUiState {
    pub active: bool,
    pub scratch: String,
    pub cursor_position: usize,
    pub proposals: Vec<AiProposal>,
    pub selected_proposal: usize,
    pub is_loading: bool,
    pub error_message: Option<String>,
    #[allow(dead_code)]
    pub history: VecDeque<String>,
    #[allow(dead_code)]
    pub history_index: Option<usize>,
    // Streaming state
    pub streaming_active: bool,
    pub streaming_text: String,
    /// Last time we requested a redraw due to a streaming chunk (for throttling)
    pub streaming_last_redraw: Option<std::time::Instant>,
    /// Inline suggestion text to render as ghost text at the terminal prompt (suffix suggestion)
    pub inline_suggestion: Option<String>,
    /// Current provider id (e.g., "openrouter", "openai", "anthropic", "ollama") for UI display
    pub current_provider: String,
    /// Current model identifier used by the provider (for compact model badge in UI)
    pub current_model: String,
    /// Compact project context line for UI
    pub project_context_line: Option<String>,
}

impl AiRuntime {
    fn ensure_project_ctx_agent(&mut self) {
        if self.project_ctx_agent.is_some() {
            return;
        }
        let cfg = openagent_terminal_ai::agents::project_context::ContextConfig::default();
        let agent = openagent_terminal_ai::agents::project_context::ProjectContextAgent::new(cfg);
        self.project_ctx_agent = Some(agent);
        // Ensure a runtime exists for async calls only if we're not already inside one
        if self.agent_rt.is_none() && tokio::runtime::Handle::try_current().is_err() {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime for project context");
            self.agent_rt = Some(rt);
        }
    }

    /// Fetch a compact project context summary (kv pairs) and set UI line
    fn fetch_project_context_kv(&mut self, working_dir: &str) -> Vec<(String, String)> {
        self.ensure_project_ctx_agent();
        let mut kv: Vec<(String, String)> = Vec::new();
        if let Some(agent) = self.project_ctx_agent.as_ref() {
            // Prefer dedicated runtime if we created one; otherwise avoid blocking inside an active runtime
            let ctx_res = if let Some(rt) = self.agent_rt.as_ref() {
                rt.block_on(agent.get_project_context(working_dir))
            } else if tokio::runtime::Handle::try_current().is_ok() {
                // We're already inside a runtime; skip heavy project context to avoid nested block_on
                return kv;
            } else {
                // Fallback: create a temporary multi-thread runtime for this call
                let rt = tokio::runtime::Runtime::new().expect("temp tokio runtime");
                rt.block_on(agent.get_project_context(working_dir))
            };
            if let Ok(info) = ctx_res {
                // Dependency aggregation from lock/manifests (best-effort)
                let mut dep_names: std::collections::BTreeSet<String> =
                    std::collections::BTreeSet::new();
                let wd_path = std::path::Path::new(working_dir);
                // Cargo.lock (TOML)
                let cargo_lock = wd_path.join("Cargo.lock");
                if cargo_lock.exists() {
                    if let Ok(s) = std::fs::read_to_string(&cargo_lock) {
                        if let Ok(val) = s.parse::<toml::Value>() {
                            if let Some(pkgs) = val.get("package").and_then(|p| p.as_array()) {
                                for p in pkgs {
                                    if let Some(n) = p.get("name").and_then(|n| n.as_str()) {
                                        dep_names.insert(n.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                // package-lock.json (JSON)
                let pkg_lock = wd_path.join("package-lock.json");
                if pkg_lock.exists() {
                    if let Ok(s) = std::fs::read_to_string(&pkg_lock) {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&s) {
                            fn walk_deps(
                                obj: &serde_json::Map<String, serde_json::Value>,
                                out: &mut std::collections::BTreeSet<String>,
                            ) {
                                if let Some(deps) =
                                    obj.get("dependencies").and_then(|d| d.as_object())
                                {
                                    for (name, v) in deps.iter() {
                                        out.insert(name.clone());
                                        if let Some(child) = v.as_object() {
                                            walk_deps(child, out);
                                        }
                                    }
                                }
                            }
                            if let Some(root) = val.as_object() {
                                walk_deps(root, &mut dep_names);
                            }
                        }
                    }
                }
                // requirements.txt
                let req = wd_path.join("requirements.txt");
                if req.exists() {
                    if let Ok(s) = std::fs::read_to_string(&req) {
                        for l in s.lines() {
                            let t = l.trim();
                            if t.is_empty() || t.starts_with('#') {
                                continue;
                            }
                            let name = t
                                .split(|c: char| {
                                    c == '=' || c == '>' || c == '<' || c.is_whitespace()
                                })
                                .next()
                                .unwrap_or("");
                            if !name.is_empty() {
                                dep_names.insert(name.to_lowercase());
                            }
                        }
                    }
                }
                // go.sum
                let go_sum = wd_path.join("go.sum");
                if go_sum.exists() {
                    if let Ok(s) = std::fs::read_to_string(&go_sum) {
                        for l in s.lines() {
                            let mut it = l.split_whitespace();
                            if let Some(name) = it.next() {
                                dep_names.insert(name.to_string());
                            }
                        }
                    }
                }
                let lang = info.language_info.primary_language.clone();
                let frameworks = if info.language_info.frameworks.is_empty() {
                    String::new()
                } else {
                    info.language_info.frameworks.join(",")
                };
                let pms = if info.language_info.package_managers.is_empty() {
                    String::new()
                } else {
                    info.language_info.package_managers.join(",")
                };
                let build =
                    info.build_system.as_ref().map(|b| format!("{:?}", b)).unwrap_or_default();
                let (clean, ahead, behind) = if let Some(repo) = info.repository_info.as_ref() {
                    (repo.status.is_clean, repo.status.ahead, repo.status.behind)
                } else {
                    (true, 0, 0)
                };
                // Important files
                let mut important_files: Vec<&str> = Vec::new();
                let wd_path = std::path::Path::new(working_dir);
                for cand in [
                    "Cargo.toml",
                    "Cargo.lock",
                    "package.json",
                    "yarn.lock",
                    "pnpm-lock.yaml",
                    "package-lock.json",
                    "pyproject.toml",
                    "requirements.txt",
                    "go.mod",
                    "go.sum",
                    "README.md",
                ] {
                    if wd_path.join(cand).exists() {
                        important_files.push(cand);
                    }
                }
                let important = if important_files.is_empty() {
                    String::new()
                } else {
                    important_files.join(",")
                };

                // Add key-values
                kv.push(("project.primary_language".into(), lang.clone()));
                if !frameworks.is_empty() {
                    kv.push(("project.frameworks".into(), frameworks.clone()));
                }
                if !pms.is_empty() {
                    kv.push(("project.package_managers".into(), pms.clone()));
                }
                if !build.is_empty() {
                    kv.push(("project.build_system".into(), build.clone()));
                }
                if !important.is_empty() {
                    kv.push(("project.important_files".into(), important.clone()));
                }
                // Dependency summary
                if !dep_names.is_empty() {
                    kv.push(("project.dep_count".into(), dep_names.len().to_string()));
                    let sample: Vec<String> = dep_names.iter().take(8).cloned().collect();
                    kv.push(("project.dep_sample".into(), sample.join(",")));
                }
                kv.push(("project.repo_clean".into(), clean.to_string()));
                kv.push(("project.ahead".into(), ahead.to_string()));
                kv.push(("project.behind".into(), behind.to_string()));

                // Compact UI line
                let mut parts: Vec<String> = Vec::new();
                if !lang.is_empty() {
                    parts.push(format!("lang={}", lang));
                }
                if !frameworks.is_empty() {
                    parts.push(format!("fw={}", frameworks));
                }
                if !pms.is_empty() {
                    parts.push(format!("pm={}", pms));
                }
                if !build.is_empty() {
                    parts.push(format!("build={}", build));
                }
                parts.push(format!(
                    "repo={} a:{} b:{}",
                    if clean { "clean" } else { "dirty" },
                    ahead,
                    behind
                ));
                self.ui.project_context_line = Some(parts.join("  ·  "));
                self.last_project_primary_language = Some(lang);
            }
        }
        kv
    }
}

impl AiRuntime {
    /// Apply history retention settings.
    pub fn set_history_retention(&mut self, retention: crate::config::ai::AiHistoryRetention) {
        self.history_retention = retention;
        self.prune_ui_history();
    }

    fn prune_ui_history(&mut self) {
        // Enforce max entries
        while self.ui.history.len() > self.history_retention.ui_max_entries {
            self.ui.history.pop_back();
        }
        // Enforce max bytes
        let mut total: usize = self.ui.history.iter().map(|s| s.len()).sum();
        while total > self.history_retention.ui_max_bytes {
            if let Some(s) = self.ui.history.pop_back() {
                total = total.saturating_sub(s.len());
            } else {
                break;
            }
        }
    }

    /// Internal: reference selected public methods so they are considered used in minimal builds
    fn _keep_public_api_reachable(&mut self) {
        let _ = AiRuntime::from_config
            as fn(Option<&str>, Option<&str>, Option<&str>, Option<&str>) -> Self;
        let _ = AiRuntime::propose as fn(&mut AiRuntime, Option<String>, Option<String>);
        let _ = AiRuntime::get_selected_commands as fn(&AiRuntime) -> Option<String>;
        let _ = AiRuntime::propose_with_context
            as fn(&mut AiRuntime, Option<openagent_terminal_core::tty::pty_manager::PtyAiContext>);
        let _ = AiRuntime::has_content as fn(&AiRuntime) -> bool;
        let _ = AiRuntime::set_provider_by_name
            as fn(&mut AiRuntime, &str, &crate::config::UiConfig) -> Result<(), String>;
    }

    /// Load previously persisted AI history (best-effort).
    fn load_history(&mut self) {
        let path = Self::history_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(entries) = serde_json::from_str::<Vec<String>>(&data) {
                for s in entries.into_iter().rev() {
                    // maintain most-recent-first order
                    self.ui.history.push_front(s);
                }
                // Prune according to configured retention
                self.prune_ui_history();
            }
        }
    }

    /// Persist AI history to disk (best-effort, synchronous, small file).
    fn save_history(&mut self) {
        // Ensure pruning before saving
        self.prune_ui_history();
        let path = Self::history_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let list: Vec<String> = self.ui.history.iter().cloned().collect();
        if let Ok(json) = serde_json::to_string_pretty(&list) {
            let _ = std::fs::write(&path, json);
        }
    }

    fn history_path() -> std::path::PathBuf {
        let base = dirs::data_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        base.join("openagent-terminal").join("ai").join("history.json")
    }
    /// Reconfigure this runtime to a new provider using secure config, preserving UI scratch/cursor/history.
    pub fn reconfigure_to(
        &mut self,
        provider_name: &str,
        config: &crate::config::ai::ProviderConfig,
    ) {
        self.ui.error_message = None;
        match self.provider_registry.create(provider_name, config) {
            Ok((p, model_opt)) => {
                if let Some(m) = model_opt {
                    self.ui.current_model = m;
                } else {
                    self.ui.current_model.clear();
                }
                self.provider = Arc::from(p);
                self.ui.current_provider = provider_name.to_string();
                // Invalidate agent manager so it can be rebuilt with the new provider
                self.agent_manager = None;
                // Reset transient result state
                self.ui.proposals.clear();
                self.ui.selected_proposal = 0;
                self.ui.is_loading = false;
                self.ui.streaming_active = false;
                self.ui.streaming_text.clear();
                self.ui.error_message = None;
            }
            Err(e) => {
                self.ui.error_message =
                    Some(format!("Failed to reconfigure provider to '{}': {}", provider_name, e));
            }
        };
    }

    /// Start background computation of an inline suggestion based on the current prompt prefix.
    /// The provider is invoked in a separate thread to avoid blocking the UI.
    pub fn start_inline_suggest(
        &mut self,
        prefix: String,
        event_proxy: EventLoopProxy<Event>,
        window_id: WindowId,
    ) {
        // Clear any previous suggestion immediately
        self.ui.inline_suggestion = None;

        // Build a lightweight prompt for inline completion
        // We bias providers towards command completion, not multi-line explanations.
        let prompt = format!(
            "You are completing a shell command for zsh on Linux.\n\
             Return only the full completed command on a single line; no commentary, no quotes.\n\
             Do not invent commands or flags. Prefer safe, non-destructive flags (e.g., -i).\n\
             Avoid placeholders; use concrete, valid examples.\n\
             Partial: {}\n\
             Completion:",
            prefix
        );

        // Derive shell from environment, default to zsh
        let shell_kind = std::env::var("SHELL")
            .ok()
            .and_then(|s| {
                let lower = s.to_ascii_lowercase();
                if lower.contains("zsh") {
                    Some("zsh".to_string())
                } else if lower.contains("bash") {
                    Some("bash".to_string())
                } else if lower.contains("fish") {
                    Some("fish".to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "zsh".to_string());

        // Gather context from NullContextProvider (lightweight, best-effort)
        let ctx_provider = crate::ai_context_provider::NullContextProvider;
        // Mark methods as used and collect context
        let (working_directory, shell_from_ctx) = {
            let ctx = ctx_provider.get_pty_context();
            crate::ai_context_provider::context_to_ai_params(&ctx)
        };
        let shell_kind = shell_from_ctx.unwrap_or_else(|| shell_kind.clone());
        // Update command context (no-op for Null provider)
        let mut ctx_provider_mut = crate::ai_context_provider::NullContextProvider;
        ctx_provider_mut.update_command_context(&prefix);
        let shell_exec = ctx_provider.get_shell_executable();
        let last_cmd = ctx_provider.get_last_command();

        let provider = self.provider.clone();
        let mut req = AiRequest {
            scratch_text: prompt,
            working_directory,
            shell_kind: Some(shell_kind.clone()),
            context: vec![
                ("mode".to_string(), "inline".to_string()),
                ("platform".to_string(), std::env::consts::OS.to_string()),
                ("shell".to_string(), shell_kind),
                (
                    "guidelines".to_string(),
                    "no-invent; prefer-safe; no-placeholders; single-line".to_string(),
                ),
            ],
        };
        if let Some(exec) = shell_exec {
            req.context.push(("shell_exec".into(), exec));
        }
        if let Some(cmd) = last_cmd {
            req.context.push(("last_cmd".into(), cmd));
        }

        let _ = std::thread::Builder::new().name("ai-inline".into()).spawn(move || {
            // Non-streaming, single-shot proposal
            let result = provider.propose(req);
            let suggestion = match result {
                Ok(mut props) => {
                    // Take the first command from the first proposal, if any
                    if let Some(prop) = props.first_mut() {
                        prop.proposed_commands.first().map(|cmd| compute_suffix(cmd, &prefix))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            };

            let payload =
                crate::event::EventType::AiInlineSuggestionReady(suggestion.unwrap_or_default());
            let _ = event_proxy.send_event(Event::new(payload, window_id));
        });

        // Helper to compute the suffix not yet typed
        fn compute_suffix(candidate: &str, typed: &str) -> String {
            if let Some(stripped) = candidate.strip_prefix(typed) {
                return stripped.to_string();
            }
            // Fallback: compute longest common prefix ignoring consecutive spaces
            let mut i = 0usize;
            let ca: Vec<char> = candidate.chars().collect();
            let ta: Vec<char> = typed.chars().collect();
            while i < ca.len() && i < ta.len() && ca[i] == ta[i] {
                i += 1;
            }
            ca[i..].iter().collect()
        }
    }
}

impl Default for AiUiState {
    fn default() -> Self {
        Self {
            active: false,
            scratch: String::new(),
            cursor_position: 0,
            proposals: Vec::new(),
            selected_proposal: 0,
            is_loading: false,
            error_message: None,
            history: VecDeque::with_capacity(DEFAULT_UI_HISTORY_CAPACITY),
            history_index: None,
            streaming_active: false,
            streaming_text: String::new(),
            streaming_last_redraw: None,
            inline_suggestion: None,
            current_provider: "null".to_string(),
            current_model: String::new(),
            project_context_line: None,
        }
    }
}

use crate::event::{Event, EventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use winit::event_loop::EventLoopProxy;
use winit::window::WindowId;

// Lightweight AI conversation persistence (JSONL) with simple log rotation.
// Best-effort; errors are ignored by callers.
fn persist_ai_conversation(
    mode: &str,
    working_directory: Option<&str>,
    shell_kind: Option<&str>,
    input: &str,
    output: &str,
) -> Result<(), String> {
    // Try SQLite first (best-effort). Fallback to JSONL on any error.
    let sqlite_ok =
        persist_ai_conversation_sqlite(mode, working_directory, shell_kind, input, output).is_ok();

    // Optionally disable JSONL fallback with OPENAGENT_AI_HISTORY_JSONL=0/false
    let jsonl_enabled = std::env::var("OPENAGENT_AI_HISTORY_JSONL")
        .ok()
        .map(|v| {
            let v = v.to_lowercase();
            !(v == "0" || v == "false" || v == "off")
        })
        .unwrap_or(true);

    if !sqlite_ok && !jsonl_enabled {
        // Best-effort: skip JSONL fallback if disabled
        return Ok(());
    }
    #[derive(serde::Serialize)]
    struct Entry<'a> {
        timestamp: String,
        mode: &'a str,
        working_directory: Option<&'a str>,
        shell_kind: Option<&'a str>,
        input: &'a str,
        output: &'a str,
    }

    let now = chrono::Utc::now().to_rfc3339();
    let entry = Entry { timestamp: now, mode, working_directory, shell_kind, input, output };

    let base = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("openagent-terminal")
        .join("ai_history");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let file = base.join("history.jsonl");

    // Simple rotation: if file exceeds threshold, rename with timestamp and keep at most N rotated files.
    const DEFAULT_MAX_BYTES: u64 = 2 * 1024 * 1024; // 2MB
    const DEFAULT_ROTATED_KEEP: usize = 5;
    const DEFAULT_MAX_AGE_DAYS: u64 = 30;
    let max_bytes = std::env::var("OPENAGENT_AI_HISTORY_MAX_BYTES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_MAX_BYTES);
    let rotated_keep = std::env::var("OPENAGENT_AI_HISTORY_ROTATED_KEEP")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_ROTATED_KEEP);
    let max_age_days = std::env::var("OPENAGENT_AI_HISTORY_JSONL_MAX_AGE_DAYS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_MAX_AGE_DAYS);

    if let Ok(meta) = std::fs::metadata(&file) {
        if meta.len() >= max_bytes {
            // Rotate: history-YYYYmmddHHMMSS.jsonl
            let ts = chrono::Local::now().format("%Y%m%d%H%M%S");
            let rotated = base.join(format!("history-{}.jsonl", ts));
            let _ = std::fs::rename(&file, rotated);
            // Prune old rotated files (keep most recent rotated_keep and older than max_age_days)
            if let Ok(entries) = std::fs::read_dir(&base) {
                let mut rotated: Vec<std::fs::DirEntry> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        if let Some(name) = e.file_name().to_str() {
                            name.starts_with("history-") && name.ends_with(".jsonl")
                        } else {
                            false
                        }
                    })
                    .collect();
                rotated.sort_by_key(|e| e.file_name());
                // Time-based prune
                let now = std::time::SystemTime::now();
                for e in &rotated {
                    if let Ok(meta) = e.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age.as_secs() > max_age_days * 24 * 3600 {
                                    let _ = std::fs::remove_file(e.path());
                                }
                            }
                        }
                    }
                }
                // Reload listing after time-based prune
                let mut rotated: Vec<std::fs::DirEntry> = std::fs::read_dir(&base)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        if let Some(name) = e.file_name().to_str() {
                            name.starts_with("history-") && name.ends_with(".jsonl")
                        } else {
                            false
                        }
                    })
                    .collect();
                rotated.sort_by_key(|e| e.file_name());
                // Keep last rotated_keep
                let to_prune = rotated.len().saturating_sub(rotated_keep);
                if to_prune > 0 {
                    for e in rotated.into_iter().take(to_prune) {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
        }
    }

    let line = serde_json::to_string(&entry).map_err(|e| e.to_string())? + "\n";
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .map_err(|e| e.to_string())?;
    f.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

fn persist_ai_conversation_sqlite(
    mode: &str,
    working_directory: Option<&str>,
    shell_kind: Option<&str>,
    input: &str,
    output: &str,
) -> Result<(), String> {
    use rusqlite::{params, Connection};

    let base = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("openagent-terminal")
        .join("ai_history");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let db_path = base.join("history.db");

    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS conversations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts TEXT NOT NULL,
            mode TEXT NOT NULL,
            working_directory TEXT,
            shell_kind TEXT,
            input TEXT NOT NULL,
            output TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    let ts = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO conversations (ts, mode, working_directory, shell_kind, input, output)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![ts, mode, working_directory.unwrap_or(""), shell_kind.unwrap_or(""), input, output],
    )
    .map_err(|e| e.to_string())?;

    // Retention policy: prune by age and max rows
    let max_age_days: u64 = std::env::var("OPENAGENT_AI_HISTORY_SQLITE_MAX_AGE_DAYS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(30);
    let cutoff = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(max_age_days as i64))
        .map(|d| d.to_rfc3339())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
    let _ = conn.execute("DELETE FROM conversations WHERE ts < ?1", params![cutoff]);

    // Cap total rows
    let max_rows: i64 = std::env::var("OPENAGENT_AI_HISTORY_SQLITE_MAX_ROWS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(20_000);
    // Delete oldest rows beyond limit
    let _ = conn.execute(
        &format!(
            "DELETE FROM conversations WHERE id IN (
                SELECT id FROM conversations ORDER BY ts ASC LIMIT (
                    SELECT MAX(COUNT(*) - {}, 0) FROM conversations
                )
            )",
            max_rows
        ),
        [],
    );

    Ok(())
}

pub struct AiRuntime {
    pub ui: AiUiState,
    pub provider: Arc<dyn AiProvider>,
    // Registered provider factories and helpers
    provider_registry: ProviderRegistry,
    cancel_flag: Arc<AtomicBool>,
    security_lens: SecurityLens,
    // Config-driven context collection policy
    context_cfg: crate::config::ai::AiContextConfig,
    // Optional agent manager for capability-based routing (lazy init)
    agent_manager: Option<openagent_terminal_ai::agents::manager::AgentManager>,
    // Persistent single-threaded runtime for agent manager async calls
    agent_rt: Option<tokio::runtime::Runtime>,
    // Project context agent (lazy init)
    project_ctx_agent: Option<openagent_terminal_ai::agents::project_context::ProjectContextAgent>,
    // Routing mode (overridable via OPENAGENT_AI_ROUTING)
    routing_mode: crate::config::ai::AiRoutingMode,
    // How to join multiple commands when applying
    apply_joiner: crate::config::ai::AiApplyJoinStrategy,
    // Last detected project primary language (for routing bias)
    last_project_primary_language: Option<String>,
    // History retention configuration
    history_retention: crate::config::ai::AiHistoryRetention,
}

/// Factory and registry for AI providers (secure, per-provider credentials)
#[derive(Default, Debug)]
pub struct ProviderRegistry;

impl ProviderRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn create(
        &self,
        provider_name: &str,
        config: &ProviderCfg,
    ) -> Result<(Box<dyn AiProvider>, Option<String>), String> {
        use crate::config::ai_providers::ProviderCredentials;
        let credentials = ProviderCredentials::from_config(provider_name, config)?;
        match provider_name {
            "openai" => {
                let api_key = credentials.require_api_key(provider_name)?.to_string();
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("https://api.openai.com/v1")
                    .to_string();
                let model = credentials.require_model(provider_name)?.to_string();
                let prov = OpenAiProvider::new(api_key, endpoint, model.clone())
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
                    .map_err(|e| e.to_string())?;
                Ok((prov, Some(model)))
            }
            "openrouter" => {
                let api_key = credentials.require_api_key(provider_name)?.to_string();
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("https://openrouter.ai/api/v1")
                    .to_string();
                let model = credentials.require_model(provider_name)?.to_string();
                let prov = OpenRouterProvider::new(api_key, endpoint, model.clone())
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
                    .map_err(|e| e.to_string())?;
                Ok((prov, Some(model)))
            }
            "anthropic" => {
                let api_key = credentials.require_api_key(provider_name)?.to_string();
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("https://api.anthropic.com")
                    .to_string();
                let model = credentials.require_model(provider_name)?.to_string();
                let prov = AnthropicProvider::new(api_key, endpoint, model.clone())
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
                    .map_err(|e| e.to_string())?;
                Ok((prov, Some(model)))
            }
            "ollama" => {
                let endpoint = credentials
                    .require_endpoint(provider_name)
                    .unwrap_or("http://localhost:11434")
                    .to_string();
                let model = credentials.require_model(provider_name)?.to_string();
                let prov = OllamaProvider::new(endpoint, model.clone())
                    .map(|p| Box::new(p) as Box<dyn AiProvider>)
                    .map_err(|e| e.to_string())?;
                Ok((prov, Some(model)))
            }
            "null" => Ok((Box::new(openagent_terminal_ai::NullProvider), None)),
            other => Err(format!("Unknown provider: {}", other)),
        }
    }
}

impl AiRuntime {
    fn ensure_agent_manager(&mut self) {
        if self.agent_manager.is_some() {
            return;
        }
        // Create persistent single-thread runtime if absent
        if self.agent_rt.is_none() {
            if tokio::runtime::Handle::try_current().is_err() {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime for agent registration");
                self.agent_rt = Some(rt);
            } else {
                // Inside an existing runtime; skip agent manager setup in this context
                return;
            }
        }
        let mgr = openagent_terminal_ai::agents::manager::AgentManager::new();
        let provider_arc = self.provider.clone();
        let cmd_agent =
            openagent_terminal_ai::agents::command::CommandAgent::new(provider_arc.clone());
        let rt = self.agent_rt.as_ref().expect("agent runtime initialized");
        let _ = rt.block_on(mgr.register_agent(Box::new(cmd_agent)));
        // Also register code generation agent for explain/refactor flows
        let code_agent =
            openagent_terminal_ai::agents::code_generation::CodeGenerationAgent::new(provider_arc);
        let _ = rt.block_on(mgr.register_agent(Box::new(code_agent)));
        self.agent_manager = Some(mgr);
    }

    /// Normalize agent-generated code into a list of runnable shell commands.
    /// - Strips Markdown code fences
    /// - Drops empty lines and standalone comments
    /// - Trims common shell prompt characters (leading `$ `)
    fn normalize_commands_from_generated(code: &str) -> Vec<String> {
        // Strip code fences if present
        let mut s = code.trim().to_string();
        if s.starts_with("```") {
            if let Some(idx) = s.find('\n') {
                s = s[idx + 1..].to_string();
            }
            if let Some(idx) = s.rfind("```") {
                s = s[..idx].to_string();
            }
        }
        // Normalize newlines
        s = s.replace("\r\n", "\n");
        let mut cmds = Vec::new();
        for line in s.lines() {
            let mut l = line.trim();
            if l.is_empty() {
                continue;
            }
            // Drop standalone comments
            if l.starts_with('#') {
                continue;
            }
            // Strip list bullets / numbering like "- ", "* ", "1. ", "(a) ", "1) "
            if l.starts_with("- ") || l.starts_with("* ") {
                l = l[2..].trim_start();
            } else if let Some(rest) = l.strip_prefix(|c: char| c.is_ascii_digit()) {
                // patterns like "1. command" or "1) command"
                let rest = rest.trim_start();
                if rest.starts_with('.') || rest.starts_with(')') {
                    l = rest[1..].trim_start();
                }
            }
            // Strip common prompt prefixes
            if let Some(stripped) = l.strip_prefix("$ ") {
                l = stripped.trim_start();
            } else if let Some(stripped) = l.strip_prefix("$") {
                l = stripped.trim_start();
            }
            if l.is_empty() {
                continue;
            }
            cmds.push(l.to_string());
        }
        cmds
    }

    /// Parse agent-generated fix output into multiple proposals when alternatives are present.
    /// Heuristics:
    /// - Split by fenced code blocks (```lang ... ```). Each shell block becomes a separate proposal.
    /// - Use the nearest preceding non-empty line as the option title when it contains keywords
    ///   like "Option", "Approach", "Alternative". Otherwise use the first command or a fallback.
    /// - If no fences exist, split by blank lines and treat each chunk as commands.
    fn parse_fix_proposals_from_agent_output(
        text: &str,
        default_title: &str,
        explanation: &str,
    ) -> Vec<openagent_terminal_ai::AiProposal> {
        #[derive(Clone, Default)]
        struct Block {
            lang: Option<String>,
            title_hint: Option<String>,
            content: String,
        }

        fn is_shell_lang(lang: &Option<String>) -> bool {
            match lang.as_deref().map(|s| s.to_ascii_lowercase()) {
                Some(ref s) if ["sh", "bash", "zsh", "shell", "console"].contains(&s.as_str()) => {
                    true
                }
                None => true, // unknown: often shell-like
                _ => false,
            }
        }

        // Extract fenced code blocks with simple scanner
        let mut blocks: Vec<Block> = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0usize;
        while i < lines.len() {
            let line = lines[i];
            if let Some(rest) = line.strip_prefix("```") {
                // Start of block
                let lang = rest.split_whitespace().next().map(|s| s.to_string());
                // Look for title hint above
                let mut title_hint = None;
                let mut j = i;
                while j > 0 {
                    j -= 1;
                    let t = lines[j].trim();
                    if t.is_empty() {
                        continue;
                    }
                    let tl = t.to_ascii_lowercase();
                    if tl.contains("option")
                        || tl.contains("approach")
                        || tl.contains("alternative")
                        || tl.contains(":")
                    {
                        title_hint = Some(t.to_string());
                    }
                    break;
                }
                // Collect until closing fence
                let mut content = String::new();
                i += 1;
                while i < lines.len() {
                    if lines[i].starts_with("```") {
                        break;
                    }
                    content.push_str(lines[i]);
                    content.push('\n');
                    i += 1;
                }
                // Skip closing fence if present
                if i < lines.len() && lines[i].starts_with("```") { /* will be incremented by loop */
                }
                blocks.push(Block { lang, title_hint, content });
            }
            i += 1;
        }

        let mut proposals: Vec<openagent_terminal_ai::AiProposal> = Vec::new();

        if !blocks.is_empty() {
            for (idx, b) in blocks.into_iter().enumerate() {
                if !is_shell_lang(&b.lang) {
                    continue;
                }
                let commands = Self::normalize_commands_from_generated(&b.content);
                if commands.is_empty() {
                    continue;
                }
                let title = b
                    .title_hint
                    .as_ref()
                    .and_then(|s| {
                        let t = s.trim();
                        if t.is_empty() {
                            None
                        } else {
                            Some(t.to_string())
                        }
                    })
                    .unwrap_or_else(|| {
                        commands
                            .first()
                            .cloned()
                            .unwrap_or_else(|| format!("Fix option {}", idx + 1))
                    });
                proposals.push(openagent_terminal_ai::AiProposal {
                    title,
                    description: if explanation.is_empty() {
                        None
                    } else {
                        Some(explanation.to_string())
                    },
                    proposed_commands: commands,
                });
            }
            if !proposals.is_empty() {
                return proposals;
            }
        }

        // No fenced blocks: split by blank-line separated chunks
        let mut current = String::new();
        let mut groups: Vec<String> = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                if !current.trim().is_empty() {
                    groups.push(current.clone());
                    current.clear();
                }
            } else {
                current.push_str(line);
                current.push('\n');
            }
        }
        if !current.trim().is_empty() {
            groups.push(current);
        }

        for (idx, g) in groups.into_iter().enumerate() {
            let commands = Self::normalize_commands_from_generated(&g);
            if commands.is_empty() {
                continue;
            }
            let title =
                commands.first().cloned().unwrap_or_else(|| format!("Fix option {}", idx + 1));
            proposals.push(openagent_terminal_ai::AiProposal {
                title,
                description: if explanation.is_empty() {
                    None
                } else {
                    Some(explanation.to_string())
                },
                proposed_commands: commands,
            });
        }

        if proposals.is_empty() {
            // Fallback: treat whole thing as one
            let commands = Self::normalize_commands_from_generated(text);
            if !commands.is_empty() {
                proposals.push(openagent_terminal_ai::AiProposal {
                    title: default_title.to_string(),
                    description: if explanation.is_empty() {
                        None
                    } else {
                        Some(explanation.to_string())
                    },
                    proposed_commands: commands,
                });
            }
        }
        proposals
    }

    pub fn new(provider: Box<dyn AiProvider>) -> Self {
        info!("AI runtime initialized with provider: {}", provider.name());
        let ui = AiUiState {
            current_provider: provider.name().to_string(),
            current_model: String::new(),
            ..AiUiState::default()
        };
        let mut rt = Self {
            ui,
            provider: Arc::from(provider),
            provider_registry: ProviderRegistry::new(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            security_lens: SecurityLens::new(SecurityPolicy::default()),
            context_cfg: crate::config::ai::AiContextConfig::default(),
            agent_manager: None,
            agent_rt: None,
            project_ctx_agent: None,
            routing_mode: crate::config::ai::AiRoutingMode::Auto,
            apply_joiner: crate::config::ai::AiApplyJoinStrategy::AndThen,
            last_project_primary_language: None,
            history_retention: crate::config::ai::AiHistoryRetention::default(),
        };
        // Load persisted history best-effort
        rt.load_history();
        // Keep selected public API reachable in minimal builds to avoid dead_code warnings
        rt._keep_public_api_reachable();
        rt
    }

    pub fn from_config(
        provider_id: Option<&str>,
        _endpoint_env: Option<&str>,
        _api_key_env: Option<&str>,
        _model_env: Option<&str>,
    ) -> Self {
        use tracing::warn;

        // Check for legacy environment variable usage
        crate::config::ai_providers::check_legacy_env_vars();

        // DEPRECATED: This method is deprecated in favor of from_secure_config
        // Maintain backward compatibility but warn users
        warn!(
            "AI runtime from_config is deprecated. Please use from_secure_config with \
             provider-specific configuration."
        );

        // For backward compatibility, attempt to create provider using legacy approach
        let provider_name = provider_id.unwrap_or("null");
        let provider_result = create_provider(provider_name);
        match provider_result {
            Ok(p) => {
                info!("Successfully created AI provider: {}", provider_name);
                Self::new(p)
            }
            Err(e) => {
                error!("Failed to create provider '{}': {}", provider_name, e);
                let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                rt.ui.error_message = Some(format!(
                    "AI provider initialization failed: {}. Please check your AI settings \
                     (provider, endpoint, api key, model). Consider migrating to secure provider \
                     configuration - see docs/AI_ENVIRONMENT_SECURITY.md",
                    e
                ));
                rt
            }
        }
    }

    /// Create AI runtime from secure provider configuration (recommended approach)
    pub fn from_secure_config(
        provider_name: &str,
        config: &crate::config::ai::ProviderConfig,
    ) -> Self {
        info!("Initializing AI runtime with secure provider configuration: {}", provider_name);

        let reg = ProviderRegistry::new();
        match reg.create(provider_name, config) {
            Ok((provider, model_opt)) => {
                info!("Successfully created secure AI provider: {}", provider_name);
                let mut rt = Self::new(provider);
                rt.provider_registry = reg;
                rt.ui.current_provider = provider_name.to_string();
                if let Some(m) = model_opt {
                    rt.ui.current_model = m;
                } else {
                    rt.ui.current_model = config.default_model.clone().unwrap_or_default();
                }
                rt
            }
            Err(e) => {
                error!("Failed to create secure provider '{}': {}", provider_name, e);
                let mut rt = Self::new(Box::new(openagent_terminal_ai::NullProvider));
                rt.provider_registry = reg;
                rt.ui.error_message = Some(format!(
                    "Secure AI provider initialization failed: {}. Please verify your \
                     configuration and credentials.",
                    e
                ));
                rt
            }
        }
    }

    /// Begin a streaming proposal in a background thread. Falls back to blocking propose if the
    /// provider doesn't support streaming.
    pub fn start_propose_stream(
        &mut self,
        working_directory: Option<String>,
        shell_kind: Option<String>,
        event_proxy: EventLoopProxy<Event>,
        window_id: WindowId,
    ) {
        let _span = tracing::info_span!(
            "ai.start_propose_stream",
            provider = %self.provider.name(),
            scratch_len = self.ui.scratch.len()
        )
        .entered();
        info!(
            "ai_runtime_stream_start provider={} scratch_len={}",
            self.provider.name(),
            self.ui.scratch.len()
        );
        if self.ui.scratch.trim().is_empty() {
            self.ui.error_message = Some("Query cannot be empty".to_string());
            return;
        }

        // Build minimal raw request; helper will reset UI state and spawn worker
        let req_raw = AiRequest {
            scratch_text: self.ui.scratch.clone(),
            working_directory,
            shell_kind,
            context: vec![("platform".to_string(), std::env::consts::OS.to_string())],
        };
        self.start_streaming_with_request(req_raw, event_proxy, window_id);
    }

    /// Internal helper to start streaming given a raw request; applies context providers and spawns worker.
    fn start_streaming_with_request(
        &mut self,
        req_raw: AiRequest,
        event_proxy: EventLoopProxy<Event>,
        window_id: WindowId,
    ) {
        // Reset state
        self.ui.proposals.clear();
        self.ui.selected_proposal = 0;
        self.ui.error_message = None;
        self.ui.is_loading = true;
        self.ui.streaming_active = true;
        self.ui.streaming_text.clear();
        self.ui.streaming_last_redraw = None;
        self.cancel_flag.store(false, Ordering::Relaxed);

        let cancel = self.cancel_flag.clone();
        let provider = self.provider.clone();

        // Build rich context with config-driven providers and sanitize
        let (cm, budget_kb) = self.build_context_manager();
        // Append compact project context summary (cached 5–10 min)
        let mut req = req_raw;
        if let Some(wd) = req
            .working_directory
            .clone()
            .or_else(|| std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()))
        {
            let kv = self.fetch_project_context_kv(&wd);
            req.context.extend(kv);
        }
        let req = build_request_with_context(req, &cm, budget_kb);

        // Spawn background worker
        let _ = thread::Builder::new().name("ai-stream".into()).spawn(move || {
            // First try streaming
            let mut batch_buf = String::new();
            let mut last_flush = std::time::Instant::now();
            let batch_ms = std::env::var("OPENAGENT_AI_STREAM_REDRAW_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(16);

            let mut on_chunk = |chunk: &str| {
                // Micro-batch: accumulate small chunks and flush at most every batch_ms
                batch_buf.push_str(chunk);
                let now = std::time::Instant::now();
                if now.saturating_duration_since(last_flush).as_millis() as u64 >= batch_ms
                    && !batch_buf.is_empty()
                {
                    let payload = std::mem::take(&mut batch_buf);
                    let _ = event_proxy
                        .send_event(Event::new(EventType::AiStreamChunk(payload), window_id));
                    last_flush = now;
                }
            };
            match provider.propose_stream(req.clone(), &mut on_chunk, &cancel) {
                Ok(true) => {
                    // Flush any pending chunk before finishing
                    if !batch_buf.is_empty() {
                        let payload = std::mem::take(&mut batch_buf);
                        let _ = event_proxy
                            .send_event(Event::new(EventType::AiStreamChunk(payload), window_id));
                    }
                    info!("ai_runtime_stream_finished provider={}", provider.name());
                    let _ =
                        event_proxy.send_event(Event::new(EventType::AiStreamFinished, window_id));
                }
                Ok(false) => {
                    info!("ai_runtime_fallback_blocking provider={}", provider.name());
                    let result = provider.propose(req);
                    match result {
                        Ok(mut proposals) => {
                            info!("ai_runtime_blocking_complete proposals={}", proposals.len());
                            // Enforce --no-pager for git lines before sending to UI
                            for p in &mut proposals {
                                for cmd in &mut p.proposed_commands {
                                    *cmd = enforce_git_no_pager_line(cmd);
                                }
                            }
                            let _ = event_proxy.send_event(Event::new(
                                EventType::AiProposals(proposals),
                                window_id,
                            ));
                        }
                        Err(e) => {
                            error!("ai_runtime_blocking_error error={}", e);
                            let _ = event_proxy
                                .send_event(Event::new(EventType::AiStreamError(e), window_id));
                        }
                    }
                }
                Err(e) => {
                    if e.eq_ignore_ascii_case("cancelled") || e.eq_ignore_ascii_case("canceled") {
                        info!("ai_runtime_stream_cancelled provider={}", provider.name());
                        // Flush any pending buffered chunk before finishing gracefully
                        if !batch_buf.is_empty() {
                            let payload = std::mem::take(&mut batch_buf);
                            let _ = event_proxy.send_event(Event::new(
                                EventType::AiStreamChunk(payload),
                                window_id,
                            ));
                        }
                        // Treat cancellation as a graceful finish, do not surface an error
                        let _ = event_proxy
                            .send_event(Event::new(EventType::AiStreamFinished, window_id));
                    } else {
                        error!("ai_runtime_stream_error error={}", e);
                        let _ = event_proxy
                            .send_event(Event::new(EventType::AiStreamError(e), window_id));
                    }
                }
            }
        });
    }

    /// Cancel any in-flight streaming.
    pub fn cancel(&mut self) {
        info!("ai_runtime_cancel_requested provider={}", self.provider.name());
        self.cancel_flag.store(true, Ordering::SeqCst);
        self.ui.streaming_active = false;
        self.ui.is_loading = false;
    }

    pub fn propose(&mut self, working_directory: Option<String>, shell_kind: Option<String>) {
        let _span = tracing::info_span!(
            "ai.propose_blocking",
            provider = %self.provider.name(),
            scratch_len = self.ui.scratch.len()
        )
        .entered();
        if self.ui.scratch.trim().is_empty() {
            self.ui.error_message = Some("Query cannot be empty".to_string());
            return;
        }

        // Add to history
        self.ui.history.push_front(self.ui.scratch.clone());
        // Prune and persist updated history
        self.save_history();
        self.ui.history_index = None;

        // Clear previous state
        self.ui.proposals.clear();
        self.ui.selected_proposal = 0;
        self.ui.error_message = None;
        self.ui.is_loading = true;

        debug!("Submitting AI query: {}", self.ui.scratch);

        let base_ctx: Vec<(String, String)> = vec![
            ("platform".to_string(), std::env::consts::OS.to_string()),
            (
                "guidelines".to_string(),
                "no-invent; prefer-safe; no-placeholders; add --no-pager for git when relevant"
                    .to_string(),
            ),
        ];
        let mut req_raw = AiRequest {
            scratch_text: self.ui.scratch.clone(),
            working_directory,
            shell_kind,
            context: base_ctx,
        };
        // Enrich with cached project context KV
        if let Some(wd) = req_raw
            .working_directory
            .clone()
            .or_else(|| std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()))
        {
            let kv = self.fetch_project_context_kv(&wd);
            req_raw.context.extend(kv);
        }
        let (cm, budget_kb) = self.build_context_manager();
        let req = build_request_with_context(req_raw, &cm, budget_kb);

        let t0 = std::time::Instant::now();
        match self.provider.propose(req) {
            Ok(mut proposals) => {
                let dt = t0.elapsed();
                info!("Received {} proposals", proposals.len());
                tracing::info!(elapsed_ms = dt.as_millis() as u64, "ai.propose_complete");
                self.enforce_git_no_pager_on_proposals(proposals.as_mut_slice());
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            }
            Err(e) => {
                let dt = t0.elapsed();
                error!("AI query failed: {}", e);
                tracing::info!(elapsed_ms = dt.as_millis() as u64, "ai.propose_error");
                self.ui.error_message = Some(format!("Query failed: {}", e));
                self.ui.is_loading = false;
            }
        }
    }

    /// Toggle AI panel visibility
    pub fn toggle_panel(&mut self) {
        self.ui.active = !self.ui.active;
        if self.ui.active {
            debug!("AI panel opened");
            self.ui.cursor_position = self.ui.scratch.len();
        } else {
            debug!("AI panel closed");
        }
    }

    /// Insert text at cursor position
    pub fn insert_text(&mut self, text: &str) {
        self.ui.scratch.insert_str(self.ui.cursor_position, text);
        self.ui.cursor_position += text.len();
    }

    /// Handle backspace
    pub fn backspace(&mut self) {
        if self.ui.cursor_position > 0 {
            self.ui.cursor_position -= 1;
            self.ui.scratch.remove(self.ui.cursor_position);
        }
    }

    /// Move cursor left
    pub fn cursor_left(&mut self) {
        if self.ui.cursor_position > 0 {
            self.ui.cursor_position -= 1;
        }
    }

    /// Move cursor right
    pub fn cursor_right(&mut self) {
        if self.ui.cursor_position < self.ui.scratch.len() {
            self.ui.cursor_position += 1;
        }
    }

    /// Forward delete at cursor (DEL key)
    pub fn delete_forward(&mut self) {
        if self.ui.cursor_position < self.ui.scratch.len() {
            self.ui.scratch.remove(self.ui.cursor_position);
        }
    }

    /// Navigate history
    pub fn history_previous(&mut self) {
        if self.ui.history.is_empty() {
            return;
        }

        let new_index = match self.ui.history_index {
            None => 0,
            Some(i) if i < self.ui.history.len() - 1 => i + 1,
            Some(i) => i,
        };

        if let Some(entry) = self.ui.history.get(new_index) {
            self.ui.scratch = entry.clone();
            self.ui.cursor_position = self.ui.scratch.len();
            self.ui.history_index = Some(new_index);
        }
    }

    pub fn history_next(&mut self) {
        match self.ui.history_index {
            Some(0) => {
                self.ui.history_index = None;
                self.ui.scratch.clear();
                self.ui.cursor_position = 0;
            }
            Some(i) => {
                let new_index = i - 1;
                if let Some(entry) = self.ui.history.get(new_index) {
                    self.ui.scratch = entry.clone();
                    self.ui.cursor_position = self.ui.scratch.len();
                    self.ui.history_index = Some(new_index);
                }
            }
            None => {}
        }
    }

    /// Select next proposal
    pub fn next_proposal(&mut self) {
        if !self.ui.proposals.is_empty() {
            self.ui.selected_proposal = (self.ui.selected_proposal + 1) % self.ui.proposals.len();
        }
    }

    /// Select previous proposal
    pub fn previous_proposal(&mut self) {
        if !self.ui.proposals.is_empty() {
            if self.ui.selected_proposal == 0 {
                self.ui.selected_proposal = self.ui.proposals.len() - 1;
            } else {
                self.ui.selected_proposal -= 1;
            }
        }
    }

    /// Get selected proposal commands
    pub fn get_selected_commands(&self) -> Option<String> {
        self.ui.proposals.get(self.ui.selected_proposal).map(|p| p.proposed_commands.join("\n"))
    }

    /// Regenerate the last proposal
    pub fn regenerate(&mut self, event_proxy: EventLoopProxy<Event>, window_id: WindowId) {
        // Clear current proposals and streaming state
        self.ui.proposals.clear();
        self.ui.selected_proposal = 0;
        self.ui.error_message = None;
        self.ui.streaming_text.clear();

        // Restart the proposal stream with the same scratch text
        // Note: Context should be provided by the caller in real usage
        // This is a standalone method that doesn't have access to context provider
        let working_directory =
            std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string());
        let shell_kind = std::env::var("SHELL").ok().map(|s| {
            openagent_terminal_core::tty::pty_manager::ShellKind::from_shell_name(&s)
                .to_str()
                .to_string()
        });
        self.start_propose_stream(working_directory, shell_kind, event_proxy, window_id);
    }

    /// Insert selected proposal text to the prompt
    pub fn insert_to_prompt(&mut self) -> Option<String> {
        if self.ui.streaming_active && !self.ui.streaming_text.is_empty() {
            // Use streaming text if available
            Some(self.ui.streaming_text.clone())
        } else {
            // Use selected proposal
            self.ui.proposals.get(self.ui.selected_proposal).map(|p| {
                let mut result = String::new();
                if let Some(desc) = &p.description {
                    result.push_str(desc);
                    result.push_str("\n\n");
                }
                result.push_str(&p.proposed_commands.join("\n"));
                result
            })
        }
    }

    /// Apply command with safe-run (dry-run by default)
    pub fn apply_command(&mut self, dry_run: bool) -> Option<(String, bool)> {
        self.ui.proposals.get(self.ui.selected_proposal).map(|p| {
            let cmds = &p.proposed_commands;
            if dry_run {
                // Build an annotated dry run for one or multiple commands
                let mut annotated = String::new();
                if cmds.len() > 1 {
                    annotated.push_str("# Multiple commands suggested:\n");
                    for (i, c) in cmds.iter().enumerate() {
                        let risk = self.security_lens.analyze_command(c);
                        annotated.push_str(&format!(
                            "# [{}] {:?}: {}\n",
                            i + 1,
                            risk.level,
                            risk.explanation
                        ));
                        annotated.push_str(&format!("#     {}\n", c));
                    }
                    let joiner = match self.apply_joiner {
                        crate::config::ai::AiApplyJoinStrategy::AndThen => " && ",
                        crate::config::ai::AiApplyJoinStrategy::Lines => "\n",
                    };
                    let combined = cmds.join(joiner);
                    annotated.push_str("#\n# Use Copy to paste, or copy individual commands.\n");
                    annotated.push_str(&format!(
                        "echo 'DRY RUN: batch ({} commands)'\\n# To execute all: {}",
                        cmds.len(),
                        combined
                    ));
                    (annotated, true)
                } else {
                    let cmd = cmds.first().cloned().unwrap_or_default();
                    let risk = self.security_lens.analyze_command(&cmd);
                    annotated.push_str(&format!(
                        "# Security Lens: {:?} - {}\n",
                        risk.level, risk.explanation
                    ));
                    #[cfg(feature = "security-lens")]
                    if !risk.mitigations.is_empty() {
                        annotated.push_str("# Suggested mitigations:\n");
                        for m in &risk.mitigations {
                            annotated.push_str(&format!("#  - {}\n", m));
                        }
                    }
                    annotated.push_str(&format!("echo 'DRY RUN: {}'\n# To execute: {}", cmd, cmd));
                    (annotated, true)
                }
            } else {
                // Combine multiple commands with && so that failure halts subsequent steps
                if cmds.len() > 1 {
                    let joiner = match self.apply_joiner {
                        crate::config::ai::AiApplyJoinStrategy::AndThen => " && ",
                        crate::config::ai::AiApplyJoinStrategy::Lines => "\n",
                    };
                    (cmds.join(joiner), false)
                } else {
                    (cmds.first().cloned().unwrap_or_default(), false)
                }
            }
        })
    }

    /// Copy output in the specified format
    pub fn copy_output(&self, format: crate::event::AiCopyFormat) -> Option<String> {
        use crate::event::AiCopyFormat;

        let content = if self.ui.streaming_active && !self.ui.streaming_text.is_empty() {
            self.ui.streaming_text.clone()
        } else if let Some(proposal) = self.ui.proposals.get(self.ui.selected_proposal) {
            let mut result = String::new();
            if let Some(desc) = &proposal.description {
                result.push_str(desc);
                result.push_str("\n\n");
            }
            result.push_str(&proposal.proposed_commands.join("\n"));
            result
        } else {
            return None;
        };

        Some(match format {
            AiCopyFormat::Text => content,
            AiCopyFormat::Code => {
                // Format as code block
                format!("```bash\n{}\n```", content)
            }
            AiCopyFormat::Markdown => {
                // Format as markdown with title and description
                let mut md = String::new();
                if let Some(proposal) = self.ui.proposals.get(self.ui.selected_proposal) {
                    md.push_str(&format!("## {}\n\n", proposal.title));
                    if let Some(desc) = &proposal.description {
                        md.push_str(desc);
                        md.push_str("\n\n");
                    }
                    if !proposal.proposed_commands.is_empty() {
                        md.push_str("### Commands\n\n");
                        md.push_str("```bash\n");
                        md.push_str(&proposal.proposed_commands.join("\n"));
                        md.push_str("\n```\n");
                    }
                } else {
                    // Fallback for streaming text
                    md.push_str("```\n");
                    md.push_str(&content);
                    md.push_str("\n```\n");
                }
                md
            }
        })
    }

    /// Context-aware propose method
    pub fn propose_with_context(
        &mut self,
        context: Option<openagent_terminal_core::tty::pty_manager::PtyAiContext>,
    ) {
        let _span = tracing::info_span!(
            "ai.propose_with_context",
            provider = %self.provider.name(),
            scratch_len = self.ui.scratch.len()
        )
        .entered();
        if self.ui.scratch.trim().is_empty() {
            self.ui.error_message = Some("Query cannot be empty".to_string());
            return;
        }

        // Add to history
        self.ui.history.push_front(self.ui.scratch.clone());
        self.prune_ui_history();
        self.ui.history_index = None;

        // Clear previous state
        self.ui.proposals.clear();
        self.ui.selected_proposal = 0;
        self.ui.error_message = None;
        self.ui.is_loading = true;

        debug!("Submitting AI query with context: {}", self.ui.scratch);

        let (working_directory, shell_kind) = if let Some(ctx) = context.as_ref() {
            let (wd, sk) = ctx.to_strings();
            (Some(wd), Some(sk))
        } else {
            (None, None)
        };

        // Build request including rich context (env/git/file_tree) and sanitize via privacy options
        let mut base_ctx: Vec<(String, String)> =
            vec![("platform".to_string(), std::env::consts::OS.to_string())];
        // If provided, augment with last command and shell executable for extra context
        if let Some(ref ctx) = context {
            if let Some(last) = ctx.last_command.clone() {
                base_ctx.push(("last_command".into(), last));
            }
            base_ctx.push(("shell_executable".into(), ctx.shell_executable.clone()));
        }
        let mut req_raw = AiRequest {
            scratch_text: self.ui.scratch.clone(),
            working_directory,
            shell_kind,
            context: base_ctx,
        };
        // Project context enrichment
        if let Some(wd) = req_raw
            .working_directory
            .clone()
            .or_else(|| std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()))
        {
            let kv = self.fetch_project_context_kv(&wd);
            req_raw.context.extend(kv);
        }
        let (cm, budget_kb) = self.build_context_manager();
        let req = build_request_with_context(req_raw, &cm, budget_kb);

        // Decide routing mode
        let mode = self.effective_routing_mode();
        let try_agent = !matches!(mode, crate::config::ai::AiRoutingMode::Provider)
            && (self.agent_rt.is_some() || tokio::runtime::Handle::try_current().is_err());

        // Try AgentManager first for capability-based routing if enabled
        if try_agent && self.agent_manager.is_none() {
            self.ensure_agent_manager();
        }
        if try_agent {
            if let Some(mgr) = &self.agent_manager {
                let t0 = std::time::Instant::now();
                let agent_req = openagent_terminal_ai::agents::AgentRequest::Command(req.clone());
                let rt = self.agent_rt.as_ref().expect("agent runtime initialized");
                match rt.block_on(mgr.process_request(agent_req)) {
                    Ok(openagent_terminal_ai::agents::AgentResponse::Commands(mut proposals)) => {
                        let dt = t0.elapsed();
                        info!("AgentManager returned {} proposals", proposals.len());
                        tracing::info!(
                            elapsed_ms = dt.as_millis() as u64,
                            "ai.propose_with_context_complete_agent"
                        );
                        self.enforce_git_no_pager_on_proposals(&mut proposals);
                        self.ui.proposals = proposals;
                        self.ui.is_loading = false;
                        return;
                    }
                    Ok(other) => {
                        let dt = t0.elapsed();
                        error!("Agent response not commands: {:?}", other);
                        tracing::info!(
                            elapsed_ms = dt.as_millis() as u64,
                            "ai.propose_with_context_wrong_variant"
                        );
                        // Fall through to direct provider
                    }
                    Err(e) => {
                        let dt = t0.elapsed();
                        error!("AgentManager error: {}", e);
                        tracing::info!(
                            elapsed_ms = dt.as_millis() as u64,
                            "ai.propose_with_context_agent_error"
                        );
                        // Fall through to direct provider
                    }
                }
            }
        }

        // Direct provider (fallback or Provider mode)
        let t0 = std::time::Instant::now();
        match self.provider.propose(req) {
            Ok(mut proposals) => {
                let dt = t0.elapsed();
                info!("Received {} proposals with context", proposals.len());
                tracing::info!(
                    elapsed_ms = dt.as_millis() as u64,
                    "ai.propose_with_context_complete"
                );
                self.enforce_git_no_pager_on_proposals(&mut proposals);
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            }
            Err(e) => {
                let dt = t0.elapsed();
                error!("AI query with context failed: {}", e);
                tracing::info!(elapsed_ms = dt.as_millis() as u64, "ai.propose_with_context_error");
                self.ui.error_message = Some(format!("Query failed: {}", e));
                self.ui.is_loading = false;
            }
        }
    }

    /// Context-aware streaming propose method
    pub fn start_propose_stream_with_context(
        &mut self,
        context: Option<openagent_terminal_core::tty::pty_manager::PtyAiContext>,
        event_proxy: EventLoopProxy<Event>,
        window_id: WindowId,
    ) {
        // Enrich request context with PTY details when available
        let (working_directory, shell_kind, extra_ctx): (
            Option<String>,
            Option<String>,
            Vec<(String, String)>,
        ) = if let Some(ctx) = context {
            let (wd, sk) = ctx.to_strings();
            let mut ext = vec![("platform".to_string(), std::env::consts::OS.to_string())];
            if let Some(last) = ctx.last_command {
                ext.push(("last_command".into(), last));
            }
            ext.push(("shell_executable".into(), ctx.shell_executable));
            (Some(wd), Some(sk), ext)
        } else {
            (None, None, vec![("platform".to_string(), std::env::consts::OS.to_string())])
        };

        let mut req_raw = AiRequest {
            scratch_text: self.ui.scratch.clone(),
            working_directory,
            shell_kind,
            context: extra_ctx,
        };
        if let Some(wd) = req_raw
            .working_directory
            .clone()
            .or_else(|| std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()))
        {
            let kv = self.fetch_project_context_kv(&wd);
            req_raw.context.extend(kv);
        }
        self.start_streaming_with_request(req_raw, event_proxy, window_id);
    }

    /// Check if we can perform actions (have content to act on)
    pub fn has_content(&self) -> bool {
        (!self.ui.streaming_text.is_empty() && self.ui.streaming_active)
            || !self.ui.proposals.is_empty()
    }

    /// Generate an explanation for a given command or output snippet.
    /// The explanation is produced by the current AI provider with context flags.
    pub fn propose_explain(
        &mut self,
        target_text: String,
        working_directory: Option<String>,
        shell_kind: Option<String>,
    ) {
        let _span = tracing::info_span!(
            "ai.propose_explain",
            provider = %self.provider.name(),
            target_len = target_text.len()
        )
        .entered();
        if target_text.trim().is_empty() {
            self.ui.error_message = Some("Nothing to explain".to_string());
            return;
        }

        self.ui.proposals.clear();
        self.ui.selected_proposal = 0;
        self.ui.error_message = None;
        self.ui.is_loading = true;

        let mut context = vec![
            ("mode".to_string(), "explain".to_string()),
            ("explain_target".to_string(), target_text.clone()),
            ("platform".to_string(), std::env::consts::OS.to_string()),
            (
                "guidelines".to_string(),
                "no-invent; prefer-safe; no-placeholders; concise".to_string(),
            ),
        ];
        if let Some(ref sh) = shell_kind {
            context.push(("shell".into(), sh.clone()));
        }
        if let Some(ref dir) = working_directory {
            context.push(("cwd".into(), dir.clone()));
        }

        // Decide routing mode
        let mode = self.effective_routing_mode();
        let try_agent = !matches!(mode, crate::config::ai::AiRoutingMode::Provider)
            && (self.agent_rt.is_some() || tokio::runtime::Handle::try_current().is_err());

        // First attempt: route through AgentManager CodeGenerationAgent with action=Explain
        if try_agent && self.agent_manager.is_none() {
            self.ensure_agent_manager();
        }
        if try_agent {
            if let Some(mgr) = &self.agent_manager {
                let rt = self.agent_rt.as_ref().expect("agent runtime initialized");
                let code_ctx = openagent_terminal_ai::agents::CodeContext {
                    current_file: None,
                    selection: Some(target_text.clone()),
                    cursor_position: None,
                    project_files: Vec::new(),
                    dependencies: Vec::new(),
                };
                let lang_bias =
                    self.last_project_primary_language.clone().map(|s| s.to_lowercase());
                let agent_req = openagent_terminal_ai::agents::AgentRequest::CodeGeneration {
                    language: lang_bias,
                    context: code_ctx,
                    prompt: String::new(),
                    action: openagent_terminal_ai::agents::CodeAction::Explain,
                };
                let t0 = std::time::Instant::now();
                match rt.block_on(mgr.process_request(agent_req)) {
                    Ok(openagent_terminal_ai::agents::AgentResponse::Code {
                        generated_code,
                        language: _,
                        explanation,
                        suggestions: _,
                    }) => {
                        let dt = t0.elapsed();
                        tracing::info!(
                            elapsed_ms = dt.as_millis() as u64,
                            "ai.propose_explain_complete_agent"
                        );
                        // Map code response into a single proposal with explanation in description
                        let desc = if explanation.is_empty() {
                            Some("Explanation".to_string())
                        } else {
                            Some(explanation)
                        };
                        let mut commands = Vec::new();
                        if !generated_code.trim().is_empty() {
                            commands.push(generated_code);
                        }
                        let proposal = openagent_terminal_ai::AiProposal {
                            title: "Explanation".to_string(),
                            description: desc,
                            proposed_commands: commands,
                        };
                        self.ui.proposals = vec![proposal];
                        self.ui.is_loading = false;
                        return;
                    }
                    Ok(other) => {
                        tracing::warn!(
                            "AgentManager returned unexpected variant for explain: {:?}",
                            other
                        );
                    }
                    Err(e) => {
                        tracing::warn!("AgentManager explain attempt failed: {}", e);
                    }
                }
            }
        }

        // Fallback: direct provider path (original behavior)
        let mut req_raw = AiRequest {
            // Keep the scratch as the current query if present, else use the target_text
            scratch_text: if self.ui.scratch.trim().is_empty() {
                format!("Explain: {}", target_text)
            } else {
                self.ui.scratch.clone()
            },
            // Clone so we can still reference these later for persistence
            working_directory: working_directory.clone(),
            shell_kind: shell_kind.clone(),
            context,
        };
        if let Some(wd) = req_raw
            .working_directory
            .clone()
            .or_else(|| std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()))
        {
            let kv = self.fetch_project_context_kv(&wd);
            req_raw.context.extend(kv);
        }
        // Enrich with configured context providers and sanitize
        let (cm, budget_kb) = self.build_context_manager();
        let req = build_request_with_context(req_raw, &cm, budget_kb);

        let t0 = std::time::Instant::now();
        match self.provider.propose(req) {
            Ok(mut proposals) => {
                let dt = t0.elapsed();
                tracing::info!(elapsed_ms = dt.as_millis() as u64, "ai.propose_explain_complete");
                // Persist conversation (input + proposals)
                let cmds: Vec<String> =
                    proposals.iter().flat_map(|p| p.proposed_commands.clone()).collect();
                let _ = persist_ai_conversation(
                    "explain",
                    working_directory.as_deref(),
                    shell_kind.as_deref(),
                    &target_text,
                    &cmds.join("\n"),
                );
                self.enforce_git_no_pager_on_proposals(proposals.as_mut_slice());
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            }
            Err(e) => {
                let dt = t0.elapsed();
                tracing::info!(elapsed_ms = dt.as_millis() as u64, "ai.propose_explain_error");
                self.ui.error_message = Some(format!("Explain failed: {}", e));
                self.ui.is_loading = false;
            }
        }
    }

    /// Suggest a fix for an error snippet, optionally with the failed command.
    pub fn propose_fix(
        &mut self,
        error_text: String,
        failed_command: Option<String>,
        working_directory: Option<String>,
        shell_kind: Option<String>,
    ) {
        let _span = tracing::info_span!(
            "ai.propose_fix",
            provider = %self.provider.name(),
            error_len = error_text.len()
        )
        .entered();
        if error_text.trim().is_empty() {
            self.ui.error_message = Some("No error text provided".to_string());
            return;
        }

        self.ui.proposals.clear();
        self.ui.selected_proposal = 0;
        self.ui.error_message = None;
        self.ui.is_loading = true;

        let mut context = vec![
            ("mode".to_string(), "fix".to_string()),
            ("error".to_string(), error_text.clone()),
            ("platform".to_string(), std::env::consts::OS.to_string()),
            (
                "guidelines".to_string(),
                "no-invent; prefer-safe; no-placeholders; suggest --no-pager for git; comment with # if unsure".to_string(),
            ),
        ];
        if let Some(ref fc) = failed_command {
            context.push(("failed_command".into(), fc.clone()));
        }
        if let Some(ref sh) = shell_kind {
            context.push(("shell".into(), sh.clone()));
        }
        if let Some(ref dir) = working_directory {
            context.push(("cwd".into(), dir.clone()));
        }

        // Agent-first path (similar to explain), unless routing is forced to Provider
        let mode = self.effective_routing_mode();
        let try_agent = !matches!(mode, crate::config::ai::AiRoutingMode::Provider)
            && (self.agent_rt.is_some() || tokio::runtime::Handle::try_current().is_err());
        if try_agent {
            if self.agent_manager.is_none() {
                self.ensure_agent_manager();
            }
            if let Some(mgr) = &self.agent_manager {
                let rt = self.agent_rt.as_ref().expect("agent runtime initialized");
                let code_ctx = openagent_terminal_ai::agents::CodeContext {
                    current_file: None,
                    selection: failed_command.clone(),
                    cursor_position: None,
                    project_files: Vec::new(),
                    dependencies: Vec::new(),
                };
                let (action, prompt) = if failed_command.is_some() {
                    (
                        openagent_terminal_ai::agents::CodeAction::Refactor,
                        format!("Fix this command to address the following error:\n{}", error_text),
                    )
                } else {
                    (
                        openagent_terminal_ai::agents::CodeAction::Generate,
                        format!(
                            "Propose a corrected shell command to address this error:\n{}",
                            error_text
                        ),
                    )
                };
                let agent_req = openagent_terminal_ai::agents::AgentRequest::CodeGeneration {
                    language: Some("shell".to_string()),
                    context: code_ctx,
                    prompt,
                    action,
                };
                let t0 = std::time::Instant::now();
                match rt.block_on(mgr.process_request(agent_req)) {
                    Ok(openagent_terminal_ai::agents::AgentResponse::Code {
                        generated_code,
                        explanation,
                        ..
                    }) => {
                        let dt = t0.elapsed();
                        tracing::info!(
                            elapsed_ms = dt.as_millis() as u64,
                            "ai.propose_fix_complete_agent"
                        );
                        // Parse into one or more proposals when alternatives are present
                        let default_title = "Fix suggestion";
                        let mut proposals = Self::parse_fix_proposals_from_agent_output(
                            &generated_code,
                            default_title,
                            &explanation,
                        );
                        // Persist conversation (agent path)
                        let input_joined = if let Some(fc) = &failed_command {
                            format!("Error encountered while running '{}':\n{}", fc, error_text)
                        } else {
                            error_text.clone()
                        };
                        let flat_cmds: Vec<String> =
                            proposals.iter().flat_map(|p| p.proposed_commands.clone()).collect();
                        let _ = persist_ai_conversation(
                            "fix",
                            working_directory.as_deref(),
                            shell_kind.as_deref(),
                            &input_joined,
                            &flat_cmds.join("\n"),
                        );
                        self.enforce_git_no_pager_on_proposals(&mut proposals);
                        self.ui.proposals = if proposals.is_empty() {
                            vec![openagent_terminal_ai::AiProposal {
                                title: default_title.to_string(),
                                description: if explanation.is_empty() {
                                    None
                                } else {
                                    Some(explanation)
                                },
                                proposed_commands: Vec::new(),
                            }]
                        } else {
                            proposals
                        };
                        self.ui.is_loading = false;
                        return;
                    }
                    Ok(other) => {
                        let dt = t0.elapsed();
                        tracing::warn!(
                            ?other,
                            elapsed_ms = dt.as_millis() as u64,
                            "ai.propose_fix_wrong_variant"
                        );
                        // fall through to provider
                    }
                    Err(e) => {
                        let dt = t0.elapsed();
                        tracing::warn!(
                            error = %e,
                            elapsed_ms = dt.as_millis() as u64,
                            "ai.propose_fix_agent_error"
                        );
                        // fall through to provider
                    }
                }
            }
        }

        // Provider fallback or Provider-only mode
        let prompt = if let Some(fc) = &failed_command {
            format!("Error encountered while running '{}':\n{}\nSuggest a fix.", fc, error_text)
        } else {
            format!("Error: {}\nSuggest a fix.", error_text)
        };

        let mut req_raw = AiRequest {
            scratch_text: prompt,
            // Clone so we can still reference these later for persistence
            working_directory: working_directory.clone(),
            shell_kind: shell_kind.clone(),
            context,
        };
        if let Some(wd) = req_raw
            .working_directory
            .clone()
            .or_else(|| std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()))
        {
            let kv = self.fetch_project_context_kv(&wd);
            req_raw.context.extend(kv);
        }
        // Enrich with configured context providers and sanitize
        let (cm, budget_kb) = self.build_context_manager();
        let req = build_request_with_context(req_raw, &cm, budget_kb);

        let t0 = std::time::Instant::now();
        match self.provider.propose(req) {
            Ok(mut proposals) => {
                let dt = t0.elapsed();
                tracing::info!(elapsed_ms = dt.as_millis() as u64, "ai.propose_fix_complete");
                // Persist conversation (input + proposals)
                let cmds: Vec<String> =
                    proposals.iter().flat_map(|p| p.proposed_commands.clone()).collect();
                let input_joined = if let Some(fc) = &failed_command {
                    format!("Error encountered while running '{}':\n{}", fc, error_text)
                } else {
                    error_text.clone()
                };
                let _ = persist_ai_conversation(
                    "fix",
                    working_directory.as_deref(),
                    shell_kind.as_deref(),
                    &input_joined,
                    &cmds.join("\n"),
                );
                self.enforce_git_no_pager_on_proposals(&mut proposals);
                self.ui.proposals = proposals;
                self.ui.is_loading = false;
            }
            Err(e) => {
                let dt = t0.elapsed();
                tracing::info!(elapsed_ms = dt.as_millis() as u64, "ai.propose_fix_error");
                self.ui.error_message = Some(format!("Fix suggestion failed: {}", e));
                self.ui.is_loading = false;
            }
        }
    }

    /// Apply runtime AI context configuration
    pub fn set_context_config(&mut self, cfg: crate::config::ai::AiContextConfig) {
        self.context_cfg = cfg;
    }

    /// Set join strategy for multi-command application
    pub fn set_apply_joiner(&mut self, joiner: crate::config::ai::AiApplyJoinStrategy) {
        self.apply_joiner = joiner;
    }

    /// Convenience: switch provider by name using the loaded UiConfig.
    /// Falls back to built-in defaults when provider is not present in config.
    /// Returns Ok(()) if reconfiguration succeeded, Err(message) if it failed.
    pub fn set_provider_by_name(
        &mut self,
        provider_name: &str,
        ui_cfg: &crate::config::UiConfig,
    ) -> Result<(), String> {
        let name = provider_name.to_ascii_lowercase();
        let prov_cfg = ui_cfg
            .ai
            .providers
            .get(&name)
            .cloned()
            .or_else(|| {
                crate::config::ai_providers::get_default_provider_configs().get(&name).cloned()
            })
            .ok_or_else(|| format!("Unknown provider: {}", name))?;
        // Attempt reconfiguration; errors are surfaced via ui.error_message.
        self.reconfigure_to(&name, &prov_cfg);
        if let Some(err) = self.ui.error_message.clone() {
            Err(err)
        } else {
            Ok(())
        }
    }

    fn build_context_manager(&self) -> (ContextManager, usize) {
        let mut cm = ContextManager::new();
        if self.context_cfg.enabled {
            // Timeouts (soft)
            cm.set_timeouts(
                Some(self.context_cfg.timeouts.per_provider_ms),
                Some(self.context_cfg.timeouts.overall_ms),
            );
            // Providers in order
            for name in &self.context_cfg.providers {
                match name.as_str() {
                    "env" => cm.add_provider_with_timeout(
                        Box::new(BasicEnvProvider),
                        self.context_cfg
                            .timeouts
                            .env_ms
                            .or(Some(self.context_cfg.timeouts.per_provider_ms)),
                    ),
                    "git" => cm.add_provider_with_timeout(
                        Box::new(GitProvider::new(
                            self.context_cfg.git.include_branch,
                            self.context_cfg.git.include_status,
                        )),
                        self.context_cfg
                            .timeouts
                            .git_ms
                            .or(Some(self.context_cfg.timeouts.per_provider_ms)),
                    ),
                    "file_tree" => {
                        let strat = match self.context_cfg.file_tree.root_strategy {
                            crate::config::ai::AiRootStrategy::Git => {
                                FileTreeRootStrategy::RepoRoot
                            }
                            crate::config::ai::AiRootStrategy::Cwd => FileTreeRootStrategy::Cwd,
                        };
                        cm.add_provider_with_timeout(
                            Box::new(FileTreeProvider::new(
                                self.context_cfg.file_tree.max_entries,
                                strat,
                            )),
                            self.context_cfg
                                .timeouts
                                .file_tree_ms
                                .or(Some(self.context_cfg.timeouts.per_provider_ms)),
                        );
                    }
                    _ => {}
                }
            }
        }
        // Convert bytes -> KB rounding up
        let mut kb = self.context_cfg.max_bytes.div_ceil(1024);
        if !self.context_cfg.enabled {
            kb = 0;
        }
        (cm, kb)
    }
}

impl AiRuntime {
    pub fn set_routing_mode(&mut self, mode: crate::config::ai::AiRoutingMode) {
        self.routing_mode = mode;
    }

    pub fn effective_routing_mode(&self) -> crate::config::ai::AiRoutingMode {
        self.routing_mode
    }

    fn enforce_git_no_pager_on_proposals(&self, proposals: &mut [AiProposal]) {
        for p in proposals.iter_mut() {
            for cmd in p.proposed_commands.iter_mut() {
                *cmd = enforce_git_no_pager_line(cmd);
            }
        }
    }
}

fn enforce_git_no_pager_line(line: &str) -> String {
    // Fast path: skip if no 'git' or already contains --no-pager
    if !line.contains("git") || line.contains("--no-pager") {
        return line.to_string();
    }
    let mut tokens: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
    if tokens.is_empty() {
        return line.to_string();
    }
    // Find 'git' token possibly after sudo
    let mut git_idx: Option<usize> = None;
    for (i, t) in tokens.iter().enumerate() {
        if t == "git" {
            git_idx = Some(i);
            break;
        }
    }
    let gi = match git_idx {
        Some(i) => i,
        None => return line.to_string(),
    };
    // Determine insertion index after handling "-C <path>" chain
    let mut insert_at = gi + 1;
    let mut i = gi + 1;
    while i + 1 < tokens.len() {
        if tokens[i] == "-C" {
            i += 2;
            insert_at = i;
        } else {
            break;
        }
    }
    tokens.insert(insert_at, "--no-pager".to_string());
    tokens.join(" ")
}

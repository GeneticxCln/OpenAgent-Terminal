// Component Initialization Module
// Integrates WGPU renderer, HarfBuzz, Blocks 2.0, Workflows, and Plugins

#[allow(unused_imports)]
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::runtime::Runtime;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

// Import new components
#[cfg(feature = "blocks")]
use crate::blocks_v2::{BlockManager, CreateBlockParams};
#[cfg(feature = "harfbuzz")]
use crate::text_shaping::harfbuzz::{HarfBuzzShaper, ShapingConfig};
#[cfg(feature = "plugins")]
use plugin_api::{CommandOutput, PluginError};
#[cfg(feature = "plugins")]
use plugin_loader::{LogLevel, PluginHost, PluginManager};
#[cfg(feature = "workflow")]
use workflow_engine::WorkflowEngine;

/// Component initialization configuration
#[allow(dead_code)]
pub struct ComponentConfig {
    /// Enable WGPU renderer
    pub enable_wgpu: bool,
    /// Enable HarfBuzz text shaping
    pub enable_harfbuzz: bool,
    /// Enable Blocks 2.0 system
    pub enable_blocks: bool,
    /// Enable workflow automation
    pub enable_workflows: bool,
    /// Enable plugin system
    pub enable_plugins: bool,
    /// Data directory for components
    pub data_dir: PathBuf,
    /// Configuration directory
    pub config_dir: PathBuf,
}

impl Default for ComponentConfig {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openagent-terminal");

        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openagent-terminal");

        Self {
            enable_wgpu: true,
            enable_harfbuzz: true,
            enable_blocks: true,
            enable_workflows: true,
            enable_plugins: true,
            data_dir,
            config_dir,
        }
    }
}

/// Initialized components container
pub struct InitializedComponents {
    #[cfg(feature = "harfbuzz")]
    pub text_shaper: Option<Arc<tokio::sync::RwLock<HarfBuzzShaper>>>,
    #[cfg(feature = "blocks")]
    pub block_manager: Option<Arc<tokio::sync::RwLock<BlockManager>>>,
    #[cfg(feature = "blocks")]
    pub notebook_manager: Option<Arc<tokio::sync::RwLock<crate::notebooks::NotebookManager>>>,
    #[cfg(feature = "workflow")]
    pub workflow_engine: Option<Arc<WorkflowEngine>>,
    #[cfg(feature = "plugins")]
    pub plugin_manager: Option<Arc<PluginManager>>,
    pub runtime: Arc<Runtime>,
}

impl std::fmt::Debug for InitializedComponents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ds = f.debug_struct("InitializedComponents");
        #[cfg(feature = "harfbuzz")]
        {
            let _ = ds.field("text_shaper", &self.text_shaper.as_ref().map(|_| "Some"));
        }
        #[cfg(feature = "blocks")]
        {
            let _ = ds.field(
                "block_manager",
                &self.block_manager.as_ref().map(|_| "Some"),
            );
        }
        #[cfg(feature = "workflow")]
        {
            let _ = ds.field(
                "workflow_engine",
                &self.workflow_engine.as_ref().map(|_| "Some"),
            );
        }
        #[cfg(feature = "plugins")]
        {
            let _ = ds.field(
                "plugin_manager",
                &self.plugin_manager.as_ref().map(|_| "Some"),
            );
        }
        ds.field("runtime", &"<runtime>").finish()
    }
}

/// Initialize all components
pub async fn initialize_components(
    config: &ComponentConfig,
    _window: &winit::window::Window,
) -> Result<InitializedComponents> {
    info!("Initializing OpenAgent Terminal components...");

    // Create async runtime for components
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()?,
    );

    // Create directories
    std::fs::create_dir_all(&config.data_dir)?;
    std::fs::create_dir_all(&config.config_dir)?;

    // WGPU renderer is initialized as part of the display path; no separate component here.

    // Initialize HarfBuzz text shaper
    #[cfg(feature = "harfbuzz")]
    let text_shaper = if config.enable_harfbuzz {
        match initialize_harfbuzz().await {
            Ok(shaper) => {
                info!("✓ HarfBuzz text shaping initialized");
                Some(Arc::new(tokio::sync::RwLock::new(shaper)))
            }
            Err(e) => {
                warn!("Failed to initialize HarfBuzz: {}", e);
                None
            }
        }
    } else {
        debug!("HarfBuzz text shaping disabled");
        None
    };

    // Initialize Blocks 2.0 system
    #[cfg(feature = "blocks")]
    let block_manager = if config.enable_blocks {
        let blocks_dir = config.data_dir.join("blocks");
        match BlockManager::new(blocks_dir).await {
            Ok(manager) => {
                info!("✓ Blocks 2.0 system initialized");
                Some(Arc::new(tokio::sync::RwLock::new(manager)))
            }
            Err(e) => {
                error!("Failed to initialize Blocks system: {}", e);
                None
            }
        }
    } else {
        debug!("Blocks 2.0 system disabled");
        None
    };

    // Initialize Notebook manager
    #[cfg(feature = "blocks")]
    let notebook_manager = if config.enable_blocks {
        let notebooks_dir = config.data_dir.join("notebooks");
        match crate::notebooks::NotebookManager::new(&notebooks_dir, block_manager.clone()).await {
            Ok(mgr) => {
                info!("✓ Command Notebooks initialized");
                Some(Arc::new(tokio::sync::RwLock::new(mgr)))
            }
            Err(e) => {
                error!("Failed to initialize Notebooks: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Initialize workflow engine
    #[cfg(feature = "workflow")]
    let workflow_engine = if config.enable_workflows {
        match initialize_workflow_engine(&config.config_dir).await {
            Ok(engine) => {
                info!("✓ Workflow engine initialized");
                Some(Arc::new(engine))
            }
            Err(e) => {
                error!("Failed to initialize workflow engine: {}", e);
                None
            }
        }
    } else {
        debug!("Workflow engine disabled");
        None
    };

    // Initialize plugin manager
    #[cfg(feature = "plugins")]
    let plugin_manager = if config.enable_plugins {
        let plugins_dir = config.data_dir.join("plugins");
        // Plugin policy toggles (Warp-like defaults with env overrides for releases)
        let enforce_signatures = true;
        // Default to strict in release builds: require signatures for all, disable hot reload.
        let require_all_default = if cfg!(debug_assertions) { false } else { true };
        let hot_reload_default = if cfg!(debug_assertions) { true } else { false };
        let require_all = std::env::var("OPENAGENT_PLUGINS_REQUIRE_ALL")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(require_all_default);
        let hot_reload = std::env::var("OPENAGENT_PLUGINS_HOT_RELOAD")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(hot_reload_default);
        let require_system = true;
        let require_user = std::env::var("OPENAGENT_PLUGINS_USER_REQUIRE_SIGNED")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let require_project = std::env::var("OPENAGENT_PLUGINS_PROJECT_REQUIRE_SIGNED")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        match initialize_plugin_manager(
            plugins_dir.clone(),
            enforce_signatures,
            require_all,
            require_system,
            require_user,
            require_project,
            hot_reload,
        )
        .await
        {
            Ok(manager) => {
                info!("✓ Plugin system initialized");
                let pm = Arc::new(manager);
                if hot_reload {
                    let watcher_dirs = vec![
                        PathBuf::from("/usr/share/openagent-terminal/plugins"),
                        dirs::config_dir()
                            .map(|d| d.join("openagent-terminal").join("plugins"))
                            .unwrap_or_default(),
                        std::env::current_dir().unwrap_or_default().join("plugins"),
                        plugins_dir.clone(),
                    ];
                    spawn_plugin_watchers(Arc::clone(&pm), watcher_dirs);
                }
                Some(pm)
            }
            Err(e) => {
                error!("Failed to initialize plugin system: {}", e);
                None
            }
        }
    } else {
        debug!("Plugin system disabled");
        None
    };

    info!("Component initialization complete");

    Ok(InitializedComponents {
        #[cfg(feature = "harfbuzz")]
        text_shaper,
        #[cfg(feature = "blocks")]
        block_manager,
        #[cfg(feature = "blocks")]
        notebook_manager,
        #[cfg(feature = "workflow")]
        workflow_engine,
        #[cfg(feature = "plugins")]
        plugin_manager,
        runtime,
    })
}

/// Initialize HarfBuzz text shaper
#[cfg(feature = "harfbuzz")]
async fn initialize_harfbuzz() -> Result<HarfBuzzShaper> {
    let config = ShapingConfig {
        enable_ligatures: true,
        enable_kerning: true,
        enable_contextual_alternates: true,
        stylistic_sets: vec![],
        default_language: "en".to_string(),
        fallback_fonts: vec![
            "Noto Sans".to_string(),
            "DejaVu Sans".to_string(),
            "Segoe UI".to_string(),
        ],
        emoji_font: Some("Noto Color Emoji".to_string()),
    };

    let shaper = HarfBuzzShaper::new(config).context("Failed to create HarfBuzz shaper")?;

    debug!("HarfBuzz text shaper created with ligature support");
    Ok(shaper)
}

/// Initialize workflow engine
#[cfg(feature = "workflow")]
async fn initialize_workflow_engine(config_dir: &std::path::Path) -> Result<WorkflowEngine> {
    let engine = WorkflowEngine::new().context("Failed to create workflow engine")?;

    // Load workflows from directory
    let workflows_dir = config_dir.join("workflows");
    if workflows_dir.exists() {
        let mut count = 0;
        let mut entries = tokio::fs::read_dir(&workflows_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                || path.extension().and_then(|s| s.to_str()) == Some("yml")
            {
                match engine.load_workflow(&path).await {
                    Ok(id) => {
                        debug!("Loaded workflow: {}", id);
                        count += 1;
                    }
                    Err(e) => {
                        warn!("Failed to load workflow {:?}: {}", path, e);
                    }
                }
            }
        }

        if count == 0 {
            // Seed curated samples on first run
            debug!("Seeding curated workflow samples in {:?}", workflows_dir);
            seed_default_workflows(&workflows_dir).await?;
            // Reload after seeding
            let mut reentries = tokio::fs::read_dir(&workflows_dir).await?;
            while let Some(entry) = reentries.next_entry().await? {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                    || path.extension().and_then(|s| s.to_str()) == Some("yml")
                {
                    if let Ok(id) = engine.load_workflow(&path).await {
                        debug!("Loaded workflow: {}", id);
                    }
                }
            }
        }

        info!("Loaded {} workflows", count);
    } else {
        debug!("No workflows directory found, creating it");
        tokio::fs::create_dir_all(&workflows_dir).await?;
        // Seed curated samples on first run
        seed_default_workflows(&workflows_dir).await?;
    }

    Ok(engine)
}

#[cfg(feature = "workflow")]
async fn seed_default_workflows(dir: &std::path::Path) -> Result<()> {
    let rust = r#"name: Cargo build
on: manual
steps:
  - run: cargo build --workspace --release
    description: Build Rust workspace in release mode
"#;
    let node = r#"name: Node test
on: manual
steps:
  - run: npm ci
  - run: npm test
"#;
    let python = r#"name: Python lint
on: manual
steps:
  - run: pip install -r requirements.txt
  - run: ruff check .
"#;
    let files = [
        ("rust.yaml", rust),
        ("node.yaml", node),
        ("python.yaml", python),
    ];
    for (name, content) in files {
        let path = dir.join(name);
        if !path.exists() {
            tokio::fs::write(&path, content).await?;
        }
    }
    Ok(())
}

/// Initialize plugin manager
#[cfg(feature = "plugins")]
async fn initialize_plugin_manager(
    plugins_dir: PathBuf,
    enforce_signatures: bool,
    require_signatures_for_all: bool,
    path_require_system: bool,
    path_require_user: bool,
    path_require_project: bool,
    hot_reload: bool,
) -> Result<PluginManager> {
    // Compute multi-location plugin directories
    let mut dirs_vec = Vec::new();
    // System
    dirs_vec.push(PathBuf::from("/usr/share/openagent-terminal/plugins"));
    // User
    if let Some(cfg) = dirs::config_dir() {
        let user_dir = cfg.join("openagent-terminal").join("plugins");
        if let Err(e) = tokio::fs::create_dir_all(&user_dir).await {
            warn!("Failed to create user plugin dir: {}", e);
        }
        dirs_vec.push(user_dir);
    }
    // Project
    if let Ok(cwd) = std::env::current_dir() {
        dirs_vec.push(cwd.join("plugins"));
    }
    // Data dir (legacy default)
    if let Err(e) = tokio::fs::create_dir_all(&plugins_dir).await {
        warn!("Failed to create data plugin dir: {}", e);
    }
    dirs_vec.push(plugins_dir.clone());

    // Create plugin host with storage dir
    let storage_dir = if let Some(data) = dirs::data_dir() {
        data.join("openagent-terminal")
            .join("plugins")
            .join("storage")
    } else {
        PathBuf::from("./.openagent-terminal/plugins/storage")
    };
    if let Err(e) = tokio::fs::create_dir_all(&storage_dir).await {
        warn!("Failed to create storage dir: {}", e);
    }
    let host = Arc::new(TerminalPluginHost::new(storage_dir));

    // Log planned plugin directories and policy
    info!("Plugin directories under management:");
    for d in &dirs_vec {
        info!("  - {:?}", d);
    }
    let trusted_keys_dir =
        dirs::config_dir().map(|d| d.join("openagent-terminal").join("trusted_keys"));
    let trusted_keys = count_trusted_keys(trusted_keys_dir.clone());
    info!(
        "Plugin signing policy: enforce_signatures={}, require_signatures_for_all={}, \
         trusted_keys={} (dir: {:?})",
        enforce_signatures, require_signatures_for_all, trusted_keys, trusted_keys_dir
    );
    info!(
        "Per-path signature requirements: system={}, user={}, project={}, hot_reload={}",
        path_require_system, path_require_user, path_require_project, hot_reload
    );

    // Create plugin manager with host and directories
    let mut manager = PluginManager::with_host_and_dirs(dirs_vec, Some(host))
        .context("Failed to create plugin manager")?;
    manager.set_enforce_signatures(enforce_signatures);

    manager.configure_signature_policy(plugin_loader::SignaturePolicy {
        require_signatures_for_all,
        require_system: path_require_system,
        require_user: path_require_user,
        require_project: path_require_project,
        system_dir: Some(PathBuf::from("/usr/share/openagent-terminal/plugins")),
        user_dir: dirs::config_dir().map(|d| d.join("openagent-terminal").join("plugins")),
        project_dir: std::env::current_dir().ok().map(|d| d.join("plugins")),
    });

    // Discover and load plugins
    match manager.discover_plugins().await {
        Ok(plugins) => {
            info!("Discovered {} plugins", plugins.len());
            for plugin_path in plugins {
                match manager.load_plugin(&plugin_path).await {
                    Ok(id) => debug!("Loaded plugin: {}", id),
                    Err(e) => warn!("Failed to load plugin {:?}: {}", plugin_path, e),
                }
            }
        }
        Err(e) => {
            warn!("Failed to discover plugins: {}", e);
        }
    }

    Ok(manager)
}

/// Terminal plugin host implementation
#[cfg(feature = "plugins")]
struct TerminalPluginHost {
    storage_dir: PathBuf,
}

#[cfg(feature = "plugins")]
impl TerminalPluginHost {
    fn new(storage_dir: PathBuf) -> Self {
        Self { storage_dir }
    }
}

#[cfg(feature = "plugins")]
impl PluginHost for TerminalPluginHost {
    fn log(&self, level: LogLevel, message: &str) {
        match level {
            LogLevel::Debug => debug!("[Plugin] {}", message),
            LogLevel::Info => info!("[Plugin] {}", message),
            LogLevel::Warning => warn!("[Plugin] {}", message),
            LogLevel::Error => error!("[Plugin] {}", message),
        }
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, PluginError> {
        std::fs::read(path).map_err(PluginError::IoError)
    }

    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), PluginError> {
        std::fs::write(path, data).map_err(PluginError::IoError)
    }

    fn execute_command(&self, command: &str) -> Result<CommandOutput, PluginError> {
        // Security Lens gating for plugin-executed commands.
        // Read policy from current UiConfig via confirm broker.
        let policy = crate::ui_confirm::get_security_policy();
        let mut lens = crate::security_lens::SecurityLens::new(policy.clone());
        let risk = lens.analyze_command(command);
        if lens.should_block(&risk) {
            return Err(PluginError::CommandFailed(format!(
                "Blocked risky plugin command ({}): {}",
                risk.level as u8, risk.explanation
            )));
        }
        // Interactive confirmation if required by policy
        let require_confirm = *policy
            .require_confirmation
            .get(&risk.level)
            .unwrap_or(&false);
        if require_confirm {
            let mut body = String::new();
            body.push_str(&format!("{}\n\n", risk.explanation));
            if !risk.mitigations.is_empty() {
                body.push_str("Suggested mitigations:\n");
                for m in &risk.mitigations {
                    body.push_str(&format!("  • {}\n", m));
                }
                body.push('\n');
            }
            body.push_str(&format!("Command:\n  {}", command));
            let title = match risk.level {
                crate::security_lens::RiskLevel::Critical => {
                    "CRITICAL: Confirm plugin command".into()
                }
                crate::security_lens::RiskLevel::Warning => {
                    "Warning: Confirm plugin command".into()
                }
                crate::security_lens::RiskLevel::Caution => {
                    "Caution: Confirm plugin command".into()
                }
                crate::security_lens::RiskLevel::Safe => "Confirm plugin command".into(),
            };
            match crate::ui_confirm::request_confirm(
                title,
                body,
                Some("Run".into()),
                Some("Cancel".into()),
                Some(30_000),
            ) {
                Ok(true) => {}
                Ok(false) => {
                    return Err(PluginError::CommandFailed("User canceled command".into()));
                }
                Err(e) => {
                    return Err(PluginError::CommandFailed(format!(
                        "Confirmation failed: {}",
                        e
                    )));
                }
            }
        }

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| PluginError::CommandFailed(e.to_string()))?;

        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            execution_time_ms: 0,
        })
    }

    fn store_data_for(&self, plugin_id: &str, key: &str, value: &[u8]) -> Result<(), PluginError> {
        let dir = self.storage_dir.join(sanitize_key_to_filename(plugin_id));
        std::fs::create_dir_all(&dir).map_err(PluginError::IoError)?;
        // Basic quota: cap per-plugin storage to ~50 MiB; reject if exceeding
        const MAX_BYTES: u64 = 50 * 1024 * 1024;
        let used = dir_size_bytes(&dir).unwrap_or(0);
        if used > MAX_BYTES {
            return Err(PluginError::IoError(std::io::Error::other(
                "Plugin storage quota exceeded",
            )));
        }
        let file = dir.join(sanitize_key_to_filename(key));
        std::fs::write(file, value).map_err(PluginError::IoError)
    }

    fn retrieve_data_for(
        &self,
        plugin_id: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>, PluginError> {
        let dir = self.storage_dir.join(sanitize_key_to_filename(plugin_id));
        let file = dir.join(sanitize_key_to_filename(key));
        match std::fs::read(file) {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(PluginError::IoError(e)),
        }
    }

    fn store_document_for(
        &self,
        plugin_id: &str,
        namespace: &str,
        doc_id: &str,
        doc_json: &str,
    ) -> Result<(), PluginError> {
        let base = self
            .storage_dir
            .join(sanitize_key_to_filename(plugin_id))
            .join("docs")
            .join(sanitize_key_to_filename(namespace));
        std::fs::create_dir_all(&base).map_err(PluginError::IoError)?;
        let file = base.join(format!("{}.json", sanitize_key_to_filename(doc_id)));
        std::fs::write(file, doc_json.as_bytes()).map_err(PluginError::IoError)
    }

    fn retrieve_document_for(
        &self,
        plugin_id: &str,
        namespace: &str,
        doc_id: &str,
    ) -> Result<Option<String>, PluginError> {
        let file = self
            .storage_dir
            .join(sanitize_key_to_filename(plugin_id))
            .join("docs")
            .join(sanitize_key_to_filename(namespace))
            .join(format!("{}.json", sanitize_key_to_filename(doc_id)));
        match std::fs::read_to_string(file) {
            Ok(json) => Ok(Some(json)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(PluginError::IoError(e)),
        }
    }
}

/// Integration helper for using components with the terminal
pub struct ComponentIntegration<'a> {
    components: &'a InitializedComponents,
}

#[allow(dead_code)]
impl<'a> ComponentIntegration<'a> {
    pub fn new(components: &'a InitializedComponents) -> Self {
        Self { components }
    }

    /// Create a new block for command execution
    #[cfg(feature = "blocks")]
    pub async fn create_command_block(&self, command: String, shell: &str) -> Result<()> {
        if let Some(manager) = &self.components.block_manager {
            let mut manager = manager.write().await;

            let params = CreateBlockParams {
                command,
                directory: Some(std::env::current_dir()?),
                environment: Some(std::env::vars().collect()),
                shell: Some(
                    crate::blocks_v2::ShellType::from_str(shell)
                        .unwrap_or(crate::blocks_v2::ShellType::Bash),
                ),
                tags: None,
                parent_id: None,
                metadata: None,
            };

            let block = manager.create_block(params).await?;
            debug!("Created block: {}", block.id.to_string());
        }

        Ok(())
    }

    /// Shape text using HarfBuzz
    #[cfg(feature = "harfbuzz")]
    pub async fn shape_text(&self, text: &str, font: &str, size: f32) -> Result<Vec<u32>> {
        if let Some(shaper) = &self.components.text_shaper {
            let mut shaper = shaper.write().await;
            let shaped = shaper.shape_text(text, font, size)?;

            Ok(shaped.glyphs.iter().map(|g| g.glyph_id).collect())
        } else {
            Ok(vec![])
        }
    }

    /// Execute a workflow
    #[cfg(feature = "workflow")]
    pub async fn execute_workflow(
        &self,
        workflow_id: &str,
        parameters: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        if let Some(engine) = &self.components.workflow_engine {
            let execution_id = engine.execute_workflow(workflow_id, parameters).await?;
            Ok(execution_id)
        } else {
            anyhow::bail!("Workflow engine not initialized")
        }
    }
}

#[cfg(feature = "plugins")]
fn count_trusted_keys(dir: Option<PathBuf>) -> usize {
    if let Some(d) = dir {
        if let Ok(entries) = std::fs::read_dir(d) {
            return entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("pub"))
                .count();
        }
    }
    0
}

#[cfg(feature = "plugins")]
fn sanitize_key_to_filename(key: &str) -> String {
    let mut s = String::with_capacity(key.len());
    for ch in key.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            s.push(ch);
        } else {
            s.push('_');
        }
    }
    // prevent empty filename
    if s.is_empty() {
        s.push('_');
    }
    s
}

#[cfg(feature = "plugins")]
fn dir_size_bytes(dir: &std::path::Path) -> Option<u64> {
    let mut total: u64 = 0;
    let rd = std::fs::read_dir(dir).ok()?;
    for entry in rd.flatten() {
        let path = entry.path();
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            } else if meta.is_dir() {
                total = total.saturating_add(dir_size_bytes(&path).unwrap_or(0));
            }
        }
    }
    Some(total)
}

#[cfg(feature = "plugins")]
fn spawn_plugin_watchers(manager: Arc<PluginManager>, dirs: Vec<PathBuf>) {
    use notify::{
        Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode,
        Result as NotifyResult, Watcher,
    };
    use std::sync::mpsc::channel;

    // Spawn a background task to receive events and process async loads/unloads
    tokio::spawn(async move {
        let (tx, rx) = channel::<Event>();

        // Watcher lives in this task scope
        let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
            move |res: NotifyResult<Event>| match res {
                Ok(ev) => {
                    let _ = tx.send(ev);
                }
                Err(e) => warn!("Plugin watcher error: {}", e),
            },
            NotifyConfig::default(),
        )
        .expect("failed to create file watcher");

        for d in dirs {
            if d.exists() {
                if let Err(e) = watcher.watch(&d, RecursiveMode::NonRecursive) {
                    warn!("Failed to watch {:?}: {}", d, e);
                } else {
                    debug!("Watching plugin dir: {:?}", d);
                }
            }
        }

        // Helper closures for load/unload
        let handle_create_or_modify = |p: &PathBuf| {
            let p = p.clone();
            let mgr = Arc::clone(&manager);
            tokio::spawn(async move {
                if p.extension().and_then(|s| s.to_str()) == Some("wasm") {
                    // Unload if already loaded (by name) to trigger cleanup
                    if let Some(name) = p
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                    {
                        let loaded = mgr.loaded_names_and_paths().await;
                        if loaded.iter().any(|(n, _)| n == &name) {
                            let _ = mgr.unload_plugin(&name).await;
                        }
                    }
                    match mgr.load_plugin(&p).await {
                        Ok(name) => info!("(watch) Loaded plugin: {}", name),
                        Err(e) => debug!("(watch) Load skipped for {:?}: {}", p, e),
                    }
                }
            });
        };

        let handle_remove = |p: &PathBuf| {
            let p = p.clone();
            let mgr = Arc::clone(&manager);
            tokio::spawn(async move {
                let loaded = mgr.loaded_names_and_paths().await;
                for (name, path) in loaded {
                    if path == p {
                        match mgr.unload_plugin(&name).await {
                            Ok(()) => info!("(watch) Unloaded plugin: {}", name),
                            Err(e) => debug!("(watch) Unload skipped for {}: {}", name, e),
                        }
                    }
                }
            });
        };

        // Event loop
        loop {
            match rx.recv() {
                Ok(event) => {
                    // Prefer to act on each path
                    for path in &event.paths {
                        match &event.kind {
                            EventKind::Create(_) | EventKind::Modify(_) => {
                                handle_create_or_modify(path)
                            }
                            EventKind::Remove(_) => handle_remove(path),
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    warn!("Plugin watcher recv error: {}", e);
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_component_config_default() {
        let config = ComponentConfig::default();
        assert!(config.enable_wgpu);
        assert!(config.enable_harfbuzz);
        assert!(config.enable_blocks);
        assert!(config.enable_workflows);
        assert!(config.enable_plugins);
    }
}

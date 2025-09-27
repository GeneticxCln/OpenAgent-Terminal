// Production-ready component initialization and integration
// Integrates WGPU renderer, HarfBuzz, Blocks 2.0, Workflows, and Plugins

#[allow(unused_imports)]
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};


// Import production-ready components
#[cfg(feature = "blocks")]
use crate::blocks_v2::{BlockManager, CreateBlockParams};
#[cfg(feature = "plugins")]
use crate::plugins_api::{PluginManager, SignaturePolicy};

#[cfg(feature = "harfbuzz")]
use crate::text_shaping::harfbuzz::{HarfBuzzShaper, ShapingConfig};

// Fallback types when harfbuzz is not available
#[cfg(not(feature = "harfbuzz"))]
pub struct HarfBuzzShaper;
#[cfg(not(feature = "harfbuzz"))]
pub struct ShapingConfig;

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
        let data_dir =
            dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("openagent-terminal");

        let config_dir =
            dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("openagent-terminal");

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

/// Workflow search result for the UI
#[derive(Debug, Clone)]
pub struct WorkflowSearchResult {
    pub id: String,                     // unique id, typically file path
    pub name: String,                   // display name
    pub description: Option<String>,    // optional description
    pub tags: Vec<String>,              // optional tags (from metadata)
    pub parameters: Vec<crate::display::workflow_panel::WorkflowParam>, // parsed params
}

/// Workflow events for event system
#[derive(Debug, Clone)]
pub enum WorkflowEvent {
    Started { execution_id: String },
    StepStarted { execution_id: String, step_id: String },
    StepCompleted { execution_id: String, step_id: String },
    StepFailed { execution_id: String, step_id: String, error: String },
    Completed { execution_id: String, status: String },
    Log { execution_id: String, step_id: Option<String>, message: String },
}

/// Real workflows engine: scans YAML in project and user config
pub struct WorkflowEngine {
    roots: Vec<std::path::PathBuf>,
    event_tx: tokio::sync::broadcast::Sender<WorkflowEvent>,
}

impl WorkflowEngine {
    pub fn new() -> anyhow::Result<Self> {
        let (event_tx, _) = tokio::sync::broadcast::channel(1000);
        let mut roots: Vec<std::path::PathBuf> = Vec::new();
        // Project roots (prefer user project-local definitions)
        if let Ok(cwd) = std::env::current_dir() {
            roots.push(cwd.join(".openagent-terminal").join("workflows"));
            roots.push(cwd.join(".warp").join("workflows")); // Warp-compatible
        }
        // User config roots
        if let Some(cfg) = dirs::config_dir() {
            roots.push(cfg.join("openagent-terminal").join("workflows"));
        }
        // Warp user dir for compatibility
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".warp").join("workflows"));
        }
        Ok(Self { roots, event_tx })
    }

    /// Read all workflow files and return search results
    pub async fn list_workflows(&self) -> anyhow::Result<Vec<WorkflowSearchResult>> {
        use tokio::fs;
        let mut results: Vec<WorkflowSearchResult> = Vec::new();
        for root in &self.roots {
            if fs::metadata(root).await.is_err() { continue; }
            let mut dir = match fs::read_dir(root).await { Ok(d) => d, Err(_) => continue };
            while let Ok(Some(entry)) = dir.next_entry().await {
                let path = entry.path();
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                if ext != "yaml" && ext != "yml" { continue; }
                if let Ok(text) = fs::read_to_string(&path).await {
                    if let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
                        if let Some(ws) = Self::to_search_result(&path, &doc) {
                            results.push(ws);
                        }
                    }
                }
            }
        }
        // Deduplicate by (name, id)
        results.sort_by(|a,b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        results.dedup_by(|a,b| a.id == b.id);
        Ok(results)
    }

    /// Get workflow JSON definition by name (case-insensitive)
    pub async fn get_workflow_by_name(&self, name: &str) -> Option<(String, serde_json::Value)> {
        let target = name.to_lowercase();
        if let Ok(list) = self.list_workflows().await {
            for w in list {
                if w.name.to_lowercase() == target {
                    if let Some((id, json)) = self.load_workflow_json(std::path::Path::new(&w.id)).await.ok().flatten() {
                        return Some((id, json));
                    }
                }
            }
        }
        None
    }

    /// Execute a workflow by id with provided params; Emits progress via broadcast channel.
    /// This executes logically (emits step events) and does not run shell commands directly;
    /// actual insertion/execution is handled by higher-level UI paths.
    pub async fn execute_workflow(
        &self,
        id: &str,
        _params: std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        let execution_id = uuid::Uuid::new_v4().to_string();
        let _ = self.event_tx.send(WorkflowEvent::Started { execution_id: execution_id.clone() });
        if let Some((_wid, json)) = self.load_workflow_json(&std::path::PathBuf::from(id)).await? {
            // Iterate steps: support steps[*].id, steps[*].name, steps[*].commands (array of strings)
            if let Some(steps) = json.get("steps").and_then(|v| v.as_array()) {
                for step in steps {
                    let step_id = step
                        .get("id")
                        .and_then(|v| v.as_str())
                        .or_else(|| step.get("name").and_then(|v| v.as_str()))
                        .unwrap_or("")
                        .to_string();
                    let label = if step_id.is_empty() { "step".to_string() } else { step_id.clone() };
                    let _ = self.event_tx.send(WorkflowEvent::StepStarted {
                        execution_id: execution_id.clone(),
                        step_id: label.clone(),
                    });
                    // Emit a simple log for preview; actual command execution is external
                    let _ = self.event_tx.send(WorkflowEvent::Log {
                        execution_id: execution_id.clone(),
                        step_id: Some(label.clone()),
                        message: "Prepared commands".to_string(),
                    });
                    let _ = self.event_tx.send(WorkflowEvent::StepCompleted {
                        execution_id: execution_id.clone(),
                        step_id: label,
                    });
                }
            }
        }
        let _ = self.event_tx.send(WorkflowEvent::Completed {
            execution_id: execution_id.clone(),
            status: "Success".to_string(),
        });
        Ok(execution_id)
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<WorkflowEvent> {
        self.event_tx.subscribe()
    }

    fn to_search_result(path: &std::path::Path, doc: &serde_yaml::Value) -> Option<WorkflowSearchResult> {
        use crate::display::workflow_panel::{WorkflowParam, WorkflowParamOption, WorkflowParamType};
        let name = doc.get("name").and_then(|v| v.as_str()).map(|s| s.to_string())
            .or_else(|| path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string()))?;
        let description = doc.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
        let tags: Vec<String> = doc.get("metadata")
            .and_then(|m| m.get("tags"))
            .and_then(|t| t.as_sequence())
            .map(|seq| seq.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        // Parse parameters
        let mut parameters: Vec<WorkflowParam> = Vec::new();
        if let Some(params) = doc.get("parameters").and_then(|v| v.as_sequence()) {
            for p in params {
                let pname = p.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if pname.is_empty() { continue; }
                let ty = p.get("type").and_then(|v| v.as_str()).unwrap_or("string").to_lowercase();
                let param_type = match ty.as_str() {
                    "number" => WorkflowParamType::Number,
                    "bool" | "boolean" => WorkflowParamType::Boolean,
                    "select" => WorkflowParamType::Select,
                    _ => WorkflowParamType::String,
                };
                let description = p.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let required = p.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
let default = p.get("default").map(Self::yaml_to_json);
                let options = p.get("options").and_then(|v| v.as_sequence()).map(|seq| {
                    seq.iter().filter_map(|opt| {
                        let label = opt.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
let value = opt.get("value").map(Self::yaml_to_json).unwrap_or(serde_json::Value::Null);
                        if label.is_empty() { None } else { Some(WorkflowParamOption { value, label }) }
                    }).collect::<Vec<_>>()
                });
                let min = p.get("min").and_then(|v| v.as_f64());
                let max = p.get("max").and_then(|v| v.as_f64());
                parameters.push(WorkflowParam { name: pname, param_type, description, required, default, options, min, max });
            }
        }
        Some(WorkflowSearchResult { id: path.to_string_lossy().to_string(), name, description, tags, parameters })
    }

    /// Convert serde_yaml::Value to serde_json::Value (best-effort)
    fn yaml_to_json(v: &serde_yaml::Value) -> serde_json::Value {
        match v {
            serde_yaml::Value::Null => serde_json::Value::Null,
            serde_yaml::Value::Bool(b) => serde_json::Value::Bool(*b),
            serde_yaml::Value::Number(n) => {
                if let Some(i) = n.as_i64() { serde_json::Value::Number(i.into()) }
                else if let Some(u) = n.as_u64() { serde_json::Value::Number(serde_json::Number::from(u)) }
                else if let Some(f) = n.as_f64() { serde_json::json!(f) }
                else { serde_json::Value::Null }
            }
            serde_yaml::Value::String(s) => serde_json::Value::String(s.clone()),
            serde_yaml::Value::Sequence(seq) => serde_json::Value::Array(seq.iter().map(Self::yaml_to_json).collect()),
            serde_yaml::Value::Mapping(map) => {
                let mut obj = serde_json::Map::new();
                for (k, v2) in map.iter() {
                    let key = match k {
                        serde_yaml::Value::String(s) => s.clone(),
                        _ => format!("{}", Self::yaml_to_json(k)),
                    };
                    obj.insert(key, Self::yaml_to_json(v2));
                }
                serde_json::Value::Object(obj)
            }
            _ => serde_json::Value::Null,
        }
    }

    async fn load_workflow_json(&self, id_or_path: &std::path::Path) -> anyhow::Result<Option<(String, serde_json::Value)>> {
        use tokio::fs;
        let path = if id_or_path.is_absolute() || id_or_path.exists() { id_or_path.to_path_buf() } else { std::path::PathBuf::from(id_or_path) };
        if fs::metadata(&path).await.is_err() {
            // Try to resolve by name across roots
            let name_lower = path.to_string_lossy().to_string().to_lowercase();
            for root in &self.roots {
                if fs::metadata(root).await.is_err() { continue; }
                let mut dir = match fs::read_dir(root).await { Ok(d) => d, Err(_) => continue };
                while let Ok(Some(entry)) = dir.next_entry().await {
                    let p = entry.path();
                    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
                    if ext != "yaml" && ext != "yml" { continue; }
                    if let Ok(text) = fs::read_to_string(&p).await {
                        if let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
                            let nm = doc.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                            if !nm.is_empty() && nm == name_lower {
                                let json = serde_json::to_value(doc).unwrap_or(serde_json::json!({}));
                                return Ok(Some((p.to_string_lossy().to_string(), json)));
                            }
                        }
                    }
                }
            }
            return Ok(None);
        }
        let text = fs::read_to_string(&path).await?;
        let doc: serde_yaml::Value = serde_yaml::from_str(&text)?;
        let json = serde_json::to_value(doc).unwrap_or(serde_json::json!({}));
        Ok(Some((path.to_string_lossy().to_string(), json)))
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
    pub workflow_engine: Option<Arc<WorkflowEngine>>,
#[cfg(all(feature = "blocks", feature = "plugins"))]
    pub plugin_manager: Option<Arc<PluginManager>>,
    #[cfg(feature = "blocks")]
    pub storage: Option<Arc<crate::storage::Storage>>,
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
            let _ = ds.field("block_manager", &self.block_manager.as_ref().map(|_| "Some"));
        }
        #[cfg(feature = "blocks")]
        {
            let _ = ds.field("workflow_engine", &self.workflow_engine.as_ref().map(|_| "Some"));
        }
#[cfg(all(feature = "blocks", feature = "plugins"))]
        {
            let _ = ds.field("plugin_manager", &self.plugin_manager.as_ref().map(|_| "Some"));
        }
        #[cfg(feature = "blocks")]
        {
            let _ = ds.field("storage", &self.storage.as_ref().map(|_| "Some"));
        }
        ds.field("runtime", &"<runtime>").finish()
    }
}

/// Initialize all components
pub async fn initialize_components(config: &ComponentConfig) -> Result<InitializedComponents> {
    info!("Initializing OpenAgent Terminal components...");

    // Create async runtime for components
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build()?,
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

    // Initialize Storage (SQLite) used by blocks/plugins persistence
    #[cfg(feature = "blocks")]
    let storage = if config.enable_blocks {
        let db_path = config.data_dir.join("terminal.db");
match crate::storage::Storage::new(&db_path) {
            Ok(storage) => {
                info!("✓ Storage initialized at {}", db_path.display());
                Some(Arc::new(storage))
            }
            Err(e) => {
                warn!("Failed to initialize storage: {}", e);
                None
            }
        }
    } else {
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
        let mgr = crate::notebooks::NotebookManager::new();
        info!("✓ Command Notebooks initialized");
        Some(Arc::new(tokio::sync::RwLock::new(mgr)))
    } else {
        None
    };

    // Initialize workflow engine
    let workflow_engine = if config.enable_workflows {
        match WorkflowEngine::new() {
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
#[cfg(all(feature = "blocks", feature = "plugins"))]
    let plugin_manager = if config.enable_plugins {
        let plugins_dir = config.data_dir.join("plugins");
        // Plugin policy toggles (Warp-like defaults with env overrides for releases)
        let enforce_signatures = true;
        // Default to strict in release builds: require signatures for all, disable hot reload.
        let require_all_default = !cfg!(debug_assertions);
        let hot_reload_default = cfg!(debug_assertions);
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
        workflow_engine,
#[cfg(all(feature = "blocks", feature = "plugins"))]
        plugin_manager,
        #[cfg(feature = "blocks")]
        storage,
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
#[cfg(feature = "blocks")]
#[allow(dead_code)]
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
                match engine.load_workflow_json(&path).await {
                    Ok(id) => {
                        debug!("Loaded workflow: {:?}", id);
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
                    if let Ok(Some((id, _))) = engine.load_workflow_json(&path).await {
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

#[cfg(feature = "blocks")]
#[allow(dead_code)]
async fn seed_default_workflows(dir: &std::path::Path) -> Result<()> {
    let rust = r#"name: Cargo Build
version: "1.0.0"
description: Build Rust workspace in release mode
metadata:
  tags: ["build", "rust"]
  icon: null
  estimated_duration: "5m"
requirements:
  - command: cargo
    required: true
parameters: []
environment: {}
steps:
  - id: build
    name: Cargo Build
    description: Build Rust workspace in release mode
    commands:
      - cargo build --workspace --release
hooks: {}
outputs: []
"#;
    let node = r#"name: Node Test
version: "1.0.0"
description: Install dependencies and run tests
metadata:
  tags: ["test", "node"]
  icon: null
  estimated_duration: "3m"
requirements:
  - command: node
    required: true
  - command: npm
    required: true
parameters: []
environment: {}
steps:
  - id: install
    name: Install Dependencies
    commands:
      - npm ci
  - id: test
    name: Run Tests
    commands:
      - npm test
hooks: {}
outputs: []
"#;
    let python = r#"name: Python Lint
version: "1.0.0"
description: Install dependencies and run linter
metadata:
  tags: ["lint", "python"]
  icon: null
  estimated_duration: "2m"
requirements:
  - command: python
    required: true
  - command: pip
    required: true
  - command: ruff
    required: false
parameters: []
environment: {}
steps:
  - id: install
    name: Install Dependencies
    commands:
      - pip install -r requirements.txt
  - id: lint
    name: Run Ruff Linter
    commands:
      - ruff check .
hooks: {}
outputs: []
"#;
    let files = [("rust.yaml", rust), ("node.yaml", node), ("python.yaml", python)];
    for (name, content) in files {
        let path = dir.join(name);
        if !path.exists() {
            tokio::fs::write(&path, content).await?;
        }
    }
    Ok(())
}

/// Initialize plugin manager
#[cfg(all(feature = "blocks", feature = "plugins"))]
pub(crate) async fn initialize_plugin_manager(
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

    // Preinstall bundled plugins (WASM) to data plugins dir so they are always present
    if let Err(e) = install_bundled_plugins(&plugins_dir).await {
        warn!("Failed to preinstall bundled plugins: {}", e);
    }


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

    // Determine signature policy
    let signature_policy = if enforce_signatures && require_signatures_for_all {
        SignaturePolicy::Required
    } else if enforce_signatures {
        SignaturePolicy::Preferred
    } else {
        SignaturePolicy::Optional
    };

    // Create plugin manager
    let mut manager = PluginManager::new(signature_policy);

    // Register search directories
    for dir in dirs_vec {
        if let Err(e) = manager.host_mut().add_plugin_directory(&dir) {
            warn!("Failed to add plugin directory {:?}: {}", dir, e);
        }
    }

    // Initialize (scan and load)
    if let Err(e) = manager.initialize().await {
        warn!("Plugin manager initialization failed: {}", e);
    }

    Ok(manager)
}


/// Integration helper for using components with the terminal
pub struct ComponentIntegration<'a> {
    #[allow(dead_code)]
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
                    shell
                        .parse::<crate::blocks_v2::ShellType>()
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

#[cfg(feature = "blocks")]
#[allow(dead_code)]
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

#[cfg(feature = "blocks")]
#[allow(dead_code)]
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

#[cfg(feature = "blocks")]
#[allow(dead_code)]
fn write_security_audit_event(
    plugin_id: Option<&str>,
    command: &str,
    action: &str,
    reason: &str,
    risk_level: &str,
) -> std::io::Result<()> {
    // Resolve data dir
    let base_dir = dirs::data_dir()
        .map(|d| d.join("openagent-terminal").join("security"))
        .unwrap_or_else(|| std::path::PathBuf::from("./.openagent-terminal/security"));
    std::fs::create_dir_all(&base_dir)?;
    let log_path = base_dir.join("audit.log");

    // Build JSONL entry
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let entry = serde_json::json!({
        "ts_ms": now,
        "source": "plugin",
        "plugin_id": plugin_id,
        "action": action,
        "risk_level": risk_level,
        "command": command,
        "reason": reason,
    });
    let line = format!("{}\n", entry);
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
}

#[cfg(feature = "blocks")]
#[allow(dead_code)]
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

#[cfg(all(feature = "blocks", feature = "plugins"))]
async fn install_bundled_plugins(dir: &PathBuf) -> Result<(), anyhow::Error> {
    use tokio::fs;
    fs::create_dir_all(dir).await.ok();

    struct Builtin<'a> {
        stem: &'a str,
        manifest: &'a str,
    }
    const BUILTINS: &[Builtin<'_>] = &[
        Builtin {
            stem: "dev-tools-bundled",
            manifest: r#"[permissions]
read_files=[]
write_files=[]
environment_variables=[]
network=false
execute_commands=false
max_memory_mb=50
timeout_ms=5000
"#,
        },
        Builtin {
            stem: "docker-helper-bundled",
            manifest: r#"[permissions]
read_files=[]
write_files=[]
environment_variables=[]
network=false
execute_commands=false
max_memory_mb=50
timeout_ms=5000
"#,
        },
        Builtin {
            stem: "git-context-bundled",
            manifest: r#"[permissions]
read_files=[]
write_files=[]
environment_variables=[]
network=false
execute_commands=false
max_memory_mb=50
timeout_ms=5000
"#,
        },
    ];

    // Try copying real WASM artifacts from target/wasm32-wasi if available; otherwise fall back to a minimal stub.
    // Helper to locate built .wasm files (release/debug; with/without lib prefix; snake/hyphen variants).
    fn locate_wasm_artifact(crate_snake: &str) -> Option<PathBuf> {
        let target_dir = std::env::var("CARGO_TARGET_DIR")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target"));
        let mut names: Vec<String> = vec![
            format!("{}.wasm", crate_snake),
            format!("lib{}.wasm", crate_snake),
            format!("{}.wasm", crate_snake.replace('-', "_")),
            format!("lib{}.wasm", crate_snake.replace('-', "_")),
        ];
        // Also accept crate names without underscores
        let no_underscore = crate_snake.replace('_', "");
        names.push(format!("{}.wasm", no_underscore));
        names.push(format!("lib{}.wasm", no_underscore));
        for prof in ["release", "debug"] {
            for n in &names {
                let p = target_dir.join("wasm32-wasi").join(prof).join(n);
                if p.exists() {
                    return Some(p);
                }
            }
        }
        None
    }

    // Minimal WASM that returns success for plugin_handle_event
    const WAT_SRC: &str = r#"(module
      (memory (export "memory") 1)
      (func (export "plugin_alloc") (param i32) (result i32)
        (i32.const 0)
      )
      (func (export "plugin_init") (result i32)
        (i32.const 0)
      )
      (func (export "plugin_cleanup") (result i32)
        (i32.const 0)
      )
      ;; handle_event(event_ptr,event_len) -> i32 rc (0=ok)
      (func (export "plugin_handle_event") (param i32 i32) (result i32)
        (i32.const 0)
      )
    )"#;
    let wasm_bytes = wat::parse_str(WAT_SRC)?;

    // Map bundled names to likely crate names (snake-case) in /plugins dir
    use std::collections::HashMap;
    let mut map: HashMap<&str, &str> = HashMap::new();
    map.insert("dev-tools-bundled", "dev-tools");
    map.insert("docker-helper-bundled", "docker-helper");
    map.insert("git-context-bundled", "git-context");

    for b in BUILTINS {
        let wasm_path = dir.join(format!("{}.wasm", b.stem));
        let toml_path = dir.join(format!("{}.toml", b.stem));
        if fs::metadata(&wasm_path).await.is_err() {
            // Prefer real artifact if found, else write stub
            let real = map.get(b.stem).and_then(|c| locate_wasm_artifact(c));
            if let Some(real_path) = real {
                if let Ok(bytes) = tokio::fs::read(&real_path).await {
                    fs::write(&wasm_path, bytes).await?;
                } else {
                    fs::write(&wasm_path, &wasm_bytes).await?;
                }
            } else {
                fs::write(&wasm_path, &wasm_bytes).await?;
            }
        }
        if fs::metadata(&toml_path).await.is_err() {
            fs::write(&toml_path, b.manifest.as_bytes()).await?;
        }
    }
    Ok(())
}

#[cfg(feature = "blocks")]
#[cfg(all(feature = "blocks", feature = "plugins", feature = "plugins-ui", feature = "never"))]
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
                    if let Some(name) =
                        p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
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

    #[cfg(all(feature = "blocks", feature = "plugins", feature = "plugins-ui"))]
    #[tokio::test]
    async fn test_plugins_manager_discovers_and_loads() {
        // Create a temporary plugins dir and a minimal WASM file
        let dir = tempfile::tempdir().expect("tmpdir");
        let plugins_dir = dir.path().to_path_buf();

        const WAT_SRC: &str = r#"(module
          (memory (export "memory") 1)
          (func (export "plugin_alloc") (param i32) (result i32)
            (i32.const 0)
          )
          (func (export "plugin_init") (result i32)
            (i32.const 0)
          )
          (func (export "plugin_cleanup") (result i32)
            (i32.const 0)
          )
          (func (export "plugin_handle_event") (param i32 i32) (result i32)
            (i32.const 0)
          )
        )"#;
        let wasm_bytes = wat::parse_str(WAT_SRC).expect("wat->wasm");
        let wasm_path = plugins_dir.join("unit_test_plugin.wasm");
        tokio::fs::write(&wasm_path, &wasm_bytes).await.expect("write wasm");

        // Initialize manager with relaxed policy
        let pm = initialize_plugin_manager(
            plugins_dir.clone(),
            false,
            false,
            false,
            false,
            false,
            false,
        )
        .await
        .expect("manager");

        // Discover and check the file is present
        let mut found = pm.discover_plugins().await.expect("discover");
        found.sort();
        assert!(found.iter().any(|p| p == &wasm_path));

        // Load and then unload
        let id = pm.load_plugin(&wasm_path).await.expect("load");
        assert_eq!(id, "unit_test_plugin");
        pm.unload_plugin(&id).await.expect("unload");
    }
}

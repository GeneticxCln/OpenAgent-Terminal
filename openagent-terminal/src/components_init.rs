// Component Initialization Module
// Integrates WGPU renderer, HarfBuzz, Blocks 2.0, Workflows, and Plugins

#[allow(unused_imports)]
use anyhow::{Context, Result};
use std::path::PathBuf;
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
use plugin_loader::{
    CommandDefinition, LogLevel, Notification, PluginHost, PluginManager, TerminalState,
};
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

/// Initialized components container
pub struct InitializedComponents {
    #[cfg(feature = "harfbuzz")]
    pub text_shaper: Option<Arc<tokio::sync::RwLock<HarfBuzzShaper>>>,
    #[cfg(feature = "blocks")]
    pub block_manager: Option<Arc<tokio::sync::RwLock<BlockManager>>>,
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
            let _ = ds.field("block_manager", &self.block_manager.as_ref().map(|_| "Some"));
        }
        #[cfg(feature = "workflow")]
        {
            let _ = ds.field("workflow_engine", &self.workflow_engine.as_ref().map(|_| "Some"));
        }
        #[cfg(feature = "plugins")]
        {
            let _ = ds.field("plugin_manager", &self.plugin_manager.as_ref().map(|_| "Some"));
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
            },
            Err(e) => {
                warn!("Failed to initialize HarfBuzz: {}", e);
                None
            },
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
            },
            Err(e) => {
                error!("Failed to initialize Blocks system: {}", e);
                None
            },
        }
    } else {
        debug!("Blocks 2.0 system disabled");
        None
    };

    // Initialize workflow engine
    #[cfg(feature = "workflow")]
    let workflow_engine = if config.enable_workflows {
        match initialize_workflow_engine(&config.config_dir).await {
            Ok(engine) => {
                info!("✓ Workflow engine initialized");
                Some(Arc::new(engine))
            },
            Err(e) => {
                error!("Failed to initialize workflow engine: {}", e);
                None
            },
        }
    } else {
        debug!("Workflow engine disabled");
        None
    };

    // Initialize plugin manager
    #[cfg(feature = "plugins")]
    let plugin_manager = if config.enable_plugins {
        let plugins_dir = config.data_dir.join("plugins");
        match initialize_plugin_manager(plugins_dir).await {
            Ok(manager) => {
                info!("✓ Plugin system initialized");
                Some(Arc::new(manager))
            },
            Err(e) => {
                error!("Failed to initialize plugin system: {}", e);
                None
            },
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
async fn initialize_workflow_engine(config_dir: &PathBuf) -> Result<WorkflowEngine> {
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
                    },
                    Err(e) => {
                        warn!("Failed to load workflow {:?}: {}", path, e);
                    },
                }
            }
        }

        info!("Loaded {} workflows", count);
    } else {
        debug!("No workflows directory found, creating it");
        tokio::fs::create_dir_all(&workflows_dir).await?;
    }

    Ok(engine)
}

/// Initialize plugin manager
#[cfg(feature = "plugins")]
async fn initialize_plugin_manager(plugins_dir: PathBuf) -> Result<PluginManager> {
    // Create plugins directory if it doesn't exist
    tokio::fs::create_dir_all(&plugins_dir).await?;

    // Create plugin host
    let host = Arc::new(TerminalPluginHost::new());

    // Create plugin manager with host
    let manager = PluginManager::with_host(plugins_dir.clone(), Some(host))
        .context("Failed to create plugin manager")?;

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
        },
        Err(e) => {
            warn!("Failed to discover plugins: {}", e);
        },
    }

    Ok(manager)
}

/// Terminal plugin host implementation
#[cfg(feature = "plugins")]
struct TerminalPluginHost;

#[cfg(feature = "plugins")]
impl TerminalPluginHost {
    fn new() -> Self {
        Self
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
        std::fs::read(path).map_err(|e| PluginError::IoError(e))
    }

    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), PluginError> {
        std::fs::write(path, data).map_err(|e| PluginError::IoError(e))
    }

    fn execute_command(&self, command: &str) -> Result<CommandOutput, PluginError> {
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

    fn get_terminal_state(&self) -> TerminalState {
        TerminalState {
            current_dir: std::env::current_dir().unwrap_or_default().to_string_lossy().to_string(),
            environment: std::env::vars().collect(),
            shell: std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string()),
            terminal_size: (80, 24),
            is_interactive: true,
            command_history: vec![],
        }
    }

    fn show_notification(&self, notification: Notification) -> Result<(), PluginError> {
        info!("[Notification] {}: {}", notification.title, notification.body);
        Ok(())
    }

    fn store_data(&self, _key: &str, _value: &[u8]) -> Result<(), PluginError> {
        // TODO: Implement persistent storage
        Ok(())
    }

    fn retrieve_data(&self, _key: &str) -> Result<Option<Vec<u8>>, PluginError> {
        // TODO: Implement persistent storage
        Ok(None)
    }

    fn register_command(&self, command: CommandDefinition) -> Result<(), PluginError> {
        debug!("Registered command: {} - {}", command.name, command.description);
        Ok(())
    }

    fn subscribe_events(&self, events: Vec<String>) -> Result<(), PluginError> {
        debug!("Subscribed to events: {:?}", events);
        Ok(())
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
                shell: Some(crate::blocks_v2::ShellType::from_str(shell)),
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

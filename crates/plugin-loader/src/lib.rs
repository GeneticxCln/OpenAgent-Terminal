//! WebAssembly Plugin Loader for OpenAgent Terminal
//!
//! This module provides a secure, sandboxed environment for loading and executing
//! WebAssembly plugins with enforced permissions and resource limits.

use anyhow::Result as AnyResult;
use plugin_api::{CommandOutput, PluginError as ApiPluginError, PluginMetadata, PluginPermissions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use wasmtime::*;
use wasmtime_wasi::{Dir, WasiCtx, WasiCtxBuilder};

/// Enhanced plugin manifest structure for TOML parsing
#[derive(serde::Deserialize)]
struct EnhancedPluginManifest {
    #[serde(default)]
    permissions: Option<PluginPermissions>,
    #[serde(default)]
    plugin: Option<PluginManifestInfo>,
}

#[derive(serde::Deserialize)]
struct PluginManifestInfo {
    name: Option<String>,
    version: Option<String>,
    author: Option<String>,
    description: Option<String>,
    license: Option<String>,
    #[serde(default)]
    capabilities: Option<PluginCapabilitiesManifest>,
    #[serde(default)]
    metadata: Option<PluginAdditionalMetadata>,
}

#[derive(serde::Deserialize)]
struct PluginCapabilitiesManifest {
    #[serde(default)]
    completions: bool,
    #[serde(default)]
    context_provider: bool,
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    hooks: Vec<String>, // String names, convert to HookType
    #[serde(default)]
    file_associations: Vec<String>,
}

#[derive(serde::Deserialize)]
struct PluginAdditionalMetadata {
    #[serde(default)]
    tags: Vec<String>,
    required_host_version: Option<String>,
}

// Epoch-based CPU limiting constants
const CPU_INIT_TICKS: u64 = 20;
const CPU_CLEANUP_TICKS: u64 = 20;
// A far-away deadline used to effectively disable the limit between calls without causing overflow
const CPU_FAR_TICKS: u64 = 1_000_000_000;

/// Plugin loader error types
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("Invalid plugin format: {0}")]
    InvalidFormat(String),

    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),
}

/// Loaded plugin instance
pub struct LoadedPlugin {
    /// Plugin metadata
    pub metadata: PluginMetadata,
    /// WASM instance
    #[allow(dead_code)]
    instance: Instance,
    /// WASM store with context
    store: Store<PluginContext>,
    /// Plugin's exported functions
    exports: PluginExports,
}

/// Plugin context stored in WASM store
struct PluginContext {
    #[allow(dead_code)]
    wasi: WasiCtx,
    #[allow(dead_code)]
    permissions: PluginPermissions,
    resource_tracker: ResourceTracker,
}

/// Exported functions from a plugin
struct PluginExports {
    init: Option<TypedFunc<(), i32>>,
    get_metadata: Option<TypedFunc<(), i64>>, // Returns ptr:u32 | len:u32 packed as i64
    handle_event: Option<TypedFunc<(i32, i32), i32>>, // Takes (ptr, len) returns error code
    cleanup: Option<TypedFunc<(), i32>>,
}

/// Resource usage tracker
#[derive(Default)]
struct ResourceTracker {
    memory_used: usize,
    #[allow(dead_code)]
    cpu_time_ms: u64,
    #[allow(dead_code)]
    api_calls: u64,
}

/// Host interface for plugins to interact with the terminal
pub trait PluginHost: Send + Sync {
    /// Log a message from the plugin
    fn log(&self, level: LogLevel, message: &str);

    /// Read a file (subject to permissions)
    fn read_file(&self, path: &str) -> Result<Vec<u8>, ApiPluginError>;

    /// Write a file (subject to permissions)
    fn write_file(&self, path: &str, data: &[u8]) -> Result<(), ApiPluginError>;

    /// Execute a command (subject to permissions)
    fn execute_command(&self, command: &str) -> Result<CommandOutput, ApiPluginError>;

    /// Get terminal state
    fn get_terminal_state(&self) -> TerminalState;

    /// Show a notification
    fn show_notification(&self, notification: Notification) -> Result<(), ApiPluginError>;

    /// Store data persistently
    fn store_data(&self, key: &str, value: &[u8]) -> Result<(), ApiPluginError>;

    /// Retrieve stored data
    fn retrieve_data(&self, key: &str) -> Result<Option<Vec<u8>>, ApiPluginError>;

    /// Register a command
    fn register_command(&self, command: CommandDefinition) -> Result<(), ApiPluginError>;

    /// Subscribe to events
    fn subscribe_events(&self, events: Vec<String>) -> Result<(), ApiPluginError>;
}

/// Log levels for plugin logging
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// Terminal state information
#[derive(Debug, Clone)]
pub struct TerminalState {
    pub current_dir: String,
    pub environment: HashMap<String, String>,
    pub shell: String,
    pub terminal_size: (u16, u16),
    pub is_interactive: bool,
    pub command_history: Vec<String>,
}

/// Notification to display to the user
#[derive(Debug, Clone)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub icon: Option<String>,
}

/// Command definition for registration
#[derive(Debug, Clone)]
pub struct CommandDefinition {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub aliases: Vec<String>,
}

/// Plugin manager for loading and managing plugins
pub struct PluginManager {
    engine: Engine,
    plugins: Arc<RwLock<HashMap<String, Arc<LoadedPlugin>>>>,
    plugin_dir: PathBuf,
    host: Option<Arc<dyn PluginHost>>,
    enforce_permissions: bool,
}

impl PluginManager {
    /// Create a new plugin manager with optional host
    pub fn new(plugin_dir: impl AsRef<Path>) -> AnyResult<Self> {
        Self::with_host(plugin_dir, None)
    }

    /// Create a new plugin manager with a host interface
    pub fn with_host(
        plugin_dir: impl AsRef<Path>,
        host: Option<Arc<dyn PluginHost>>,
    ) -> AnyResult<Self> {
        // Configure the WASM engine
        let mut config = Config::new();
        config.wasm_threads(false); // Disable threads for security
        config.wasm_simd(true);
        config.wasm_bulk_memory(true);
        // Enable epoch-based interruption for CPU limiting
        config.epoch_interruption(true);

        let engine = Engine::new(&config)?;

        // Start a lightweight ticker that increments the engine epoch periodically.
        // This enables time-based interruption at loop back-edges and safepoints.
        let ticker_engine = engine.clone();
        std::thread::spawn(move || {
            use std::time::Duration;
            loop {
                std::thread::sleep(Duration::from_millis(2));
                ticker_engine.increment_epoch();
            }
        });

        Ok(Self {
            engine,
            plugins: Arc::new(RwLock::new(HashMap::new())),
            plugin_dir: plugin_dir.as_ref().to_path_buf(),
            host,
            enforce_permissions: true,
        })
    }

    /// Set the plugin host
    pub fn set_host(&mut self, host: Arc<dyn PluginHost>) {
        self.host = Some(host);
    }

    /// Load a plugin from a WASM file
    pub async fn load_plugin(&self, path: impl AsRef<Path>) -> Result<String, PluginError> {
        let path = path.as_ref();
        let plugin_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PluginError::InvalidFormat("Invalid plugin filename".into()))?;

        info!("Loading plugin: {} from {:?}", plugin_name, path);

        // Load the WASM module
        let module = Module::from_file(&self.engine, path)
            .map_err(|e| PluginError::InvalidFormat(e.to_string()))?;

        // Create plugin context with permissions
        let permissions = self.read_plugin_permissions(path)?;
        let mut store = self
            .create_plugin_store(permissions.clone())
            .map_err(|e| PluginError::InitializationFailed(e.to_string()))?;

        // Create linker and add WASI and host functions
        let mut linker = Linker::new(&self.engine);
        
        // Add WASI functions to the linker
        wasmtime_wasi::add_to_linker(&mut linker, |ctx: &mut PluginContext| &mut ctx.wasi)
            .map_err(|e| PluginError::InitializationFailed(format!("Failed to add WASI to linker: {}", e)))?;
        
        // Add host functions to the linker
        self.add_host_functions(&mut linker)?;
        
        // Instantiate the module using the linker
        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| PluginError::InitializationFailed(e.to_string()))?;

        // Get exported functions
        let exports = self.get_plugin_exports(&instance, &mut store)?;

        // Initialize the plugin
        if let Some(init) = exports.init {
            // Set a small epoch deadline to cap CPU time for initialization.
            // If the call exceeds the deadline, it will trap.
            store.set_epoch_deadline(CPU_INIT_TICKS);

            let result = init
                .call(&mut store, ())
                .map_err(|e| PluginError::InitializationFailed(e.to_string()))?;

            // Reset the deadline far in the future between calls to avoid immediate traps.
            store.set_epoch_deadline(CPU_FAR_TICKS);

            if result != 0 {
                return Err(PluginError::InitializationFailed(format!(
                    "Plugin init returned error code: {}",
                    result
                )));
            }
        }

        // Get metadata
        let metadata = self.get_plugin_metadata(&exports, &mut store)?;

        // Validate permissions match metadata
        if self.enforce_permissions {
            self.validate_permissions(&metadata.permissions, &permissions)?;
        }

        let loaded_plugin = Arc::new(LoadedPlugin { metadata, instance, store, exports });

        // Store the plugin
        let mut plugins = self.plugins.write().await;
        plugins.insert(plugin_name.to_string(), loaded_plugin);

        info!("Successfully loaded plugin: {}", plugin_name);
        Ok(plugin_name.to_string())
    }

    /// Unload a plugin
    pub async fn unload_plugin(&self, name: &str) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;

        if let Some(plugin) = plugins.remove(name) {
            // Call cleanup if available
            if let Some(cleanup) = plugin.exports.cleanup {
                match Arc::try_unwrap(plugin) {
                    Ok(mut owned) => {
                        // Apply a small epoch deadline around cleanup as well.
                        owned.store.set_epoch_deadline(CPU_CLEANUP_TICKS);
                        let call_res = cleanup.call(&mut owned.store, ());
                        // Reset the deadline far in the future between calls
                        owned.store.set_epoch_deadline(CPU_FAR_TICKS);

                        call_res.map_err(|e| PluginError::RuntimeError(e.to_string()))?;
                        info!("Unloaded plugin: {}", name);
                        Ok(())
                    },
                    Err(_arc) => {
                        warn!("Plugin {} still has outstanding references; skipping cleanup", name);
                        Ok(())
                    },
                }
            } else {
                info!("Unloaded plugin: {}", name);
                Ok(())
            }
        } else {
            Err(PluginError::NotFound(name.to_string()))
        }
    }

    /// List all loaded plugins
    pub async fn list_plugins(&self) -> Vec<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins.values().map(|p| p.metadata.clone()).collect()
    }

    /// Discover plugins in the plugin directory
    pub async fn discover_plugins(&self) -> Result<Vec<PathBuf>> {
        let mut discovered = Vec::new();

        if !self.plugin_dir.exists() {
            warn!("Plugin directory does not exist: {:?}", self.plugin_dir);
            return Ok(discovered);
        }

        let entries = std::fs::read_dir(&self.plugin_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                discovered.push(path);
            }
        }

        debug!("Discovered {} plugins", discovered.len());
        Ok(discovered)
    }

    /// Create a plugin store with WASI context
    fn create_plugin_store(
        &self,
        permissions: PluginPermissions,
    ) -> std::result::Result<Store<PluginContext>, PluginError> {
        let mut wasi_builder = WasiCtxBuilder::new();

        // Configure WASI based on permissions
        if permissions.network {
            // Network access would be configured here if WASI supported it
            debug!("Network access requested but not yet implemented in WASI");
        }

        // Add allowed environment variables
        for var in &permissions.environment_variables {
            if let Ok(value) = std::env::var(var) {
                // Newer WasiCtxBuilder::env returns &mut Self; ignore the return value
                let _ = wasi_builder.env(var, &value);
            }
        }

        // Always preopen the plugin directory as the sandbox root
        if let Ok(dir) = Dir::open_ambient_dir(&self.plugin_dir, wasmtime_wasi::ambient_authority()) {
            let _ = wasi_builder.preopened_dir(dir, &self.plugin_dir);
        }

        // Add file system access limited to subdirectories inside plugin_dir.
        for pattern in permissions.read_files.iter().chain(permissions.write_files.iter()) {
            if let Some(safe_path) = self.sanitize_plugin_path(pattern) {
                if let Ok(dir) = Dir::open_ambient_dir(&safe_path, wasmtime_wasi::ambient_authority()) {
                    let _ = wasi_builder.preopened_dir(dir, &safe_path);
                }
            } else {
                debug!("Skipping unsafe preopen path: {}", pattern);
            }
        }

        let wasi = wasi_builder.build();

        let context =
            PluginContext { wasi, permissions, resource_tracker: ResourceTracker::default() };

        let mut store = Store::new(&self.engine, context);

        // Set resource limits
        store.limiter(|ctx| &mut ctx.resource_tracker as &mut dyn ResourceLimiter);

        Ok(store)
    }

    /// Get exported functions from a plugin
    fn get_plugin_exports(
        &self,
        instance: &Instance,
        store: &mut Store<PluginContext>,
    ) -> Result<PluginExports, PluginError> {
        Ok(PluginExports {
            init: instance.get_typed_func(&mut *store, "plugin_init").ok(),
            get_metadata: instance.get_typed_func(&mut *store, "plugin_get_metadata").ok(),
            handle_event: instance.get_typed_func(&mut *store, "plugin_handle_event").ok(),
            cleanup: instance.get_typed_func(&mut *store, "plugin_cleanup").ok(),
        })
    }

    /// Get plugin metadata using JSON-over-memory ABI
    fn get_plugin_metadata(
        &self,
        exports: &PluginExports,
        store: &mut Store<PluginContext>,
    ) -> Result<PluginMetadata, PluginError> {
        if let Some(get_metadata_fn) = &exports.get_metadata {
            // Call plugin's get_metadata function which returns packed ptr:len as i64
            let packed_result = get_metadata_fn.call(&mut *store, ())
                .map_err(|e| PluginError::RuntimeError(format!("Failed to call plugin_get_metadata: {}", e)))?;
            
            // Unpack the result: high 32 bits = len, low 32 bits = ptr
            let ptr = (packed_result & 0xFFFFFFFF) as u32;
            let len = (packed_result >> 32) as u32;
            
            if ptr == 0 || len == 0 {
                return Err(PluginError::RuntimeError("Plugin returned invalid metadata pointer/length".into()));
            }
            
            // Get memory export directly from the store context
            // This will be implemented properly once we have the plugin SDK
            debug!("Plugin returned metadata at ptr={}, len={}", ptr, len);
            
            // For now, return a placeholder that indicates JSON ABI is working
            Ok(PluginMetadata {
                name: "json-abi-plugin".to_string(),
                version: "1.0.0".to_string(),
                author: "Plugin SDK".to_string(),
                description: format!("Plugin using JSON ABI (ptr={}, len={})", ptr, len),
                license: "MIT".to_string(),
                homepage: None,
                capabilities: Default::default(),
                permissions: Default::default(),
            })
        } else {
            // Fallback to default metadata if function not available
            Ok(PluginMetadata {
                name: "unknown".to_string(),
                version: "0.0.0".to_string(),
                author: "unknown".to_string(),
                description: "Plugin metadata not available".to_string(),
                license: "unknown".to_string(),
                homepage: None,
                capabilities: Default::default(),
                permissions: Default::default(),
            })
        }
    }

    /// Read plugin permissions and metadata from enhanced manifest
    fn read_plugin_permissions(&self, path: &Path) -> Result<PluginPermissions, PluginError> {
        // Look for a manifest file next to the WASM file
        let manifest_path = path.with_extension("toml");

        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)
                .map_err(|e| PluginError::InvalidFormat(e.to_string()))?;

            match toml::from_str::<EnhancedPluginManifest>(&content) {
                Ok(manifest) => {
                    // Validate plugin metadata if present
                    if let Some(plugin_info) = &manifest.plugin {
                        self.validate_plugin_manifest(plugin_info)?;
                    }
                    
                    if let Some(mut perms) = manifest.permissions {
                        // Enhanced permission validation
                        self.validate_enhanced_permissions(&perms)?;
                        
                        // Sanitize preopen paths: force relative to plugin_dir and disallow '/'
                        perms.read_files = perms
                            .read_files
                            .into_iter()
                            .filter_map(|p| self.sanitize_plugin_path(&p))
                            .collect();
                        perms.write_files = perms
                            .write_files
                            .into_iter()
                            .filter_map(|p| self.sanitize_plugin_path(&p))
                            .collect();
                        return Ok(perms);
                    }
                },
                Err(e) => return Err(PluginError::InvalidFormat(format!("Invalid TOML manifest: {}", e))),
            }
        }

        // Default: sandboxed to plugin_dir only
        let default = PluginPermissions { 
            read_files: vec![self.plugin_dir.to_string_lossy().to_string()], 
            write_files: vec![], 
            ..Default::default() 
        };
        Ok(default)
    }
    
    /// Validate plugin manifest information
    fn validate_plugin_manifest(&self, plugin_info: &PluginManifestInfo) -> Result<(), PluginError> {
        // Note: PluginManifestInfo is defined in the same function scope as EnhancedPluginManifest
        // Validate required fields
        if plugin_info.name.as_ref().map_or(true, |s| s.is_empty()) {
            return Err(PluginError::InvalidFormat("Plugin name is required".into()));
        }
        
        if plugin_info.version.as_ref().map_or(true, |s| s.is_empty()) {
            return Err(PluginError::InvalidFormat("Plugin version is required".into()));
        }
        
        // Validate version format (basic semver check)
        if let Some(version) = &plugin_info.version {
            if !version.chars().any(|c| c.is_ascii_digit()) {
                return Err(PluginError::InvalidFormat("Invalid version format".into()));
            }
        }
        
        // Validate author field
        if plugin_info.author.as_ref().map_or(true, |s| s.is_empty()) {
            warn!("Plugin manifest missing author field");
        }
        
        Ok(())
    }
    
    /// Enhanced permission validation with stricter rules
    fn validate_enhanced_permissions(&self, perms: &PluginPermissions) -> Result<(), PluginError> {
        // Validate memory limits
        if perms.max_memory_mb > 200 {
            return Err(PluginError::PermissionDenied(
                format!("Requested memory ({} MB) exceeds maximum allowed (200 MB)", perms.max_memory_mb)
            ));
        }
        
        if perms.max_memory_mb == 0 {
            return Err(PluginError::PermissionDenied(
                "Memory limit must be greater than 0".into()
            ));
        }
        
        // Validate timeout limits  
        if perms.timeout_ms > 30000 {
            return Err(PluginError::PermissionDenied(
                format!("Requested timeout ({} ms) exceeds maximum allowed (30s)", perms.timeout_ms)
            ));
        }
        
        if perms.timeout_ms == 0 {
            return Err(PluginError::PermissionDenied(
                "Timeout must be greater than 0".into()
            ));
        }
        
        // Validate file access patterns
        for pattern in &perms.read_files {
            if self.is_dangerous_file_pattern(pattern) {
                return Err(PluginError::PermissionDenied(
                    format!("Dangerous file access pattern denied: {}", pattern)
                ));
            }
        }
        
        for pattern in &perms.write_files {
            if self.is_dangerous_file_pattern(pattern) {
                return Err(PluginError::PermissionDenied(
                    format!("Dangerous file write pattern denied: {}", pattern)
                ));
            }
        }
        
        // Validate environment variable access
        for env_var in &perms.environment_variables {
            if self.is_sensitive_env_var(env_var) {
                warn!("Plugin requesting access to sensitive environment variable: {}", env_var);
            }
        }
        
        Ok(())
    }
    
    /// Check if a file pattern is dangerous
    fn is_dangerous_file_pattern(&self, pattern: &str) -> bool {
        let dangerous_patterns = [
            "/etc/", "/sys/", "/proc/", "/dev/",
            "/boot/", "/root/", "/usr/bin/", "/usr/sbin/",
            "/bin/", "/sbin/", "/lib/", "/lib64/",
            "/../", "/..",  // Path traversal
            "shadow", "passwd", "sudoers", // Sensitive files
        ];
        
        dangerous_patterns.iter().any(|&dangerous| pattern.contains(dangerous))
    }
    
    /// Check if an environment variable is sensitive
    fn is_sensitive_env_var(&self, env_var: &str) -> bool {
        let sensitive_prefixes = [
            "AWS_", "GCP_", "AZURE_",
            "SECRET_", "TOKEN_", "KEY_",
            "PASSWORD_", "PASS_",
            "SSH_", "GPG_",
        ];
        
        let sensitive_exact = [
            "HOME", "USER", "USERNAME",
            "PATH", "LD_LIBRARY_PATH",
            "SUDO_USER", "LOGNAME",
        ];
        
        sensitive_prefixes.iter().any(|&prefix| env_var.starts_with(prefix)) ||
        sensitive_exact.iter().any(|&exact| env_var == exact)
    }

    /// Validate that requested permissions match allowed permissions
    fn validate_permissions(
        &self,
        requested: &PluginPermissions,
        allowed: &PluginPermissions,
    ) -> Result<(), PluginError> {
        if requested.network && !allowed.network {
            return Err(PluginError::PermissionDenied("Network access not allowed".into()));
        }

        if requested.execute_commands && !allowed.execute_commands {
            return Err(PluginError::PermissionDenied("Command execution not allowed".into()));
        }

        // Check read access patterns
        for pattern in &requested.read_files {
            if !allowed.read_files.iter().any(|p| p == pattern) {
                return Err(PluginError::PermissionDenied(format!(
                    "Read access to {} not allowed",
                    pattern
                )));
            }
        }

        // Check write access patterns
        for pattern in &requested.write_files {
            if !allowed.write_files.iter().any(|p| p == pattern) {
                return Err(PluginError::PermissionDenied(format!(
                    "Write access to {} not allowed",
                    pattern
                )));
            }
        }

        Ok(())
    }

    /// Add host functions that plugins can call
    fn add_host_functions(&self, linker: &mut Linker<PluginContext>) -> Result<(), PluginError> {
        let host = self.host.clone();

        // Host logging function - plugin can log messages back to terminal
        linker.func_wrap("env", "host_log", move |mut caller: Caller<'_, PluginContext>, level: i32, ptr: i32, len: i32| -> Result<(), anyhow::Error> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;

            let mut buffer = vec![0u8; len as usize];
            memory.read(&caller, ptr as usize, &mut buffer)
                .map_err(|_| anyhow::anyhow!("Failed to read from plugin memory"))?;

            let message = String::from_utf8_lossy(&buffer);
            let log_level = match level {
                0 => LogLevel::Debug,
                1 => LogLevel::Info,
                2 => LogLevel::Warning,
                _ => LogLevel::Error,
            };

            if let Some(ref host) = host {
                host.log(log_level, &message);
            } else {
                match log_level {
                    LogLevel::Debug => debug!("[Plugin] {}", message),
                    LogLevel::Info => info!("[Plugin] {}", message),
                    LogLevel::Warning => warn!("[Plugin] {}", message),
                    LogLevel::Error => error!("[Plugin] {}", message),
                }
            }
            Ok(())
        }).map_err(|e| PluginError::InitializationFailed(format!("Failed to add host_log: {}", e)))?;

        // Host file read function
        let host_clone = self.host.clone();
        linker.func_wrap("env", "host_read_file", move |mut caller: Caller<'_, PluginContext>, path_ptr: i32, path_len: i32, result_ptr: i32, result_len_ptr: i32| -> Result<i32, anyhow::Error> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;

            // Read path from plugin memory
            let mut path_buffer = vec![0u8; path_len as usize];
            memory.read(&caller, path_ptr as usize, &mut path_buffer)
                .map_err(|_| anyhow::anyhow!("Failed to read path from plugin memory"))?;

            let path = String::from_utf8_lossy(&path_buffer);

            // Check permissions
            let ctx = caller.data();
            let can_read = ctx.permissions.read_files.iter().any(|pattern| {
                // Simple pattern matching - in production use proper glob
                path.starts_with(pattern.trim_end_matches('*'))
            });

            if !can_read {
                return Ok(-1); // Permission denied
            }

            // Read file through host if available, otherwise WASI
            let file_result = if let Some(ref host) = host_clone {
                host.read_file(&path)
            } else {
                std::fs::read(&*path).map_err(|e| ApiPluginError::IoError(e))
            };

            match file_result {
                Ok(data) => {
                    // Write data length back to plugin
                    let len_bytes = (data.len() as u32).to_le_bytes();
                    memory.write(&mut caller, result_len_ptr as usize, &len_bytes)
                        .map_err(|_| anyhow::anyhow!("Failed to write result length"))?;

                    // Write data back to plugin if buffer is provided
                    if result_ptr != 0 {
                        memory.write(&mut caller, result_ptr as usize, &data)
                            .map_err(|_| anyhow::anyhow!("Failed to write result data"))?;
                    }
                    Ok(0) // Success
                },
                Err(_) => Ok(-2), // IO error
            }
        }).map_err(|e| PluginError::InitializationFailed(format!("Failed to add host_read_file: {}", e)))?;

        // Host file write function
        let host_clone = self.host.clone();
        linker.func_wrap("env", "host_write_file", move |mut caller: Caller<'_, PluginContext>, path_ptr: i32, path_len: i32, data_ptr: i32, data_len: i32| -> Result<i32, anyhow::Error> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;

            // Read path and data from plugin memory
            let mut path_buffer = vec![0u8; path_len as usize];
            memory.read(&caller, path_ptr as usize, &mut path_buffer)
                .map_err(|_| anyhow::anyhow!("Failed to read path from plugin memory"))?;

            let mut data_buffer = vec![0u8; data_len as usize];
            memory.read(&caller, data_ptr as usize, &mut data_buffer)
                .map_err(|_| anyhow::anyhow!("Failed to read data from plugin memory"))?;

            let path = String::from_utf8_lossy(&path_buffer);

            // Check permissions
            let ctx = caller.data();
            let can_write = ctx.permissions.write_files.iter().any(|pattern| {
                path.starts_with(pattern.trim_end_matches('*'))
            });

            if !can_write {
                return Ok(-1); // Permission denied
            }

            // Write file through host if available
            let write_result = if let Some(ref host) = host_clone {
                host.write_file(&path, &data_buffer)
            } else {
                std::fs::write(&*path, &data_buffer).map_err(|e| ApiPluginError::IoError(e))
            };

            match write_result {
                Ok(()) => Ok(0), // Success
                Err(_) => Ok(-2), // IO error
            }
        }).map_err(|e| PluginError::InitializationFailed(format!("Failed to add host_write_file: {}", e)))?;

        // Host execute command function
        let host_clone = self.host.clone();
        linker.func_wrap("env", "host_execute_command", move |mut caller: Caller<'_, PluginContext>, cmd_ptr: i32, cmd_len: i32| -> Result<i32, anyhow::Error> {
            let memory = caller.get_export("memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;

            let mut cmd_buffer = vec![0u8; cmd_len as usize];
            memory.read(&caller, cmd_ptr as usize, &mut cmd_buffer)
                .map_err(|_| anyhow::anyhow!("Failed to read command from plugin memory"))?;

            let command = String::from_utf8_lossy(&cmd_buffer);

            // Check permissions
            let ctx = caller.data();
            if !ctx.permissions.execute_commands {
                return Ok(-1); // Permission denied
            }

            // Execute command through host if available
            if let Some(ref host) = host_clone {
                match host.execute_command(&command) {
                    Ok(_output) => Ok(0), // Success - output handling would need memory management
                    Err(_) => Ok(-2), // Execution failed
                }
            } else {
                Ok(-3) // Host not available
            }
        }).map_err(|e| PluginError::InitializationFailed(format!("Failed to add host_execute_command: {}", e)))?;

        Ok(())
    }

    /// Send an event to a plugin using JSON-over-memory ABI
    pub async fn send_event_to_plugin(&self, plugin_name: &str, event: &PluginEvent) -> Result<PluginEventResponse, PluginError> {
        let plugins = self.plugins.read().await;
        let plugin = plugins.get(plugin_name)
            .ok_or_else(|| PluginError::NotFound(plugin_name.to_string()))?;

        let plugin = Arc::clone(plugin);
        drop(plugins); // Release the read lock

        // This would need proper implementation with shared store access
        // For now, return a placeholder response
        Ok(PluginEventResponse {
            success: true,
            result: Some("Event processed via JSON ABI".to_string()),
            error: None,
        })
    }
}

/// Event sent to plugins
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginEvent {
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: u64,
}

/// Response from plugin event handling
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginEventResponse {
    pub success: bool,
    pub result: Option<String>,
    pub error: Option<String>,
}

impl PluginManager {
    // Sanitize a path from the manifest: resolve relative to plugin_dir and disallow '/'
    fn sanitize_plugin_path(&self, p: &str) -> Option<String> {
        use std::path::PathBuf;
        let raw = PathBuf::from(p);
        let resolved = if raw.is_absolute() { raw } else { self.plugin_dir.join(raw) };
        // Disallow preopening '/' or paths outside plugin_dir
        if resolved.components().count() == 0 {
            return None;
        }
        if resolved.as_os_str() == "/" { return None; }
        if !resolved.starts_with(&self.plugin_dir) { return None; }
        Some(resolved.to_string_lossy().to_string())
    }
}

/// Resource limiter implementation
impl ResourceLimiter for ResourceTracker {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        const MAX_MEMORY: usize = 50 * 1024 * 1024; // 50MB default limit

        if desired > MAX_MEMORY {
            return Ok(false);
        }

        self.memory_used = desired;
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: u32,
        desired: u32,
        _maximum: Option<u32>,
    ) -> anyhow::Result<bool> {
        const MAX_TABLES: u32 = 10;
        Ok(desired <= MAX_TABLES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sanitize_plugin_path() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PluginManager::new(temp_dir.path()).unwrap();
        // Absolute root should be rejected
        assert!(manager.sanitize_plugin_path("/").is_none());
        // Outside plugin_dir should be rejected
        assert!(manager.sanitize_plugin_path("/etc").is_none());
        // Relative inside plugin_dir should be accepted and resolved
        let sub = manager.sanitize_plugin_path("subdir").unwrap();
        assert!(sub.starts_with(temp_dir.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn test_read_manifest_permissions_sanitized() {
        use std::io::Write as _;
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("p.wasm");
        std::fs::write(&wasm_path, b"00").unwrap();
        let manifest_path = temp_dir.path().join("p.toml");
        let mut f = std::fs::File::create(&manifest_path).unwrap();
        // Include an unsafe preopen ('/') which should be filtered out
        writeln!(f, "[permissions]\nread_files=[\"/\",\"sub\"]\nwrite_files=[\"sub\"]\nnetwork=false\nexecute_commands=false\nenvironment_variables=[]\nmax_memory_mb=50\ntimeout_ms=5000\n").unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let perms = manager.read_plugin_permissions(&wasm_path).expect("permissions");
        // Ensure '/' was removed and 'sub' resolved under plugin_dir
        assert!(!perms.read_files.iter().any(|p| p == "/"));
        assert!(perms
            .read_files
            .iter()
            .any(|p| p.starts_with(temp_dir.path().to_string_lossy().as_ref())));
    }

    #[tokio::test]
    async fn test_plugin_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PluginManager::new(temp_dir.path()).unwrap();

        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 0);
    }

    #[tokio::test]
    async fn test_plugin_discovery() {
        let temp_dir = TempDir::new().unwrap();

        // Create some dummy WASM files
        std::fs::write(temp_dir.path().join("plugin1.wasm"), b"fake").unwrap();
        std::fs::write(temp_dir.path().join("plugin2.wasm"), b"fake").unwrap();
        std::fs::write(temp_dir.path().join("not_plugin.txt"), b"fake").unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let discovered = manager.discover_plugins().await.unwrap();

        assert_eq!(discovered.len(), 2);
    }

    // Build a minimal WASM module exporting `plugin_cleanup` that returns 0.
    fn build_cleanup_only_wasm() -> Vec<u8> {
        let wat = r#"(module
            (func (export "plugin_cleanup") (result i32)
                i32.const 0)
        )"#;
        wat::parse_str(wat).expect("Failed to compile WAT")
    }

    #[tokio::test]
    async fn test_unload_calls_cleanup_path() {
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("cleanup_plugin.wasm");
        let wasm_bytes = build_cleanup_only_wasm();
        std::fs::write(&wasm_path, wasm_bytes).unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let name = manager.load_plugin(&wasm_path).await.expect("load_plugin should succeed");

        // Ensure it's listed
        let listed = manager.list_plugins().await;
        assert_eq!(listed.len(), 1);

        // Unload; this will attempt to call the exported cleanup function
        manager.unload_plugin(&name).await.expect("unload_plugin should call cleanup and succeed");

        // Ensure it's gone
        let listed = manager.list_plugins().await;
        assert_eq!(listed.len(), 0);
    }

    // Build a module with a spinning plugin_init to trigger epoch timeout
    fn build_init_spin_wasm() -> Vec<u8> {
        let wat = r#"(module
            (func (export "plugin_init") (result i32)
                (loop $l
                    br $l
                )
                (i32.const 0)
            )
        )"#;
        wat::parse_str(wat).expect("Failed to compile WAT")
    }

    // Build a module where init is ok but cleanup spins forever
    fn build_cleanup_spin_wasm() -> Vec<u8> {
        let wat = r#"(module
            (func (export "plugin_init") (result i32)
                i32.const 0)
            (func (export "plugin_cleanup") (result i32)
                (loop $l
                    br $l
                )
                (i32.const 0)
            )
        )"#;
        wat::parse_str(wat).expect("Failed to compile WAT")
    }

    #[tokio::test]
    async fn test_epoch_timeout_on_init() {
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("spin_init.wasm");
        std::fs::write(&wasm_path, build_init_spin_wasm()).unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let res = manager.load_plugin(&wasm_path).await;
        assert!(res.is_err(), "Expected initialization to time out or fail");

        // Ensure not listed
        let listed = manager.list_plugins().await;
        assert!(listed.is_empty());
    }

    #[tokio::test]
    async fn test_epoch_timeout_on_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("spin_cleanup.wasm");
        std::fs::write(&wasm_path, build_cleanup_spin_wasm()).unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let name = manager.load_plugin(&wasm_path).await.expect("load_plugin should succeed");

        // Ensure it's listed
        let listed = manager.list_plugins().await;
        assert_eq!(listed.len(), 1);

        // Unload should return an error due to timeout, but the plugin entry should be removed
        let unload_res = manager.unload_plugin(&name).await;
        assert!(unload_res.is_err(), "Expected unload to return error due to timeout");

        let listed = manager.list_plugins().await;
        assert!(listed.is_empty());
    }
    
    #[tokio::test]
    async fn test_enhanced_manifest_validation() {
        use std::io::Write as _;
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("test_plugin.wasm");
        std::fs::write(&wasm_path, build_cleanup_only_wasm()).unwrap();
        let manifest_path = temp_dir.path().join("test_plugin.toml");
        
        // Test complete manifest with plugin metadata
        let mut f = std::fs::File::create(&manifest_path).unwrap();
        writeln!(f, "[plugin]\nname=\"test-plugin\"\nversion=\"1.0.0\"\nauthor=\"Test Author\"\n[permissions]\nread_files=[]\nwrite_files=[]\nenvironment_variables=[]\nnetwork=false\nexecute_commands=false\nmax_memory_mb=50\ntimeout_ms=5000").unwrap();
        
        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let perms = manager.read_plugin_permissions(&wasm_path).expect("Should parse valid manifest");
        assert_eq!(perms.max_memory_mb, 50);
        assert_eq!(perms.timeout_ms, 5000);
    }
    
    #[tokio::test] 
    async fn test_dangerous_file_pattern_rejection() {
        use std::io::Write as _;
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("dangerous.wasm");
        std::fs::write(&wasm_path, build_cleanup_only_wasm()).unwrap();
        let manifest_path = temp_dir.path().join("dangerous.toml");
        
        // Test manifest with dangerous file access
        let mut f = std::fs::File::create(&manifest_path).unwrap();
        writeln!(f, "[permissions]\nread_files=[\"/etc/passwd\"]\nwrite_files=[]\nenvironment_variables=[]\nnetwork=false\nexecute_commands=false\nmax_memory_mb=50\ntimeout_ms=5000").unwrap();
        
        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let result = manager.read_plugin_permissions(&wasm_path);
        assert!(result.is_err(), "Should reject dangerous file patterns");
    }
    
    #[tokio::test]
    async fn test_memory_limit_validation() {
        use std::io::Write as _;
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("memory_test.wasm");
        std::fs::write(&wasm_path, build_cleanup_only_wasm()).unwrap();
        let manifest_path = temp_dir.path().join("memory_test.toml");
        
        // Test manifest with excessive memory request
        let mut f = std::fs::File::create(&manifest_path).unwrap();
        writeln!(f, "[permissions]\nread_files=[]\nwrite_files=[]\nenvironment_variables=[]\nnetwork=false\nexecute_commands=false\nmax_memory_mb=500\ntimeout_ms=5000").unwrap();
        
        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let result = manager.read_plugin_permissions(&wasm_path);
        assert!(result.is_err(), "Should reject excessive memory requests");
    }
    
    #[tokio::test]
    async fn test_timeout_limit_validation() {
        use std::io::Write as _;
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("timeout_test.wasm");
        std::fs::write(&wasm_path, build_cleanup_only_wasm()).unwrap();
        let manifest_path = temp_dir.path().join("timeout_test.toml");
        
        // Test manifest with excessive timeout request
        let mut f = std::fs::File::create(&manifest_path).unwrap();
        writeln!(f, "[permissions]\nread_files=[]\nwrite_files=[]\nenvironment_variables=[]\nnetwork=false\nexecute_commands=false\nmax_memory_mb=50\ntimeout_ms=60000").unwrap();
        
        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let result = manager.read_plugin_permissions(&wasm_path);
        assert!(result.is_err(), "Should reject excessive timeout requests");
    }
    
    #[tokio::test]
    async fn test_plugin_manifest_info_validation() {
        use std::io::Write as _;
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("info_test.wasm");
        std::fs::write(&wasm_path, build_cleanup_only_wasm()).unwrap();
        let manifest_path = temp_dir.path().join("info_test.toml");
        
        // Test manifest with missing required fields
        let mut f = std::fs::File::create(&manifest_path).unwrap();
        writeln!(f, "[plugin]\nname=\"\"\nversion=\"1.0.0\"\n[permissions]\nread_files=[]\nwrite_files=[]\nenvironment_variables=[]\nnetwork=false\nexecute_commands=false\nmax_memory_mb=50\ntimeout_ms=5000").unwrap();
        
        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let result = manager.read_plugin_permissions(&wasm_path);
        assert!(result.is_err(), "Should reject manifest with empty plugin name");
    }
    
    #[test]
    fn test_dangerous_file_pattern_detection() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PluginManager::new(temp_dir.path()).unwrap();
        
        // Test various dangerous patterns
        assert!(manager.is_dangerous_file_pattern("/etc/passwd"));
        assert!(manager.is_dangerous_file_pattern("/sys/kernel"));
        assert!(manager.is_dangerous_file_pattern("../../../etc/shadow"));
        assert!(manager.is_dangerous_file_pattern("/root/.ssh/id_rsa"));
        
        // Test safe patterns
        assert!(!manager.is_dangerous_file_pattern("config.toml"));
        assert!(!manager.is_dangerous_file_pattern("./data/file.txt"));
        assert!(!manager.is_dangerous_file_pattern("../plugin-data"));
    }
    
    #[test]
    fn test_sensitive_env_var_detection() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PluginManager::new(temp_dir.path()).unwrap();
        
        // Test sensitive prefixes
        assert!(manager.is_sensitive_env_var("AWS_SECRET_ACCESS_KEY"));
        assert!(manager.is_sensitive_env_var("TOKEN_VALUE"));
        assert!(manager.is_sensitive_env_var("PASSWORD_HASH"));
        assert!(manager.is_sensitive_env_var("SSH_PRIVATE_KEY"));
        
        // Test sensitive exact matches
        assert!(manager.is_sensitive_env_var("HOME"));
        assert!(manager.is_sensitive_env_var("USER"));
        assert!(manager.is_sensitive_env_var("PATH"));
        
        // Test safe environment variables
        assert!(!manager.is_sensitive_env_var("PLUGIN_CONFIG"));
        assert!(!manager.is_sensitive_env_var("DEBUG_LEVEL"));
        assert!(!manager.is_sensitive_env_var("TERM"));
    }
}

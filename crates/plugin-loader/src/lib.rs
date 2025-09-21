//! WebAssembly Plugin Loader for OpenAgent Terminal
//!
//! This module provides a secure, sandboxed environment for loading and executing
//! WebAssembly plugins with enforced permissions and resource limits.
#![allow(
    clippy::pedantic,
    clippy::doc_markdown,
    clippy::wildcard_imports,
    clippy::missing_errors_doc,
    clippy::unnecessary_wraps,
    clippy::default_trait_access,
    clippy::manual_let_else,
    clippy::uninlined_format_args,
    clippy::redundant_closure_for_method_calls,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::needless_raw_string_hashes,
    clippy::unused_async,
    clippy::too_many_lines,
    clippy::match_same_arms
)]

use anyhow::Result as AnyResult;
use plugin_api::{CommandOutput, PluginError as ApiPluginError, PluginMetadata, PluginPermissions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use wasmtime::*;
use wasmtime_wasi::preview1::WasiP1Ctx;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

mod security_audit;
// Security audit types available for future integration
#[allow(unused_imports)]
use security_audit::{SecurityAuditor, SecurityConfig, AccessType, SeverityLevel};

/// Enhanced plugin manifest structure for TOML parsing
#[derive(serde::Deserialize)]
struct EnhancedPluginManifest {
    #[serde(default)]
    permissions: Option<PluginPermissions>,
    #[serde(default)]
    plugin: Option<PluginManifestInfo>,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
    /// WASM store with context wrapped for safe mutation across async calls
    store: tokio::sync::Mutex<Store<PluginContext>>,
    /// Plugin's exported functions
    exports: PluginExports,
}

/// Plugin context stored in WASM store
struct PluginContext {
    #[allow(dead_code)]
    wasi: WasiP1Ctx,
    #[allow(dead_code)]
    permissions: PluginPermissions,
    /// Absolute base directory for the plugin (WASI sandbox root ".")
    plugin_base_dir: std::path::PathBuf,
    /// Stable plugin identifier (derived from filename stem)
    plugin_id: String,
    resource_tracker: ResourceTracker,
}

/// Exported functions from a plugin
struct PluginExports {
    init: Option<TypedFunc<(), i32>>,
    get_metadata: Option<TypedFunc<(), i64>>, // Returns ptr:u32 | len:u32 packed as i64
    #[allow(dead_code)]
    handle_event: Option<TypedFunc<(i32, i32), i32>>, // Takes (ptr, len) returns error code
    cleanup: Option<TypedFunc<(), i32>>,
}

/// Resource usage tracker
#[derive(Default)]
struct ResourceTracker {
    memory_used: usize,
    /// Per-plugin maximum linear memory in bytes (from manifest)
    max_memory_bytes: usize,
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

    /// Store data persistently (namespaced to the given plugin_id)
    fn store_data_for(
        &self,
        plugin_id: &str,
        key: &str,
        value: &[u8],
    ) -> Result<(), ApiPluginError>;

    /// Retrieve stored data (namespaced to the given plugin_id)
    fn retrieve_data_for(
        &self,
        plugin_id: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>, ApiPluginError>;

    /// Store a JSON document in a namespace with a document id
    fn store_document_for(
        &self,
        plugin_id: &str,
        namespace: &str,
        doc_id: &str,
        doc_json: &str,
    ) -> Result<(), ApiPluginError>;

    /// Retrieve a JSON document
    fn retrieve_document_for(
        &self,
        plugin_id: &str,
        namespace: &str,
        doc_id: &str,
    ) -> Result<Option<String>, ApiPluginError>;
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
    loaded_paths: Arc<RwLock<HashMap<String, PathBuf>>>,
    plugin_dirs: Vec<PathBuf>,
    host: Option<Arc<dyn PluginHost>>,
    enforce_permissions: bool,
    enforce_signatures: bool,
    signature_policy: Option<SignaturePolicy>,
}

impl PluginManager {
    /// Create a new plugin manager with optional host
    pub fn new(plugin_dir: impl AsRef<Path>) -> AnyResult<Self> {
        Self::with_host_and_dirs(vec![plugin_dir.as_ref().to_path_buf()], None)
    }

    /// Create a new plugin manager with a host interface
    pub fn with_host(
        plugin_dir: impl AsRef<Path>,
        host: Option<Arc<dyn PluginHost>>,
    ) -> AnyResult<Self> {
        Self::with_host_and_dirs(vec![plugin_dir.as_ref().to_path_buf()], host)
    }

    /// Create a new plugin manager with multiple plugin directories and a host interface
    pub fn with_host_and_dirs(
        plugin_dirs: Vec<PathBuf>,
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
            loaded_paths: Arc::new(RwLock::new(HashMap::new())),
            plugin_dirs,
            host,
            enforce_permissions: true,
            enforce_signatures: false,
            signature_policy: None,
        })
    }

    /// Set the plugin host
    pub fn set_host(&mut self, host: Arc<dyn PluginHost>) {
        self.host = Some(host);
    }

    /// Enforce signature verification (when true, unsigned or invalid signatures will fail to load)
    pub fn set_enforce_signatures(&mut self, enforce: bool) {
        self.enforce_signatures = enforce;
    }

    /// Add an additional plugin directory to discover from
    pub fn add_plugin_dir(&mut self, dir: impl AsRef<Path>) {
        self.plugin_dirs.push(dir.as_ref().to_path_buf());
    }

    /// Configure signature policy
    pub fn configure_signature_policy(&mut self, policy: SignaturePolicy) {
        self.signature_policy = Some(policy);
    }

    /// Load a plugin from a WASM file
    pub async fn load_plugin(&self, path: impl AsRef<Path>) -> Result<String, PluginError> {
        let path = path.as_ref();
        let plugin_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PluginError::InvalidFormat("Invalid plugin filename".into()))?;

        info!("Loading plugin: {} from {:?}", plugin_name, path);

        // Signature requirements by policy
        if let Some(policy) = &self.signature_policy {
            let parent = path.parent().unwrap_or(Path::new("."));
            let kind = policy.kind_for_dir(parent);
            let sig_path = path.with_extension("sig");
            let sig_exists = sig_path.exists();

            if (policy.require_signatures_for_all || policy.require_for_kind(kind)) && !sig_exists {
                return Err(PluginError::InvalidFormat(
                    "Signature required but not present".into(),
                ));
            }
        }

        // Verify signature and capture verifying key (if present)
        let _verified_key_hex = match self.verify_signature_and_get_key(path) {
            Ok(v) => v, // Option<String>
            Err(e) => {
                if self.enforce_signatures {
                    return Err(PluginError::InvalidFormat(format!(
                        "Signature verification failed: {}",
                        e
                    )));
                } else {
                    warn!("Signature verification skipped or failed: {}", e);
                    None
                }
            }
        };

        // Load the WASM module
        let module = Module::from_file(&self.engine, path)
            .map_err(|e| PluginError::InvalidFormat(e.to_string()))?;

        // Determine plugin base directory (sandbox root)
        let plugin_base_dir = path.parent().unwrap_or(Path::new("."));

        // Create plugin context with permissions
        let permissions = self.read_plugin_permissions(path, plugin_base_dir)?;
        let mut store = self
            .create_plugin_store(permissions.clone(), plugin_base_dir, plugin_name)
            .map_err(|e| PluginError::InitializationFailed(e.to_string()))?;

        // Create linker and add WASI and host functions
        let mut linker = Linker::new(&self.engine);

        // Add WASI functions to the linker
        wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |ctx: &mut PluginContext| {
            &mut ctx.wasi
        })
        .map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to add WASI to linker: {}", e))
        })?;

        // Add host functions to the linker
        self.add_host_functions(&mut linker)?;

        // Instantiate the module using the linker
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| PluginError::InitializationFailed(e.to_string()))?;

        // Get exported functions
        let exports = self.get_plugin_exports(&instance, &mut store)?;

        // Initialize the plugin
        if let Some(ref init) = exports.init {
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
        let metadata = self.get_plugin_metadata(&instance, &exports, &mut store)?;

        // Validate permissions match metadata
        if self.enforce_permissions {
            self.validate_permissions(&metadata.permissions, &permissions, plugin_base_dir)?;
        }

        let loaded_plugin = Arc::new(LoadedPlugin {
            metadata,
            instance,
            store: tokio::sync::Mutex::new(store),
            exports,
        });

        // Store the plugin and path
        let mut plugins = self.plugins.write().await;
        plugins.insert(plugin_name.to_string(), Arc::clone(&loaded_plugin));
        drop(plugins);
        let mut paths = self.loaded_paths.write().await;
        paths.insert(plugin_name.to_string(), path.to_path_buf());

        info!("Successfully loaded plugin: {}", plugin_name);
        Ok(plugin_name.to_string())
    }

    /// Unload a plugin
    pub async fn unload_plugin(&self, name: &str) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().await;

        if let Some(plugin) = plugins.remove(name) {
            // Call cleanup if available
            if let Some(cleanup_fn) = &plugin.exports.cleanup {
                let mut store = plugin.store.lock().await;
                // Apply a small epoch deadline around cleanup as well.
                store.set_epoch_deadline(CPU_CLEANUP_TICKS);
                let call_res = cleanup_fn.call(&mut *store, ());
                // Reset the deadline far in the future between calls
                store.set_epoch_deadline(CPU_FAR_TICKS);

                call_res.map_err(|e| PluginError::RuntimeError(e.to_string()))?;
            }
            info!("Unloaded plugin: {}", name);
            Ok(())
        } else {
            Err(PluginError::NotFound(name.to_string()))
        }
    }

    /// List all loaded plugins
    pub async fn list_plugins(&self) -> Vec<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins.values().map(|p| p.metadata.clone()).collect()
    }

    /// Return loaded plugin names and their file paths
    pub async fn loaded_names_and_paths(&self) -> Vec<(String, PathBuf)> {
        let paths = self.loaded_paths.read().await;
        paths.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    /// Discover plugins in the plugin directory
    pub async fn discover_plugins(&self) -> Result<Vec<PathBuf>> {
        let mut discovered = Vec::new();

        for dir in &self.plugin_dirs {
            if !dir.exists() {
                debug!("Plugin directory does not exist: {:?}", dir);
                continue;
            }
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                    discovered.push(path);
                }
            }
        }

        debug!("Discovered {} plugins", discovered.len());
        Ok(discovered)
    }

    /// Create a plugin store with WASI context
    fn create_plugin_store(
        &self,
        permissions: PluginPermissions,
        plugin_base_dir: &Path,
        plugin_id: &str,
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

        // Always preopen the plugin base directory as the sandbox root
        if let Err(e) =
            wasi_builder.preopened_dir(plugin_base_dir, ".", DirPerms::all(), FilePerms::all())
        {
            debug!("Failed to preopen plugin base directory: {}", e);
        }

        // Add file system access limited to subdirectories inside plugin_dir.
        for pattern in permissions.read_files.iter().chain(permissions.write_files.iter()) {
            if let Some(safe_path) = self.sanitize_plugin_path(plugin_base_dir, pattern) {
                // Determine permissions based on whether this is read or write access
                let is_write = permissions.write_files.contains(pattern);
                let dir_perms = if is_write { DirPerms::all() } else { DirPerms::READ };
                let file_perms = if is_write { FilePerms::all() } else { FilePerms::READ };

                if let Err(e) = wasi_builder.preopened_dir(
                    &safe_path, pattern, // Use original pattern as guest path
                    dir_perms, file_perms,
                ) {
                    debug!("Failed to preopen path {}: {}", safe_path, e);
                }
            } else {
                debug!("Skipping unsafe preopen path: {}", pattern);
            }
        }

        // Build WASIp1 context from the WasiCtxBuilder
        let wasi = wasi_builder.build_p1();

        // Configure per-plugin resource tracker with max linear memory from manifest
        let tracker = ResourceTracker {
            max_memory_bytes: (permissions.max_memory_mb as usize).saturating_mul(1024 * 1024),
            ..Default::default()
        };

        let context = PluginContext {
            wasi,
            permissions,
            plugin_base_dir: plugin_base_dir.to_path_buf(),
            plugin_id: plugin_id.to_string(),
            resource_tracker: tracker,
        };

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
        instance: &Instance,
        exports: &PluginExports,
        store: &mut Store<PluginContext>,
    ) -> Result<PluginMetadata, PluginError> {
        if let Some(get_metadata_fn) = &exports.get_metadata {
            // Call plugin's get_metadata function which returns packed ptr:len as i64
            let packed_result = get_metadata_fn.call(&mut *store, ()).map_err(|e| {
                PluginError::RuntimeError(format!("Failed to call plugin_get_metadata: {}", e))
            })?;

            // Unpack the result: high 32 bits = len, low 32 bits = ptr
            let ptr = (packed_result & 0xFFFF_FFFF) as u32;
            let len = (packed_result >> 32) as u32;

            if ptr == 0 || len == 0 {
                return Err(PluginError::RuntimeError(
                    "Plugin returned invalid metadata pointer/length".into(),
                ));
            }

            // Read JSON from the plugin's linear memory
            let memory = instance
                .get_export(&mut *store, "memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| {
                PluginError::RuntimeError("Plugin missing memory export".into())
            })?;

            let mut buffer = vec![0u8; len as usize];
            memory.read(&mut *store, ptr as usize, &mut buffer).map_err(|_| {
                PluginError::RuntimeError("Failed to read plugin metadata from memory".into())
            })?;

            let metadata: PluginMetadata = serde_json::from_slice(&buffer).map_err(|e| {
                PluginError::RuntimeError(format!("Invalid plugin metadata JSON: {}", e))
            })?;

            Ok(metadata)
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
    fn read_plugin_permissions(
        &self,
        path: &Path,
        plugin_base_dir: &Path,
    ) -> Result<PluginPermissions, PluginError> {
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
                            .filter_map(|p| self.sanitize_plugin_path(plugin_base_dir, &p))
                            .collect();
                        perms.write_files = perms
                            .write_files
                            .into_iter()
                            .filter_map(|p| self.sanitize_plugin_path(plugin_base_dir, &p))
                            .collect();
                        return Ok(perms);
                    }
                }
                Err(e) => {
                    return Err(PluginError::InvalidFormat(format!("Invalid TOML manifest: {}", e)))
                }
            }
        }

        // Default: sandboxed to plugin_dir only
        let default = PluginPermissions {
            read_files: vec![plugin_base_dir.to_string_lossy().to_string()],
            write_files: vec![],
            ..Default::default()
        };
        Ok(default)
    }

    /// Validate plugin manifest information
    fn validate_plugin_manifest(
        &self,
        plugin_info: &PluginManifestInfo,
    ) -> Result<(), PluginError> {
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
            return Err(PluginError::PermissionDenied(format!(
                "Requested memory ({} MB) exceeds maximum allowed (200 MB)",
                perms.max_memory_mb
            )));
        }

        if perms.max_memory_mb == 0 {
            return Err(PluginError::PermissionDenied(
                "Memory limit must be greater than 0".into(),
            ));
        }

        // Validate timeout limits
        if perms.timeout_ms > 30000 {
            return Err(PluginError::PermissionDenied(format!(
                "Requested timeout ({} ms) exceeds maximum allowed (30s)",
                perms.timeout_ms
            )));
        }

        if perms.timeout_ms == 0 {
            return Err(PluginError::PermissionDenied("Timeout must be greater than 0".into()));
        }

        // Validate file access patterns
        for pattern in &perms.read_files {
            if self.is_dangerous_file_pattern(pattern) {
                return Err(PluginError::PermissionDenied(format!(
                    "Dangerous file access pattern denied: {}",
                    pattern
                )));
            }
        }

        for pattern in &perms.write_files {
            if self.is_dangerous_file_pattern(pattern) {
                return Err(PluginError::PermissionDenied(format!(
                    "Dangerous file write pattern denied: {}",
                    pattern
                )));
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
            "/etc/",
            "/sys/",
            "/proc/",
            "/dev/",
            "/boot/",
            "/root/",
            "/usr/bin/",
            "/usr/sbin/",
            "/bin/",
            "/sbin/",
            "/lib/",
            "/lib64/",
            "/../",
            "/..", // Path traversal
            "shadow",
            "passwd",
            "sudoers", // Sensitive files
        ];

        dangerous_patterns.iter().any(|&dangerous| pattern.contains(dangerous))
    }

    /// Check if an environment variable is sensitive
    fn is_sensitive_env_var(&self, env_var: &str) -> bool {
        let sensitive_prefixes = [
            "AWS_",
            "GCP_",
            "AZURE_",
            "SECRET_",
            "TOKEN_",
            "KEY_",
            "PASSWORD_",
            "PASS_",
            "SSH_",
            "GPG_",
        ];

        let sensitive_exact =
            ["HOME", "USER", "USERNAME", "PATH", "LD_LIBRARY_PATH", "SUDO_USER", "LOGNAME"];

        sensitive_prefixes.iter().any(|&prefix| env_var.starts_with(prefix))
            || sensitive_exact.contains(&env_var)
    }

    /// Validate that requested permissions match allowed permissions
    fn validate_permissions(
        &self,
        requested: &PluginPermissions,
        allowed: &PluginPermissions,
        plugin_base_dir: &Path,
    ) -> Result<(), PluginError> {
        if requested.network && !allowed.network {
            return Err(PluginError::PermissionDenied("Network access not allowed".into()));
        }

        if requested.execute_commands && !allowed.execute_commands {
            return Err(PluginError::PermissionDenied("Command execution not allowed".into()));
        }

        // Helper to sanitize a requested path relative to the plugin_dir and compare to allowed.
        let is_allowed_path = |pattern: &str, allowed_set: &Vec<String>| {
            if let Some(sanitized) = self.sanitize_plugin_path(plugin_base_dir, pattern) {
                allowed_set.iter().any(|p| p == &sanitized)
            } else {
                false
            }
        };

        // Check read access patterns (requested must be a subset of allowed after sanitization)
        for pattern in &requested.read_files {
            if !is_allowed_path(pattern, &allowed.read_files) {
                return Err(PluginError::PermissionDenied(format!(
                    "Read access to {} not allowed",
                    pattern
                )));
            }
        }

        // Check write access patterns
        for pattern in &requested.write_files {
            if !is_allowed_path(pattern, &allowed.write_files) {
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
        linker
            .func_wrap(
                "env",
                "host_log",
                move |mut caller: Caller<'_, PluginContext>,
                      level: i32,
                      ptr: i32,
                      len: i32|
                      -> Result<(), anyhow::Error> {
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;

                    let mut buffer = vec![0u8; len as usize];
                    memory
                        .read(&caller, ptr as usize, &mut buffer)
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
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to add host_log: {}", e))
            })?;

        // Host file read function
        let host_clone = self.host.clone();
        linker
            .func_wrap(
                "env",
                "host_read_file",
                move |mut caller: Caller<'_, PluginContext>,
                      path_ptr: i32,
                      path_len: i32,
                      result_ptr: i32,
                      result_len_ptr: i32|
                      -> Result<i32, anyhow::Error> {
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;

                    // Read path from plugin memory
                    let mut path_buffer = vec![0u8; path_len as usize];
                    memory
                        .read(&caller, path_ptr as usize, &mut path_buffer)
                        .map_err(|_| anyhow::anyhow!("Failed to read path from plugin memory"))?;

                    let path_str = String::from_utf8_lossy(&path_buffer).to_string();

                    // Resolve against plugin base dir if relative and canonicalize
                    let ctx = caller.data();
                    let mut resolved = std::path::PathBuf::from(&path_str);
                    if resolved.is_relative() {
                        resolved = ctx.plugin_base_dir.join(&resolved);
                    }
                    let resolved = match std::fs::canonicalize(&resolved) {
                        Ok(p) => p,
                        Err(_) => return Ok(-2), // IO error
                    };

                    // Check permissions using canonical paths
                    if !is_allowed_path(ctx, &resolved, false) {
                        return Ok(-1); // Permission denied
                    }

                    // Read file through host if available, otherwise std fs
                    let file_result = if let Some(ref host) = host_clone {
                        host.read_file(&resolved.to_string_lossy())
                    } else {
                        std::fs::read(&resolved).map_err(ApiPluginError::IoError)
                    };

                    match file_result {
                        Ok(data) => {
                            // Write data length back to plugin
                            let len_bytes = (data.len() as u32).to_le_bytes();
                            memory
                                .write(&mut caller, result_len_ptr as usize, &len_bytes)
                                .map_err(|_| anyhow::anyhow!("Failed to write result length"))?;

                            // Write data back to plugin if buffer is provided
                            if result_ptr != 0 {
                                memory
                                    .write(&mut caller, result_ptr as usize, &data)
                                    .map_err(|_| anyhow::anyhow!("Failed to write result data"))?;
                            }
                            Ok(0) // Success
                        }
                        Err(_) => Ok(-2), // IO error
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to add host_read_file: {}", e))
            })?;

        // Host file write function
        let host_clone = self.host.clone();
        linker
            .func_wrap(
                "env",
                "host_write_file",
                move |mut caller: Caller<'_, PluginContext>,
                      path_ptr: i32,
                      path_len: i32,
                      data_ptr: i32,
                      data_len: i32|
                      -> Result<i32, anyhow::Error> {
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;

                    // Read path and data from plugin memory
                    let mut path_buffer = vec![0u8; path_len as usize];
                    memory
                        .read(&caller, path_ptr as usize, &mut path_buffer)
                        .map_err(|_| anyhow::anyhow!("Failed to read path from plugin memory"))?;

                    let mut data_buffer = vec![0u8; data_len as usize];
                    memory
                        .read(&caller, data_ptr as usize, &mut data_buffer)
                        .map_err(|_| anyhow::anyhow!("Failed to read data from plugin memory"))?;

                    let path_str = String::from_utf8_lossy(&path_buffer).to_string();

                    // Resolve against plugin base dir if relative and canonicalize
                    let ctx = caller.data();
                    let mut resolved = std::path::PathBuf::from(&path_str);
                    if resolved.is_relative() {
                        resolved = ctx.plugin_base_dir.join(&resolved);
                    }
                    let resolved = match std::fs::canonicalize(&resolved) {
                        Ok(p) => p,
                        Err(_) => return Ok(-2), // IO error
                    };

                    // Check permissions
                    if !is_allowed_path(ctx, &resolved, true) {
                        return Ok(-1); // Permission denied
                    }

                    // Write file through host if available
                    let write_result = if let Some(ref host) = host_clone {
                        host.write_file(&resolved.to_string_lossy(), &data_buffer)
                    } else {
                        std::fs::write(&resolved, &data_buffer).map_err(ApiPluginError::IoError)
                    };

                    match write_result {
                        Ok(()) => Ok(0),  // Success
                        Err(_) => Ok(-2), // IO error
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to add host_write_file: {}", e))
            })?;

        // Host execute command function (two-call, variable-size JSON result)
        let host_clone = self.host.clone();
        linker
            .func_wrap(
                "env",
                "host_execute_command",
                move |mut caller: Caller<'_, PluginContext>,
                      cmd_ptr: i32,
                      cmd_len: i32,
                      result_ptr: i32,
                      result_len_ptr: i32|
                      -> Result<i32, anyhow::Error> {
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;

                    let mut cmd_buffer = vec![0u8; cmd_len as usize];
                    memory.read(&caller, cmd_ptr as usize, &mut cmd_buffer).map_err(|_| {
                        anyhow::anyhow!("Failed to read command from plugin memory")
                    })?;

                    let command = String::from_utf8_lossy(&cmd_buffer);

                    // Check permissions
                    let ctx = caller.data();
                    if !ctx.permissions.execute_commands {
                        return Ok(-1); // Permission denied
                    }

                    // Execute command through host if available
                    if let Some(ref host) = host_clone {
                        match host.execute_command(&command) {
                            Ok(output) => {
                                // Serialize to JSON and publish via two-call ABI
                                let json =
                                    serde_json::to_vec(&output).unwrap_or_else(|_| b"{}".to_vec());
                                // Always write the length
                                let len_bytes = (json.len() as u32).to_le_bytes();
                                memory
                                    .write(&mut caller, result_len_ptr as usize, &len_bytes)
                                    .map_err(|_| {
                                        anyhow::anyhow!("Failed to write result length")
                                    })?;
                                if result_ptr != 0 {
                                    memory.write(&mut caller, result_ptr as usize, &json).map_err(
                                        |_| anyhow::anyhow!("Failed to write result data"),
                                    )?;
                                }
                                Ok(0)
                            }
                            Err(_) => Ok(-2), // Execution failed
                        }
                    } else {
                        Ok(-3) // Host not available
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to add host_execute_command: {}",
                    e
                ))
            })?;

        // Host store data function
        let host_clone = self.host.clone();
        linker
            .func_wrap(
                "env",
                "host_store_data",
                move |mut caller: Caller<'_, PluginContext>,
                      key_ptr: i32,
                      key_len: i32,
                      data_ptr: i32,
                      data_len: i32|
                      -> Result<i32, anyhow::Error> {
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;

                    let mut key_buf = vec![0u8; key_len as usize];
                    memory
                        .read(&caller, key_ptr as usize, &mut key_buf)
                        .map_err(|_| anyhow::anyhow!("Failed to read key from plugin memory"))?;
                    let key = String::from_utf8_lossy(&key_buf);

                    let mut data_buf = vec![0u8; data_len as usize];
                    memory
                        .read(&caller, data_ptr as usize, &mut data_buf)
                        .map_err(|_| anyhow::anyhow!("Failed to read data from plugin memory"))?;

                    // Permission check
                    let ctx = caller.data();
                    if !ctx.permissions.storage {
                        return Ok(-1);
                    }
                    if let Some(ref host) = host_clone {
                        match host.store_data_for(&ctx.plugin_id, &key, &data_buf) {
                            Ok(()) => Ok(0),
                            Err(_) => Ok(-2),
                        }
                    } else {
                        Ok(-3)
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to add host_store_data: {}", e))
            })?;

        // Host retrieve data function
        let host_clone = self.host.clone();
        linker
            .func_wrap(
                "env",
                "host_retrieve_data",
                move |mut caller: Caller<'_, PluginContext>,
                      key_ptr: i32,
                      key_len: i32,
                      result_ptr: i32,
                      result_len_ptr: i32|
                      -> Result<i32, anyhow::Error> {
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;

                    let mut key_buf = vec![0u8; key_len as usize];
                    memory
                        .read(&caller, key_ptr as usize, &mut key_buf)
                        .map_err(|_| anyhow::anyhow!("Failed to read key from plugin memory"))?;
                    let key = String::from_utf8_lossy(&key_buf);

                    // Permission check
                    let ctx = caller.data();
                    if !ctx.permissions.storage {
                        return Ok(-1);
                    }
                    if let Some(ref host) = host_clone {
                        match host.retrieve_data_for(&ctx.plugin_id, &key) {
                            Ok(Some(data)) => {
                                let len_bytes = (data.len() as u32).to_le_bytes();
                                memory
                                    .write(&mut caller, result_len_ptr as usize, &len_bytes)
                                    .map_err(|_| {
                                        anyhow::anyhow!("Failed to write result length")
                                    })?;
                                if result_ptr != 0 {
                                    memory.write(&mut caller, result_ptr as usize, &data).map_err(
                                        |_| anyhow::anyhow!("Failed to write result data"),
                                    )?;
                                }
                                Ok(0)
                            }
                            Ok(None) => Ok(-4),
                            Err(_) => Ok(-2),
                        }
                    } else {
                        Ok(-3)
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to add host_retrieve_data: {}",
                    e
                ))
            })?;

        // Host store document function
        let host_clone = self.host.clone();
        linker
            .func_wrap(
                "env",
                "host_store_document",
                move |mut caller: Caller<'_, PluginContext>,
                      ns_ptr: i32,
                      ns_len: i32,
                      id_ptr: i32,
                      id_len: i32,
                      json_ptr: i32,
                      json_len: i32|
                      -> Result<i32, anyhow::Error> {
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;
                    let mut ns_buf = vec![0u8; ns_len as usize];
                    memory
                        .read(&caller, ns_ptr as usize, &mut ns_buf)
                        .map_err(|_| anyhow::anyhow!("Failed to read ns"))?;
                    let namespace = String::from_utf8_lossy(&ns_buf).to_string();
                    let mut id_buf = vec![0u8; id_len as usize];
                    memory
                        .read(&caller, id_ptr as usize, &mut id_buf)
                        .map_err(|_| anyhow::anyhow!("Failed to read id"))?;
                    let doc_id = String::from_utf8_lossy(&id_buf).to_string();
                    let mut json_buf = vec![0u8; json_len as usize];
                    memory
                        .read(&caller, json_ptr as usize, &mut json_buf)
                        .map_err(|_| anyhow::anyhow!("Failed to read json"))?;
                    let json_str = String::from_utf8_lossy(&json_buf).to_string();

                    let ctx = caller.data();
                    if !ctx.permissions.storage {
                        return Ok(-1);
                    }
                    if let Some(ref host) = host_clone {
                        match host.store_document_for(
                            &ctx.plugin_id,
                            &namespace,
                            &doc_id,
                            &json_str,
                        ) {
                            Ok(()) => Ok(0),
                            Err(_) => Ok(-2),
                        }
                    } else {
                        Ok(-3)
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to add host_store_document: {}",
                    e
                ))
            })?;

        // Host retrieve document function
        let host_clone = self.host.clone();
        linker
            .func_wrap(
                "env",
                "host_retrieve_document",
                move |mut caller: Caller<'_, PluginContext>,
                      ns_ptr: i32,
                      ns_len: i32,
                      id_ptr: i32,
                      id_len: i32,
                      result_ptr: i32,
                      result_len_ptr: i32|
                      -> Result<i32, anyhow::Error> {
                    let memory = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .ok_or_else(|| anyhow::anyhow!("Plugin missing memory export"))?;
                    let mut ns_buf = vec![0u8; ns_len as usize];
                    memory
                        .read(&caller, ns_ptr as usize, &mut ns_buf)
                        .map_err(|_| anyhow::anyhow!("Failed to read ns"))?;
                    let namespace = String::from_utf8_lossy(&ns_buf).to_string();
                    let mut id_buf = vec![0u8; id_len as usize];
                    memory
                        .read(&caller, id_ptr as usize, &mut id_buf)
                        .map_err(|_| anyhow::anyhow!("Failed to read id"))?;
                    let doc_id = String::from_utf8_lossy(&id_buf).to_string();

                    let ctx = caller.data();
                    if !ctx.permissions.storage {
                        return Ok(-1);
                    }
                    if let Some(ref host) = host_clone {
                        match host.retrieve_document_for(&ctx.plugin_id, &namespace, &doc_id) {
                            Ok(Some(json)) => {
                                let data = json.into_bytes();
                                let len_bytes = (data.len() as u32).to_le_bytes();
                                memory
                                    .write(&mut caller, result_len_ptr as usize, &len_bytes)
                                    .map_err(|_| {
                                        anyhow::anyhow!("Failed to write result length")
                                    })?;
                                if result_ptr != 0 {
                                    memory.write(&mut caller, result_ptr as usize, &data).map_err(
                                        |_| anyhow::anyhow!("Failed to write result data"),
                                    )?;
                                }
                                Ok(0)
                            }
                            Ok(None) => Ok(-4),
                            Err(_) => Ok(-2),
                        }
                    } else {
                        Ok(-3)
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to add host_retrieve_document: {}",
                    e
                ))
            })?;

        Ok(())
    }

    /// Send an event to a plugin using JSON-over-memory ABI
    pub async fn send_event_to_plugin(
        &self,
        plugin_name: &str,
        event: &PluginEvent,
    ) -> Result<PluginEventResponse, PluginError> {
        // Get plugin
        let plugins = self.plugins.read().await;
        let plugin = plugins
            .get(plugin_name)
            .cloned()
            .ok_or_else(|| PluginError::NotFound(plugin_name.to_string()))?;
        let instance = &plugin.instance;
        let exports = &plugin.exports;
        drop(plugins); // Release the read lock early

        // Ensure event handler exists
        let handle_event = match &exports.handle_event {
            Some(f) => f,
            None => {
                return Ok(PluginEventResponse {
                    success: false,
                    result: None,
                    error: Some("Plugin does not implement event handler".into()),
                })
            }
        };

        // Serialize event to JSON
        let event_json = serde_json::to_vec(event)
            .map_err(|e| PluginError::RuntimeError(format!("Failed to serialize event: {}", e)))?;

        // Lock store for this call
        let mut store = plugin.store.lock().await;

        // Allocate memory inside plugin if allocator exists
        let alloc = instance.get_typed_func::<i32, i32>(&mut *store, "plugin_alloc").ok();
        let memory = instance
            .get_export(&mut *store, "memory")
            .and_then(|e| e.into_memory())
            .ok_or_else(|| PluginError::RuntimeError("Plugin missing memory export".into()))?;

        let (ptr, len) = if let Some(alloc_fn) = alloc {
            let p = alloc_fn
                .call(&mut *store, event_json.len() as i32)
                .map_err(|e| PluginError::RuntimeError(format!("plugin_alloc failed: {}", e)))?;
            // Write event JSON into plugin memory
            memory.write(&mut *store, p as usize, &event_json).map_err(|_| {
                PluginError::RuntimeError("Failed to write event data to plugin memory".into())
            })?;
            (p, event_json.len() as i32)
        } else {
            // Without an allocator we cannot safely write into plugin memory
            return Ok(PluginEventResponse {
                success: false,
                result: None,
                error: Some("Plugin allocator not available (plugin_alloc)".into()),
            });
        };

        // Apply a small epoch deadline to bound CPU usage during event handling
        store.set_epoch_deadline(CPU_INIT_TICKS);
        let rc = handle_event
            .call(&mut *store, (ptr, len))
            .map_err(|e| PluginError::RuntimeError(format!("plugin_handle_event failed: {}", e)))?;
        store.set_epoch_deadline(CPU_FAR_TICKS);

        if rc != 0 {
            return Ok(PluginEventResponse {
                success: false,
                result: None,
                error: Some(format!("Plugin returned error code: {}", rc)),
            });
        }

        // Try to fetch optional last response from plugin
        let last_resp_fn =
            instance.get_typed_func::<(), i64>(&mut *store, "plugin_get_last_response").ok();

        let result = if let Some(get_resp) = last_resp_fn {
            let packed = get_resp.call(&mut *store, ()).map_err(|e| {
                PluginError::RuntimeError(format!("plugin_get_last_response failed: {}", e))
            })?;
            let rptr = (packed & 0xFFFF_FFFF) as u32;
            let rlen = (packed >> 32) as u32;
            if rptr != 0 && rlen > 0 {
                let mut buf = vec![0u8; rlen as usize];
                memory.read(&mut *store, rptr as usize, &mut buf).map_err(|_| {
                    PluginError::RuntimeError("Failed to read plugin response".into())
                })?;
                Some(String::from_utf8_lossy(&buf).to_string())
            } else {
                None
            }
        } else {
            None
        };

        Ok(PluginEventResponse { success: true, result, error: None })
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
    fn sanitize_plugin_path(&self, base: &Path, p: &str) -> Option<String> {
        use std::path::PathBuf;
        let raw = PathBuf::from(p);
        let resolved = if raw.is_absolute() { raw } else { base.join(raw) };
        // Disallow preopening '/' or paths outside base
        if resolved.components().count() == 0 {
            return None;
        }
        if resolved.as_os_str() == "/" {
            return None;
        }
        if !resolved.starts_with(base) {
            return None;
        }
        Some(resolved.to_string_lossy().to_string())
    }
}

/// Check if a canonicalized resolved path is allowed under permissions
fn is_allowed_path(ctx: &PluginContext, resolved: &std::path::Path, for_write: bool) -> bool {
    let list = if for_write { &ctx.permissions.write_files } else { &ctx.permissions.read_files };
    for allowed in list {
        // Support both absolute and relative manifest entries by trying direct canonicalize and base-joined canonicalize.
        let allowed_path = std::path::PathBuf::from(allowed);
        let canon = std::fs::canonicalize(&allowed_path)
            .ok()
            .or_else(|| std::fs::canonicalize(ctx.plugin_base_dir.join(&allowed_path)).ok());
        if let Some(allowed_canon) = canon {
            if resolved.starts_with(&allowed_canon) {
                return true;
            }
        }
    }
    false
}

/// Resource limiter implementation
impl ResourceLimiter for ResourceTracker {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        // Enforce per-plugin configured limit; fall back to 50MB if unset
        let max = if self.max_memory_bytes == 0 { 50 * 1024 * 1024 } else { self.max_memory_bytes };

        if desired > max {
            return Ok(false);
        }

        self.memory_used = desired;
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        const MAX_TABLES: usize = 10;
        Ok(desired <= MAX_TABLES)
    }
}

impl PluginManager {
    /// Broadcast an event to all loaded plugins
    pub async fn broadcast_event(&self, event: &PluginEvent) -> Vec<(String, PluginEventResponse)> {
        let names: Vec<String> = {
            let plugins = self.plugins.read().await;
            plugins.keys().cloned().collect()
        };
        let mut results = Vec::new();
        for name in names {
            match self.send_event_to_plugin(&name, event).await {
                Ok(resp) => results.push((name, resp)),
                Err(e) => results.push((
                    name,
                    PluginEventResponse {
                        success: false,
                        result: None,
                        error: Some(e.to_string()),
                    },
                )),
            }
        }
        results
    }

    /// Verify plugin signature if .sig file is present next to the .wasm
    #[allow(dead_code)]
    fn verify_signature_if_present(&self, wasm_path: &Path) -> anyhow::Result<()> {
        match self.verify_signature_and_get_key(wasm_path)? {
            Some(_) => Ok(()),
            None => Ok(()),
        }
    }

    /// Verify signature if present; return verifying key hex on success.
    fn verify_signature_and_get_key(&self, wasm_path: &Path) -> anyhow::Result<Option<String>> {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        use sha2::{Digest, Sha256};

        let sig_path = wasm_path.with_extension("sig");
        if !sig_path.exists() {
            return Ok(None); // nothing to verify
        }
        let wasm_bytes = std::fs::read(wasm_path)?;
        let sig_bytes_hex = std::fs::read_to_string(&sig_path)?;
        let sig_bytes = hex::decode(sig_bytes_hex.trim()).map_err(|e| anyhow::anyhow!(e))?;
        let signature = Signature::from_slice(&sig_bytes).map_err(|e| anyhow::anyhow!(e))?;

        // Hash the wasm for a stable-size message
        let digest = Sha256::digest(&wasm_bytes);

        // Load trusted keys from config dir
        if let Some(config_dir) = dirs::config_dir() {
            let keys_dir = config_dir.join("openagent-terminal").join("trusted_keys");
            if keys_dir.exists() {
                for entry in std::fs::read_dir(&keys_dir)? {
                    let entry = entry?;
                    if entry.path().extension().and_then(|s| s.to_str()) != Some("pub") {
                        continue;
                    }
                    let key_hex = std::fs::read_to_string(entry.path())?;
                    let key_bytes = hex::decode(key_hex.trim()).map_err(|e| anyhow::anyhow!(e))?;
                    if let Ok(vk) = VerifyingKey::from_bytes(
                        &key_bytes.try_into().map_err(|_| anyhow::anyhow!("invalid key length"))?,
                    ) {
                        if vk.verify(&digest, &signature).is_ok() {
                            return Ok(Some(hex::encode(vk.to_bytes())));
                        }
                    }
                }
            }
        }

        Err(anyhow::anyhow!("No trusted key verified the signature"))
    }
}

/// Directory kind for signature policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirKind {
    System,
    User,
    Project,
    Other,
}

#[derive(Debug, Clone)]
pub struct SignaturePolicy {
    pub require_signatures_for_all: bool,
    pub require_system: bool,
    pub require_user: bool,
    pub require_project: bool,
    pub system_dir: Option<PathBuf>,
    pub user_dir: Option<PathBuf>,
    pub project_dir: Option<PathBuf>,
}

impl SignaturePolicy {
    pub fn kind_for_dir(&self, dir: &Path) -> DirKind {
        let canon = std::fs::canonicalize(dir).unwrap_or(dir.to_path_buf());
        if let Some(sd) = &self.system_dir {
            if std::fs::canonicalize(sd).unwrap_or(sd.clone()) == canon {
                return DirKind::System;
            }
        }
        if let Some(ud) = &self.user_dir {
            if std::fs::canonicalize(ud).unwrap_or(ud.clone()) == canon {
                return DirKind::User;
            }
        }
        if let Some(pd) = &self.project_dir {
            if std::fs::canonicalize(pd).unwrap_or(pd.clone()) == canon {
                return DirKind::Project;
            }
        }
        DirKind::Other
    }

    pub fn require_for_kind(&self, kind: DirKind) -> bool {
        match kind {
            DirKind::System => self.require_system,
            DirKind::User => self.require_user,
            DirKind::Project => self.require_project,
            DirKind::Other => false,
        }
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
        let base = temp_dir.path();
        // Absolute root should be rejected
        assert!(manager.sanitize_plugin_path(base, "/").is_none());
        // Outside plugin_dir should be rejected
        assert!(manager.sanitize_plugin_path(base, "/etc").is_none());
        // Relative inside plugin_dir should be accepted and resolved
        let sub = manager.sanitize_plugin_path(base, "subdir").unwrap();
        assert!(sub.starts_with(temp_dir.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn test_read_manifest_permissions_sanitized() {
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("p.wasm");
        std::fs::write(&wasm_path, b"00").unwrap();
        let manifest_path = temp_dir.path().join("p.toml");
        // Include an unsafe preopen ('/') which should be filtered out
        let content = r#"[permissions]
read_files=["/","sub"]
write_files=["sub"]
environment_variables=[]
network=false
execute_commands=false
max_memory_mb=50
timeout_ms=5000
"#;
        std::fs::write(&manifest_path, content).unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let perms = manager
            .read_plugin_permissions(&wasm_path, wasm_path.parent().unwrap())
            .expect("permissions");
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

    #[tokio::test]
    async fn loads_minimal_module_without_panicking() {
        let temp = TempDir::new().unwrap();
        let wasm_path = temp.path().join("mod.wasm");
        let wat = r#"(module (memory (export "memory") 1))"#;
        let bytes = wat::parse_str(wat).unwrap();
        std::fs::write(&wasm_path, bytes).unwrap();

        let mgr = PluginManager::with_host_and_dirs(vec![temp.path().to_path_buf()], None).unwrap();
        let res = mgr.load_plugin(&wasm_path).await;
        assert!(res.is_ok(), "failed to load minimal module: {:?}", res);
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
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("test_plugin.wasm");
        std::fs::write(&wasm_path, build_cleanup_only_wasm()).unwrap();
        let manifest_path = temp_dir.path().join("test_plugin.toml");

        // Test complete manifest with plugin metadata
        let content = r#"[plugin]
name="test-plugin"
version="1.0.0"
author="Test Author"
[permissions]
read_files=[]
write_files=[]
environment_variables=[]
network=false
execute_commands=false
max_memory_mb=50
timeout_ms=5000
"#;
        std::fs::write(&manifest_path, content).unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let perms = manager
            .read_plugin_permissions(&wasm_path, wasm_path.parent().unwrap())
            .expect("Should parse valid manifest");
        assert_eq!(perms.max_memory_mb, 50);
        assert_eq!(perms.timeout_ms, 5000);
    }

    #[tokio::test]
    async fn test_dangerous_file_pattern_rejection() {
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("dangerous.wasm");
        std::fs::write(&wasm_path, build_cleanup_only_wasm()).unwrap();
        let manifest_path = temp_dir.path().join("dangerous.toml");

        // Test manifest with dangerous file access
        let content = r#"[permissions]
read_files=["/etc/passwd"]
write_files=[]
environment_variables=[]
network=false
execute_commands=false
max_memory_mb=50
timeout_ms=5000
"#;
        std::fs::write(&manifest_path, content).unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let result = manager.read_plugin_permissions(&wasm_path, wasm_path.parent().unwrap());
        assert!(result.is_err(), "Should reject dangerous file patterns");
    }

    #[tokio::test]
    async fn test_memory_limit_validation() {
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("memory_test.wasm");
        std::fs::write(&wasm_path, build_cleanup_only_wasm()).unwrap();
        let manifest_path = temp_dir.path().join("memory_test.toml");

        // Test manifest with excessive memory request
        let content = r#"[permissions]
read_files=[]
write_files=[]
environment_variables=[]
network=false
execute_commands=false
max_memory_mb=500
timeout_ms=5000
"#;
        std::fs::write(&manifest_path, content).unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let result = manager.read_plugin_permissions(&wasm_path, wasm_path.parent().unwrap());
        assert!(result.is_err(), "Should reject excessive memory requests");
    }

    #[tokio::test]
    async fn test_timeout_limit_validation() {
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("timeout_test.wasm");
        std::fs::write(&wasm_path, build_cleanup_only_wasm()).unwrap();
        let manifest_path = temp_dir.path().join("timeout_test.toml");

        // Test manifest with excessive timeout request
        let content = r#"[permissions]
read_files=[]
write_files=[]
environment_variables=[]
network=false
execute_commands=false
max_memory_mb=50
timeout_ms=60000
"#;
        std::fs::write(&manifest_path, content).unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let result = manager.read_plugin_permissions(&wasm_path, wasm_path.parent().unwrap());
        assert!(result.is_err(), "Should reject excessive timeout requests");
    }

    #[tokio::test]
    async fn test_plugin_manifest_info_validation() {
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("info_test.wasm");
        std::fs::write(&wasm_path, build_cleanup_only_wasm()).unwrap();
        let manifest_path = temp_dir.path().join("info_test.toml");

        // Test manifest with missing required fields
        let content = r#"[plugin]
name=""
version="1.0.0"
[permissions]
read_files=[]
write_files=[]
environment_variables=[]
network=false
execute_commands=false
max_memory_mb=50
timeout_ms=5000
"#;
        std::fs::write(&manifest_path, content).unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let result = manager.read_plugin_permissions(&wasm_path, wasm_path.parent().unwrap());
        assert!(result.is_err(), "Should reject dangerous file patterns");
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

    #[tokio::test]
    async fn test_metadata_and_event_flow_minimal() {
        let temp_dir = TempDir::new().unwrap();
        let wasm_path = temp_dir.path().join("meta_event.wasm");

        // JSON metadata to embed at offset 1024
        let meta_json = serde_json::json!({
            "name": "test-plugin",
            "version": "1.0.0",
            "author": "tester",
            "description": "Test plugin",
            "license": "MIT",
            "homepage": null,
            "capabilities": {
                "completions": false,
                "context_provider": false,
                "commands": [],
                "hooks": [],
                "file_associations": []
            },
            "permissions": {
                "read_files": [],
                "write_files": [],
                "network": false,
                "execute_commands": false,
                "environment_variables": [],
                "max_memory_mb": 50,
                "timeout_ms": 5000
            }
        })
        .to_string();
        let ptr = 1024u32;
        let len = meta_json.len() as u32;
        let packed = ((len as i64) << 32) | (ptr as i64);

        let wat = format!(
            r#"(module
                (memory (export "memory") 4)
                (data (i32.const 1024) "{meta}")
                (func (export "plugin_init") (result i32) i32.const 0)
                (func (export "plugin_cleanup") (result i32) i32.const 0)
                ;; Return packed ptr/len for metadata
                (func (export "plugin_get_metadata") (result i64)
                    i64.const {packed}
                )
                ;; Minimal allocator returning start of memory
                (func (export "plugin_alloc") (param i32) (result i32)
                    (i32.const 0)
                )
                ;; Minimal event handler returning success
                (func (export "plugin_handle_event") (param i32 i32) (result i32)
                    (i32.const 0)
                )
            )"#,
            meta = meta_json.escape_default(),
            packed = packed
        );

        let wasm_bytes = wat::parse_str(&wat).expect("WAT compile");
        std::fs::write(&wasm_path, wasm_bytes).unwrap();

        let manager = PluginManager::new(temp_dir.path()).unwrap();
        let name = manager.load_plugin(&wasm_path).await.expect("load");

        // Verify metadata
        let listed = manager.list_plugins().await;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "test-plugin");

        // Send a minimal event (will succeed, no response)
        let event =
            PluginEvent { event_type: "ping".into(), data: serde_json::json!({}), timestamp: 0 };
        let resp = manager.send_event_to_plugin(&name, &event).await.expect("event ok");
        assert!(resp.success);
    }

    #[test]
    fn test_is_allowed_path_traversal_and_symlink_denied() {
        use std::fs;
        use std::os::unix::fs::symlink;
        let temp = tempfile::TempDir::new().unwrap();
        let base = temp.path();

        // Layout: base/allowed, base/allowed/file.txt
        let allowed = base.join("allowed");
        fs::create_dir_all(&allowed).unwrap();
        fs::write(allowed.join("file.txt"), b"ok").unwrap();

        // Build a minimal PluginContext-like struct
        let permissions = plugin_api::PluginPermissions {
            read_files: vec![allowed.to_string_lossy().to_string()],
            write_files: vec![],
            ..Default::default()
        };
        // Build a minimal Wasi context via builder
        let wasi = wasmtime_wasi::WasiCtxBuilder::new().inherit_stdio().build_p1();
        let ctx = PluginContext {
            // Unused fields
            wasi,
            permissions,
            plugin_base_dir: base.to_path_buf(),
            plugin_id: "test".into(),
            resource_tracker: Default::default(),
        };

        // Inside allowed
        let inside = fs::canonicalize(allowed.join("file.txt")).unwrap();
        assert!(super::is_allowed_path(&ctx, &inside, false));

        // Traversal attempt: base/allowed/../outside
        let outside_dir = base.join("outside");
        fs::create_dir_all(&outside_dir).unwrap();
        let traversal = fs::canonicalize(allowed.join("..").join("outside")).unwrap();
        assert!(!super::is_allowed_path(&ctx, &traversal, false));

        // Symlink escape: allowed/link -> outside
        let link_path = allowed.join("link");
        symlink(&outside_dir, &link_path).unwrap();
        let escaped = fs::canonicalize(link_path).unwrap();
        assert!(!super::is_allowed_path(&ctx, &escaped, false));
    }
}

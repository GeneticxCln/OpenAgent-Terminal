//! Sandbox functionality for plugin runtime

use crate::{RuntimeConfig, RuntimeError, RuntimeResult};

/// WASM sandbox manager
#[derive(Debug)]
pub struct WasmSandbox {
    config: RuntimeConfig,
}

impl WasmSandbox {
    pub fn new(config: &RuntimeConfig) -> RuntimeResult<Self> {
        tracing::info!("Initializing WASM sandbox manager");
        Ok(Self { config: config.clone() })
    }

    /// Execute a function on a plugin within the sandbox.
    ///
    /// This enforces simple capability gates based on the function namespace:
    /// - "fs:" prefix indicates filesystem access
    /// - "net:" prefix indicates network access
    /// - "host:" prefix indicates host API calls; only a small allow-list is permitted
    pub fn execute_plugin(
        &mut self,
        plugin_id: &str,
        function: &str,
        _args: &[u8],
    ) -> RuntimeResult<Vec<u8>> {
        tracing::debug!("Executing plugin {} function {} in sandbox", plugin_id, function);

        // Filesystem capability guard
        if function.starts_with("fs:") && self.config.sandbox_filesystem {
            return Err(RuntimeError::SandboxViolation(
                "Filesystem access denied by sandbox".into(),
            ));
        }

        // Network capability guard
        if function.starts_with("net:") && self.config.sandbox_network {
            return Err(RuntimeError::SandboxViolation("Network access denied by sandbox".into()));
        }

        // Host API boundary guard: allow only explicit, well-defined host APIs
        if function.starts_with("host:") {
            let allowed = matches!(function, "host:log" | "host:emit_event" | "host:version");
            if !allowed {
                return Err(RuntimeError::SandboxViolation(format!(
                    "Host API '{}' is not permitted",
                    function
                )));
            }
            // Return a small response for known host APIs
            return Ok(Vec::new());
        }

        // Default: allow pure compute-only functions (no I/O namespaces)
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cfg() -> RuntimeConfig {
        RuntimeConfig::default()
    }

    #[test]
    fn sandbox_fs_network_denied_by_default() {
        let cfg = default_cfg();
        let mut sb = WasmSandbox::new(&cfg).unwrap();

        let err_fs = sb.execute_plugin("p1", "fs:read", b"");
        assert!(
            matches!(err_fs, Err(RuntimeError::SandboxViolation(msg)) if msg.contains("Filesystem"))
        );

        let err_net = sb.execute_plugin("p1", "net:connect", b"");
        assert!(
            matches!(err_net, Err(RuntimeError::SandboxViolation(msg)) if msg.contains("Network"))
        );
    }

    #[test]
    fn sandbox_allows_when_declared_off() {
        let mut cfg = default_cfg();
        cfg.sandbox_filesystem = false;
        cfg.sandbox_network = false;
        let mut sb = WasmSandbox::new(&cfg).unwrap();

        let ok_fs = sb.execute_plugin("p1", "fs:read", b"");
        assert!(ok_fs.is_ok());
        let ok_net = sb.execute_plugin("p1", "net:connect", b"");
        assert!(ok_net.is_ok());
    }

    #[test]
    fn host_api_boundaries_enforced() {
        let cfg = default_cfg();
        let mut sb = WasmSandbox::new(&cfg).unwrap();

        // Allowed host APIs
        assert!(sb.execute_plugin("p1", "host:log", b"hello").is_ok());
        assert!(sb.execute_plugin("p1", "host:emit_event", b"{}").is_ok());

        // Forbidden/unknown host API
        let err = sb.execute_plugin("p1", "host:read_secret", b"");
        assert!(
            matches!(err, Err(RuntimeError::SandboxViolation(msg)) if msg.contains("not permitted"))
        );
    }
}

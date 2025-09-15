use plugin_api::{CommandOutput, PluginError as ApiPluginError};
use plugin_loader::{PluginHost, PluginManager, PluginEvent, LogLevel};
use std::sync::Arc;

/// A simple mock host for testing host_execute_command behavior.
pub struct MockHost {
    pub executed: std::sync::Mutex<Vec<String>>,    
}

impl MockHost {
    pub fn new() -> Self { Self { executed: std::sync::Mutex::new(Vec::new()) } }
}

impl PluginHost for MockHost {
    fn log(&self, _level: LogLevel, _message: &str) {}
    fn read_file(&self, _path: &str) -> Result<Vec<u8>, ApiPluginError> { Ok(vec![]) }
    fn write_file(&self, _path: &str, _data: &[u8]) -> Result<(), ApiPluginError> { Ok(()) }
    fn execute_command(&self, command: &str) -> Result<CommandOutput, ApiPluginError> {
        self.executed.lock().unwrap().push(command.to_string());
        Ok(CommandOutput { stdout: "ok".into(), stderr: String::new(), exit_code: 0, execution_time_ms: 1 })
    }
    fn store_data_for(&self, _plugin_id: &str, _key: &str, _value: &[u8]) -> Result<(), ApiPluginError> { Ok(()) }
    fn retrieve_data_for(&self, _plugin_id: &str, _key: &str) -> Result<Option<Vec<u8>>, ApiPluginError> { Ok(None) }
}

/// Build a tiny WASM module that calls env.host_execute_command once.
fn build_exec_wasm(cmd: &str) -> Vec<u8> {
    // Host ABI: host_execute_command(cmd_ptr, cmd_len, result_ptr, result_len_ptr) -> i32
    // We'll just pass our inline string at a fixed memory offset (data section) and ask for length only.
    let data_offset = 1024;
    let wat = format!(r#"(module
        (import "env" "host_execute_command" (func $host_execute_command (param i32 i32 i32 i32) (result i32)))
        (memory (export "memory") 2)
        (data (i32.const {off}) "{cmd}")
        (func (export "plugin_init") (result i32) (i32.const 0))
        (func (export "plugin_cleanup") (result i32) (i32.const 0))
        ;; Minimal allocator: always return 0; host will write event JSON at memory[0..len]
        (func (export "plugin_alloc") (param i32) (result i32)
            (i32.const 0)
        )
        ;; Call into host to execute command; pass result_len_ptr but result_ptr=0
        (func (export "plugin_handle_event") (param i32 i32) (result i32)
            (local $len i32)
            (local.set $len (i32.const 0))
            ;; result_ptr=0, result_len_ptr=0 for minimal probe
            (call $host_execute_command (i32.const {off}) (i32.const {len}) (i32.const 0) (i32.const 0))
        )
    )"#, off = data_offset, cmd = cmd.escape_default(), len = cmd.len());
    wat::parse_str(&wat).expect("WAT compile")
}

#[tokio::test]
async fn test_host_execute_command_permission_denied() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();

    // WASM wants to run `echo hi` via host
    let wasm_bytes = build_exec_wasm("echo hi");
    let wasm_path = temp.path().join("exec_test.wasm");
    std::fs::write(&wasm_path, wasm_bytes).unwrap();

    // Permissions: execute_commands=false by default; ensure manifest present (minimal)
    let manifest_path = temp.path().join("exec_test.toml");
    std::fs::write(&manifest_path, "[permissions]\nread_files=[]\nwrite_files=[]\nenvironment_variables=[]\nnetwork=false\nexecute_commands=false\nmax_memory_mb=50\ntimeout_ms=5000\n").unwrap();

    let host = Arc::new(MockHost::new());
    let mgr = PluginManager::with_host_and_dirs(vec![temp.path().to_path_buf()], Some(host)).unwrap();

    // Loading should succeed (module exports needed functions)
    let name = mgr.load_plugin(&wasm_path).await.expect("load");

    // Send an event to trigger plugin_handle_event; expect permission denial path to bubble as error (-1) mapped to error result
    let evt = PluginEvent { event_type: "run".into(), data: serde_json::json!({}), timestamp: 0 };
    let resp = mgr.send_event_to_plugin(&name, &evt).await.expect("event call ok");

    // host_execute_command returns -1 on permission denied; our send_event surface returns success=true for handle_event rc=0 only.
    // With this minimal module, plugin_handle_event just calls host and returns rc as-is. If host returns -1, send_event will surface non-zero rc.
    assert_eq!(resp.success, false);
}

#[tokio::test]
async fn test_host_execute_command_allowed() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();

    let wasm_bytes = build_exec_wasm("echo hi");
    let wasm_path = temp.path().join("exec_ok.wasm");
    std::fs::write(&wasm_path, wasm_bytes).unwrap();

    // Allow execute_commands
    let manifest_path = temp.path().join("exec_ok.toml");
    std::fs::write(&manifest_path, "[permissions]\nread_files=[]\nwrite_files=[]\nenvironment_variables=[]\nnetwork=false\nexecute_commands=true\nmax_memory_mb=50\ntimeout_ms=5000\n").unwrap();

    let host = Arc::new(MockHost::new());
    let mgr = PluginManager::with_host_and_dirs(vec![temp.path().to_path_buf()], Some(host.clone())).unwrap();

    let name = mgr.load_plugin(&wasm_path).await.expect("load");

    let evt = PluginEvent { event_type: "run".into(), data: serde_json::json!({}), timestamp: 0 };
    let resp = mgr.send_event_to_plugin(&name, &evt).await.expect("event call ok");

    // Our minimal handler returns host rc directly; success expected (0)
    assert_eq!(resp.success, true);

    // Ensure host observed the execution request
    let seen = host.executed.lock().unwrap().clone();
    assert!(seen.iter().any(|c| c.contains("echo hi")));
}

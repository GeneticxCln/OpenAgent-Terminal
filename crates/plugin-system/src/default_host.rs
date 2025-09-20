use crate::api::{CommandOutput, PluginError};
use crate::host::{HostInterface, LogLevel, NetRequest, NetResponse, Notification, TerminalState};
use crate::permissions::SecurityPolicy;
use regex::Regex;
use reqwest::blocking::Client as HttpClient;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Default host implementation that enforces SecurityPolicy and executes
/// requested operations for WASM plugins.
pub struct DefaultHost {
    policy: SecurityPolicy,
    http: HttpClient,
}

impl DefaultHost {
    pub fn new(policy: SecurityPolicy) -> Result<Self, PluginError> {
        let http = HttpClient::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| PluginError::Internal(format!("http client: {e}")))?;
        Ok(Self { policy, http })
    }

    fn ensure_read_allowed(&self, path: &str, max_bytes: Option<u64>) -> Result<(), PluginError> {
        let p = Path::new(path);
        // Glob-based allowlist
        let ok_glob = self
            .policy
            .can_read_path(p)
            .map_err(|e| PluginError::PermissionDenied(format!("path check: {e:?}")))?;
        if !ok_glob {
            return Err(PluginError::PermissionDenied("read not allowed".into()));
        }
        if let Some(rules) = (!self.policy.permissions.file_read_allow.is_empty())
            .then_some(&self.policy.permissions.file_read_allow)
        {
            // At least one rule must match the path
            let mut matched = false;
            for rule in rules {
                let root = Path::new(&rule.root);
                if rule.recursive {
                    if let Ok(canon) = dunce::canonicalize(p) {
                        if let Ok(root_canon) = dunce::canonicalize(root) {
                            if canon.starts_with(&root_canon) {
                                matched = true;
                                break;
                            }
                        }
                    }
                } else if let Ok(canon) = dunce::canonicalize(p) {
                    if let Ok(root_canon) = dunce::canonicalize(root) {
                        if canon == root_canon {
                            matched = true;
                            break;
                        }
                    }
                }
            }
            if !matched {
                return Err(PluginError::PermissionDenied("read outside allowed roots".into()));
            }
        }
        if let Some(max) = max_bytes {
            // We'll enforce when reading bytes below; this is a soft pre-check
            let _ = max;
        }
        Ok(())
    }

    fn exec_allowed(
        &self,
        cmd: &str,
        args: &[String],
        cwd: Option<&str>,
    ) -> Result<(u64, usize), PluginError> {
        if !self.policy.permissions.execute_commands {
            return Err(PluginError::PermissionDenied("exec disabled".into()));
        }
        if self.policy.permissions.exec_allow.is_empty() {
            return Err(PluginError::PermissionDenied("no exec rules configured".into()));
        }
        for rule in &self.policy.permissions.exec_allow {
            if rule.cmd != cmd {
                continue;
            }
            if let Some(ref pat) = rule.args_pattern {
                let re = Regex::new(pat)
                    .map_err(|e| PluginError::InvalidInput(format!("bad args_pattern: {e}")))?;
                let joined = args.join(" ");
                if !re.is_match(&joined) {
                    continue;
                }
            }
            if let Some(cwd_str) = cwd {
                if !rule.cwd_allow.is_empty() {
                    let mut ok = false;
                    for allow in &rule.cwd_allow {
                        if let (Ok(c), Ok(a)) =
                            (dunce::canonicalize(cwd_str), dunce::canonicalize(allow))
                        {
                            if c.starts_with(a) {
                                ok = true;
                                break;
                            }
                        }
                    }
                    if !ok {
                        continue;
                    }
                }
            }
            let timeout =
                rule.timeout_ms.or(Some(self.policy.permissions.timeout_ms)).unwrap_or(5000);
            let max_out = rule.max_output_bytes.unwrap_or(256 * 1024) as usize;
            return Ok((timeout, max_out));
        }
        Err(PluginError::PermissionDenied("exec rule not matched".into()))
    }
}

impl HostInterface for DefaultHost {
    fn log(&self, level: LogLevel, message: &str) {
        match level {
            LogLevel::Debug => tracing::debug!(target: "plugin", "{}", message),
            LogLevel::Info => tracing::info!(target: "plugin", "{}", message),
            LogLevel::Warning => tracing::warn!(target: "plugin", "{}", message),
            LogLevel::Error => tracing::error!(target: "plugin", "{}", message),
        }
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, PluginError> {
        // Enforce path policy
        let max = self.policy.permissions.file_read_allow.iter().filter_map(|p| p.max_bytes).min();
        self.ensure_read_allowed(path, max)?;

        let mut f = fs::File::open(path).map_err(PluginError::IoError)?;
        let mut buf = Vec::new();
        if let Some(limit) = max {
            let mut take = f.by_ref().take(limit);
            take.read_to_end(&mut buf).map_err(PluginError::IoError)?;
        } else {
            f.read_to_end(&mut buf).map_err(PluginError::IoError)?;
        }
        Ok(buf)
    }

    fn write_file(&self, _path: &str, _data: &[u8]) -> Result<(), PluginError> {
        Err(PluginError::PermissionDenied("write disabled".into()))
    }

    fn execute_command(&self, command: &str) -> Result<CommandOutput, PluginError> {
        // Convenience: call spawn with zero args
        self.spawn(command, &[], None)
    }

    fn spawn(
        &self,
        cmd: &str,
        args: &[String],
        cwd: Option<&str>,
    ) -> Result<CommandOutput, PluginError> {
        let (timeout_ms, max_output) = self.exec_allowed(cmd, args, cwd)?;
        let start = Instant::now();
        let mut command = Command::new(cmd);
        command.args(args);
        if let Some(c) = cwd {
            command.current_dir(c);
        }
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child =
            command.spawn().map_err(|e| PluginError::CommandFailed(format!("spawn: {e}")))?;
        // Simple blocking wait with timeout
        let timeout = Duration::from_millis(timeout_ms);
        let end_time = start + timeout;
        while Instant::now() < end_time {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let mut out = String::new();
                    let mut err = String::new();
                    if let Some(mut so) = child.stdout.take() {
                        let _ = so.read_to_string(&mut out);
                    }
                    if let Some(mut se) = child.stderr.take() {
                        let _ = se.read_to_string(&mut err);
                    }
                    if out.len() > max_output {
                        out.truncate(max_output);
                    }
                    if err.len() > max_output {
                        err.truncate(max_output);
                    }
                    return Ok(CommandOutput {
                        stdout: out,
                        stderr: err,
                        exit_code: status.code().unwrap_or(-1),
                        execution_time_ms: start.elapsed().as_millis() as u64,
                    });
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => return Err(PluginError::CommandFailed(format!("wait: {e}"))),
            }
        }
        // Kill on timeout
        let _ = child.kill();
        Err(PluginError::Timeout)
    }

    fn net_fetch(&self, req: NetRequest) -> Result<NetResponse, PluginError> {
        if !self.policy.permissions.network {
            return Err(PluginError::PermissionDenied("network disabled".into()));
        }
        // Basic URL parse and domain allowlist check
        let url = req
            .url
            .parse::<reqwest::Url>()
            .map_err(|e| PluginError::InvalidInput(format!("bad url: {e}")))?;
        let domain = url.host_str().unwrap_or("");
        if !self.policy.permissions.net_allow_domains.is_empty()
            && !self
                .policy
                .permissions
                .net_allow_domains
                .iter()
                .any(|d| d.eq_ignore_ascii_case(domain))
        {
            return Err(PluginError::PermissionDenied("domain not allowed".into()));
        }
        let method = req.method.to_uppercase();
        if !self.policy.permissions.net_methods_allow.is_empty()
            && !self
                .policy
                .permissions
                .net_methods_allow
                .iter()
                .any(|m| m.eq_ignore_ascii_case(&method))
        {
            return Err(PluginError::PermissionDenied("method not allowed".into()));
        }
        let timeout_ms = req
            .timeout_ms
            .or(self.policy.permissions.net_timeout_ms)
            .unwrap_or(self.policy.permissions.timeout_ms);
        let client = self.http.clone();
        let mut builder = match method.as_str() {
            "GET" => client.get(url.clone()),
            "POST" => client.post(url.clone()),
            "PUT" => client.put(url.clone()),
            "DELETE" => client.delete(url.clone()),
            _ => return Err(PluginError::InvalidInput("unsupported method".into())),
        };
        for (k, v) in &req.headers {
            builder = builder.header(k, v);
        }
        if let Some(body) = &req.body {
            builder = builder.body(body.clone());
        }
        let resp = builder
            .timeout(Duration::from_millis(timeout_ms))
            .send()
            .map_err(|e| PluginError::IoError(std::io::Error::other(format!("http: {e}"))))?;
        let status = resp.status().as_u16();
        let mut headers = Vec::new();
        for (k, v) in resp.headers().iter() {
            headers.push((k.to_string(), v.to_str().unwrap_or("").to_string()));
        }
        let mut body = Vec::new();
        let max_bytes = req
            .max_response_bytes
            .or(self.policy.permissions.net_max_response_bytes)
            .unwrap_or(1024 * 1024);
        let reader = resp;
        let mut take = reader.take(max_bytes);
        use std::io::Read as _;
        take.read_to_end(&mut body)
            .map_err(|e| PluginError::IoError(std::io::Error::other(format!("http read: {e}"))))?;
        Ok(NetResponse { status, headers, body })
    }

    fn get_terminal_state(&self) -> TerminalState {
        TerminalState {
            current_dir: String::new(),
            environment: Default::default(),
            shell: String::new(),
            terminal_size: (80, 24),
            is_interactive: true,
            command_history: vec![],
        }
    }

    fn show_notification(&self, _notification: Notification) -> Result<(), PluginError> {
        Ok(())
    }

    fn store_data(&self, _key: &str, _value: &[u8]) -> Result<(), PluginError> {
        Ok(())
    }

    fn retrieve_data(&self, _key: &str) -> Result<Option<Vec<u8>>, PluginError> {
        Ok(None)
    }
}

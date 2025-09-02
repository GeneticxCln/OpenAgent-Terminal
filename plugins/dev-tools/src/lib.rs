// Developer Tools Plugin - Comprehensive support for Node.js, Python, Kubernetes, and system monitoring

use plugin_api::*;
use regex::Regex;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct DevToolsPlugin {
    config: Option<PluginConfig>,
    node_version: Option<String>,
    python_version: Option<String>,
    kubectl_version: Option<String>,
    cache: DevToolsCache,
}

struct DevToolsCache {
    npm_packages: Vec<String>,
    pip_packages: Vec<String>,
    k8s_contexts: Vec<String>,
    k8s_namespaces: Vec<String>,
    last_update: std::time::Instant,
}

impl Default for DevToolsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl DevToolsPlugin {
    pub fn new() -> Self {
        Self {
            config: None,
            node_version: None,
            python_version: None,
            kubectl_version: None,
            cache: DevToolsCache {
                npm_packages: Vec::new(),
                pip_packages: Vec::new(),
                k8s_contexts: Vec::new(),
                k8s_namespaces: Vec::new(),
                last_update: std::time::Instant::now() - std::time::Duration::from_secs(60),
            },
        }
    }

    fn run_command(&self, cmd: &str, args: &[&str]) -> Result<String, PluginError> {
        let output = Command::new(cmd)
            .args(args)
            .output()
            .map_err(|e| PluginError::CommandError(format!("Failed to run {}: {}", cmd, e)))?;

        if !output.status.success() {
            return Err(PluginError::CommandError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn update_cache(&mut self) {
        if self.cache.last_update.elapsed() < std::time::Duration::from_secs(30) {
            return;
        }

        // Update NPM packages
        if let Ok(output) = self.run_command("npm", &["list", "-g", "--depth=0", "--json"]) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output) {
                if let Some(deps) = json["dependencies"].as_object() {
                    self.cache.npm_packages = deps.keys().cloned().collect();
                }
            }
        }

        // Update pip packages
        if let Ok(output) = self.run_command("pip", &["list", "--format=json"]) {
            if let Ok(packages) = serde_json::from_str::<Vec<serde_json::Value>>(&output) {
                self.cache.pip_packages =
                    packages.iter().filter_map(|p| p["name"].as_str().map(String::from)).collect();
            }
        }

        // Update Kubernetes contexts
        if let Ok(output) = self.run_command("kubectl", &["config", "get-contexts", "-o", "name"]) {
            self.cache.k8s_contexts = output.lines().map(String::from).collect();
        }

        // Update Kubernetes namespaces
        if let Ok(output) = self.run_command(
            "kubectl",
            &["get", "namespaces", "-o", "jsonpath={.items[*].metadata.name}"],
        ) {
            self.cache.k8s_namespaces = output.split_whitespace().map(String::from).collect();
        }

        self.cache.last_update = std::time::Instant::now();
    }

    fn get_system_stats(&self) -> HashMap<String, String> {
        let mut stats = HashMap::new();

        // CPU usage
        if let Ok(loadavg) = fs::read_to_string("/proc/loadavg") {
            let parts: Vec<&str> = loadavg.split_whitespace().collect();
            if parts.len() >= 3 {
                stats.insert("load_1min".to_string(), parts[0].to_string());
                stats.insert("load_5min".to_string(), parts[1].to_string());
                stats.insert("load_15min".to_string(), parts[2].to_string());
            }
        }

        // Memory usage
        if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
            let re = Regex::new(r"(\w+):\s+(\d+)").unwrap();
            for cap in re.captures_iter(&meminfo) {
                if &cap[1] == "MemTotal" || &cap[1] == "MemAvailable" {
                    stats.insert(cap[1].to_lowercase(), cap[2].to_string());
                }
            }
        }

        // Disk usage
        if let Ok(output) = self.run_command("df", &["-h", "/"]) {
            let lines: Vec<&str> = output.lines().collect();
            if lines.len() > 1 {
                let parts: Vec<&str> = lines[1].split_whitespace().collect();
                if parts.len() >= 5 {
                    stats.insert("disk_used".to_string(), parts[2].to_string());
                    stats.insert("disk_available".to_string(), parts[3].to_string());
                    stats.insert("disk_percent".to_string(), parts[4].to_string());
                }
            }
        }

        stats
    }
}

impl Plugin for DevToolsPlugin {
    fn metadata(&self) -> PluginMetadata {
        plugin_metadata! {
            name: "dev-tools",
            version: "1.0.0",
            author: "OpenAgent Team",
            description: "Developer tools for Node.js, Python, Kubernetes, and system monitoring",
            capabilities: {
                completions: true,
                context_provider: true,
                commands: vec![
                    "node-info".to_string(),
                    "python-info".to_string(),
                    "k8s-status".to_string(),
                    "system-stats".to_string(),
                    "dev-check".to_string()
                ],
                hooks: vec![HookType::PreCommand]
            },
            permissions: {
                execute_commands: true,
                read_files: vec![
                    "package.json".to_string(),
                    "requirements.txt".to_string(),
                    "*.yaml".to_string(),
                    "*.yml".to_string(),
                    "/proc/*".to_string()
                ],
                environment_variables: vec![
                    "NODE_*".to_string(),
                    "PYTHON_*".to_string(),
                    "KUBECONFIG".to_string()
                ]
            }
        }
    }

    fn init(&mut self, config: PluginConfig) -> Result<(), PluginError> {
        self.config = Some(config);

        // Detect installed versions
        self.node_version = self.run_command("node", &["--version"]).ok();
        self.python_version = self
            .run_command("python", &["--version"])
            .ok()
            .or_else(|| self.run_command("python3", &["--version"]).ok());
        self.kubectl_version =
            self.run_command("kubectl", &["version", "--client", "--short"]).ok();

        // Initial cache update
        self.update_cache();

        Ok(())
    }

    fn provide_completions(&self, context: CompletionContext) -> Vec<Completion> {
        let mut completions = Vec::new();
        let parts: Vec<&str> = context.input.split_whitespace().collect();

        if parts.is_empty() {
            return completions;
        }

        match parts[0] {
            "npm" => {
                if parts.len() == 1 {
                    let npm_commands = vec![
                        ("install", "Install packages", "📦"),
                        ("run", "Run scripts", "▶️"),
                        ("test", "Run tests", "🧪"),
                        ("start", "Start application", "🚀"),
                        ("build", "Build project", "🔨"),
                        ("publish", "Publish package", "📤"),
                        ("audit", "Security audit", "🔒"),
                        ("outdated", "Check outdated packages", "🔄"),
                    ];

                    for (cmd, desc, icon) in npm_commands {
                        completions.push(Completion {
                            value: format!("npm {}", cmd),
                            display: cmd.to_string(),
                            description: Some(desc.to_string()),
                            kind: CompletionKind::Command,
                            score: 1.0,
                            icon: Some(icon.to_string()),
                        });
                    }
                } else if parts[1] == "run" && parts.len() == 2 {
                    // Suggest package.json scripts
                    if let Ok(package_json) = fs::read_to_string("package.json") {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&package_json) {
                            if let Some(scripts) = json["scripts"].as_object() {
                                for (script_name, _) in scripts {
                                    completions.push(Completion {
                                        value: format!("npm run {}", script_name),
                                        display: script_name.clone(),
                                        description: Some("NPM script".to_string()),
                                        kind: CompletionKind::Argument,
                                        score: 0.9,
                                        icon: Some("📜".to_string()),
                                    });
                                }
                            }
                        }
                    }
                }
            },

            "pip" | "pip3" => {
                if parts.len() == 1 {
                    let pip_commands = vec![
                        ("install", "Install packages", "📦"),
                        ("uninstall", "Uninstall packages", "🗑️"),
                        ("list", "List installed packages", "📋"),
                        ("freeze", "Output installed packages", "❄️"),
                        ("show", "Show package details", "🔍"),
                        ("search", "Search PyPI", "🔎"),
                        ("check", "Check dependencies", "✅"),
                    ];

                    for (cmd, desc, icon) in pip_commands {
                        completions.push(Completion {
                            value: format!("{} {}", parts[0], cmd),
                            display: cmd.to_string(),
                            description: Some(desc.to_string()),
                            kind: CompletionKind::Command,
                            score: 1.0,
                            icon: Some(icon.to_string()),
                        });
                    }
                }
            },

            "kubectl" => {
                if parts.len() == 1 {
                    let kubectl_commands = vec![
                        ("get", "Display resources", "📋"),
                        ("describe", "Show resource details", "📝"),
                        ("create", "Create resources", "➕"),
                        ("apply", "Apply configuration", "✅"),
                        ("delete", "Delete resources", "🗑️"),
                        ("logs", "Print container logs", "📜"),
                        ("exec", "Execute in container", "⚡"),
                        ("port-forward", "Forward ports", "🔌"),
                        ("scale", "Scale resources", "📊"),
                        ("rollout", "Manage rollouts", "🔄"),
                    ];

                    for (cmd, desc, icon) in kubectl_commands {
                        completions.push(Completion {
                            value: format!("kubectl {}", cmd),
                            display: cmd.to_string(),
                            description: Some(desc.to_string()),
                            kind: CompletionKind::Command,
                            score: 1.0,
                            icon: Some(icon.to_string()),
                        });
                    }
                } else if parts[1] == "get" && parts.len() == 2 {
                    let resources = vec![
                        ("pods", "Pod resources"),
                        ("services", "Service resources"),
                        ("deployments", "Deployment resources"),
                        ("configmaps", "ConfigMap resources"),
                        ("secrets", "Secret resources"),
                        ("ingresses", "Ingress resources"),
                        ("namespaces", "Namespace resources"),
                        ("nodes", "Node resources"),
                    ];

                    for (resource, desc) in resources {
                        completions.push(Completion {
                            value: format!("kubectl get {}", resource),
                            display: resource.to_string(),
                            description: Some(desc.to_string()),
                            kind: CompletionKind::Argument,
                            score: 0.9,
                            icon: Some("☸️".to_string()),
                        });
                    }
                }
            },

            "python" | "python3" => {
                if parts.len() == 1 {
                    completions.push(Completion {
                        value: format!("{} -m", parts[0]),
                        display: "-m".to_string(),
                        description: Some("Run module as script".to_string()),
                        kind: CompletionKind::Option,
                        score: 0.9,
                        icon: Some("🐍".to_string()),
                    });
                }
            },

            _ => {},
        }

        completions
    }

    fn collect_context(&self, request: ContextRequest) -> Option<Context> {
        let _config = self.config.as_ref()?;
        let mut context_data = HashMap::new();

        // Node.js context
        if request.purpose.contains("node") || request.purpose.contains("all") {
            if let Some(ref version) = self.node_version {
                context_data.insert("node_version", json!(version));
            }

            // Package.json info
            if let Ok(package_json) = fs::read_to_string("package.json") {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&package_json) {
                    let mut node_info = HashMap::new();
                    node_info.insert("name", json["name"].as_str().unwrap_or("").to_string());
                    node_info.insert("version", json["version"].as_str().unwrap_or("").to_string());

                    if let Some(scripts) = json["scripts"].as_object() {
                        let script_names: Vec<String> = scripts.keys().cloned().collect();
                        node_info.insert("scripts", script_names.join(", "));
                    }

                    context_data.insert("node_project", json!(node_info));
                }
            }

            context_data.insert("npm_global_packages", json!(self.cache.npm_packages));
        }

        // Python context
        if request.purpose.contains("python") || request.purpose.contains("all") {
            if let Some(ref version) = self.python_version {
                context_data.insert("python_version", json!(version));
            }

            // Requirements.txt info
            if let Ok(requirements) = fs::read_to_string("requirements.txt") {
                let packages: Vec<String> = requirements
                    .lines()
                    .filter(|l| !l.starts_with('#') && !l.is_empty())
                    .map(String::from)
                    .collect();
                context_data.insert("python_requirements", json!(packages));
            }

            context_data.insert("pip_packages", json!(self.cache.pip_packages));
        }

        // Kubernetes context
        if request.purpose.contains("kubernetes")
            || request.purpose.contains("k8s")
            || request.purpose.contains("all")
        {
            if let Some(ref version) = self.kubectl_version {
                context_data.insert("kubectl_version", json!(version));
            }

            context_data.insert("k8s_contexts", json!(self.cache.k8s_contexts));
            context_data.insert("k8s_namespaces", json!(self.cache.k8s_namespaces));

            // Current context
            if let Ok(current_context) = self.run_command("kubectl", &["config", "current-context"])
            {
                context_data.insert("k8s_current_context", json!(current_context));
            }
        }

        // System monitoring context
        if request.purpose.contains("system")
            || request.purpose.contains("monitoring")
            || request.purpose.contains("all")
        {
            let stats = self.get_system_stats();
            context_data.insert("system_stats", json!(stats));
        }

        let content = serde_json::to_string_pretty(&context_data).ok()?;
        let size = content.len();

        Some(Context {
            name: "Developer Tools Context".to_string(),
            content,
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("plugin".to_string(), "dev-tools".to_string());
                meta
            },
            sensitivity: SensitivityLevel::Internal,
            size_bytes: size,
        })
    }

    fn execute_command(&self, cmd: &str, _args: &[String]) -> Result<CommandOutput, PluginError> {
        let start = std::time::Instant::now();

        match cmd {
            "node-info" => {
                let mut output = String::new();
                output.push_str("🟢 Node.js Environment\n");
                output.push_str("=".repeat(40).as_str());
                output.push_str("\n\n");

                if let Some(ref version) = self.node_version {
                    output.push_str(&format!("Node Version: {}\n", version));
                }

                if let Ok(npm_version) = self.run_command("npm", &["--version"]) {
                    output.push_str(&format!("NPM Version: {}\n", npm_version));
                }

                // Check for package.json
                if Path::new("package.json").exists() {
                    output.push_str("\n📦 Project Info:\n");

                    if let Ok(package_json) = fs::read_to_string("package.json") {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&package_json) {
                            output.push_str(&format!(
                                "  Name: {}\n",
                                json["name"].as_str().unwrap_or("N/A")
                            ));
                            output.push_str(&format!(
                                "  Version: {}\n",
                                json["version"].as_str().unwrap_or("N/A")
                            ));

                            if let Some(scripts) = json["scripts"].as_object() {
                                output.push_str("\n📜 Available Scripts:\n");
                                for (name, _) in scripts {
                                    output.push_str(&format!("  - npm run {}\n", name));
                                }
                            }
                        }
                    }
                }

                Ok(CommandOutput {
                    stdout: output,
                    stderr: String::new(),
                    exit_code: 0,
                    execution_time_ms: start.elapsed().as_millis() as u64,
                })
            },

            "python-info" => {
                let mut output = String::new();
                output.push_str("🐍 Python Environment\n");
                output.push_str("=".repeat(40).as_str());
                output.push_str("\n\n");

                if let Some(ref version) = self.python_version {
                    output.push_str(&format!("Python Version: {}\n", version));
                }

                if let Ok(pip_version) = self.run_command("pip", &["--version"]) {
                    output.push_str(&format!("Pip: {}\n", pip_version));
                }

                // Virtual environment check
                if let Ok(venv) = std::env::var("VIRTUAL_ENV") {
                    output.push_str(&format!("\n🔧 Virtual Environment: {}\n", venv));
                }

                // Requirements file
                if Path::new("requirements.txt").exists() {
                    output.push_str("\n📋 Requirements.txt found\n");

                    if let Ok(reqs) = fs::read_to_string("requirements.txt") {
                        let count =
                            reqs.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).count();
                        output.push_str(&format!("  {} packages specified\n", count));
                    }
                }

                Ok(CommandOutput {
                    stdout: output,
                    stderr: String::new(),
                    exit_code: 0,
                    execution_time_ms: start.elapsed().as_millis() as u64,
                })
            },

            "k8s-status" => {
                let mut output = String::new();
                output.push_str("☸️  Kubernetes Status\n");
                output.push_str("=".repeat(40).as_str());
                output.push_str("\n\n");

                if self.kubectl_version.is_none() {
                    return Ok(CommandOutput {
                        stdout:
                            "kubectl not found. Please install kubectl to use Kubernetes features."
                                .to_string(),
                        stderr: String::new(),
                        exit_code: 1,
                        execution_time_ms: start.elapsed().as_millis() as u64,
                    });
                }

                if let Ok(context) = self.run_command("kubectl", &["config", "current-context"]) {
                    output.push_str(&format!("Current Context: {}\n", context));
                }

                if let Ok(cluster_info) =
                    self.run_command("kubectl", &["cluster-info", "--request-timeout=2s"])
                {
                    output.push_str("\n🌐 Cluster Info:\n");
                    for line in cluster_info.lines().take(2) {
                        output.push_str(&format!("  {}\n", line));
                    }
                }

                // Node status
                if let Ok(nodes) = self.run_command("kubectl", &["get", "nodes", "--no-headers"]) {
                    let node_count = nodes.lines().count();
                    output.push_str(&format!("\n📊 Nodes: {} total\n", node_count));
                }

                // Pod status
                if let Ok(pods) = self
                    .run_command("kubectl", &["get", "pods", "--all-namespaces", "--no-headers"])
                {
                    let pod_count = pods.lines().count();
                    let running = pods.lines().filter(|l| l.contains("Running")).count();
                    output
                        .push_str(&format!("📦 Pods: {} total ({} running)\n", pod_count, running));
                }

                Ok(CommandOutput {
                    stdout: output,
                    stderr: String::new(),
                    exit_code: 0,
                    execution_time_ms: start.elapsed().as_millis() as u64,
                })
            },

            "system-stats" => {
                let mut output = String::new();
                output.push_str("💻 System Statistics\n");
                output.push_str("=".repeat(40).as_str());
                output.push_str("\n\n");

                let stats = self.get_system_stats();

                // CPU Load
                output.push_str("🔥 CPU Load:\n");
                output.push_str(&format!(
                    "  1 min:  {}\n",
                    stats.get("load_1min").unwrap_or(&"N/A".to_string())
                ));
                output.push_str(&format!(
                    "  5 min:  {}\n",
                    stats.get("load_5min").unwrap_or(&"N/A".to_string())
                ));
                output.push_str(&format!(
                    "  15 min: {}\n",
                    stats.get("load_15min").unwrap_or(&"N/A".to_string())
                ));

                // Memory
                if let (Some(total), Some(available)) =
                    (stats.get("memtotal"), stats.get("memavailable"))
                {
                    let total_mb = total.parse::<u64>().unwrap_or(0) / 1024;
                    let available_mb = available.parse::<u64>().unwrap_or(0) / 1024;
                    let used_mb = total_mb - available_mb;
                    let percent = ((used_mb as f64 / total_mb as f64) * 100.0) as u32;

                    output.push_str("\n💾 Memory:\n");
                    output.push_str(&format!("  Total: {} MB\n", total_mb));
                    output.push_str(&format!("  Used:  {} MB ({}%)\n", used_mb, percent));
                    output.push_str(&format!("  Free:  {} MB\n", available_mb));
                }

                // Disk
                output.push_str("\n💿 Disk Usage (/):\n");
                output.push_str(&format!(
                    "  Used:      {}\n",
                    stats.get("disk_used").unwrap_or(&"N/A".to_string())
                ));
                output.push_str(&format!(
                    "  Available: {}\n",
                    stats.get("disk_available").unwrap_or(&"N/A".to_string())
                ));
                output.push_str(&format!(
                    "  Usage:     {}\n",
                    stats.get("disk_percent").unwrap_or(&"N/A".to_string())
                ));

                // Network interfaces
                if let Ok(interfaces) = self.run_command("ip", &["-br", "addr"]) {
                    output.push_str("\n🌐 Network Interfaces:\n");
                    for line in interfaces.lines() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 && parts[1] == "UP" {
                            output.push_str(&format!("  {} - {}\n", parts[0], parts[2]));
                        }
                    }
                }

                Ok(CommandOutput {
                    stdout: output,
                    stderr: String::new(),
                    exit_code: 0,
                    execution_time_ms: start.elapsed().as_millis() as u64,
                })
            },

            "dev-check" => {
                let mut output = String::new();
                output.push_str("🔧 Development Environment Check\n");
                output.push_str("=".repeat(40).as_str());
                output.push_str("\n\n");

                // Check various tools
                let tools = vec![
                    ("node", "Node.js"),
                    ("npm", "NPM"),
                    ("python", "Python"),
                    ("pip", "Pip"),
                    ("git", "Git"),
                    ("docker", "Docker"),
                    ("kubectl", "Kubectl"),
                    ("cargo", "Rust/Cargo"),
                    ("go", "Go"),
                    ("java", "Java"),
                ];

                for (cmd, name) in tools {
                    let status = if self.run_command(cmd, &["--version"]).is_ok() {
                        "✅ Installed"
                    } else {
                        "❌ Not found"
                    };
                    output.push_str(&format!("{:<15} {}\n", format!("{}:", name), status));
                }

                Ok(CommandOutput {
                    stdout: output,
                    stderr: String::new(),
                    exit_code: 0,
                    execution_time_ms: start.elapsed().as_millis() as u64,
                })
            },

            _ => Err(PluginError::CommandError(format!("Unknown command: {}", cmd))),
        }
    }

    fn handle_hook(&mut self, hook: HookEvent) -> Result<HookResponse, PluginError> {
        match hook.hook_type {
            HookType::PreCommand => {
                // Update cache before relevant commands
                if let HookData::Command { ref cmd, .. } = hook.data {
                    if cmd == "npm" || cmd == "pip" || cmd == "kubectl" {
                        self.update_cache();
                    }
                }

                Ok(HookResponse {
                    modified_command: None,
                    prevent_execution: false,
                    messages: vec![],
                })
            },
            _ => Ok(HookResponse {
                modified_command: None,
                prevent_execution: false,
                messages: vec![],
            }),
        }
    }

    fn cleanup(&mut self) -> Result<(), PluginError> {
        self.cache.npm_packages.clear();
        self.cache.pip_packages.clear();
        self.cache.k8s_contexts.clear();
        self.cache.k8s_namespaces.clear();
        Ok(())
    }
}

// Register the plugin
register_plugin!(DevToolsPlugin);

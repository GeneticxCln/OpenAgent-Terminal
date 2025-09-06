// Docker Helper Plugin - Provides Docker completions, context, and utilities

use plugin_api::*;
use regex::Regex;
use serde_json::json;
use std::collections::HashMap;
use std::process::Command;

pub struct DockerHelperPlugin {
    config: Option<PluginConfig>,
    docker_binary: String,
    compose_binary: String,
    cached_images: Vec<DockerImage>,
    cached_containers: Vec<DockerContainer>,
    last_cache_update: std::time::Instant,
}

#[derive(Clone, Debug)]
struct DockerImage {
    repository: String,
    tag: String,
    id: String,
    size: String,
}

#[derive(Clone, Debug)]
struct DockerContainer {
    id: String,
    name: String,
    image: String,
    status: String,
    ports: String,
}

impl Default for DockerHelperPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl DockerHelperPlugin {
    pub fn new() -> Self {
        Self {
            config: None,
            docker_binary: "docker".to_string(),
            compose_binary: "docker-compose".to_string(),
            cached_images: Vec::new(),
            cached_containers: Vec::new(),
            last_cache_update: std::time::Instant::now() - std::time::Duration::from_secs(60),
        }
    }

    fn run_docker_command(&self, args: &[&str]) -> Result<String, PluginError> {
        let output = Command::new(&self.docker_binary)
            .args(args)
            .output()
            .map_err(|e| PluginError::CommandError(format!("Failed to run docker: {}", e)))?;

        if !output.status.success() {
            return Err(PluginError::CommandError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn update_cache(&mut self) {
        // Update cache every 10 seconds
        if self.last_cache_update.elapsed() < std::time::Duration::from_secs(10) {
            return;
        }

        // Cache images
        if let Ok(output) = self.run_docker_command(&[
            "images",
            "--format",
            "{{.Repository}}|{{.Tag}}|{{.ID}}|{{.Size}}",
        ]) {
            self.cached_images.clear();
            for line in output.lines() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 4 {
                    self.cached_images.push(DockerImage {
                        repository: parts[0].to_string(),
                        tag: parts[1].to_string(),
                        id: parts[2].to_string(),
                        size: parts[3].to_string(),
                    });
                }
            }
        }

        // Cache containers
        if let Ok(output) = self.run_docker_command(&[
            "ps",
            "-a",
            "--format",
            "{{.ID}}|{{.Names}}|{{.Image}}|{{.Status}}|{{.Ports}}",
        ]) {
            self.cached_containers.clear();
            for line in output.lines() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 4 {
                    self.cached_containers.push(DockerContainer {
                        id: parts[0].to_string(),
                        name: parts[1].to_string(),
                        image: parts[2].to_string(),
                        status: parts[3].to_string(),
                        ports: parts.get(4).unwrap_or(&"").to_string(),
                    });
                }
            }
        }

        self.last_cache_update = std::time::Instant::now();
    }

    fn get_docker_stats(&self) -> Result<Vec<HashMap<String, String>>, PluginError> {
        let output = self.run_docker_command(&[
            "stats",
            "--no-stream",
            "--format",
            "{{.Container}}|{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}|{{.NetIO}}|{{.BlockIO}}",
        ])?;

        let mut stats = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 6 {
                let mut stat = HashMap::new();
                stat.insert("container_id".to_string(), parts[0].to_string());
                stat.insert("name".to_string(), parts[1].to_string());
                stat.insert("cpu_percent".to_string(), parts[2].to_string());
                stat.insert("memory_usage".to_string(), parts[3].to_string());
                stat.insert("network_io".to_string(), parts[4].to_string());
                stat.insert("block_io".to_string(), parts[5].to_string());
                stats.push(stat);
            }
        }

        Ok(stats)
    }

    fn get_docker_networks(&self) -> Vec<String> {
        if let Ok(output) = self.run_docker_command(&["network", "ls", "--format", "{{.Name}}"]) {
            output.lines().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        }
    }

    fn get_docker_volumes(&self) -> Vec<String> {
        if let Ok(output) = self.run_docker_command(&["volume", "ls", "--format", "{{.Name}}"]) {
            output.lines().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        }
    }
}

impl Plugin for DockerHelperPlugin {
    fn metadata(&self) -> PluginMetadata {
        plugin_metadata! {
            name: "docker-helper",
            version: "1.0.0",
            author: "OpenAgent Team",
            description: "Docker completions, monitoring, and utilities",
            capabilities: {
                completions: true,
                context_provider: true,
                commands: vec![
                    "docker-status".to_string(),
                    "docker-cleanup".to_string(),
                    "docker-stats".to_string()
                ],
                hooks: vec![HookType::PreCommand]
            },
            permissions: {
                execute_commands: true,
                environment_variables: vec!["DOCKER_*".to_string()],
                timeout_ms: 10000
            }
        }
    }

    fn init(&mut self, config: PluginConfig) -> Result<(), PluginError> {
        self.config = Some(config.clone());

        // Check for custom binary paths
        if let Some(docker_path) = config.settings.get("docker_binary") {
            if let Some(path) = docker_path.as_str() {
                self.docker_binary = path.to_string();
            }
        }

        if let Some(compose_path) = config.settings.get("compose_binary") {
            if let Some(path) = compose_path.as_str() {
                self.compose_binary = path.to_string();
            }
        }

        // Verify Docker is available
        Command::new(&self.docker_binary)
            .arg("--version")
            .output()
            .map_err(|e| PluginError::InitError(format!("Docker not found: {}", e)))?;

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

        if parts[0] == "docker" {
            if parts.len() == 1 {
                // Docker subcommands
                let subcommands = vec![
                    ("run", "Run a command in a new container", "🚀"),
                    ("ps", "List containers", "📋"),
                    ("images", "List images", "📦"),
                    ("pull", "Pull an image", "⬇️"),
                    ("push", "Push an image", "⬆️"),
                    ("build", "Build an image", "🔨"),
                    ("exec", "Run command in running container", "⚡"),
                    ("stop", "Stop containers", "🛑"),
                    ("start", "Start containers", "▶️"),
                    ("rm", "Remove containers", "🗑️"),
                    ("rmi", "Remove images", "🗑️"),
                    ("logs", "Fetch container logs", "📝"),
                    ("inspect", "Display detailed information", "🔍"),
                    ("network", "Manage networks", "🌐"),
                    ("volume", "Manage volumes", "💾"),
                    ("compose", "Docker Compose operations", "📚"),
                ];

                for (cmd, desc, icon) in subcommands {
                    completions.push(Completion {
                        value: format!("docker {}", cmd),
                        display: cmd.to_string(),
                        description: Some(desc.to_string()),
                        kind: CompletionKind::Command,
                        score: 1.0,
                        icon: Some(icon.to_string()),
                    });
                }
            } else {
                // Context-aware completions
                match parts[1] {
                    "run" | "pull" => {
                        // Suggest popular images
                        let popular_images = vec![
                            ("ubuntu:latest", "Ubuntu Linux"),
                            ("alpine:latest", "Alpine Linux"),
                            ("nginx:latest", "NGINX web server"),
                            ("postgres:latest", "PostgreSQL database"),
                            ("redis:latest", "Redis cache"),
                            ("node:latest", "Node.js runtime"),
                            ("python:3", "Python runtime"),
                            ("mysql:latest", "MySQL database"),
                        ];

                        for (image, desc) in popular_images {
                            completions.push(Completion {
                                value: format!("{} {}", context.input, image),
                                display: image.to_string(),
                                description: Some(desc.to_string()),
                                kind: CompletionKind::Argument,
                                score: 0.9,
                                icon: Some("📦".to_string()),
                            });
                        }

                        // Also suggest local images
                        for image in &self.cached_images {
                            let display = format!("{}:{}", image.repository, image.tag);
                            completions.push(Completion {
                                value: format!("{} {}", context.input, display),
                                display: display.clone(),
                                description: Some(format!("Local image ({})", image.size)),
                                kind: CompletionKind::Argument,
                                score: 0.8,
                                icon: Some("💾".to_string()),
                            });
                        }
                    },

                    "exec" | "stop" | "start" | "rm" | "logs" | "inspect" => {
                        // Suggest container names/IDs
                        for container in &self.cached_containers {
                            completions.push(Completion {
                                value: format!("{} {}", context.input, container.name),
                                display: container.name.clone(),
                                description: Some(format!(
                                    "{} ({})",
                                    container.image, container.status
                                )),
                                kind: CompletionKind::Argument,
                                score: 0.9,
                                icon: Some(
                                    if container.status.contains("Up") { "🟢" } else { "🔴" }
                                        .to_string(),
                                ),
                            });
                        }
                    },

                    "rmi" => {
                        // Suggest image names/IDs
                        for image in &self.cached_images {
                            let display = format!("{}:{}", image.repository, image.tag);
                            completions.push(Completion {
                                value: format!("{} {}", context.input, display),
                                display,
                                description: Some(format!("Size: {}", image.size)),
                                kind: CompletionKind::Argument,
                                score: 0.9,
                                icon: Some("📦".to_string()),
                            });
                        }
                    },

                    "network" => {
                        if parts.len() == 2 {
                            let network_cmds = vec![
                                ("ls", "List networks"),
                                ("create", "Create a network"),
                                ("rm", "Remove networks"),
                                ("inspect", "Display network details"),
                                ("connect", "Connect container to network"),
                                ("disconnect", "Disconnect container from network"),
                            ];

                            for (cmd, desc) in network_cmds {
                                completions.push(Completion {
                                    value: format!("docker network {}", cmd),
                                    display: cmd.to_string(),
                                    description: Some(desc.to_string()),
                                    kind: CompletionKind::Command,
                                    score: 0.9,
                                    icon: Some("🌐".to_string()),
                                });
                            }
                        }
                    },

                    _ => {},
                }
            }
        }

        completions
    }

    fn collect_context(&self, request: ContextRequest) -> Option<Context> {
        let mut context_data = HashMap::new();

        // Docker version
        if let Ok(version) = self.run_docker_command(&["version", "--format", "{{json .}}"]) {
            context_data.insert("docker_version", json!(version));
        }

        // Container information
        if request.purpose.contains("containers") || request.purpose.contains("all") {
            let containers: Vec<_> = self
                .cached_containers
                .iter()
                .map(|c| {
                    let mut container = HashMap::new();
                    container.insert("id", c.id.clone());
                    container.insert("name", c.name.clone());
                    container.insert("image", c.image.clone());
                    container.insert("status", c.status.clone());
                    container.insert("ports", c.ports.clone());
                    container
                })
                .collect();

            context_data.insert("containers", json!(containers));
        }

        // Image information
        if request.purpose.contains("images") || request.purpose.contains("all") {
            let images: Vec<_> = self
                .cached_images
                .iter()
                .map(|i| {
                    let mut image = HashMap::new();
                    image.insert("repository", i.repository.clone());
                    image.insert("tag", i.tag.clone());
                    image.insert("id", i.id.clone());
                    image.insert("size", i.size.clone());
                    image
                })
                .collect();

            context_data.insert("images", json!(images));
        }

        // Network information
        if request.purpose.contains("networks") || request.purpose.contains("all") {
            let networks = self.get_docker_networks();
            context_data.insert("networks", json!(networks));
        }

        // Volume information
        if request.purpose.contains("volumes") || request.purpose.contains("all") {
            let volumes = self.get_docker_volumes();
            context_data.insert("volumes", json!(volumes));
        }

        // Stats if requested
        if request.purpose.contains("stats") {
            if let Ok(stats) = self.get_docker_stats() {
                context_data.insert("container_stats", json!(stats));
            }
        }

        let content = serde_json::to_string_pretty(&context_data).ok()?;
        let size = content.len();

        if size > request.max_size_bytes {
            return None;
        }

        Some(Context {
            name: "Docker Environment Context".to_string(),
            content,
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("plugin".to_string(), "docker-helper".to_string());
                meta.insert(
                    "container_count".to_string(),
                    self.cached_containers.len().to_string(),
                );
                meta.insert("image_count".to_string(), self.cached_images.len().to_string());
                meta
            },
            sensitivity: SensitivityLevel::Internal,
            size_bytes: size,
        })
    }

    fn execute_command(&self, cmd: &str, _args: &[String]) -> Result<CommandOutput, PluginError> {
        let start = std::time::Instant::now();

        match cmd {
            "docker-status" => {
                let mut output = String::new();
                output.push_str("🐳 Docker Status\n");
                output.push_str("=".repeat(40).as_str());
                output.push_str("\n\n");

                // Docker version
                if let Ok(version) =
                    self.run_docker_command(&["version", "--format", "Server: {{.Server.Version}}"])
                {
                    output.push_str(&format!("📌 Docker Version: {}\n\n", version));
                }

                // Running containers
                let running: Vec<_> =
                    self.cached_containers.iter().filter(|c| c.status.contains("Up")).collect();

                output.push_str(&format!("🟢 Running Containers: {}\n", running.len()));
                for container in running.iter().take(5) {
                    output.push_str(&format!(
                        "  • {} ({}) - {}\n",
                        container.name, container.image, container.ports
                    ));
                }

                // Stopped containers
                let stopped = self.cached_containers.len() - running.len();
                output.push_str(&format!("\n🔴 Stopped Containers: {}\n", stopped));

                // Images
                output.push_str(&format!("\n📦 Images: {}\n", self.cached_images.len()));
                let total_size: f64 = self
                    .cached_images
                    .iter()
                    .filter_map(|i| {
                        // Parse size string (e.g., "123MB", "1.2GB")
                        let re = Regex::new(r"(\d+\.?\d*)(MB|GB|KB)").ok()?;
                        let caps = re.captures(&i.size)?;
                        let value: f64 = caps[1].parse().ok()?;
                        let unit = &caps[2];

                        Some(match unit {
                            "KB" => value / 1024.0,
                            "MB" => value,
                            "GB" => value * 1024.0,
                            _ => 0.0,
                        })
                    })
                    .sum();

                output.push_str(&format!("  Total size: ~{:.1} MB\n", total_size));

                // Networks and volumes
                let networks = self.get_docker_networks();
                let volumes = self.get_docker_volumes();
                output.push_str(&format!("\n🌐 Networks: {}\n", networks.len()));
                output.push_str(&format!("💾 Volumes: {}\n", volumes.len()));

                Ok(CommandOutput {
                    stdout: output,
                    stderr: String::new(),
                    exit_code: 0,
                    execution_time_ms: start.elapsed().as_millis() as u64,
                })
            },

            "docker-stats" => match self.get_docker_stats() {
                Ok(stats) => {
                    let mut output = String::new();
                    output.push_str("📊 Docker Container Statistics\n");
                    output.push_str("=".repeat(60).as_str());
                    output.push_str("\n\n");

                    if stats.is_empty() {
                        output.push_str("No running containers\n");
                    } else {
                        output.push_str(
                            "Container         CPU      Memory         Net I/O        Block I/O\n",
                        );
                        output.push_str("-".repeat(60).as_str());
                        output.push('\n');

                        for stat in stats {
                            output.push_str(&format!(
                                "{:<15} {:>7} {:>14} {:>13} {:>13}\n",
                                stat.get("name").unwrap_or(&String::new()),
                                stat.get("cpu_percent").unwrap_or(&String::new()),
                                stat.get("memory_usage").unwrap_or(&String::new()),
                                stat.get("network_io").unwrap_or(&String::new()),
                                stat.get("block_io").unwrap_or(&String::new()),
                            ));
                        }
                    }

                    Ok(CommandOutput {
                        stdout: output,
                        stderr: String::new(),
                        exit_code: 0,
                        execution_time_ms: start.elapsed().as_millis() as u64,
                    })
                },
                Err(e) => Ok(CommandOutput {
                    stdout: String::new(),
                    stderr: format!("Failed to get stats: {}", e),
                    exit_code: 1,
                    execution_time_ms: start.elapsed().as_millis() as u64,
                }),
            },

            "docker-cleanup" => {
                let mut output = String::new();
                output.push_str("🧹 Docker Cleanup\n");
                output.push_str("=".repeat(40).as_str());
                output.push_str("\n\n");

                // Remove stopped containers
                if let Ok(result) = self.run_docker_command(&["container", "prune", "-f"]) {
                    output.push_str(&format!("✅ Removed stopped containers:\n{}\n\n", result));
                }

                // Remove unused images
                if let Ok(result) = self.run_docker_command(&["image", "prune", "-f"]) {
                    output.push_str(&format!("✅ Removed unused images:\n{}\n\n", result));
                }

                // Remove unused volumes
                if let Ok(result) = self.run_docker_command(&["volume", "prune", "-f"]) {
                    output.push_str(&format!("✅ Removed unused volumes:\n{}\n\n", result));
                }

                // Remove unused networks
                if let Ok(result) = self.run_docker_command(&["network", "prune", "-f"]) {
                    output.push_str(&format!("✅ Removed unused networks:\n{}\n", result));
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
                // Update cache before docker commands
                if let HookData::Command { ref cmd, .. } = hook.data {
                    if cmd == "docker" {
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
        self.cached_images.clear();
        self.cached_containers.clear();
        Ok(())
    }
}

// Register the plugin
register_plugin!(DockerHelperPlugin);

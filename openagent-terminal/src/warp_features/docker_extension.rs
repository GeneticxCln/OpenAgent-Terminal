use std::process::{Command, Stdio};
use std::collections::HashMap;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};

/// Warp-style Docker extension for container management
pub struct DockerExtension {
    enabled: bool,
    docker_available: bool,
    cached_containers: Vec<DockerContainer>,
    last_cache_update: Option<SystemTime>,
    cache_duration_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub ports: Vec<String>,
    pub created: String,
    pub command: String,
    pub available_shells: Vec<Shell>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContainerStatus {
    Running,
    Stopped,
    Paused,
    Restarting,
    Dead,
    Created,
    Exited,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Sh,
    Ash,
}

#[derive(Debug, Clone)]
pub struct DockerConnectionOptions {
    pub container_id: String,
    pub shell: Shell,
    pub working_directory: Option<String>,
    pub user: Option<String>,
    pub environment_vars: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum DockerError {
    DockerNotInstalled,
    DockerNotRunning,
    ContainerNotFound(String),
    ShellNotAvailable(Shell),
    ConnectionFailed(String),
    PermissionDenied,
}

impl DockerExtension {
    pub fn new() -> Self {
        let docker_available = Self::check_docker_availability();
        
        Self {
            enabled: docker_available,
            docker_available,
            cached_containers: Vec::new(),
            last_cache_update: None,
            cache_duration_secs: 30, // Cache for 30 seconds
        }
    }

    /// Check if Docker is available on the system
    fn check_docker_availability() -> bool {
        match Command::new("docker")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) => status.success(),
            Err(_) => false,
        }
    }

    /// Check if Docker daemon is running
    pub fn is_docker_running(&self) -> bool {
        match Command::new("docker")
            .arg("info")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) => status.success(),
            Err(_) => false,
        }
    }

    /// Enable or disable the Docker extension
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled && self.docker_available;
    }

    /// Check if the Docker extension is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// List all Docker containers with caching
    pub fn list_containers(&mut self, include_stopped: bool) -> Result<Vec<DockerContainer>, DockerError> {
        if !self.enabled {
            return Err(DockerError::DockerNotInstalled);
        }

        if !self.is_docker_running() {
            return Err(DockerError::DockerNotRunning);
        }

        // Check cache validity
        if let Some(last_update) = self.last_cache_update {
            if let Ok(elapsed) = SystemTime::now().duration_since(last_update) {
                if elapsed.as_secs() < self.cache_duration_secs {
                    return Ok(self.cached_containers.clone());
                }
            }
        }

        // Refresh cache
        self.refresh_container_cache(include_stopped)?;
        Ok(self.cached_containers.clone())
    }

    /// Refresh the container cache
    fn refresh_container_cache(&mut self, include_stopped: bool) -> Result<(), DockerError> {
        let mut args = vec!["ps", "--format", "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}\t{{.CreatedAt}}\t{{.Command}}"];
        
        if include_stopped {
            args.push("--all");
        }

        let output = Command::new("docker")
            .args(&args)
            .output()
            .map_err(|e| DockerError::ConnectionFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(DockerError::ConnectionFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut containers = Vec::new();

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 7 {
                let container = DockerContainer {
                    id: parts[0].to_string(),
                    name: parts[1].to_string(),
                    image: parts[2].to_string(),
                    status: Self::parse_container_status(parts[3]),
                    ports: if parts[4].is_empty() { 
                        Vec::new() 
                    } else { 
                        parts[4].split(", ").map(|s| s.to_string()).collect() 
                    },
                    created: parts[5].to_string(),
                    command: parts[6].to_string(),
                    available_shells: Vec::new(), // Will be populated when needed
                };
                containers.push(container);
            }
        }

        self.cached_containers = containers;
        self.last_cache_update = Some(SystemTime::now());
        Ok(())
    }

    /// Parse container status string
    fn parse_container_status(status_str: &str) -> ContainerStatus {
        let status_lower = status_str.to_lowercase();
        if status_lower.contains("up") {
            ContainerStatus::Running
        } else if status_lower.contains("exited") {
            ContainerStatus::Exited
        } else if status_lower.contains("paused") {
            ContainerStatus::Paused
        } else if status_lower.contains("restarting") {
            ContainerStatus::Restarting
        } else if status_lower.contains("dead") {
            ContainerStatus::Dead
        } else if status_lower.contains("created") {
            ContainerStatus::Created
        } else {
            ContainerStatus::Stopped
        }
    }

    /// Get available shells for a container
    pub fn detect_available_shells(&self, container_id: &str) -> Result<Vec<Shell>, DockerError> {
        if !self.enabled {
            return Err(DockerError::DockerNotInstalled);
        }

        let shells_to_check = [
            ("/bin/bash", Shell::Bash),
            ("/usr/bin/bash", Shell::Bash),
            ("/bin/zsh", Shell::Zsh),
            ("/usr/bin/zsh", Shell::Zsh),
            ("/usr/bin/fish", Shell::Fish),
            ("/bin/fish", Shell::Fish),
            ("/bin/sh", Shell::Sh),
            ("/bin/ash", Shell::Ash),
        ];

        let mut available_shells = Vec::new();

        for (shell_path, shell_type) in &shells_to_check {
            let output = Command::new("docker")
                .args(&["exec", container_id, "test", "-x", shell_path])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            if let Ok(status) = output {
                if status.success() {
                    available_shells.push(shell_type.clone());
                }
            }
        }

        // Remove duplicates while preserving order
        let mut unique_shells = Vec::new();
        for shell in available_shells {
            if !unique_shells.contains(&shell) {
                unique_shells.push(shell);
            }
        }

        if unique_shells.is_empty() {
            unique_shells.push(Shell::Sh); // Fallback to sh
        }

        Ok(unique_shells)
    }

    /// Connect to a container with specified options
    pub fn connect_to_container(&self, options: DockerConnectionOptions) -> Result<String, DockerError> {
        if !self.enabled {
            return Err(DockerError::DockerNotInstalled);
        }

        if !self.is_docker_running() {
            return Err(DockerError::DockerNotRunning);
        }

        // Build docker exec command
        let mut args = vec!["exec".to_string(), "-it".to_string()];

        // Add user if specified
        if let Some(user) = &options.user {
            args.push("--user".to_string());
            args.push(user.clone());
        }

        // Add working directory if specified
        if let Some(workdir) = &options.working_directory {
            args.push("--workdir".to_string());
            args.push(workdir.clone());
        }

        // Add environment variables
        for (key, value) in &options.environment_vars {
            args.push("--env".to_string());
            args.push(format!("{}={}", key, value));
        }

        args.push(options.container_id.clone());
        args.push(self.shell_to_path(&options.shell));

        // Return the command that should be executed
        let command = format!("docker {}", args.join(" "));
        Ok(command)
    }

    /// Convert Shell enum to executable path
    fn shell_to_path(&self, shell: &Shell) -> String {
        match shell {
            Shell::Bash => "/bin/bash".to_string(),
            Shell::Zsh => "/bin/zsh".to_string(),
            Shell::Fish => "/usr/bin/fish".to_string(),
            Shell::Sh => "/bin/sh".to_string(),
            Shell::Ash => "/bin/ash".to_string(),
        }
    }

    /// Get container by ID or name
    pub fn get_container(&mut self, identifier: &str) -> Result<DockerContainer, DockerError> {
        let containers = self.list_containers(true)?;
        
        for container in containers {
            if container.id.starts_with(identifier) || container.name == identifier {
                return Ok(container);
            }
        }

        Err(DockerError::ContainerNotFound(identifier.to_string()))
    }

    /// Start a stopped container
    pub fn start_container(&self, container_id: &str) -> Result<(), DockerError> {
        if !self.enabled {
            return Err(DockerError::DockerNotInstalled);
        }

        let output = Command::new("docker")
            .args(&["start", container_id])
            .output()
            .map_err(|e| DockerError::ConnectionFailed(e.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(DockerError::ConnectionFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ))
        }
    }

    /// Stop a running container
    pub fn stop_container(&self, container_id: &str) -> Result<(), DockerError> {
        if !self.enabled {
            return Err(DockerError::DockerNotInstalled);
        }

        let output = Command::new("docker")
            .args(&["stop", container_id])
            .output()
            .map_err(|e| DockerError::ConnectionFailed(e.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(DockerError::ConnectionFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ))
        }
    }

    /// Get quick connection options for a container (Warp-style)
    pub fn get_quick_connect_options(&mut self, container_id: &str) -> Result<Vec<DockerConnectionOptions>, DockerError> {
        let container = self.get_container(container_id)?;
        let available_shells = self.detect_available_shells(&container.id)?;

        let mut options = Vec::new();
        
        for shell in available_shells {
            options.push(DockerConnectionOptions {
                container_id: container.id.clone(),
                shell: shell.clone(),
                working_directory: Some("/".to_string()), // Default to root
                user: None, // Use container default
                environment_vars: HashMap::new(),
            });

            // Add option with root working directory if different
            if shell == Shell::Bash || shell == Shell::Zsh {
                options.push(DockerConnectionOptions {
                    container_id: container.id.clone(),
                    shell: shell.clone(),
                    working_directory: Some("/app".to_string()), // Common app directory
                    user: None,
                    environment_vars: HashMap::new(),
                });
            }
        }

        Ok(options)
    }

    /// Format container for display (Warp-style)
    pub fn format_container_display(&self, container: &DockerContainer) -> String {
        let status_icon = match container.status {
            ContainerStatus::Running => "🟢",
            ContainerStatus::Stopped => "🔴",
            ContainerStatus::Paused => "🟡",
            ContainerStatus::Exited => "⚫",
            _ => "⚪",
        };

        let ports_str = if container.ports.is_empty() {
            "".to_string()
        } else {
            format!(" [{}]", container.ports.join(", "))
        };

        format!(
            "{} {} ({}) - {}{}",
            status_icon,
            container.name,
            &container.id[..12], // Short container ID
            container.image,
            ports_str
        )
    }

    /// Clear the container cache
    pub fn clear_cache(&mut self) {
        self.cached_containers.clear();
        self.last_cache_update = None;
    }
}

impl Shell {
    /// Get display name for the shell
    pub fn display_name(&self) -> &'static str {
        match self {
            Shell::Bash => "Bash",
            Shell::Zsh => "Zsh", 
            Shell::Fish => "Fish",
            Shell::Sh => "sh",
            Shell::Ash => "ash",
        }
    }

    /// Get shell icon/emoji
    pub fn icon(&self) -> &'static str {
        match self {
            Shell::Bash => "🐚",
            Shell::Zsh => "⚡",
            Shell::Fish => "🐟",
            Shell::Sh => "📟",
            Shell::Ash => "🔧",
        }
    }
}

impl Default for DockerExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DockerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DockerError::DockerNotInstalled => write!(f, "Docker is not installed"),
            DockerError::DockerNotRunning => write!(f, "Docker daemon is not running"),
            DockerError::ContainerNotFound(id) => write!(f, "Container '{}' not found", id),
            DockerError::ShellNotAvailable(shell) => write!(f, "Shell '{}' not available in container", shell.display_name()),
            DockerError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            DockerError::PermissionDenied => write!(f, "Permission denied - check Docker permissions"),
        }
    }
}

impl std::error::Error for DockerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_extension_creation() {
        let extension = DockerExtension::new();
        // Should handle case where Docker might not be available
        assert!(extension.docker_available == DockerExtension::check_docker_availability());
    }

    #[test]
    fn test_container_status_parsing() {
        assert_eq!(DockerExtension::parse_container_status("Up 2 hours"), ContainerStatus::Running);
        assert_eq!(DockerExtension::parse_container_status("Exited (0) 2 minutes ago"), ContainerStatus::Exited);
        assert_eq!(DockerExtension::parse_container_status("Created"), ContainerStatus::Created);
    }

    #[test]
    fn test_shell_display_names() {
        assert_eq!(Shell::Bash.display_name(), "Bash");
        assert_eq!(Shell::Zsh.display_name(), "Zsh");
        assert_eq!(Shell::Fish.display_name(), "Fish");
    }

    #[test]
    fn test_shell_to_path() {
        let extension = DockerExtension::new();
        assert_eq!(extension.shell_to_path(&Shell::Bash), "/bin/bash");
        assert_eq!(extension.shell_to_path(&Shell::Zsh), "/bin/zsh");
        assert_eq!(extension.shell_to_path(&Shell::Fish), "/usr/bin/fish");
    }

    #[test]
    fn test_container_display_formatting() {
        let extension = DockerExtension::new();
        let container = DockerContainer {
            id: "1234567890abcdef".to_string(),
            name: "test_container".to_string(),
            image: "ubuntu:20.04".to_string(),
            status: ContainerStatus::Running,
            ports: vec!["8080:80".to_string()],
            created: "2024-01-01".to_string(),
            command: "/bin/bash".to_string(),
            available_shells: vec![Shell::Bash],
        };

        let display = extension.format_container_display(&container);
        assert!(display.contains("🟢"));
        assert!(display.contains("test_container"));
        assert!(display.contains("123456789012"));
        assert!(display.contains("ubuntu:20.04"));
        assert!(display.contains("8080:80"));
    }

    #[test]
    fn test_docker_connection_options() {
        let mut env_vars = HashMap::new();
        env_vars.insert("TERM".to_string(), "xterm-256color".to_string());

        let options = DockerConnectionOptions {
            container_id: "test123".to_string(),
            shell: Shell::Bash,
            working_directory: Some("/app".to_string()),
            user: Some("developer".to_string()),
            environment_vars: env_vars,
        };

        assert_eq!(options.container_id, "test123");
        assert_eq!(options.shell, Shell::Bash);
        assert_eq!(options.working_directory, Some("/app".to_string()));
        assert_eq!(options.user, Some("developer".to_string()));
        assert!(options.environment_vars.contains_key("TERM"));
    }
}
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub ports: Vec<PortMapping>,
    pub volumes: Vec<VolumeMapping>,
    pub environment: HashMap<String, String>,
    pub networks: Vec<String>,
    pub created: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerStatus {
    Running,
    Stopped,
    Paused,
    Restarting,
    Created,
    Dead,
    Exited(i32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub container_port: u16,
    pub host_port: Option<u16>,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMapping {
    pub host_path: PathBuf,
    pub container_path: PathBuf,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerContext {
    pub in_container: bool,
    pub container_id: Option<String>,
    pub host_working_dir: Option<PathBuf>,
    pub container_working_dir: Option<PathBuf>,
    pub available_containers: Vec<ContainerInfo>,
    pub compose_services: Vec<ComposeService>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeService {
    pub name: String,
    pub image: String,
    pub container_name: Option<String>,
    pub ports: Vec<String>,
    pub volumes: Vec<String>,
    pub environment: HashMap<String, String>,
    pub depends_on: Vec<String>,
    pub health_status: Option<String>,
}

pub struct DockerIntegration {
    context: DockerContext,
    docker_available: bool,
    compose_available: bool,
}

impl DockerIntegration {
    pub async fn new() -> Result<Self> {
        let docker_available = Self::check_docker_availability().await?;
        let compose_available = Self::check_compose_availability().await?;
        
        let context = if docker_available {
            Self::detect_context().await?
        } else {
            DockerContext {
                in_container: false,
                container_id: None,
                host_working_dir: None,
                container_working_dir: None,
                available_containers: Vec::new(),
                compose_services: Vec::new(),
            }
        };

        Ok(Self {
            context,
            docker_available,
            compose_available,
        })
    }

    async fn check_docker_availability() -> Result<bool> {
        let output = Command::new("docker")
            .args(&["version", "--format", "{{.Server.Version}}"])
            .output()
            .map_err(|_| anyhow!("Docker not found"))?;

        Ok(output.status.success())
    }

    async fn check_compose_availability() -> Result<bool> {
        let output = Command::new("docker")
            .args(&["compose", "version"])
            .output()
            .map_err(|_| anyhow!("Docker Compose not found"))?;

        Ok(output.status.success())
    }

    async fn detect_context() -> Result<DockerContext> {
        let in_container = Self::is_running_in_container().await?;
        let container_id = if in_container {
            Self::get_container_id().await?
        } else {
            None
        };

        let available_containers = Self::get_running_containers().await?;
        let compose_services = Self::get_compose_services().await.unwrap_or_default();

        let (host_working_dir, container_working_dir) = if in_container {
            Self::get_working_directories(&container_id).await?
        } else {
            (None, None)
        };

        Ok(DockerContext {
            in_container,
            container_id,
            host_working_dir,
            container_working_dir,
            available_containers,
            compose_services,
        })
    }

    async fn is_running_in_container() -> Result<bool> {
        // Check for container indicators
        if let Ok(cgroup_content) = fs::read_to_string("/proc/1/cgroup").await {
            if cgroup_content.contains("docker") || cgroup_content.contains("containerd") {
                return Ok(true);
            }
        }

        // Check for .dockerenv file
        if fs::metadata("/.dockerenv").await.is_ok() {
            return Ok(true);
        }

        // Check hostname patterns (Docker containers often have random hostnames)
        if let Ok(hostname) = fs::read_to_string("/etc/hostname").await {
            let hostname = hostname.trim();
            if hostname.len() == 12 && hostname.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn get_container_id() -> Result<Option<String>> {
        // Try to read from cgroup
        if let Ok(cgroup_content) = fs::read_to_string("/proc/self/cgroup").await {
            for line in cgroup_content.lines() {
                if let Some(id) = Self::extract_container_id_from_cgroup(line) {
                    return Ok(Some(id));
                }
            }
        }

        // Try hostname as fallback
        if let Ok(hostname) = fs::read_to_string("/etc/hostname").await {
            let hostname = hostname.trim();
            if hostname.len() >= 12 && hostname.chars().all(|c| c.is_ascii_hexdigit()) {
                return Ok(Some(hostname.to_string()));
            }
        }

        Ok(None)
    }

    fn extract_container_id_from_cgroup(line: &str) -> Option<String> {
        // Parse different cgroup formats
        if line.contains("docker/") {
            if let Some(start) = line.rfind("docker/") {
                let id_part = &line[start + 7..];
                if let Some(end) = id_part.find('.') {
                    return Some(id_part[..end].to_string());
                }
                return Some(id_part.to_string());
            }
        }
        None
    }

    async fn get_running_containers() -> Result<Vec<ContainerInfo>> {
        let output = Command::new("docker")
            .args(&[
                "ps",
                "--format",
                "{{.ID}}|{{.Names}}|{{.Image}}|{{.Status}}|{{.Ports}}|{{.CreatedAt}}",
            ])
            .output()
            .map_err(|e| anyhow!("Failed to get containers: {}", e))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let mut containers = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(container) = Self::parse_container_line(line).await? {
                containers.push(container);
            }
        }

        Ok(containers)
    }

    async fn parse_container_line(line: &str) -> Result<Option<ContainerInfo>> {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 6 {
            return Ok(None);
        }

        let id = parts[0].to_string();
        let name = parts[1].to_string();
        let image = parts[2].to_string();
        let status_str = parts[3];
        let ports_str = parts[4];

        let status = Self::parse_container_status(status_str);
        let ports = Self::parse_port_mappings(ports_str);

        // Get detailed container info
        let inspect_output = Command::new("docker")
            .args(&["inspect", "--format", "{{json .}}", &id])
            .output()
            .map_err(|e| anyhow!("Failed to inspect container {}: {}", id, e))?;

        let mut environment = HashMap::new();
        let mut volumes = Vec::new();
        let mut networks = Vec::new();
        let mut created = chrono::Utc::now();

        if inspect_output.status.success() {
            if let Ok(inspect_data) = serde_json::from_slice::<serde_json::Value>(&inspect_output.stdout) {
                // Parse environment variables
                if let Some(env_array) = inspect_data["Config"]["Env"].as_array() {
                    for env_var in env_array {
                        if let Some(env_str) = env_var.as_str() {
                            if let Some(eq_pos) = env_str.find('=') {
                                let key = env_str[..eq_pos].to_string();
                                let value = env_str[eq_pos + 1..].to_string();
                                environment.insert(key, value);
                            }
                        }
                    }
                }

                // Parse volume mounts
                if let Some(mounts) = inspect_data["Mounts"].as_array() {
                    for mount in mounts {
                        if let (Some(source), Some(destination), Some(mode)) = (
                            mount["Source"].as_str(),
                            mount["Destination"].as_str(),
                            mount["Mode"].as_str(),
                        ) {
                            volumes.push(VolumeMapping {
                                host_path: PathBuf::from(source),
                                container_path: PathBuf::from(destination),
                                mode: mode.to_string(),
                            });
                        }
                    }
                }

                // Parse networks
                if let Some(network_settings) = inspect_data["NetworkSettings"]["Networks"].as_object() {
                    for network_name in network_settings.keys() {
                        networks.push(network_name.clone());
                    }
                }

                // Parse creation time
                if let Some(created_str) = inspect_data["Created"].as_str() {
                    if let Ok(parsed_time) = chrono::DateTime::parse_from_rfc3339(created_str) {
                        created = parsed_time.with_timezone(&chrono::Utc);
                    }
                }
            }
        }

        Ok(Some(ContainerInfo {
            id,
            name,
            image,
            status,
            ports,
            volumes,
            environment,
            networks,
            created,
        }))
    }

    fn parse_container_status(status_str: &str) -> ContainerStatus {
        if status_str.starts_with("Up") {
            ContainerStatus::Running
        } else if status_str.starts_with("Exited") {
            // Extract exit code
            if let Some(start) = status_str.find('(') {
                if let Some(end) = status_str.find(')') {
                    if let Ok(code) = status_str[start + 1..end].parse::<i32>() {
                        return ContainerStatus::Exited(code);
                    }
                }
            }
            ContainerStatus::Exited(0)
        } else if status_str.contains("Paused") {
            ContainerStatus::Paused
        } else if status_str.contains("Restarting") {
            ContainerStatus::Restarting
        } else if status_str.contains("Created") {
            ContainerStatus::Created
        } else {
            ContainerStatus::Dead
        }
    }

    fn parse_port_mappings(ports_str: &str) -> Vec<PortMapping> {
        let mut mappings = Vec::new();
        
        for port_mapping in ports_str.split(',') {
            let mapping = port_mapping.trim();
            if mapping.is_empty() {
                continue;
            }

            // Parse format like "0.0.0.0:8080->80/tcp"
            if let Some(arrow_pos) = mapping.find("->") {
                let host_part = &mapping[..arrow_pos];
                let container_part = &mapping[arrow_pos + 2..];

                let host_port = if let Some(colon_pos) = host_part.rfind(':') {
                    host_part[colon_pos + 1..].parse().ok()
                } else {
                    None
                };

                if let Some(slash_pos) = container_part.find('/') {
                    let container_port_str = &container_part[..slash_pos];
                    let protocol = &container_part[slash_pos + 1..];

                    if let Ok(container_port) = container_port_str.parse::<u16>() {
                        mappings.push(PortMapping {
                            container_port,
                            host_port,
                            protocol: protocol.to_string(),
                        });
                    }
                }
            }
        }

        mappings
    }

    async fn get_working_directories(container_id: &Option<String>) -> Result<(Option<PathBuf>, Option<PathBuf>)> {
        if let Some(id) = container_id {
            let output = Command::new("docker")
                .args(&["inspect", "--format", "{{.Config.WorkingDir}}", id])
                .output()
                .map_err(|e| anyhow!("Failed to get container working dir: {}", e))?;

            if output.status.success() {
let wd_raw = String::from_utf8_lossy(&output.stdout).to_string();
                let container_wd = wd_raw.trim().to_string();
                let container_working_dir = if !container_wd.is_empty() {
Some(PathBuf::from(&container_wd))
                } else {
                    None
                };

                // Try to find host working directory from volume mounts
                let host_working_dir = Self::find_host_working_dir(id).await?;

                return Ok((host_working_dir, container_working_dir));
            }
        }

        Ok((None, None))
    }

    async fn find_host_working_dir(container_id: &str) -> Result<Option<PathBuf>> {
        let output = Command::new("docker")
            .args(&["inspect", "--format", "{{json .Mounts}}", container_id])
            .output()
            .map_err(|e| anyhow!("Failed to get container mounts: {}", e))?;

        if !output.status.success() {
            return Ok(None);
        }

        if let Ok(mounts) = serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout) {
            for mount in mounts {
                if let (Some(source), Some(destination)) = (
                    mount["Source"].as_str(),
                    mount["Destination"].as_str(),
                ) {
                    // Look for common project directories
                    if destination.contains("/workspace") || destination.contains("/app") || destination.contains("/src") {
                        return Ok(Some(PathBuf::from(source)));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn get_compose_services() -> Result<Vec<ComposeService>> {
        // Check if docker-compose.yml exists
        let compose_files = ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"];
        
        for file in &compose_files {
            if fs::metadata(file).await.is_ok() {
                return Self::parse_compose_file(file).await;
            }
        }

        Ok(Vec::new())
    }

    async fn parse_compose_file(file: &str) -> Result<Vec<ComposeService>> {
        let content = fs::read_to_string(file).await?;
        let compose_data: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse compose file: {}", e))?;

        let mut services = Vec::new();

        if let Some(services_map) = compose_data["services"].as_mapping() {
            for (service_name, service_config) in services_map {
                if let Some(service_name_str) = service_name.as_str() {
                    let service = Self::parse_compose_service(service_name_str, service_config).await?;
                    services.push(service);
                }
            }
        }

        // Get health status for running services
        for service in &mut services {
            service.health_status = Self::get_service_health(&service.name).await?;
        }

        Ok(services)
    }

    async fn parse_compose_service(name: &str, config: &serde_yaml::Value) -> Result<ComposeService> {
        let image = config["image"].as_str().unwrap_or("unknown").to_string();
        let container_name = config["container_name"].as_str().map(|s| s.to_string());

        let mut ports = Vec::new();
        if let Some(ports_array) = config["ports"].as_sequence() {
            for port in ports_array {
                if let Some(port_str) = port.as_str() {
                    ports.push(port_str.to_string());
                }
            }
        }

        let mut volumes = Vec::new();
        if let Some(volumes_array) = config["volumes"].as_sequence() {
            for volume in volumes_array {
                if let Some(volume_str) = volume.as_str() {
                    volumes.push(volume_str.to_string());
                }
            }
        }

        let mut environment = HashMap::new();
        if let Some(env_map) = config["environment"].as_mapping() {
            for (key, value) in env_map {
                if let (Some(k), Some(v)) = (key.as_str(), value.as_str()) {
                    environment.insert(k.to_string(), v.to_string());
                }
            }
        }

        let mut depends_on = Vec::new();
        if let Some(deps_array) = config["depends_on"].as_sequence() {
            for dep in deps_array {
                if let Some(dep_str) = dep.as_str() {
                    depends_on.push(dep_str.to_string());
                }
            }
        }

        Ok(ComposeService {
            name: name.to_string(),
            image,
            container_name,
            ports,
            volumes,
            environment,
            depends_on,
            health_status: None,
        })
    }

    async fn get_service_health(service_name: &str) -> Result<Option<String>> {
        let output = Command::new("docker")
            .args(&["compose", "ps", "--format", "json", service_name])
            .output()
            .map_err(|e| anyhow!("Failed to get service health: {}", e))?;

        if output.status.success() {
            if let Ok(ps_data) = serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout) {
                if let Some(service) = ps_data.first() {
                    if let Some(health) = service["Health"].as_str() {
                        return Ok(Some(health.to_string()));
                    }
                    if let Some(state) = service["State"].as_str() {
                        return Ok(Some(state.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn get_context(&self) -> &DockerContext {
        &self.context
    }

    pub async fn refresh_context(&mut self) -> Result<()> {
        if self.docker_available {
            self.context = Self::detect_context().await?;
        }
        Ok(())
    }

    pub async fn execute_in_container(&self, container_id: &str, command: &str) -> Result<String> {
        let output = Command::new("docker")
            .args(&["exec", "-t", container_id, "sh", "-c", command])
            .output()
            .map_err(|e| anyhow!("Failed to execute in container: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "Command failed in container: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub async fn copy_file_to_container(
        &self,
        container_id: &str,
        host_path: &Path,
        container_path: &Path,
    ) -> Result<()> {
        let output = Command::new("docker")
            .args(&[
                "cp",
                host_path.to_str().unwrap(),
                &format!("{}:{}", container_id, container_path.to_str().unwrap()),
            ])
            .output()
            .map_err(|e| anyhow!("Failed to copy file to container: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "Copy failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    pub async fn copy_file_from_container(
        &self,
        container_id: &str,
        container_path: &Path,
        host_path: &Path,
    ) -> Result<()> {
        let output = Command::new("docker")
            .args(&[
                "cp",
                &format!("{}:{}", container_id, container_path.to_str().unwrap()),
                host_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| anyhow!("Failed to copy file from container: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "Copy failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    pub async fn start_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("docker")
            .args(&["compose", "up", "-d", service_name])
            .output()
            .map_err(|e| anyhow!("Failed to start service: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to start service {}: {}",
                service_name,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    pub async fn stop_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("docker")
            .args(&["compose", "stop", service_name])
            .output()
            .map_err(|e| anyhow!("Failed to stop service: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to stop service {}: {}",
                service_name,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    pub async fn get_container_logs(&self, container_id: &str, lines: Option<usize>) -> Result<String> {
        let mut args: Vec<String> = vec!["logs".to_string()];
        
        if let Some(n) = lines {
            args.push("--tail".to_string());
            args.push(n.to_string());
        }
        
        args.push(container_id.to_string());

        let output = Command::new("docker")
            .args(&args)
            .output()
            .map_err(|e| anyhow!("Failed to get container logs: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to get logs: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn generate_context_prompt(&self) -> String {
        let mut prompt = String::new();
        
        if self.context.in_container {
            prompt.push_str("🐳 Running inside container");
            if let Some(id) = &self.context.container_id {
                prompt.push_str(&format!(" ({})", &id[..8]));
            }
        } else {
            prompt.push_str("🏠 Running on host");
        }

        if !self.context.available_containers.is_empty() {
            prompt.push_str(&format!(" • {} containers available", self.context.available_containers.len()));
        }

        if !self.context.compose_services.is_empty() {
            prompt.push_str(&format!(" • {} compose services", self.context.compose_services.len()));
        }

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_docker_integration_creation() {
        let result = DockerIntegration::new().await;
        // Should not fail even if Docker is not available
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_port_mappings() {
        let port_str = "0.0.0.0:8080->80/tcp, 0.0.0.0:8443->443/tcp";
        let mappings = DockerIntegration::parse_port_mappings(port_str);
        
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].host_port, Some(8080));
        assert_eq!(mappings[0].container_port, 80);
        assert_eq!(mappings[0].protocol, "tcp");
    }

    #[test]
    fn test_parse_container_status() {
        assert!(matches!(
            DockerIntegration::parse_container_status("Up 2 hours"),
            ContainerStatus::Running
        ));
        
        assert!(matches!(
            DockerIntegration::parse_container_status("Exited (0) 5 minutes ago"),
            ContainerStatus::Exited(0)
        ));
    }
}

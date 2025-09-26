//! Block Sharing System
//!
//! Implements free, local, and peer-to-peer block sharing without requiring
//! cloud services or user signup. Uses Git repositories, local files, and
//! network discovery for sharing command blocks between users.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Write};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::{info, warn, debug, error};

use crate::blocks_v2::{BlockRecord, BlockId, CreateBlockParams, ShellType};

/// Configuration for block sharing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharingConfig {
    /// Local directory for shared block repositories
    pub shared_repos_dir: PathBuf,
    
    /// User's default sharing settings
    pub user_settings: UserSharingSettings,
    
    /// Known block repositories
    pub repositories: Vec<BlockRepository>,
    
    /// Local network sharing settings
    pub network_sharing: NetworkSharingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSharingSettings {
    /// User's display name for sharing
    pub display_name: String,
    
    /// Default tags to add to shared blocks
    pub default_tags: Vec<String>,
    
    /// Auto-import from trusted repositories
    pub auto_import_trusted: bool,
    
    /// Enable local network discovery
    pub enable_network_discovery: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSharingConfig {
    /// Port for local network sharing
    pub port: u16,
    
    /// Enable mDNS discovery
    pub enable_mdns: bool,
    
    /// Trusted network devices
    pub trusted_devices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockRepository {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub trusted: bool,
    pub last_sync: Option<DateTime<Utc>>,
    pub local_path: PathBuf,
    pub tags: Vec<String>,
}

/// Shareable block format for export/import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareableBlock {
    /// Unique identifier for the block
    pub id: String,
    
    /// The command that was executed
    pub command: String,
    
    /// Brief description of what the command does
    pub description: String,
    
    /// Tags for categorization
    pub tags: Vec<String>,
    
    /// Shell type this command is designed for
    pub shell: ShellType,
    
    /// Working directory (relative or absolute)
    pub directory: Option<String>,
    
    /// Environment variables needed
    pub environment: HashMap<String, String>,
    
    /// Example output (sanitized)
    pub example_output: Option<String>,
    
    /// Usage notes
    pub notes: Option<String>,
    
    /// Author information
    pub author: Option<String>,
    
    /// When this block was shared
    pub shared_at: DateTime<Utc>,
    
    /// Version of the block
    pub version: String,
    
    /// Prerequisites (other commands, packages, etc.)
    pub prerequisites: Vec<String>,
}

/// Collection of shareable blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockCollection {
    pub name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub blocks: Vec<ShareableBlock>,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
}

/// Block sharing and synchronization manager
pub struct BlockSharingManager {
    config: SharingConfig,
    local_repos_dir: PathBuf,
}

impl Default for SharingConfig {
    fn default() -> Self {
        Self {
            shared_repos_dir: PathBuf::from("~/.openagent/shared-blocks"),
            user_settings: UserSharingSettings {
                display_name: "Anonymous".to_string(),
                default_tags: vec!["shared".to_string()],
                auto_import_trusted: true,
                enable_network_discovery: false,
            },
            repositories: vec![
                // Default curated repositories
                BlockRepository {
                    id: Uuid::new_v4(),
                    name: "OpenAgent Official".to_string(),
                    url: "https://github.com/openagent/terminal-blocks.git".to_string(),
                    description: Some("Official curated command blocks".to_string()),
                    trusted: true,
                    last_sync: None,
                    local_path: PathBuf::from("openagent-official"),
                    tags: vec!["official".to_string(), "curated".to_string()],
                },
                BlockRepository {
                    id: Uuid::new_v4(),
                    name: "Community Dev Tools".to_string(),
                    url: "https://github.com/openagent/dev-blocks.git".to_string(),
                    description: Some("Development tool command blocks".to_string()),
                    trusted: true,
                    last_sync: None,
                    local_path: PathBuf::from("dev-tools"),
                    tags: vec!["dev".to_string(), "tools".to_string()],
                },
            ],
            network_sharing: NetworkSharingConfig {
                port: 7890,
                enable_mdns: false,
                trusted_devices: Vec::new(),
            },
        }
    }
}

impl BlockSharingManager {
    /// Create a new block sharing manager
    pub async fn new(config: SharingConfig) -> Result<Self> {
        let local_repos_dir = shellexpand::tilde(&config.shared_repos_dir.to_string_lossy()).into_owned();
        let local_repos_dir = PathBuf::from(local_repos_dir);
        
        // Ensure directories exist
        tokio::fs::create_dir_all(&local_repos_dir)
            .await
            .context("Failed to create shared repositories directory")?;
        
        Ok(Self {
            config,
            local_repos_dir,
        })
    }
    
    /// Export blocks to a shareable file format
    pub async fn export_blocks(
        &self,
        blocks: Vec<BlockRecord>,
        collection_name: String,
        output_path: Option<PathBuf>,
    ) -> Result<PathBuf> {
        let shareable_blocks: Vec<ShareableBlock> = blocks
            .into_iter()
            .map(|block| self.block_to_shareable(block))
            .collect();
        
        let collection = BlockCollection {
            name: collection_name.clone(),
            description: Some(format!("Exported blocks from {}", self.config.user_settings.display_name)),
            author: Some(self.config.user_settings.display_name.clone()),
            version: "1.0.0".to_string(),
            created_at: Utc::now(),
            blocks: shareable_blocks,
            tags: vec!["export".to_string()],
            metadata: HashMap::new(),
        };
        
        let json_data = serde_json::to_string_pretty(&collection)
            .context("Failed to serialize block collection")?;
        
        let output_path = output_path.unwrap_or_else(|| {
            PathBuf::from(format!("{}-blocks.json", collection_name.replace(' ', "-").to_lowercase()))
        });
        
        tokio::fs::write(&output_path, json_data)
            .await
            .context("Failed to write exported blocks")?;
        
        info!("Exported {} blocks to {}", collection.blocks.len(), output_path.display());
        Ok(output_path)
    }
    
    /// Import blocks from a shareable file
    pub async fn import_blocks(&self, file_path: &Path) -> Result<Vec<ShareableBlock>> {
        let file_content = tokio::fs::read_to_string(file_path)
            .await
            .context("Failed to read blocks file")?;
        
        let collection: BlockCollection = serde_json::from_str(&file_content)
            .context("Failed to parse block collection")?;
        
        info!("Imported collection '{}' with {} blocks", collection.name, collection.blocks.len());
        
        Ok(collection.blocks)
    }
    
    /// Generate QR code for small block collections
    pub async fn generate_qr_code(&self, blocks: Vec<ShareableBlock>) -> Result<String> {
        if blocks.len() > 3 {
            return Err(anyhow::anyhow!("Too many blocks for QR code (max 3)"));
        }
        
        let collection = BlockCollection {
            name: "QR Share".to_string(),
            description: Some("Blocks shared via QR code".to_string()),
            author: Some(self.config.user_settings.display_name.clone()),
            version: "1.0.0".to_string(),
            created_at: Utc::now(),
            blocks,
            tags: vec!["qr".to_string()],
            metadata: HashMap::new(),
        };
        
        let json_data = serde_json::to_string(&collection)
            .context("Failed to serialize for QR code")?;
        
        // In a real implementation, would use a QR code library
        // For now, return the data that would be encoded
        Ok(json_data)
    }
    
    /// Clone or update a Git repository of blocks
    pub async fn sync_repository(&mut self, repo_id: Uuid) -> Result<usize> {
        let repo = self.config.repositories
            .iter_mut()
            .find(|r| r.id == repo_id)
            .ok_or_else(|| anyhow::anyhow!("Repository not found"))?;
        
        let repo_path = self.local_repos_dir.join(&repo.local_path);
        
        let blocks_count = if repo_path.exists() {
            // Update existing repository
            info!("Updating repository: {}", repo.name);
            self.git_pull(&repo_path).await?
        } else {
            // Clone new repository
            info!("Cloning repository: {}", repo.name);
            self.git_clone(&repo.url, &repo_path).await?
        };
        
        repo.last_sync = Some(Utc::now());
        
        // Scan for block files in the repository
        let discovered_blocks = self.scan_repository_blocks(&repo_path).await?;
        
        info!("Repository '{}' synced: {} blocks available", repo.name, discovered_blocks);
        Ok(discovered_blocks)
    }
    
    /// Add a new repository to track
    pub async fn add_repository(
        &mut self,
        name: String,
        url: String,
        description: Option<String>,
        trusted: bool,
    ) -> Result<Uuid> {
        let repo_id = Uuid::new_v4();
        let local_path = PathBuf::from(name.replace(' ', "-").to_lowercase());
        
        let repository = BlockRepository {
            id: repo_id,
            name,
            url,
            description,
            trusted,
            last_sync: None,
            local_path,
            tags: Vec::new(),
        };
        
        self.config.repositories.push(repository);
        
        // Attempt initial sync
        if let Err(e) = self.sync_repository(repo_id).await {
            warn!("Initial sync failed for new repository: {}", e);
        }
        
        info!("Added new repository: {}", repo_id);
        Ok(repo_id)
    }
    
    /// List available block collections from all repositories
    pub async fn list_available_collections(&self) -> Result<Vec<AvailableCollection>> {
        let mut collections = Vec::new();
        
        for repo in &self.config.repositories {
            let repo_path = self.local_repos_dir.join(&repo.local_path);
            if repo_path.exists() {
                let repo_collections = self.scan_repository_collections(&repo_path, &repo.name).await?;
                collections.extend(repo_collections);
            }
        }
        
        Ok(collections)
    }
    
    /// Install blocks from a repository collection
    pub async fn install_collection(&self, collection_id: &str, target_tags: Vec<String>) -> Result<Vec<ShareableBlock>> {
        let collections = self.list_available_collections().await?;
        let collection = collections
            .iter()
            .find(|c| c.id == collection_id)
            .ok_or_else(|| anyhow::anyhow!("Collection not found"))?;
        
        let blocks = self.import_blocks(&collection.path).await?;
        
        info!("Installed collection '{}' with {} blocks", collection.name, blocks.len());
        Ok(blocks)
    }
    
    /// Create a personal block repository
    pub async fn create_personal_repo(&self, repo_name: String, initial_blocks: Vec<ShareableBlock>) -> Result<PathBuf> {
        let repo_path = self.local_repos_dir.join(format!("personal-{}", repo_name.replace(' ', "-").to_lowercase()));
        
        // Create directory structure
        tokio::fs::create_dir_all(&repo_path).await?;
        tokio::fs::create_dir_all(repo_path.join("collections")).await?;
        
        // Create initial collection
        let collection = BlockCollection {
            name: repo_name.clone(),
            description: Some("Personal block collection".to_string()),
            author: Some(self.config.user_settings.display_name.clone()),
            version: "1.0.0".to_string(),
            created_at: Utc::now(),
            blocks: initial_blocks,
            tags: vec!["personal".to_string()],
            metadata: HashMap::new(),
        };
        
        let collection_file = repo_path.join("collections").join("main.json");
        let json_data = serde_json::to_string_pretty(&collection)?;
        tokio::fs::write(&collection_file, json_data).await?;
        
        // Initialize git repository
        self.git_init(&repo_path).await?;
        
        // Create README
        let readme_content = format!(
            "# {}\n\nPersonal block collection by {}\n\nCreated: {}\n\n## Usage\n\n```bash\nopenagent import-blocks collections/main.json\n```\n",
            repo_name,
            self.config.user_settings.display_name,
            Utc::now().format("%Y-%m-%d")
        );
        
        tokio::fs::write(repo_path.join("README.md"), readme_content).await?;
        
        info!("Created personal repository at: {}", repo_path.display());
        Ok(repo_path)
    }
    
    /// Share blocks via local network (simple HTTP server)
    pub async fn start_local_sharing_server(&self) -> Result<()> {
        info!("Starting local block sharing server on port {}", self.config.network_sharing.port);
        
        // In a real implementation, would start an HTTP server
        // For now, just show the concept
        println!("Local sharing server would be available at:");
        println!("  http://localhost:{}/blocks", self.config.network_sharing.port);
        println!("  http://[local-ip]:{}/blocks", self.config.network_sharing.port);
        
        Ok(())
    }
    
    /// Discover other sharing servers on local network
    pub async fn discover_local_servers(&self) -> Result<Vec<LocalServer>> {
        // In a real implementation, would use mDNS/Bonjour discovery
        // For now, return empty list
        Ok(Vec::new())
    }
    
    // Private helper methods
    
    /// Convert BlockRecord to ShareableBlock
    fn block_to_shareable(&self, block: BlockRecord) -> ShareableBlock {
        ShareableBlock {
            id: format!("block-{}", block.id),
            command: block.command,
            description: self.generate_description(&block),
            tags: block.tags,
            shell: block.shell,
            directory: Some(block.directory.to_string_lossy().to_string()),
            environment: HashMap::new(), // Would extract from metadata
            example_output: if block.output.len() > 200 {
                Some(format!("{}...", &block.output[..200]))
            } else {
                Some(block.output)
            },
            notes: None,
            author: Some(self.config.user_settings.display_name.clone()),
            shared_at: Utc::now(),
            version: "1.0.0".to_string(),
            prerequisites: Vec::new(),
        }
    }
    
    /// Generate a description for a command
    fn generate_description(&self, block: &BlockRecord) -> String {
        // Simple heuristic-based description generation
        let cmd = &block.command;
        
        if cmd.starts_with("git") {
            "Git command".to_string()
        } else if cmd.starts_with("cargo") {
            "Rust/Cargo command".to_string()
        } else if cmd.starts_with("npm") || cmd.starts_with("node") {
            "Node.js command".to_string()
        } else if cmd.starts_with("python") || cmd.starts_with("pip") {
            "Python command".to_string()
        } else if cmd.starts_with("docker") {
            "Docker command".to_string()
        } else if cmd.contains("grep") || cmd.contains("find") || cmd.contains("sed") {
            "Text processing command".to_string()
        } else {
            format!("Terminal command: {}", cmd.split_whitespace().next().unwrap_or("unknown"))
        }
    }
    
    /// Execute git clone
    async fn git_clone(&self, url: &str, path: &Path) -> Result<usize> {
        let output = Command::new("git")
            .arg("clone")
            .arg(url)
            .arg(path)
            .output()
            .await?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Git clone failed: {}", error));
        }
        
        self.scan_repository_blocks(path).await
    }
    
    /// Execute git pull
    async fn git_pull(&self, path: &Path) -> Result<usize> {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("pull")
            .output()
            .await?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Git pull failed: {}", error));
        }
        
        self.scan_repository_blocks(path).await
    }
    
    /// Initialize git repository
    async fn git_init(&self, path: &Path) -> Result<()> {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("init")
            .output()
            .await?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Git init failed: {}", error));
        }
        
        Ok(())
    }
    
    /// Scan repository for block files
    async fn scan_repository_blocks(&self, path: &Path) -> Result<usize> {
        let mut count = 0;
        
        if let Ok(mut dir) = tokio::fs::read_dir(path).await {
            while let Ok(Some(entry)) = dir.next_entry().await {
                if let Some(ext) = entry.path().extension() {
                    if ext == "json" {
                        // Try to parse as block collection
                        if self.is_valid_block_collection(&entry.path()).await {
                            count += 1;
                        }
                    }
                }
            }
        }
        
        // Also check collections subdirectory
        let collections_path = path.join("collections");
        if collections_path.exists() {
            if let Ok(mut dir) = tokio::fs::read_dir(collections_path).await {
                while let Ok(Some(entry)) = dir.next_entry().await {
                    if let Some(ext) = entry.path().extension() {
                        if ext == "json" {
                            if self.is_valid_block_collection(&entry.path()).await {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        
        Ok(count)
    }
    
    /// Scan repository for available collections
    async fn scan_repository_collections(&self, path: &Path, repo_name: &str) -> Result<Vec<AvailableCollection>> {
        let mut collections = Vec::new();
        
        let dirs_to_scan = vec![path.to_path_buf(), path.join("collections")];
        
        for scan_dir in dirs_to_scan {
            if !scan_dir.exists() {
                continue;
            }
            
            if let Ok(mut dir) = tokio::fs::read_dir(&scan_dir).await {
                while let Ok(Some(entry)) = dir.next_entry().await {
                    if let Some(ext) = entry.path().extension() {
                        if ext == "json" {
                            if let Ok(Some(collection)) = self.parse_collection_info(&entry.path(), repo_name).await {
                                collections.push(collection);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(collections)
    }
    
    /// Check if file is a valid block collection
    async fn is_valid_block_collection(&self, path: &Path) -> bool {
        if let Ok(content) = tokio::fs::read_to_string(path).await {
            serde_json::from_str::<BlockCollection>(&content).is_ok()
        } else {
            false
        }
    }
    
    /// Parse collection info from file
    async fn parse_collection_info(&self, path: &Path, repo_name: &str) -> Result<Option<AvailableCollection>> {
        if let Ok(content) = tokio::fs::read_to_string(path).await {
            if let Ok(collection) = serde_json::from_str::<BlockCollection>(&content) {
                return Ok(Some(AvailableCollection {
                    id: format!("{}/{}", repo_name, path.file_stem().unwrap().to_string_lossy()),
                    name: collection.name,
                    description: collection.description,
                    author: collection.author,
                    block_count: collection.blocks.len(),
                    tags: collection.tags,
                    path: path.to_path_buf(),
                    repository: repo_name.to_string(),
                }));
            }
        }
        Ok(None)
    }
}

/// Information about an available block collection
#[derive(Debug, Clone)]
pub struct AvailableCollection {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub block_count: usize,
    pub tags: Vec<String>,
    pub path: PathBuf,
    pub repository: String,
}

/// Information about a local sharing server
#[derive(Debug, Clone)]
pub struct LocalServer {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub collections: Vec<String>,
}

/// Command-line interface for block sharing
pub struct BlockSharingCLI {
    manager: BlockSharingManager,
}

impl BlockSharingCLI {
    pub async fn new() -> Result<Self> {
        let config = SharingConfig::default();
        let manager = BlockSharingManager::new(config).await?;
        
        Ok(Self { manager })
    }
    
    /// Export blocks command
    pub async fn cmd_export(&self, collection_name: String, output: Option<PathBuf>) -> Result<()> {
        println!("📤 Exporting blocks to collection: {}", collection_name);
        
        // In real implementation, would get blocks from BlockManager
        let sample_blocks = vec![
            // Would come from actual block database
        ];
        
        let output_path = self.manager.export_blocks(sample_blocks, collection_name, output).await?;
        
        println!("✅ Blocks exported to: {}", output_path.display());
        println!("\n💡 Share this file with others:");
        println!("   • Send via email/chat");
        println!("   • Upload to GitHub/GitLab");
        println!("   • Share on local network");
        
        Ok(())
    }
    
    /// Import blocks command
    pub async fn cmd_import(&self, file_path: PathBuf) -> Result<()> {
        println!("📥 Importing blocks from: {}", file_path.display());
        
        let blocks = self.manager.import_blocks(&file_path).await?;
        
        println!("✅ Imported {} blocks", blocks.len());
        
        for block in blocks.iter().take(5) {
            println!("   • {} ({})", block.command, block.tags.join(", "));
        }
        
        if blocks.len() > 5 {
            println!("   ... and {} more", blocks.len() - 5);
        }
        
        Ok(())
    }
    
    /// List available collections
    pub async fn cmd_list(&self) -> Result<()> {
        println!("📚 Available Block Collections:\n");
        
        let collections = self.manager.list_available_collections().await?;
        
        if collections.is_empty() {
            println!("   No collections found.");
            println!("\n💡 Add repositories with:");
            println!("   openagent blocks add-repo <name> <git-url>");
            return Ok(());
        }
        
        for collection in collections {
            println!("🔖 {}", collection.name);
            if let Some(desc) = collection.description {
                println!("   {}", desc);
            }
            println!("   {} blocks • {} • {}", 
                     collection.block_count, 
                     collection.repository, 
                     collection.tags.join(", "));
            println!("   ID: {}\n", collection.id);
        }
        
        Ok(())
    }
    
    /// Install a collection
    pub async fn cmd_install(&self, collection_id: String) -> Result<()> {
        println!("📦 Installing collection: {}", collection_id);
        
        let blocks = self.manager.install_collection(&collection_id, vec!["imported".to_string()]).await?;
        
        println!("✅ Installed {} blocks from collection", blocks.len());
        
        Ok(())
    }
    
    /// Sync repositories
    pub async fn cmd_sync(&mut self) -> Result<()> {
        println!("🔄 Syncing block repositories...\n");
        
        let repo_ids: Vec<Uuid> = self.manager.config.repositories.iter().map(|r| r.id).collect();
        
        for repo_id in repo_ids {
            let repo_name = self.manager.config.repositories
                .iter()
                .find(|r| r.id == repo_id)
                .map(|r| r.name.clone())
                .unwrap_or_default();
                
            print!("   {} ... ", repo_name);
            io::stdout().flush().unwrap();
            
            match self.manager.sync_repository(repo_id).await {
                Ok(count) => println!("✅ {} blocks", count),
                Err(e) => println!("❌ {}", e),
            }
        }
        
        println!("\n✅ Repository sync completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_export_import_blocks() {
        let temp_dir = TempDir::new().unwrap();
        let config = SharingConfig {
            shared_repos_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let manager = BlockSharingManager::new(config).await.unwrap();
        
        // Create test blocks
        let test_blocks = vec![
            // Would use real BlockRecord instances
        ];
        
        // Test export
        let export_path = manager.export_blocks(
            test_blocks,
            "Test Collection".to_string(),
            Some(temp_dir.path().join("test.json"))
        ).await.unwrap();
        
        assert!(export_path.exists());
        
        // Test import
        let imported_blocks = manager.import_blocks(&export_path).await.unwrap();
        assert_eq!(imported_blocks.len(), 0); // Empty test blocks
    }
    
    #[tokio::test]
    async fn test_repository_management() {
        let temp_dir = TempDir::new().unwrap();
        let config = SharingConfig {
            shared_repos_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let mut manager = BlockSharingManager::new(config).await.unwrap();
        
        // Test adding repository
        let repo_id = manager.add_repository(
            "Test Repo".to_string(),
            "https://github.com/test/blocks.git".to_string(),
            Some("Test repository".to_string()),
            false
        ).await.unwrap();
        
        assert!(manager.config.repositories.iter().any(|r| r.id == repo_id));
    }
}
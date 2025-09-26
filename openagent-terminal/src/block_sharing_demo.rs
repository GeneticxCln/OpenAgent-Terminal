//! Block Sharing System Demo
//!
//! Demonstrates comprehensive block sharing capabilities including:
//! - File-based export/import
//! - Git repository management
//! - Local network sharing
//! - QR code generation for quick sharing
//! - Personal repository creation
//! - Community repository integration

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use tracing::{info, debug};

use crate::block_sharing::{
    BlockSharingManager, BlockSharingCLI, SharingConfig, ShareableBlock,
    UserSharingSettings, NetworkSharingConfig,
};
use crate::blocks_v2::{BlockRecord, BlockId, ShellType};

/// Comprehensive demonstration of block sharing features
pub struct BlockSharingDemo {
    sharing_cli: BlockSharingCLI,
    demo_blocks: Vec<ShareableBlock>,
}

impl BlockSharingDemo {
    /// Create a new block sharing demo
    pub async fn new() -> Result<Self> {
        let sharing_cli = BlockSharingCLI::new().await?;
        let demo_blocks = Self::create_demo_blocks();
        
        Ok(Self {
            sharing_cli,
            demo_blocks,
        })
    }
    
    /// Run the complete block sharing demonstration
    pub async fn run_demo(&mut self) -> Result<()> {
        info!("🎯 Starting Block Sharing System Demo");
        
        // Demo 1: Basic file export/import
        self.demo_file_sharing().await?;
        
        // Demo 2: Git repository management
        self.demo_git_repositories().await?;
        
        // Demo 3: QR code sharing
        self.demo_qr_sharing().await?;
        
        // Demo 4: Personal repository creation
        self.demo_personal_repository().await?;
        
        // Demo 5: Community collections
        self.demo_community_collections().await?;
        
        // Demo 6: Local network sharing
        self.demo_local_network_sharing().await?;
        
        // Demo 7: Integration scenarios
        self.demo_integration_scenarios().await?;
        
        info!("✅ Block Sharing System Demo completed successfully");
        Ok(())
    }
    
    /// Demo 1: Basic file-based export and import
    async fn demo_file_sharing(&mut self) -> Result<()> {
        info!("📁 Demo 1: File-Based Block Sharing");
        
        println!("\n=== File-Based Block Sharing ===");
        
        // Create a custom sharing manager for this demo
        let config = SharingConfig {
            shared_repos_dir: PathBuf::from("/tmp/openagent-demo/shared-blocks"),
            user_settings: UserSharingSettings {
                display_name: "Demo User".to_string(),
                default_tags: vec!["demo".to_string(), "shared".to_string()],
                auto_import_trusted: true,
                enable_network_discovery: false,
            },
            ..Default::default()
        };
        
        let manager = BlockSharingManager::new(config).await?;
        
        // Convert demo blocks to BlockRecords for export
        let block_records: Vec<BlockRecord> = self.demo_blocks
            .iter()
            .take(3)
            .enumerate()
            .map(|(i, shareable)| BlockRecord {
                id: BlockId(i as u64 + 1),
                command: shareable.command.clone(),
                output: shareable.example_output.clone().unwrap_or_default(),
                error_output: String::new(),
                directory: PathBuf::from(shareable.directory.clone().unwrap_or("/tmp".to_string())),
                created_at: shareable.shared_at,
                modified_at: shareable.shared_at,
                exit_code: 0,
                duration_ms: 100,
                starred: false,
                tags: shareable.tags.clone(),
                shell: shareable.shell,
                status: "completed".to_string(),
            })
            .collect();
        
        // Export blocks to file
        println!("📤 Exporting {} blocks to file...", block_records.len());
        let export_path = manager.export_blocks(
            block_records,
            "My Favorite Commands".to_string(),
            Some(PathBuf::from("/tmp/my-favorite-commands.json")),
        ).await?;
        
        println!("✅ Exported to: {}", export_path.display());
        
        // Import blocks from file
        println!("📥 Importing blocks from file...");
        let imported_blocks = manager.import_blocks(&export_path).await?;
        
        println!("✅ Imported {} blocks:", imported_blocks.len());
        for (i, block) in imported_blocks.iter().enumerate() {
            println!("   {}. {} - {}", i + 1, block.command, block.description);
        }
        
        // Show file sharing workflow
        println!("\n💡 File Sharing Workflow:");
        println!("   1. Export: openagent export-blocks --collection \"My Commands\" --output blocks.json");
        println!("   2. Share: Send blocks.json via email, chat, or file sharing");
        println!("   3. Import: openagent import-blocks blocks.json");
        
        Ok(())
    }
    
    /// Demo 2: Git repository management
    async fn demo_git_repositories(&mut self) -> Result<()> {
        info!("📚 Demo 2: Git Repository Management");
        
        println!("\n=== Git Repository Block Sharing ===");
        
        // Demonstrate repository concepts (without actual git operations)
        println!("🔄 Repository Management Features:");
        println!("   • Clone community block repositories");
        println!("   • Sync with upstream changes");
        println!("   • Create personal repositories");
        println!("   • Version control for block collections");
        
        println!("\n📋 Example Repositories:");
        println!("   • OpenAgent Official: https://github.com/openagent/terminal-blocks");
        println!("   • Dev Tools: https://github.com/openagent/dev-blocks");
        println!("   • Rust Commands: https://github.com/user/rust-commands");
        println!("   • DevOps Blocks: https://github.com/team/devops-blocks");
        
        println!("\n💻 Usage Commands:");
        println!("   openagent blocks add-repo \"DevOps\" https://github.com/team/devops-blocks.git");
        println!("   openagent blocks sync  # Update all repositories");
        println!("   openagent blocks list  # Show available collections");
        println!("   openagent blocks install devops/kubernetes  # Install specific collection");
        
        println!("\n🔧 Repository Structure:");
        println!("   my-blocks/");
        println!("   ├── README.md");
        println!("   ├── collections/");
        println!("   │   ├── git-commands.json");
        println!("   │   ├── docker-workflows.json");
        println!("   │   └── rust-development.json");
        println!("   └── .git/");
        
        Ok(())
    }
    
    /// Demo 3: QR code sharing for quick transfers
    async fn demo_qr_sharing(&mut self) -> Result<()> {
        info!("📱 Demo 3: QR Code Block Sharing");
        
        println!("\n=== QR Code Block Sharing ===");
        
        let config = SharingConfig::default();
        let manager = BlockSharingManager::new(config).await?;
        
        // Take a few blocks for QR sharing
        let qr_blocks = self.demo_blocks.iter().take(2).cloned().collect();
        
        println!("📱 Generating QR code for {} blocks...", qr_blocks.len());
        
        let qr_data = manager.generate_qr_code(qr_blocks).await?;
        
        println!("✅ QR code data generated ({} bytes)", qr_data.len());
        
        println!("\n💡 QR Code Sharing Workflow:");
        println!("   1. Select 1-3 blocks to share");
        println!("   2. Generate QR code: openagent blocks qr-share <block-ids>");
        println!("   3. Display QR code in terminal or save as image");
        println!("   4. Scan QR code with mobile app or another terminal");
        println!("   5. Blocks automatically imported");
        
        println!("\n📱 Perfect for:");
        println!("   • Quick sharing between nearby devices");
        println!("   • Conference/meetup block exchanges");
        println!("   • Mobile to desktop transfers");
        println!("   • Offline sharing without internet");
        
        Ok(())
    }
    
    /// Demo 4: Creating personal repositories
    async fn demo_personal_repository(&mut self) -> Result<()> {
        info!("👤 Demo 4: Personal Repository Creation");
        
        println!("\n=== Personal Block Repository ===");
        
        let config = SharingConfig {
            shared_repos_dir: PathBuf::from("/tmp/openagent-demo/personal"),
            ..Default::default()
        };
        let manager = BlockSharingManager::new(config).await?;
        
        // Create personal repository
        println!("🏠 Creating personal block repository...");
        
        let personal_blocks = self.demo_blocks.iter().take(5).cloned().collect();
        let repo_path = manager.create_personal_repo(
            "My Development Workflow".to_string(),
            personal_blocks,
        ).await?;
        
        println!("✅ Personal repository created at: {}", repo_path.display());
        
        println!("\n📂 Repository Contents:");
        println!("   • README.md - Repository description");
        println!("   • collections/main.json - Your block collection");
        println!("   • .git/ - Version control");
        
        println!("\n🔄 Next Steps:");
        println!("   1. git add . && git commit -m 'Initial blocks'");
        println!("   2. git remote add origin <your-repo-url>");
        println!("   3. git push -u origin main");
        println!("   4. Share repository URL with team/community");
        
        println!("\n🌟 Benefits:");
        println!("   • Version control for your blocks");
        println!("   • Easy sharing via Git platforms");
        println!("   • Collaborative development");
        println!("   • Backup and synchronization");
        
        Ok(())
    }
    
    /// Demo 5: Community collections
    async fn demo_community_collections(&mut self) -> Result<()> {
        info!("🌐 Demo 5: Community Block Collections");
        
        println!("\n=== Community Block Collections ===");
        
        println!("🎯 Available Collections (Mock):");
        
        let mock_collections = vec![
            ("Official/Essential", "Core terminal commands everyone should know", 25),
            ("DevOps/Kubernetes", "Kubernetes management and debugging commands", 42),
            ("Dev/Rust", "Rust development workflow commands", 18),
            ("Dev/Python", "Python development and data science", 33),
            ("SysAdmin/Linux", "System administration and troubleshooting", 56),
            ("Security/Pentest", "Security testing and analysis tools", 29),
            ("Git/Workflows", "Advanced Git workflows and aliases", 21),
            ("Docker/Compose", "Docker and Docker Compose management", 37),
        ];
        
        for (collection, desc, count) in mock_collections {
            println!("   🔖 {} - {} blocks", collection, count);
            println!("      {}", desc);
        }
        
        println!("\n💻 Usage Examples:");
        println!("   openagent blocks install Official/Essential");
        println!("   openagent blocks install DevOps/Kubernetes --tags k8s,devops");
        println!("   openagent blocks search rust --collections");
        
        println!("\n🏆 Community Features:");
        println!("   • Curated collections by experts");
        println!("   • Rating and review system");
        println!("   • Automatic updates and improvements");
        println!("   • Tag-based organization");
        println!("   • Usage statistics and popularity");
        
        Ok(())
    }
    
    /// Demo 6: Local network sharing
    async fn demo_local_network_sharing(&mut self) -> Result<()> {
        info!("🌐 Demo 6: Local Network Block Sharing");
        
        println!("\n=== Local Network Block Sharing ===");
        
        let config = SharingConfig {
            network_sharing: NetworkSharingConfig {
                port: 7890,
                enable_mdns: true,
                trusted_devices: vec!["alice-laptop".to_string(), "bob-desktop".to_string()],
            },
            ..Default::default()
        };
        let manager = BlockSharingManager::new(config).await?;
        
        println!("🖥️  Starting local sharing server...");
        manager.start_local_sharing_server().await?;
        
        println!("\n🔍 Discovering other sharing servers...");
        let servers = manager.discover_local_servers().await?;
        
        if servers.is_empty() {
            println!("   No other servers found (demo mode)");
            println!("   In real usage, would discover:");
            println!("   • alice-laptop: 3 collections available");
            println!("   • bob-desktop: 7 collections available");
            println!("   • team-server: 15 collections available");
        }
        
        println!("\n🔧 Local Network Features:");
        println!("   • Automatic device discovery via mDNS");
        println!("   • Secure peer-to-peer sharing");
        println!("   • Real-time block synchronization");
        println!("   • Trusted device management");
        println!("   • Offline operation (no internet required)");
        
        println!("\n💡 Use Cases:");
        println!("   • Team development environments");
        println!("   • Workshop/training sessions");
        println!("   • Conference networking");
        println!("   • Office block libraries");
        
        Ok(())
    }
    
    /// Demo 7: Integration scenarios
    async fn demo_integration_scenarios(&mut self) -> Result<()> {
        info!("🔗 Demo 7: Integration Scenarios");
        
        println!("\n=== Real-World Integration Scenarios ===");
        
        println!("🎯 Scenario 1: Developer Onboarding");
        println!("   1. New developer joins team");
        println!("   2. openagent blocks sync  # Get latest team collections");
        println!("   3. openagent blocks install team/dev-setup");
        println!("   4. openagent blocks install team/project-workflows");
        println!("   5. Developer immediately has team's best practices");
        
        println!("\n🎯 Scenario 2: Conference Block Exchange");
        println!("   1. Speaker shares useful blocks via QR code");
        println!("   2. Attendees scan QR code during presentation");
        println!("   3. Blocks automatically imported with speaker attribution");
        println!("   4. Follow-up: Speaker shares GitHub repo for more blocks");
        
        println!("\n🎯 Scenario 3: Open Source Contribution");
        println!("   1. Developer creates useful workflow blocks");
        println!("   2. openagent blocks create-repo \"My DevOps Blocks\"");
        println!("   3. Push repository to GitHub");
        println!("   4. Community discovers and uses blocks");
        println!("   5. Contributors add improvements via pull requests");
        
        println!("\n🎯 Scenario 4: Enterprise Block Library");
        println!("   1. IT team curates approved command collections");
        println!("   2. Collections stored in corporate Git repositories");
        println!("   3. Employees sync corporate collections automatically");
        println!("   4. New procedures distributed instantly to all users");
        println!("   5. Compliance and security ensured through curation");
        
        println!("\n🎯 Scenario 5: Cross-Platform Sharing");
        println!("   1. Developer works on multiple machines (Linux, macOS, Windows)");
        println!("   2. Personal blocks synced via Git across all platforms");
        println!("   3. Shell-specific adaptations handled automatically");
        println!("   4. Consistent workflow regardless of platform");
        
        println!("\n✨ Key Benefits:");
        println!("   🆓 Completely free - no subscriptions or accounts");
        println!("   🔒 Privacy-focused - your data stays local");
        println!("   🌐 Works offline - no internet dependency");
        println!("   🤝 Community-driven - shared knowledge benefits all");
        println!("   ⚡ Lightning fast - local storage and Git efficiency");
        
        Ok(())
    }
    
    /// Create sample blocks for demonstration
    fn create_demo_blocks() -> Vec<ShareableBlock> {
        vec![
            ShareableBlock {
                id: "demo-git-status".to_string(),
                command: "git status --porcelain".to_string(),
                description: "Show clean git status output".to_string(),
                tags: vec!["git".to_string(), "status".to_string()],
                shell: ShellType::Bash,
                directory: Some(".".to_string()),
                environment: HashMap::new(),
                example_output: Some(" M src/main.rs\n?? new_file.txt".to_string()),
                notes: Some("Useful for scripts and automation".to_string()),
                author: Some("Demo User".to_string()),
                shared_at: chrono::Utc::now(),
                version: "1.0.0".to_string(),
                prerequisites: vec!["git".to_string()],
            },
            ShareableBlock {
                id: "demo-docker-cleanup".to_string(),
                command: "docker system prune -f".to_string(),
                description: "Clean up unused Docker resources".to_string(),
                tags: vec!["docker".to_string(), "cleanup".to_string()],
                shell: ShellType::Bash,
                directory: None,
                environment: HashMap::new(),
                example_output: Some("Deleted Images: 3\nTotal reclaimed space: 1.2GB".to_string()),
                notes: Some("Run periodically to free disk space".to_string()),
                author: Some("Demo User".to_string()),
                shared_at: chrono::Utc::now(),
                version: "1.0.0".to_string(),
                prerequisites: vec!["docker".to_string()],
            },
            ShareableBlock {
                id: "demo-cargo-update".to_string(),
                command: "cargo update && cargo check".to_string(),
                description: "Update dependencies and check compilation".to_string(),
                tags: vec!["rust".to_string(), "cargo".to_string(), "dependencies".to_string()],
                shell: ShellType::Bash,
                directory: Some(".".to_string()),
                environment: HashMap::new(),
                example_output: Some("Updating crates.io index\nFinished dev [unoptimized + debuginfo] target(s)".to_string()),
                notes: Some("Good practice before starting development".to_string()),
                author: Some("Demo User".to_string()),
                shared_at: chrono::Utc::now(),
                version: "1.0.0".to_string(),
                prerequisites: vec!["rust".to_string(), "cargo".to_string()],
            },
            ShareableBlock {
                id: "demo-find-large".to_string(),
                command: "find . -type f -size +100M -exec ls -lh {} \\; | awk '{ print $9 \": \" $5 }'".to_string(),
                description: "Find files larger than 100MB".to_string(),
                tags: vec!["find".to_string(), "disk".to_string(), "cleanup".to_string()],
                shell: ShellType::Bash,
                directory: Some("/".to_string()),
                environment: HashMap::new(),
                example_output: Some("./large_file.zip: 250M\n./video.mp4: 1.2G".to_string()),
                notes: Some("Useful for disk space investigation".to_string()),
                author: Some("Demo User".to_string()),
                shared_at: chrono::Utc::now(),
                version: "1.0.0".to_string(),
                prerequisites: vec!["find".to_string(), "awk".to_string()],
            },
            ShareableBlock {
                id: "demo-npm-audit".to_string(),
                command: "npm audit --audit-level high".to_string(),
                description: "Check for high-severity security vulnerabilities".to_string(),
                tags: vec!["npm".to_string(), "security".to_string(), "audit".to_string()],
                shell: ShellType::Bash,
                directory: Some(".".to_string()),
                environment: HashMap::new(),
                example_output: Some("found 0 vulnerabilities".to_string()),
                notes: Some("Run before deploying Node.js applications".to_string()),
                author: Some("Demo User".to_string()),
                shared_at: chrono::Utc::now(),
                version: "1.0.0".to_string(),
                prerequisites: vec!["npm".to_string()],
            },
        ]
    }
}

/// Run the block sharing demonstration
pub async fn run_block_sharing_demo() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    let mut demo = BlockSharingDemo::new().await?;
    demo.run_demo().await?;
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    run_block_sharing_demo().await
}
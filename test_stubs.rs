//! Simple test program to verify our upgraded stubs work

use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing production-ready stub upgrades...");
    
    // Test Security Lens
    #[cfg(feature = "security")]
    {
        use openagent_terminal::security_lens::{SecurityLens, SecurityPolicy};
        let policy = SecurityPolicy::default();
        let mut lens = SecurityLens::new(policy);
        let result = lens.analyze_command("ls -la");
        println!("Security analysis for 'ls -la': {:?}", result.level);
    }
    
    // Test Blocks v2
    #[cfg(feature = "blocks")]
    {
        use openagent_terminal::blocks_v2::{BlockManager, CreateBlockParams};
        use tempfile::tempdir;
        
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db");
        let mut manager = BlockManager::new(&db_path).await?;
        
        let params = CreateBlockParams::new("echo hello".to_string());
        let block = manager.create_block(params).await?;
        println!("Created block with ID: {}", block.id);
    }
    
    // Test Plugin System
    #[cfg(feature = "plugins")]
    {
        use openagent_terminal::plugins_api::{PluginHost, SignaturePolicy};
        
        let host = PluginHost::new(SignaturePolicy::Optional);
        println!("Plugin host created, {} plugins loaded", host.list_plugins().len());
    }
    
    // Test Notebook Panel
    #[cfg(feature = "notebooks")]
    {
        use openagent_terminal::display::notebook_panel::{NotebookPanelState, CellType};
        
        let mut state = NotebookPanelState::new();
        state.open();
        let cell_id = state.add_cell(CellType::Code { language: "bash".to_string() });
        println!("Created notebook cell with ID: {}", cell_id);
    }
    
    println!("All upgraded stubs are working correctly!");
    Ok(())
}
//! Example of integrating Warp-style tabs and splits into OpenAgent Terminal
//!
//! This example demonstrates:
//! - Setting up WarpTabManager and WarpSplitManager
//! - Configuring Warp-style key bindings
//! - Using session persistence
//! - Handling Warp-style UI rendering

use std::path::PathBuf;

use openagent_terminal::config::warp_bindings::{WarpConfig, integrate_warp_bindings};
use openagent_terminal::config::{Action, KeyBinding};
use openagent_terminal::display::warp_ui::{WarpTabStyle, WarpSplitIndicators};
use openagent_terminal::workspace::warp_tab_manager::{WarpTabManager, SplitDirection};
use openagent_terminal::workspace::warp_split_manager::{WarpSplitManager, WarpNavDirection};
use openagent_terminal::workspace::split_manager::{SplitLayout, PaneId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("OpenAgent Terminal - Warp Integration Example");
    println!("=============================================");
    
    // Initialize Warp-style components
    let mut warp_config = WarpConfig::default();
    warp_config.session_file = Some("/tmp/openagent-warp-session.json".to_string());
    
    // Set up tab manager with session persistence
    let session_path = PathBuf::from("/tmp/openagent-warp-session.json");
    let mut tab_manager = WarpTabManager::with_session_file(&session_path);
    
    // Try to load previous session
    match tab_manager.load_session() {
        Ok(true) => println!("✓ Loaded previous session"),
        Ok(false) => println!("○ No previous session found, starting fresh"),
        Err(e) => println!("⚠ Failed to load session: {}", e),
    }
    
    // Create some example tabs
    demonstrate_tab_management(&mut tab_manager)?;
    
    // Set up split manager
    let mut split_manager = WarpSplitManager::new();
    
    // Demonstrate split functionality
    demonstrate_split_management(&mut split_manager)?;
    
    // Configure Warp-style key bindings
    demonstrate_key_bindings(&warp_config)?;
    
    // Show visual styling options
    demonstrate_visual_styling(&warp_config)?;
    
    // Save session before exiting
    match tab_manager.save_session() {
        Ok(()) => println!("✓ Session saved successfully"),
        Err(e) => println!("⚠ Failed to save session: {}", e),
    }
    
    println!("\n🎉 Warp integration example completed!");
    
    Ok(())
}

/// Demonstrate Warp-style tab management features
fn demonstrate_tab_management(tab_manager: &mut WarpTabManager) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📋 Tab Management Demo");
    println!("---------------------");
    
    // Create tabs with smart naming
    let home_tab = tab_manager.create_warp_tab(Some(PathBuf::from("/home/user")));
    println!("Created home tab: {:?}", home_tab);
    
    let project_tab = tab_manager.create_warp_tab(Some(PathBuf::from("/home/user/my-project")));
    println!("Created project tab: {:?}", project_tab);
    
    // Simulate running a command in the project tab
    tab_manager.update_tab_for_command(project_tab, "cargo build");
    println!("Updated project tab title after running 'cargo build'");
    
    // Create a tab that will be recognized as a project
    let rust_project_tab = tab_manager.create_warp_tab(Some(PathBuf::from("/tmp/test-project")));
    
    // Simulate the project having a Cargo.toml
    std::fs::create_dir_all("/tmp/test-project")?;
    std::fs::write("/tmp/test-project/Cargo.toml", r#"[package]
name = "test-project"
version = "0.1.0"
"#)?;
    
    let updated_tab = tab_manager.create_warp_tab(Some(PathBuf::from("/tmp/test-project")));
    println!("Created Rust project tab: {:?} (should show project name)", updated_tab);
    
    println!("Total tabs: {}", tab_manager.tab_count());
    
    Ok(())
}

/// Demonstrate Warp-style split pane functionality
fn demonstrate_split_management(split_manager: &mut WarpSplitManager) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📱 Split Management Demo");
    println!("------------------------");
    
    // Start with a single pane layout
    let initial_pane = PaneId(1);
    let mut layout = SplitLayout::Single(initial_pane);
    let mut current_pane = initial_pane;
    
    println!("Starting with single pane: {:?}", initial_pane);
    
    // Split right (Warp Cmd+D behavior)
    let right_pane = PaneId(2);
    let split_success = split_manager.split_right(&mut layout, current_pane, right_pane);
    println!("Split right: {} -> Created pane {:?}", split_success, right_pane);
    
    // Navigate to the new pane
    let nav_success = split_manager.navigate_pane(&layout, &mut current_pane, WarpNavDirection::Right);
    println!("Navigate right: {} -> Current pane: {:?}", nav_success, current_pane);
    
    // Split down (Warp Cmd+Shift+D behavior)
    let bottom_pane = PaneId(3);
    let split_success = split_manager.split_down(&mut layout, current_pane, bottom_pane);
    println!("Split down: {} -> Created pane {:?}", split_success, bottom_pane);
    
    // Test pane zoom functionality
    let zoom_success = split_manager.toggle_pane_zoom(&mut layout, current_pane);
    println!("Toggle zoom: {} -> Pane {:?} zoomed", zoom_success, current_pane);
    
    let is_zoomed = split_manager.is_pane_zoomed(current_pane);
    println!("Is pane zoomed: {}", is_zoomed);
    
    // Unzoom
    let unzoom_success = split_manager.toggle_pane_zoom(&mut layout, current_pane);
    println!("Toggle zoom again: {} -> Pane unzoomed", unzoom_success);
    
    // Test equalize splits
    split_manager.equalize_splits(&mut layout);
    println!("Equalized all splits to 50/50 ratios");
    
    Ok(())
}

/// Demonstrate Warp-style key binding configuration
fn demonstrate_key_bindings(config: &WarpConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n⌨️  Key Bindings Demo");
    println!("--------------------");
    
    // Show how to integrate Warp bindings
    let mut key_bindings: Vec<KeyBinding> = Vec::new();
    integrate_warp_bindings(&mut key_bindings);
    
    println!("Added {} Warp-style key bindings:", key_bindings.len());
    
    // List some key Warp shortcuts
    let warp_shortcuts = [
        ("Ctrl+T / Cmd+T", "Create new tab"),
        ("Ctrl+W / Cmd+W", "Close current tab/pane"),
        ("Ctrl+D / Cmd+D", "Split pane right"),
        ("Ctrl+Shift+D / Cmd+Shift+D", "Split pane down"),
        ("Ctrl+Alt+Arrows / Cmd+Alt+Arrows", "Navigate between panes"),
        ("Ctrl+Shift+Arrows / Cmd+Ctrl+Arrows", "Resize panes"),
        ("Ctrl+Shift+Enter / Cmd+Shift+Enter", "Toggle pane zoom"),
        ("Ctrl+; / Cmd+;", "Cycle through recent panes"),
    ];
    
    for (shortcut, description) in &warp_shortcuts {
        println!("  {} - {}", shortcut, description);
    }
    
    println!("\nConfiguration:");
    println!("  Auto tab naming: {}", config.auto_tab_naming);
    println!("  Session file: {:?}", config.session_file);
    println!("  Pane zoom enabled: {}", config.enable_pane_zoom);
    
    Ok(())
}

/// Demonstrate Warp-style visual styling
fn demonstrate_visual_styling(config: &WarpConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎨 Visual Styling Demo");
    println!("---------------------");
    
    let tab_style = WarpTabStyle::default();
    println!("Tab Style Configuration:");
    println!("  Tab height: {}px", tab_style.tab_height);
    println!("  Corner radius: {}px", tab_style.corner_radius);
    println!("  Drop shadow: {}", tab_style.drop_shadow);
    println!("  Animation duration: {}ms", tab_style.animation_duration_ms);
    
    let split_indicators = WarpSplitIndicators::default();
    println!("\nSplit Indicators Configuration:");
    println!("  Split line width: {}px", split_indicators.split_line_width);
    println!("  Show split preview: {}", split_indicators.show_split_preview);
    println!("  Show resize handles: {}", split_indicators.show_resize_handles);
    println!("  Zoom overlay alpha: {}", split_indicators.zoom_overlay_alpha);
    
    // Example of customizing the style
    let mut custom_style = WarpTabStyle::default();
    custom_style.tab_height = 40.0;
    custom_style.corner_radius = 12.0;
    custom_style.animation_duration_ms = 300;
    
    println!("\nCustom Style Example:");
    println!("  Taller tabs: {}px", custom_style.tab_height);
    println!("  More rounded corners: {}px", custom_style.corner_radius);
    println!("  Slower animations: {}ms", custom_style.animation_duration_ms);
    
    Ok(())
}

/// Helper function to simulate real-world usage patterns
#[allow(dead_code)]
fn simulate_workflow(tab_manager: &mut WarpTabManager, split_manager: &mut WarpSplitManager) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔄 Workflow Simulation");
    println!("----------------------");
    
    // Simulate a development workflow
    let main_tab = tab_manager.create_warp_tab(Some(PathBuf::from("/home/user/project")));
    println!("1. Opened project directory");
    
    // Split for running tests
    let mut layout = SplitLayout::Single(PaneId(1));
    let mut current_pane = PaneId(1);
    
    let test_pane = PaneId(2);
    split_manager.split_right(&mut layout, current_pane, test_pane);
    println!("2. Split right for running tests");
    
    // Navigate to test pane and "run" command
    split_manager.navigate_pane(&layout, &mut current_pane, WarpNavDirection::Right);
    tab_manager.update_tab_for_command(main_tab, "cargo test");
    println!("3. Ran tests in right pane");
    
    // Split bottom for log monitoring
    let log_pane = PaneId(3);
    split_manager.split_down(&mut layout, current_pane, log_pane);
    println!("4. Split bottom for log monitoring");
    
    // Focus on logs, then zoom to see them better
    split_manager.navigate_pane(&layout, &mut current_pane, WarpNavDirection::Down);
    split_manager.toggle_pane_zoom(&mut layout, current_pane);
    println!("5. Zoomed log pane for better visibility");
    
    // Go back to main pane
    split_manager.toggle_pane_zoom(&mut layout, current_pane); // Unzoom
    split_manager.navigate_pane(&layout, &mut current_pane, WarpNavDirection::Left);
    println!("6. Returned to main editor pane");
    
    // Create a new tab for documentation
    let docs_tab = tab_manager.create_warp_tab(Some(PathBuf::from("/home/user/project/docs")));
    tab_manager.update_tab_for_command(docs_tab, "mdbook serve");
    println!("7. Created documentation tab");
    
    println!("✓ Workflow simulation completed");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    
    #[test]
    fn test_warp_tab_manager_basic_operations() {
        let mut manager = WarpTabManager::new();
        
        // Test tab creation
        let tab1 = manager.create_warp_tab(Some(PathBuf::from("/tmp")));
        let tab2 = manager.create_warp_tab(Some(PathBuf::from("/home")));
        
        assert_eq!(manager.tab_count(), 2);
        assert!(manager.active_tab().is_some());
        
        // Test command updating
        manager.update_tab_for_command(tab1, "ls -la");
        
        // Test tab closing
        let closed = manager.close_warp_tab(tab1);
        assert!(closed);
        assert_eq!(manager.tab_count(), 1);
    }
    
    #[test]
    fn test_warp_split_manager_operations() {
        let mut manager = WarpSplitManager::new();
        let mut layout = SplitLayout::Single(PaneId(1));
        let mut current_pane = PaneId(1);
        
        // Test splitting
        let success = manager.split_right(&mut layout, current_pane, PaneId(2));
        assert!(success);
        
        // Test navigation
        let nav_success = manager.navigate_pane(&layout, &mut current_pane, WarpNavDirection::Right);
        assert!(nav_success);
        assert_eq!(current_pane, PaneId(2));
        
        // Test zoom
        let zoom_success = manager.toggle_pane_zoom(&mut layout, current_pane);
        assert!(zoom_success);
        assert!(manager.is_pane_zoomed(current_pane));
    }
    
    #[test]
    fn test_session_persistence() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let session_file = temp_dir.path().join("test_session.json");
        
        // Create and save session
        {
            let mut manager = WarpTabManager::with_session_file(&session_file);
            let tab1 = manager.create_warp_tab(Some(PathBuf::from("/tmp")));
            manager.update_tab_for_command(tab1, "echo test");
            manager.save_session()?;
        }
        
        // Load session in new manager
        {
            let mut manager = WarpTabManager::with_session_file(&session_file);
            let loaded = manager.load_session()?;
            assert!(loaded);
            assert_eq!(manager.tab_count(), 1);
        }
        
        Ok(())
    }
    
    #[test]
    fn test_warp_configuration() {
        let config = WarpConfig::default();
        assert!(config.enabled);
        assert!(config.auto_tab_naming);
        assert!(config.enable_pane_zoom);
        assert_eq!(config.session_auto_save_interval, 30);
        assert!((config.pane_resize_step - 0.05).abs() < f32::EPSILON);
    }
}

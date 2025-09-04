use crate::cli::TerminalType;
use crate::config::MigrationConfig;
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use walkdir::WalkDir;

/// Auto-detect terminal configurations on the current system
pub fn auto_detect_configs() -> Result<Vec<MigrationConfig>> {
    let mut detected = Vec::new();

    for terminal_type in TerminalType::all() {
        // Skip terminals not compatible with current platform
        if !terminal_type.is_platform_compatible() {
            continue;
        }

        if let Ok(configs) = detect_terminal_configs(&terminal_type) {
            detected.extend(configs);
        }
    }

    // Sort by terminal type for consistent output
    detected.sort_by(|a, b| {
        a.terminal_type.to_string().cmp(&b.terminal_type.to_string())
    });

    Ok(detected)
}

/// Detect configurations for a specific terminal type
pub fn detect_terminal_configs(terminal_type: &TerminalType) -> Result<Vec<MigrationConfig>> {
    let config_locations = get_typical_config_locations(terminal_type)?;
    let mut found = Vec::new();

    for location in config_locations {
        if location.exists() {
            found.push(MigrationConfig {
                terminal_type: terminal_type.clone(),
                config_path: location,
                detected_automatically: true,
            });
        }
    }

    Ok(found)
}

/// Get typical configuration file locations for a terminal type
pub fn get_typical_config_locations(terminal_type: &TerminalType) -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();

    match terminal_type {
        TerminalType::Alacritty => {
            locations.extend(get_alacritty_locations()?);
        }
        TerminalType::ITerm2 => {
            locations.extend(get_iterm2_locations()?);
        }
        TerminalType::WindowsTerminal => {
            locations.extend(get_windows_terminal_locations()?);
        }
        TerminalType::Kitty => {
            locations.extend(get_kitty_locations()?);
        }
        TerminalType::Hyper => {
            locations.extend(get_hyper_locations()?);
        }
        TerminalType::Warp => {
            locations.extend(get_warp_locations()?);
        }
        TerminalType::WezTerm => {
            locations.extend(get_wezterm_locations()?);
        }
        TerminalType::GnomeTerminal => {
            locations.extend(get_gnome_terminal_locations()?);
        }
        TerminalType::Konsole => {
            locations.extend(get_konsole_locations()?);
        }
        TerminalType::Terminator => {
            locations.extend(get_terminator_locations()?);
        }
        TerminalType::Tilix => {
            locations.extend(get_tilix_locations()?);
        }
        TerminalType::Tabby => {
            locations.extend(get_tabby_locations()?);
        }
    }

    Ok(locations)
}

/// Get the default/first config path for a terminal type (used when no path specified)
pub fn get_default_config_path(terminal_type: &TerminalType) -> Result<PathBuf> {
    let locations = get_typical_config_locations(terminal_type)?;
    
    // Try to find an existing config first
    for location in &locations {
        if location.exists() {
            return Ok(location.clone());
        }
    }

    // If no existing config found, return the first typical location
    locations.first()
        .cloned()
        .ok_or_else(|| anyhow!("No typical config locations defined for {}", terminal_type))
}

fn get_alacritty_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    if let Some(config_dir) = dirs::config_dir() {
        let alacritty_dir = config_dir.join("alacritty");
        locations.push(alacritty_dir.join("alacritty.toml"));
        locations.push(alacritty_dir.join("alacritty.yml"));
        locations.push(alacritty_dir.join("alacritty.yaml"));
    }
    
    if let Some(home_dir) = dirs::home_dir() {
        locations.push(home_dir.join(".alacritty.toml"));
        locations.push(home_dir.join(".alacritty.yml"));
        locations.push(home_dir.join(".alacritty.yaml"));
    }

    Ok(locations)
}

fn get_iterm2_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    if let Some(home_dir) = dirs::home_dir() {
        locations.push(home_dir.join("Library/Preferences/com.googlecode.iterm2.plist"));
        locations.push(home_dir.join("Library/Application Support/iTerm2/DynamicProfiles"));
    }

    Ok(locations)
}

fn get_windows_terminal_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    if let Some(appdata) = std::env::var_os("LOCALAPPDATA") {
        let appdata_path = PathBuf::from(appdata);
        locations.push(appdata_path.join("Packages/Microsoft.WindowsTerminal_8wekyb3d8bbwe/LocalState/settings.json"));
        locations.push(appdata_path.join("Microsoft/Windows Terminal/settings.json"));
    }

    Ok(locations)
}

fn get_kitty_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    if let Some(config_dir) = dirs::config_dir() {
        let kitty_dir = config_dir.join("kitty");
        locations.push(kitty_dir.join("kitty.conf"));
    }
    
    if let Some(home_dir) = dirs::home_dir() {
        locations.push(home_dir.join(".config/kitty/kitty.conf"));
    }

    Ok(locations)
}

fn get_hyper_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    if let Some(home_dir) = dirs::home_dir() {
        locations.push(home_dir.join(".hyper.js"));
        locations.push(home_dir.join(".hyperterm.js")); // Legacy name
        
        // Also check in app data directories
        if let Some(appdata) = dirs::data_dir() {
            locations.push(appdata.join("Hyper/.hyper.js"));
        }
    }

    Ok(locations)
}

fn get_warp_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    if let Some(config_dir) = dirs::config_dir() {
        let warp_dir = config_dir.join("warp-terminal");
        locations.push(warp_dir.join("user_preferences.yaml"));
        locations.push(warp_dir.join("prefs.yaml"));
    }

    Ok(locations)
}

fn get_wezterm_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    if let Some(config_dir) = dirs::config_dir() {
        let wezterm_dir = config_dir.join("wezterm");
        locations.push(wezterm_dir.join("wezterm.lua"));
        locations.push(wezterm_dir.join("wezterm.toml"));
    }
    
    if let Some(home_dir) = dirs::home_dir() {
        locations.push(home_dir.join(".wezterm.lua"));
        locations.push(home_dir.join(".wezterm.toml"));
    }

    Ok(locations)
}

fn get_gnome_terminal_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    // GNOME Terminal uses dconf, which is more complex to handle
    // For now, just indicate where the settings would typically be stored
    if let Some(home_dir) = dirs::home_dir() {
        locations.push(home_dir.join(".config/dconf/user")); // Binary dconf database
    }

    Ok(locations)
}

fn get_konsole_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    if let Some(config_dir) = dirs::config_dir() {
        locations.push(config_dir.join("konsolerc"));
        
        // Profile files are separate
        let konsole_dir = config_dir.join("konsole");
        // We'd need to scan for .profile files, but for now just indicate the directory
        if konsole_dir.exists() {
            for entry in std::fs::read_dir(&konsole_dir)? {
                let entry = entry?;
                if let Some(extension) = entry.path().extension() {
                    if extension == "profile" {
                        locations.push(entry.path());
                    }
                }
            }
        }
    }

    Ok(locations)
}

fn get_terminator_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    if let Some(config_dir) = dirs::config_dir() {
        locations.push(config_dir.join("terminator/config"));
    }

    Ok(locations)
}

fn get_tilix_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    // Tilix also uses dconf
    if let Some(home_dir) = dirs::home_dir() {
        locations.push(home_dir.join(".config/dconf/user"));
    }

    Ok(locations)
}

fn get_tabby_locations() -> Result<Vec<PathBuf>> {
    let mut locations = Vec::new();
    
    if let Some(config_dir) = dirs::config_dir() {
        let tabby_dir = config_dir.join("tabby");
        locations.push(tabby_dir.join("config.yaml"));
        locations.push(tabby_dir.join("config.yml"));
    }

    Ok(locations)
}

/// Search for config files in common locations using patterns
pub fn search_configs_by_pattern(terminal_type: &TerminalType) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let config_names = terminal_type.config_names();
    let extensions = terminal_type.config_extensions();

    // Search common directories
    let search_dirs = get_search_directories()?;

    for search_dir in search_dirs {
        if !search_dir.exists() {
            continue;
        }

        // Use walkdir for recursive search with depth limit
        for entry in WalkDir::new(&search_dir).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            
            // Skip if it's not a file
            if !path.is_file() {
                continue;
            }

            let file_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Check if filename matches any of the expected config names
            if config_names.iter().any(|name| file_name == *name) {
                found.push(path.to_path_buf());
                continue;
            }

            // Check if extension matches
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    // Additional check to see if filename suggests it's for this terminal
                    let terminal_name = terminal_type.to_string().to_lowercase();
                    if file_name.to_lowercase().contains(&terminal_name) {
                        found.push(path.to_path_buf());
                    }
                }
            }
        }
    }

    // Remove duplicates
    found.sort();
    found.dedup();

    Ok(found)
}

fn get_search_directories() -> Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();

    if let Some(home) = dirs::home_dir() {
        dirs.push(home.clone());
        dirs.push(home.join(".config"));
    }

    if let Some(config) = dirs::config_dir() {
        dirs.push(config);
    }

    if let Some(data) = dirs::data_dir() {
        dirs.push(data);
    }

    // Platform-specific directories
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join("Library/Preferences"));
            dirs.push(home.join("Library/Application Support"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            dirs.push(PathBuf::from(appdata));
        }
        if let Some(localappdata) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(localappdata));
        }
    }

    Ok(dirs)
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn test_alacritty_locations() {
        let locations = get_alacritty_locations().unwrap();
        assert!(!locations.is_empty());
        
        // Should contain both .config/alacritty/* and ~/.alacritty.* patterns
        let has_config_dir = locations.iter().any(|p| p.to_string_lossy().contains("alacritty/alacritty"));
        let has_home_file = locations.iter().any(|p| p.file_name().unwrap().to_string_lossy().starts_with(".alacritty"));
        
        assert!(has_config_dir || has_home_file);
    }

    #[test]
    fn test_get_default_config_path() {
        // This should not panic for any supported terminal
        for terminal in TerminalType::all() {
            if terminal.is_platform_compatible() {
                let result = get_default_config_path(&terminal);
                assert!(result.is_ok(), "Failed to get default path for {}", terminal);
            }
        }
    }

    #[test]
    fn test_auto_detect_configs() {
        // Should run without panicking
        let result = auto_detect_configs();
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_directories() {
        let dirs = get_search_directories().unwrap();
        assert!(!dirs.is_empty());
        
        // Should at least contain home directory
        if let Some(home) = dirs::home_dir() {
            assert!(dirs.contains(&home));
        }
    }
}

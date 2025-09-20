use crate::config::{PackageManifest, Theme, ThemeAsset, ThemePackage};
use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;

pub fn export_theme(theme: &Theme, output_path: &Path) -> Result<()> {
    // Create theme package
    let package =
        ThemePackage { theme: theme.clone(), assets: None, manifest: create_manifest(theme)? };

    // Serialize to TOML
    let content = toml::to_string_pretty(&package)?;

    // Write to file
    let mut file = std::fs::File::create(output_path)?;
    file.write_all(content.as_bytes())?;

    Ok(())
}

pub fn import_theme(theme_path: &Path) -> Result<Theme> {
    let content = std::fs::read_to_string(theme_path)?;

    // Try to parse as a theme package first
    if let Ok(package) = toml::from_str::<ThemePackage>(&content) {
        return Ok(package.theme);
    }

    // Fall back to parsing as a plain theme
    let theme: Theme =
        toml::from_str(&content).map_err(|e| anyhow!("Failed to parse theme file: {}", e))?;

    Ok(theme)
}

pub fn export_theme_with_assets(
    theme: &Theme,
    assets: Vec<ThemeAsset>,
    output_path: &Path,
) -> Result<()> {
    let package = ThemePackage {
        theme: theme.clone(),
        assets: Some(assets),
        manifest: create_manifest(theme)?,
    };

    let content = toml::to_string_pretty(&package)?;
    let mut file = std::fs::File::create(output_path)?;
    file.write_all(content.as_bytes())?;

    Ok(())
}

pub fn create_theme_bundle(theme: &Theme, _assets_dir: Option<&Path>) -> Result<Vec<u8>> {
    // This would create a compressed bundle (ZIP) containing the theme and assets
    // For now, just return the theme as TOML bytes
    let content = toml::to_string_pretty(theme)?;
    Ok(content.into_bytes())
}

fn create_manifest(theme: &Theme) -> Result<PackageManifest> {
    let theme_content = toml::to_string(theme)?;
    let checksum = calculate_checksum(theme_content.as_bytes());

    Ok(PackageManifest {
        format_version: "1.0".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        created_by: "openagent-terminal-themes".to_string(),
        checksum,
        signature: None,
    })
}

fn calculate_checksum(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub fn verify_theme_integrity(package: &ThemePackage) -> Result<()> {
    let theme_content = toml::to_string(&package.theme)?;
    let calculated_checksum = calculate_checksum(theme_content.as_bytes());

    if calculated_checksum != package.manifest.checksum {
        return Err(anyhow!("Theme integrity check failed: checksum mismatch"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use std::collections::HashMap;
    use tempfile::NamedTempFile;

    #[test]
    fn test_export_import_theme() {
        let theme = create_test_theme();

        let temp_file = NamedTempFile::new().unwrap();
        export_theme(&theme, temp_file.path()).unwrap();

        let imported_theme = import_theme(temp_file.path()).unwrap();
        assert_eq!(theme.metadata.name, imported_theme.metadata.name);
    }

    #[test]
    fn test_checksum_calculation() {
        let data = b"test data";
        let checksum = calculate_checksum(data);
        assert!(!checksum.is_empty());

        // Same data should produce same checksum
        let checksum2 = calculate_checksum(data);
        assert_eq!(checksum, checksum2);
    }

    fn create_test_theme() -> Theme {
        Theme {
            metadata: ThemeMetadata {
                name: "test".to_string(),
                display_name: "Test Theme".to_string(),
                description: "A test theme".to_string(),
                version: "1.0.0".to_string(),
                author: "Test Author".to_string(),
                license: None,
                homepage: None,
                repository: None,
                tags: vec!["test".to_string()],
                compatibility: ThemeCompatibility {
                    min_version: "1.0.0".to_string(),
                    max_version: None,
                    features: vec![],
                },
                marketplace: MarketplaceInfo::default(),
            },
            tokens: ThemeTokens::default(),
            ui: UiConfig::default(),
            terminal: TerminalConfig::default(),
            extensions: HashMap::new(),
        }
    }
}

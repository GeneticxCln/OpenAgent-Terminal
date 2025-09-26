#!/bin/bash
set -euo pipefail

echo "🔧 Starting utility crates consolidation..."

# Define the crates to merge
UTIL_CRATES=("openagent-terminal-themes" "openagent-terminal-snippets" "openagent-terminal-migrate")
NEW_CRATE="openagent-terminal-utils"
NEW_CRATE_PATH="crates/$NEW_CRATE"

# Create the new consolidated crate directory
mkdir -p "$NEW_CRATE_PATH/src"

echo "📁 Created directory structure for $NEW_CRATE"

# Create the new Cargo.toml by merging dependencies
cat > "$NEW_CRATE_PATH/Cargo.toml" << 'EOF'
[package]
name = "openagent-terminal-utils"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "Utility functions for OpenAgent Terminal (themes, snippets, migration)"

[dependencies]
# Core dependencies
anyhow.workspace = true
thiserror.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
serde_yaml.workspace = true
tracing.workspace = true

# File system and path utilities
dirs = "6.0.0"

# Configuration parsing
toml.workspace = true
toml_edit.workspace = true

# Hash and crypto (for theme integrity)
sha2 = "0.10"

# Optional database for migration tracking
rusqlite = { workspace = true, optional = true }

[features]
default = ["themes", "snippets"]
themes = []
snippets = []
migrate = ["rusqlite"]
all = ["themes", "snippets", "migrate"]
EOF

# Create the main lib.rs file
cat > "$NEW_CRATE_PATH/src/lib.rs" << 'EOF'
//! Utility functions for OpenAgent Terminal
//! 
//! This crate consolidates utility functionality including:
//! - Theme management and loading
//! - Code snippets and templates
//! - Migration tools for configuration and data

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

#[cfg(feature = "themes")]
pub mod themes;

#[cfg(feature = "snippets")]
pub mod snippets;

#[cfg(feature = "migrate")]
pub mod migrate;

/// Common utility error types
#[derive(Debug, thiserror::Error)]
pub enum UtilsError {
    #[error("Theme error: {0}")]
    Theme(String),
    
    #[error("Snippet error: {0}")]
    Snippet(String),
    
    #[error("Migration error: {0}")]
    Migration(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
    
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

pub type UtilsResult<T> = Result<T, UtilsError>;

/// Common utility configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UtilsConfig {
    pub enable_themes: bool,
    pub enable_snippets: bool,
    pub enable_migration: bool,
    pub theme_directory: Option<std::path::PathBuf>,
    pub snippet_directory: Option<std::path::PathBuf>,
}

impl Default for UtilsConfig {
    fn default() -> Self {
        Self {
            enable_themes: true,
            enable_snippets: true,
            enable_migration: false,
            theme_directory: None,
            snippet_directory: None,
        }
    }
}
EOF

# Copy source files from existing crates and adapt them
for crate_name in "${UTIL_CRATES[@]}"; do
    crate_path="crates/$crate_name"
    if [ -d "$crate_path" ]; then
        echo "📄 Processing $crate_name..."
        
        # Determine the module name (remove openagent-terminal- prefix)
        module_name=${crate_name#openagent-terminal-}
        
        # Copy and adapt the source files
        if [ -f "$crate_path/src/lib.rs" ]; then
            # Create module file in new crate
            cp "$crate_path/src/lib.rs" "$NEW_CRATE_PATH/src/${module_name}.rs"
            
            # Add basic module structure if the file is very minimal
            if [ $(wc -l < "$NEW_CRATE_PATH/src/${module_name}.rs") -lt 10 ]; then
                cat > "$NEW_CRATE_PATH/src/${module_name}.rs" << EOF
//! ${module_name^} functionality for OpenAgent Terminal
//! 
//! This module provides ${module_name} management capabilities.

use crate::{UtilsError, UtilsResult};
use std::path::Path;

/// ${module_name^} manager
#[derive(Debug, Default)]
pub struct ${module_name^}Manager {}

impl ${module_name^}Manager {
    pub fn new() -> Self { Self::default() }
    
    pub fn initialize(&mut self) -> UtilsResult<()> {
        tracing::info!("Initializing ${module_name} manager");
        Ok(())
    }
    
    pub fn load_from_directory(&mut self, _path: &Path) -> UtilsResult<()> {
        tracing::info!("Loading ${module_name} from directory");
        Ok(())
    }
}
EOF
            fi
        else
            # Create a basic module if no source exists
            cat > "$NEW_CRATE_PATH/src/${module_name}.rs" << EOF
//! ${module_name^} functionality for OpenAgent Terminal

use crate::{UtilsError, UtilsResult};
use std::path::Path;

/// ${module_name^} manager  
#[derive(Debug, Default)]
pub struct ${module_name^}Manager {}

impl ${module_name^}Manager {
    pub fn new() -> Self { Self::default() }
    
    pub fn load_from_directory(&mut self, _path: &Path) -> UtilsResult<()> {
        tracing::info!("Loading ${module_name} from directory");
        Ok(())
    }
}
EOF
        fi
        
        echo "✅ Created ${module_name}.rs"
    else
        echo "⚠️  $crate_path not found, creating placeholder module"
        
        # Create placeholder for missing crate
        module_name=${crate_name#openagent-terminal-}
        cat > "$NEW_CRATE_PATH/src/${module_name}.rs" << EOF
//! ${module_name^} functionality for OpenAgent Terminal

use crate::{UtilsError, UtilsResult};

/// ${module_name^} manager  
#[derive(Debug, Default)]
pub struct ${module_name^}Manager {
    // Placeholder for ${module_name} functionality
}

impl ${module_name^}Manager {
    pub fn new() -> Self {
        Self::default()
    }
}
EOF
        echo "⚠️  Created placeholder ${module_name}.rs"
    fi
done

echo "✅ Utility crates consolidation structure created"
echo "📁 New consolidated crate: $NEW_CRATE_PATH"
echo ""
echo "Next steps:"
echo "1. Update workspace Cargo.toml to include new crate"
echo "2. Update references in other crates"
echo "3. Test compilation"
echo "4. Remove old crates after validation"
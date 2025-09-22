#!/bin/bash
set -euo pipefail

echo "🔧 Starting IDE crate consolidation..."

# Define the crates to merge
IDE_CRATES=("openagent-terminal-ide-editor" "openagent-terminal-ide-lsp" "openagent-terminal-ide-indexer" "openagent-terminal-ide-dap")
NEW_CRATE="openagent-terminal-ide"
NEW_CRATE_PATH="crates/$NEW_CRATE"

# Create the new consolidated crate directory
mkdir -p "$NEW_CRATE_PATH/src"

echo "📁 Created directory structure for $NEW_CRATE"

# Create the new Cargo.toml by merging dependencies
cat > "$NEW_CRATE_PATH/Cargo.toml" << 'EOF'
[package]
name = "openagent-terminal-ide"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "IDE features for OpenAgent Terminal (LSP, DAP, editor, indexer)"

[dependencies]
# Core dependencies
anyhow.workspace = true
thiserror.workspace = true
tokio.workspace = true
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true

# LSP dependencies
lsp-types = { version = "0.95.1", optional = true }

# Web editor dependencies  
axum = { workspace = true, optional = true }
hyper = { workspace = true, optional = true }
tokio-tungstenite = { workspace = true, optional = true }
wry = { workspace = true, optional = true }

[features]
default = []
lsp = ["lsp-types"]
editor = []
indexer = []
dap = []
web-editors = ["axum", "hyper", "tokio-tungstenite", "wry"]
all = ["lsp", "editor", "indexer", "dap", "web-editors"]
EOF

# Create the main lib.rs file
cat > "$NEW_CRATE_PATH/src/lib.rs" << 'EOF'
//! IDE features for OpenAgent Terminal
//! 
//! This crate consolidates IDE functionality including:
//! - LSP (Language Server Protocol) client
//! - DAP (Debug Adapter Protocol) client  
//! - Editor integration
//! - Code indexing and search
//! - Web-based editors

#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

#[cfg(feature = "editor")]
pub mod editor;

#[cfg(feature = "lsp")]
pub mod lsp;

#[cfg(feature = "indexer")]
pub mod indexer;

#[cfg(feature = "dap")]
pub mod dap;

#[cfg(feature = "web-editors")]
pub mod web_editors;

/// Common IDE error types
#[derive(Debug, thiserror::Error)]
pub enum IdeError {
    #[error("LSP error: {0}")]
    Lsp(String),
    
    #[error("DAP error: {0}")]  
    Dap(String),
    
    #[error("Editor error: {0}")]
    Editor(String),
    
    #[error("Indexer error: {0}")]
    Indexer(String),
    
    #[error("Web editor error: {0}")]
    WebEditor(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type IdeResult<T> = Result<T, IdeError>;

/// Common IDE configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IdeConfig {
    pub enable_lsp: bool,
    pub enable_dap: bool,
    pub enable_editor: bool,
    pub enable_indexer: bool,
    pub enable_web_editors: bool,
}

impl Default for IdeConfig {
    fn default() -> Self {
        Self {
            enable_lsp: false,
            enable_dap: false, 
            enable_editor: false,
            enable_indexer: false,
            enable_web_editors: false,
        }
    }
}
EOF

# Copy source files from existing crates and adapt them
for crate_name in "${IDE_CRATES[@]}"; do
    crate_path="crates/$crate_name"
    if [ -d "$crate_path" ]; then
        echo "📄 Processing $crate_name..."
        
        # Determine the module name (remove openagent-terminal-ide- prefix)
        module_name=${crate_name#openagent-terminal-ide-}
        
        # Copy and adapt the source files
        if [ -f "$crate_path/src/lib.rs" ]; then
            # Create module file in new crate
            cp "$crate_path/src/lib.rs" "$NEW_CRATE_PATH/src/${module_name}.rs"
            
            # Add basic module structure if the file is very minimal
            if [ $(wc -l < "$NEW_CRATE_PATH/src/${module_name}.rs") -lt 10 ]; then
                cat > "$NEW_CRATE_PATH/src/${module_name}.rs" << EOF
//! ${module_name^} functionality for OpenAgent Terminal IDE
//! 
//! This module provides ${module_name} integration capabilities.

use crate::{IdeError, IdeResult};

/// ${module_name^} manager
#[derive(Debug, Default)]
pub struct ${module_name^}Manager {
    // TODO: Implement ${module_name} functionality
}

impl ${module_name^}Manager {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn initialize(&mut self) -> IdeResult<()> {
        tracing::info!("Initializing ${module_name} manager");
        // TODO: Add initialization logic
        Ok(())
    }
}
EOF
            fi
        else
            # Create a basic module if no source exists
            cat > "$NEW_CRATE_PATH/src/${module_name}.rs" << EOF
//! ${module_name^} functionality for OpenAgent Terminal IDE

use crate::{IdeError, IdeResult};

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
        fi
        
        echo "✅ Created ${module_name}.rs"
    else
        echo "⚠️  $crate_path not found, creating placeholder module"
    fi
done

# Create web_editors.rs for web editor functionality
cat > "$NEW_CRATE_PATH/src/web_editors.rs" << 'EOF'
//! Web editor functionality for OpenAgent Terminal IDE

use crate::{IdeError, IdeResult};

/// Web editor manager
#[derive(Debug, Default)]
pub struct WebEditorManager {
    // Placeholder for web editor functionality
}

impl WebEditorManager {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn start_server(&mut self, port: u16) -> IdeResult<()> {
        tracing::info!("Starting web editor server on port {}", port);
        // TODO: Implement web editor server
        Ok(())
    }
}
EOF

echo "✅ IDE crate consolidation structure created"
echo "📁 New consolidated crate: $NEW_CRATE_PATH"
echo ""
echo "Next steps:"
echo "1. Update workspace Cargo.toml to include new crate"
echo "2. Update references in other crates"
echo "3. Test compilation"
echo "4. Remove old crates after validation"
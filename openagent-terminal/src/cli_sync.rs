use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::{SyncCliOptions, SyncCommand, SyncScopeArg};
use crate::config::UiConfig;

pub fn run_sync_cli(opts: &SyncCliOptions, _config: &UiConfig) -> Result<i32> {
    match &opts.command {
        SyncCommand::Export { scope, to } => {
            let src = source_dir(*scope);
            let dst = to.clone();
            copy_dir_recursive(&src, &dst)?;
            println!("Exported {:?} to {}", scope, dst.display());
            Ok(0)
        }
        SyncCommand::Import { scope, from } => {
            let dst = source_dir(*scope);
            let src = from.clone();
            copy_dir_recursive(&src, &dst)?;
            println!("Imported {:?} from {}", scope, src.display());
            Ok(0)
        }
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        anyhow::bail!("Source path does not exist: {}", src.display());
    }
    fs::create_dir_all(dst).with_context(|| format!("Failed to create {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("Failed to read {}", src.display()))? {
        let entry = entry?;
        let src_path = entry.path();
        let name = entry.file_name();
        let dst_path = dst.join(name);
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "Failed to copy {} -> {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn source_dir(scope: SyncScopeArg) -> PathBuf {
    match scope {
        SyncScopeArg::Settings => {
            let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
            config_dir.join("openagent-terminal")
        }
        SyncScopeArg::History => {
            let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
            data_dir.join("openagent-terminal")
        }
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::{SyncCliOptions, SyncCommand, SyncScopeArg, SyncTrustOptions, TrustSubcommand};
use crate::config::UiConfig;

#[cfg(feature = "sync")]
use openagent_terminal_sync as sync_api;

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
        SyncCommand::Trust(t) => run_trust_cmd(t),
    }
}

#[cfg(feature = "sync")]
fn run_trust_cmd(t: &SyncTrustOptions) -> Result<i32> {
    use sync_api::{PeerInfo, SecureSyncProvider, SyncConfig as SyncCfg};

    // Initialize secure provider with default paths
    let cfg = SyncCfg {
        provider: "secure".to_string(),
        data_dir: None,
        endpoint_env: None,
        encryption_key_env: None,
    };
    let mut provider = SecureSyncProvider::new(&cfg)
        .map_err(|e| anyhow::anyhow!("failed to init secure sync provider: {:?}", e))?;

    match &t.command {
        TrustSubcommand::Add { installation_id, display_name, public_key_hex } => {
            let public_key = hex_to_bytes(public_key_hex)?;
            let info = PeerInfo {
                installation_id: installation_id.clone(),
                display_name: display_name.clone().unwrap_or_else(|| installation_id.clone()),
                last_seen: 0,
                public_key,
                capabilities: vec![],
            };
            provider.add_peer(info).map_err(|e| anyhow::anyhow!("failed to add peer: {:?}", e))?;
            println!("Added peer: {}", installation_id);
            Ok(0)
        }
        TrustSubcommand::List { all } => {
            if *all {
                // Show all peer records, including revoked
                for rec in provider.list_peer_records() {
                    let fp = sha256_hex(&rec.info.public_key);
                    let status = if rec.revoked { "revoked" } else { "active" };
                    println!(
                        "{}\t{}\t{}\t{}",
                        rec.info.installation_id, rec.info.display_name, fp, status
                    );
                }
            } else {
                for p in provider.list_peers() {
                    let fp = sha256_hex(&p.public_key);
                    println!("{}\t{}\t{}", p.installation_id, p.display_name, fp);
                }
            }
            Ok(0)
        }
        TrustSubcommand::Remove { installation_id } => {
            let removed = provider
                .remove_peer(installation_id)
                .map_err(|e| anyhow::anyhow!("failed to remove peer: {:?}", e))?;
            if removed {
                println!("Removed peer: {}", installation_id);
            } else {
                println!("Peer not found: {}", installation_id);
            }
            Ok(0)
        }
        TrustSubcommand::Revoke { installation_id } => {
            let ok = provider
                .revoke_peer(installation_id)
                .map_err(|e| anyhow::anyhow!("failed to revoke peer: {:?}", e))?;
            if ok {
                println!("Revoked peer: {}", installation_id);
            } else {
                println!("Peer not found: {}", installation_id);
            }
            Ok(0)
        }
        TrustSubcommand::Rotate { installation_id, new_public_key_hex } => {
            let new_key = hex_to_bytes(new_public_key_hex)?;
            let ok = provider
                .rotate_peer_key(installation_id, new_key)
                .map_err(|e| anyhow::anyhow!("failed to rotate peer key: {:?}", e))?;
            if ok {
                println!("Rotated key for peer: {}", installation_id);
            } else {
                println!("Peer not found: {}", installation_id);
            }
            Ok(0)
        }
    }
}

#[cfg(not(feature = "sync"))]
fn run_trust_cmd(_t: &SyncTrustOptions) -> Result<i32> {
    anyhow::bail!("sync feature not enabled")
}

fn hex_to_bytes(s: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        anyhow::bail!("hex length must be even");
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = from_hex_digit(bytes[i])?;
        let lo = from_hex_digit(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn from_hex_digit(b: u8) -> Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => anyhow::bail!("invalid hex digit: {}", b as char),
    }
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
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
                format!("Failed to copy {} -> {}", src_path.display(), dst_path.display())
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

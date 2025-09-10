use anyhow::{Context, Result};

use crate::cli::{SecurityCliOptions, SecurityCommand};
use crate::config::UiConfig;
#[cfg(feature = "security-lens")]
use crate::security::security_lens::{SecurityLens, SecurityPolicy};

pub fn run_security_cli(opts: &SecurityCliOptions, _config: &UiConfig) -> Result<i32> {
    match &opts.command {
        SecurityCommand::Validate {
            policy,
            dry_run: _,
            json,
        } => {
            let content = std::fs::read_to_string(policy)
                .with_context(|| format!("Failed to read {}", policy.display()))?;
            let policy: SecurityPolicy =
                toml::from_str(&content).with_context(|| "Failed to parse policy TOML")?;
            let mut lens = SecurityLens::new(policy.clone());
            // Sample commands to exercise the policy quickly
            let samples = vec![
                ("echo hello", false),
                ("rm -rf /", true),
                ("aws s3 rm s3://bucket --recursive", true),
            ];
            let mut findings = vec![];
            for (cmd, expect_risky) in samples {
                let risk = lens.analyze_command(cmd);
                let risky = !matches!(risk.level, crate::security::security_lens::RiskLevel::Safe);
                findings.push((cmd.to_string(), risk.level, risky));
                if expect_risky && !risky {
                    eprintln!("Warning: Expected risky classification for '{}'", cmd);
                }
            }
            if *json {
                // Convert findings to JSON
                let json_val = serde_json::json!({
                    "ok": true,
                    "policy": {
                        "enabled": policy.enabled,
                        "block_critical": policy.block_critical,
                    },
                    "findings": findings.iter().map(|(cmd, level, risky)| serde_json::json!({
                        "cmd": cmd,
                        "level": format!("{:?}", level),
                        "risky": risky,
                    })).collect::<Vec<_>>()
                });
                println!("{}", json_val);
            } else {
                println!("✓ Policy parsed and loaded");
                for (cmd, level, risky) in findings {
                    println!("  - '{}' => level={:?} risky={}", cmd, level, risky);
                }
                println!("Docs: {}", policy.docs_base_url);
            }
            Ok(0)
        }
    }
}

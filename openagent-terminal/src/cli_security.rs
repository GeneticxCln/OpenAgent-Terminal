use anyhow::{Context, Result};

use crate::cli::{SecurityCliOptions, SecurityCommand};
use crate::config::UiConfig;
#[cfg(feature = "security-lens")]
use crate::security::security_lens::{SecurityLens, SecurityPolicy};

pub fn run_security_cli(opts: &SecurityCliOptions, config: &UiConfig) -> Result<i32> {
    match &opts.command {
        SecurityCommand::Validate { policy, dry_run: _, json } => {
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
        SecurityCommand::Assess { command, policy, json } => {
            // Determine policy: CLI override path or loaded config
            let policy_obj: SecurityPolicy = if let Some(path) = policy {
                let content = std::fs::read_to_string(path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                toml::from_str(&content).with_context(|| "Failed to parse policy TOML")?
            } else {
                config.security.clone()
            };
            let mut lens = SecurityLens::new(policy_obj.clone());
            let risk = lens.analyze_command(command);
            if *json {
                let out = serde_json::json!({
                    "cmd": command,
                    "level": format!("{:?}", risk.level),
                    "explanation": risk.explanation,
                    "requires_confirmation": policy_obj
                        .require_confirmation
                        .get(&risk.level)
                        .copied()
                        .unwrap_or(false),
                    "mitigations": risk.mitigations,
                    "links": risk.mitigation_links.iter().map(|l| serde_json::json!({
                        "title": l.title,
                        "url": l.url,
                    })).collect::<Vec<_>>()
                });
                println!("{}", out);
            } else {
                println!("Command: {}", command);
                println!("Level: {:?}", risk.level);
                println!("Explanation: {}", risk.explanation);
                let req = policy_obj
                    .require_confirmation
                    .get(&risk.level)
                    .copied()
                    .unwrap_or(false);
                println!("Requires confirmation: {}", req);
                if !risk.mitigations.is_empty() {
                    println!("Mitigations:");
                    for m in risk.mitigations {
                        println!("  • {}", m);
                    }
                }
            }
            Ok(0)
        }
        SecurityCommand::ListPatterns { policy, json } => {
            let policy_obj: SecurityPolicy = if let Some(path) = policy {
                let content = std::fs::read_to_string(path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                toml::from_str(&content).with_context(|| "Failed to parse policy TOML")?
            } else {
                config.security.clone()
            };
            let lens = SecurityLens::new(policy_obj);
            let patterns = lens.patterns_summary();
            if *json {
                let out = serde_json::json!({
                    "count": patterns.len(),
                    "patterns": patterns.iter().map(|p| serde_json::json!({
                        "category": p.category,
                        "pattern": p.pattern,
                        "risk_level": format!("{:?}", p.risk_level),
                        "platform_specific": p.platform_specific,
                    })).collect::<Vec<_>>()
                });
                println!("{}", out);
            } else {
                println!("Active patterns ({}):", patterns.len());
                for p in patterns {
                    println!(
                        "  - [{}] ({}) {}",
                        format!("{:?}", p.risk_level),
                        if p.platform_specific { "platform" } else { "global" },
                        p.pattern
                    );
                }
            }
            Ok(0)
        }
    }
}

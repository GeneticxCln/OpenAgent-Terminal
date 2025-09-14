use std::collections::HashMap;
use std::fs;
use std::io::Write;

use anyhow::{anyhow, Context, Result};

use crate::cli::{AiCommand, AiOptions};
use crate::config::ai::ProviderConfig as ProviderCfg;
use crate::config::ai_providers::{get_default_provider_configs, ProviderCredentials};
use crate::config::UiConfig;

pub fn run_ai_cli(opts: &AiOptions, config: &UiConfig) -> Result<i32> {
    match &opts.command {
        AiCommand::Validate {
            provider,
            include_defaults,
            json,
        } => {
            let mut provider_map: HashMap<String, ProviderCfg> = HashMap::new();
            // Include configured providers
            for (k, v) in &config.ai.providers {
                provider_map.insert(k.clone(), v.clone());
            }
            if *include_defaults {
                for (k, v) in get_default_provider_configs() {
                    provider_map.entry(k).or_insert(v);
                }
            }
            // If a specific provider requested, filter down
            if let Some(p) = provider {
                if let Some(cfg) = provider_map.get(p).cloned() {
                    provider_map.clear();
                    provider_map.insert(p.clone(), cfg);
                } else {
                    return Err(anyhow!("Unknown provider: {}", p));
                }
            }
            // Nothing to validate
            if provider_map.is_empty() {
                eprintln!(
                    "No AI providers configured. Use --include-defaults to check known providers."
                );
                return Ok(2);
            }
            // Validate
            let mut results = Vec::new();
            for (name, pcfg) in provider_map {
                let res = match ProviderCredentials::from_config(&name, &pcfg) {
                    Ok(creds) => Ok((name, true, creds)),
                    Err(e) => Err((name, false, e)),
                };
                results.push(res);
            }
            if *json {
                // Emit a simple JSON report
                let mut report = serde_json::Map::new();
                for r in &results {
                    match r {
                        Ok((name, _, _creds)) => {
                            report.insert(name.clone(), serde_json::json!({"ok": true}));
                        }
                        Err((name, _, err)) => {
                            report.insert(
                                name.clone(),
                                serde_json::json!({"ok": false, "error": err}),
                            );
                        }
                    }
                }
                println!("{}", serde_json::Value::Object(report));
            } else {
                for r in &results {
                    match r {
                        Ok((name, _, _)) => println!("✓ {}: OK", name),
                        Err((name, _, err)) => {
                            println!("✗ {}: {}", name, err);
                            println!("  Hint: Set provider-specific env vars (OPENAGENT_*). See docs/AI_ENVIRONMENT_SECURITY.md");
                        }
                    }
                }
            }
            // Exit code
            let ok = results.iter().all(|r| r.is_ok());
            Ok(if ok { 0 } else { 1 })
        }
        AiCommand::Migrate {
            config_out,
            apply,
            write_env_snippet,
        } => {
            // Detect legacy envs without revealing secrets
            let legacy_to_secure: Vec<(&str, &str)> = vec![
                ("OPENAI_API_KEY", "OPENAGENT_OPENAI_API_KEY"),
                ("OPENAI_API_BASE", "OPENAGENT_OPENAI_ENDPOINT"),
                ("OPENAI_MODEL", "OPENAGENT_OPENAI_MODEL"),
                ("ANTHROPIC_API_KEY", "OPENAGENT_ANTHROPIC_API_KEY"),
            ];
            let mut found = Vec::new();
            for (legacy, secure) in &legacy_to_secure {
                if std::env::var(legacy).is_ok() {
                    found.push((*legacy, *secure));
                }
            }
            if found.is_empty() {
                println!("No legacy AI env vars detected. Your setup may already be secure.");
            } else {
                println!("Found legacy AI env vars:");
                for (legacy, secure) in &found {
                    println!("  - {} -> {}", legacy, secure);
                }
            }
            // Generate secure provider config snippet
            let snippet = String::from("[ai]\nprovider = \"openai\"\n\n[ai.providers.openai]\napi_key_env = \"OPENAGENT_OPENAI_API_KEY\"\nmodel_env = \"OPENAGENT_OPENAI_MODEL\"\nendpoint_env = \"OPENAGENT_OPENAI_ENDPOINT\"\n\n[ai.providers.anthropic]\napi_key_env = \"OPENAGENT_ANTHROPIC_API_KEY\"\n\n[ai.providers.ollama]\n# No API key required for local Ollama\n\n");
            // Write config_out if requested
            if let Some(path) = config_out {
                if *apply {
                    let mut f = fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(false)
                        .open(path)
                        .with_context(|| format!("Failed to open {}", path.display()))?;
                    // Separate section with a header comment
                    writeln!(f, "\n# --- AI Providers (migrated) ---\n{}", snippet)?;
                    println!("Wrote provider sections to {}", path.display());
                } else {
                    println!("-- Begin provider config snippet (TOML) --\n{}\n-- End provider config snippet --", snippet);
                }
            } else {
                println!("-- Begin provider config snippet (TOML) --\n{}\n-- End provider config snippet --", snippet);
            }
            // Write env snippet if requested
            if let Some(env_path) = write_env_snippet {
                let mut s = String::new();
                s.push_str("# OpenAgent Terminal - Secure AI provider exports (redacted)\n");
                for (legacy, secure) in &found {
                    // Reference existing legacy var without revealing value
                    s.push_str(&format!("export {}=${}\n", secure, legacy));
                }
                fs::write(env_path, s).with_context(|| "Failed to write env snippet")?;
                println!("Wrote secure env exports to file (values referenced, not inlined)");
            }
            println!("Next steps:\n  1) Source the env snippet in your shell rc, or export OPENAGENT_* vars manually.\n  2) Ensure your config contains [ai.providers.*] as shown above.\n  3) Run: openagent-terminal ai validate --include-defaults");
            Ok(0)
        }
    }
}

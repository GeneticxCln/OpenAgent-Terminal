use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, Write};

use anyhow::{anyhow, Context, Result};

use crate::cli::{AiCommand, AiOptions};
use crate::config::ai::ProviderConfig as ProviderCfg;
use crate::config::ai_providers::{get_default_provider_configs, ProviderCredentials};
use crate::config::UiConfig;
use rusqlite::Connection;

pub fn run_ai_cli(opts: &AiOptions, config: &UiConfig) -> Result<i32> {
    match &opts.command {
        AiCommand::Validate { provider, include_defaults, json } => {
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
        AiCommand::Migrate { config_out, apply, write_env_snippet } => {
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

        AiCommand::Provider { command } => {
            match command {
                crate::cli::AiProviderCommand::List { include_defaults, json } => {
                    // Aggregate providers: configured + optionally defaults
                    let mut provider_map: HashMap<String, ProviderCfg> = HashMap::new();
                    for (k, v) in &config.ai.providers {
                        provider_map.insert(k.clone(), v.clone());
                    }
                    if *include_defaults {
                        for (k, v) in get_default_provider_configs() {
                            provider_map.entry(k).or_insert(v);
                        }
                    }
                    // Sort names for stable output
                    let mut names: Vec<String> = provider_map.keys().cloned().collect();
                    names.sort();
                    let active = config.ai.provider.clone().unwrap_or_else(|| "null".to_string());
                    if *json {
                        let list: Vec<serde_json::Value> = names
                            .into_iter()
                            .map(|n| {
                                let pcfg = provider_map.get(&n);
                                serde_json::json!({
                                    "name": n,
                                    "active": (n == active),
                                    "configured": pcfg.is_some(),
                                    "model_default": pcfg.and_then(|c| c.default_model.clone()),
                                    "endpoint_default": pcfg.and_then(|c| c.default_endpoint.clone()),
                                    "api_key_env": pcfg.and_then(|c| c.api_key_env.clone()).is_some(),
                                })
                            })
                            .collect();
                        println!("{}", serde_json::json!({"providers": list, "active": active}));
                    } else {
                        println!("Available AI providers:");
                        for n in names {
                            let mark = if n == active { "*" } else { " " };
                            println!(" {} {}", mark, n);
                        }
                        println!("Active: {}", active);
                    }
                    Ok(0)
                }
                crate::cli::AiProviderCommand::Set { name, config_file, dry_run, json } => {
                    // Determine target config path
                    let path = if let Some(p) = config_file {
                        p.clone()
                    } else if let Some(first) = config.config_paths.first() {
                        first.clone()
                    } else {
                        eprintln!("No loaded config file path available. Use --config-file to specify a target.");
                        return Ok(2);
                    };
                    // Load or create TOML
                    let mut doc: toml::Value = if path.exists() {
                        let s = std::fs::read_to_string(&path)
                            .with_context(|| format!("Failed to read {}", path.display()))?;
                        toml::from_str(&s).with_context(|| {
                            format!("Failed to parse TOML at {}", path.display())
                        })?
                    } else {
                        toml::Value::Table(toml::map::Map::new())
                    };
                    if !doc.is_table() {
                        doc = toml::Value::Table(toml::map::Map::new());
                    }
                    let tbl = doc.as_table_mut().unwrap();
                    let ai = tbl
                        .entry("ai")
                        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                    if !ai.is_table() {
                        *ai = toml::Value::Table(toml::map::Map::new());
                    }
                    let ai_tbl = ai.as_table_mut().unwrap();
                    ai_tbl.insert("provider".to_string(), toml::Value::String(name.clone()));

                    if *dry_run {
                        if *json {
                            let out = serde_json::json!({"config_file": path.display().to_string(), "set_provider": name});
                            println!("{}", out);
                        } else {
                            println!(
                                "Would set [ai].provider = \"{}\" in {}",
                                name,
                                path.display()
                            );
                        }
                        return Ok(0);
                    }

                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).with_context(|| {
                            format!("Failed creating parent dir {}", parent.display())
                        })?;
                    }
                    let s =
                        toml::to_string_pretty(&doc).with_context(|| "Failed to serialize TOML")?;
                    std::fs::write(&path, s)
                        .with_context(|| format!("Failed to write {}", path.display()))?;

                    if *json {
                        let out = serde_json::json!({"ok": true, "config_file": path.display().to_string(), "active_provider": name});
                        println!("{}", out);
                    } else {
                        println!("Set active AI provider to '{}' in {}", name, path.display());
                    }
                    Ok(0)
                }
            }
        }

        AiCommand::HistoryExport { format, to } => {
            let base = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("openagent-terminal")
                .join("ai_history");
            let db_path = base.join("history.db");

            eprintln!("🔍 Looking for AI history at: {}", base.display());

            // Validate output format
            if !matches!(format.as_str(), "json" | "csv" | "jsonl") {
                return Err(anyhow!(
                    "Unsupported export format: '{}'. Supported formats: json, csv, jsonl",
                    format
                ));
            }

            // Ensure output directory exists
            if let Some(parent) = to.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!("Failed to create output directory: {}", parent.display())
                    })?;
                }
            }

            let conn = match Connection::open(&db_path) {
                Ok(c) => {
                    eprintln!("📊 Found SQLite database, reading history...");
                    c
                }
                Err(e) => {
                    eprintln!("⚠️  AI history database not available at {} ({}). Attempting JSONL fallback...",
                        db_path.display(), e
                    );
                    // Fallback to JSONL line-by-line export
                    let jsonl = base.join("history.jsonl");
                    if !jsonl.exists() {
                        eprintln!(
                            "❌ No JSONL history found at {}. Nothing to export.",
                            jsonl.display()
                        );
                        eprintln!("💡 Tip: AI history is created after using the AI features in the terminal.");
                        return Ok(2);
                    }
                    eprintln!("📝 Using JSONL fallback from: {}", jsonl.display());
                    // Read JSONL records with progress reporting
                    let file = std::fs::File::open(&jsonl)
                        .with_context(|| format!("Failed to open {}", jsonl.display()))?;
                    let reader = std::io::BufReader::new(file);
                    let mut records: Vec<serde_json::Value> = Vec::new();
                    let mut line_count = 0;
                    let mut skipped_lines = 0;

                    eprintln!("⏳ Reading JSONL records...");

                    for l in reader.lines().map_while(Result::ok) {
                        line_count += 1;
                        if line_count % 1000 == 0 {
                            eprintln!("   Read {} lines...", line_count);
                        }

                        if l.trim().is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<serde_json::Value>(&l) {
                            Ok(v) => records.push(v),
                            Err(parse_err) => {
                                skipped_lines += 1;
                                if skipped_lines <= 5 {
                                    // Only warn about first 5 errors
                                    eprintln!(
                                        "⚠️  Skipping invalid JSONL line {}: {}",
                                        line_count, parse_err
                                    );
                                }
                            }
                        }
                    }

                    if skipped_lines > 0 {
                        eprintln!("⚠️  Skipped {} invalid lines total", skipped_lines);
                    }
                    if records.is_empty() {
                        eprintln!("❌ JSONL history is empty. Nothing to export.");
                        return Ok(2);
                    }

                    eprintln!("✅ Loaded {} history records", records.len());
                    // Write output from JSONL
                    eprintln!("💾 Writing {} format to: {}", format.to_uppercase(), to.display());

                    match format.as_str() {
                        "json" => {
                            eprintln!("   Serializing to JSON...");
                            let s = serde_json::to_string_pretty(&records)
                                .with_context(|| "Failed to serialize records to JSON")?;
                            eprintln!("   Writing {} bytes to file...", s.len());
                            std::fs::write(to, s)
                                .with_context(|| format!("Failed to write {}", to.display()))?;
                            println!(
                                "✅ Exported {} AI history entries to {} (JSON via JSONL fallback)",
                                records.len(),
                                to.display()
                            );
                        }
                        "jsonl" => {
                            // Export back to JSONL format (useful for filtering/reformatting)
                            let mut output = Vec::new();
                            for record in &records {
                                serde_json::to_writer(&mut output, record)
                                    .with_context(|| "Failed to serialize record")?;
                                output.push(b'\n');
                            }
                            std::fs::write(to, output)
                                .with_context(|| format!("Failed to write {}", to.display()))?;
                            println!(
                                "✅ Exported {} AI history entries to {} (JSONL via JSONL fallback)",
                                records.len(),
                                to.display()
                            );
                        }
                        "csv" => {
                            eprintln!("   Creating CSV writer...");
                            let mut wtr = csv::Writer::from_path(to).with_context(|| {
                                format!("Failed to open {} for CSV", to.display())
                            })?;

                            eprintln!("   Writing CSV header...");
                            wtr.write_record([
                                "timestamp",
                                "mode",
                                "working_directory",
                                "shell_kind",
                                "input",
                                "output",
                            ])?;

                            eprintln!("   Writing {} records to CSV...", records.len());
                            let mut written_records = 0;
                            for (i, rec) in records.iter().enumerate() {
                                if i > 0 && i % 500 == 0 {
                                    eprintln!("   Processed {} records...", i);
                                }

                                let ts = rec.get("ts").and_then(|v| v.as_str()).unwrap_or("");
                                let mode = rec.get("mode").and_then(|v| v.as_str()).unwrap_or("");
                                let wd = rec
                                    .get("working_directory")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let sh =
                                    rec.get("shell_kind").and_then(|v| v.as_str()).unwrap_or("");
                                let input = rec.get("input").and_then(|v| v.as_str()).unwrap_or("");
                                let output =
                                    rec.get("output").and_then(|v| v.as_str()).unwrap_or("");

                                match wtr.write_record([ts, mode, wd, sh, input, output]) {
                                    Ok(_) => written_records += 1,
                                    Err(e) => {
                                        eprintln!("⚠️  Failed to write record {}: {}", i + 1, e);
                                    }
                                }
                            }
                            wtr.flush()?;
                            println!(
                                "✅ Exported {} AI history entries to {} (CSV via JSONL fallback)",
                                written_records,
                                to.display()
                            );
                            if written_records != records.len() {
                                eprintln!(
                                    "⚠️  Note: {} records failed to write",
                                    records.len() - written_records
                                );
                            }
                        }
                        other => {
                            return Err(anyhow!("Unsupported export format: '{}'. Supported formats: json, csv, jsonl", other));
                        }
                    }
                    return Ok(0);
                }
            };
            let mut stmt = match conn
                .prepare("SELECT ts, mode, working_directory, shell_kind, input, output FROM conversations ORDER BY id ASC")
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        "SQLite prepare failed ({}). Attempting JSONL fallback...",
                        e
                    );
                    let jsonl = base.join("history.jsonl");
                    if !jsonl.exists() {
                        tracing::warn!(
                            "No JSONL history found at {}. Nothing to export.",
                            jsonl.display()
                        );
                        return Ok(2);
                    }
                    // Read JSONL records
                    let file = std::fs::File::open(&jsonl)
                        .with_context(|| format!("Failed to open {}", jsonl.display()))?;
                    let reader = std::io::BufReader::new(file);
                    let mut records: Vec<serde_json::Value> = Vec::new();
                    for l in reader.lines().map_while(Result::ok) {
                        if l.trim().is_empty() { continue; }
                        match serde_json::from_str::<serde_json::Value>(&l) {
                            Ok(v) => records.push(v),
                            Err(parse_err) => {
                                tracing::warn!("Skipping invalid JSONL line: {}", parse_err);
                            }
                        }
                    }
                    if records.is_empty() {
                        tracing::warn!("JSONL history is empty. Nothing to export.");
                        return Ok(2);
                    }
                    // Write output from JSONL
                    match format.as_str() {
                        "json" => {
                            let s = serde_json::to_string_pretty(&records)?;
                            std::fs::write(to, s)
                                .with_context(|| format!("Failed to write {}", to.display()))?;
                            println!(
                                "Exported {} AI history entries to {} (JSON via JSONL fallback)",
                                records.len(),
                                to.display()
                            );
                        }
                        "jsonl" => {
                            let mut output = Vec::new();
                            for record in &records {
                                serde_json::to_writer(&mut output, record)
                                    .with_context(|| "Failed to serialize record")?;
                                output.push(b'\n');
                            }
                            std::fs::write(to, output)
                                .with_context(|| format!("Failed to write {}", to.display()))?;
                            println!(
                                "Exported {} AI history entries to {} (JSONL via JSONL fallback)",
                                records.len(),
                                to.display()
                            );
                        }
                        "csv" => {
                            let mut wtr = csv::Writer::from_path(to)
                                .with_context(|| format!("Failed to open {} for CSV", to.display()))?;
                            wtr.write_record([
                                "ts",
                                "mode",
                                "working_directory",
                                "shell_kind",
                                "input",
                                "output",
                            ])?;
                            for rec in &records {
                                let ts = rec.get("ts").and_then(|v| v.as_str()).unwrap_or("");
                                let mode = rec.get("mode").and_then(|v| v.as_str()).unwrap_or("");
                                let wd = rec
                                    .get("working_directory")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let sh = rec.get("shell_kind").and_then(|v| v.as_str()).unwrap_or("");
                                let input = rec.get("input").and_then(|v| v.as_str()).unwrap_or("");
                                let output = rec.get("output").and_then(|v| v.as_str()).unwrap_or("");
                                wtr.write_record([ts, mode, wd, sh, input, output])?;
                            }
                            wtr.flush()?;
                            println!(
                                "Exported {} AI history entries to {} (CSV via JSONL fallback)",
                                records.len(),
                                to.display()
                            );
                        }
                        other => {
                            return Err(anyhow!(format!("Unsupported export format: {}", other)));
                        }
                    }
                    return Ok(0);
                }
            };
            let rows = stmt
                .query_map([], |row| {
                    Ok(serde_json::json!({
                        "ts": row.get::<_, String>(0)?,
                        "mode": row.get::<_, String>(1)?,
                        "working_directory": row.get::<_, String>(2).ok(),
                        "shell_kind": row.get::<_, String>(3).ok(),
                        "input": row.get::<_, String>(4)?,
                        "output": row.get::<_, String>(5)?,
                    }))
                })
                .map_err(|e| anyhow!(e.to_string()))?;

            let mut records: Vec<serde_json::Value> = Vec::new();
            for r in rows {
                records.push(r.map_err(|e| anyhow!(e.to_string()))?);
            }

            // Write output
            match format.as_str() {
                "json" => {
                    let s = serde_json::to_string_pretty(&records)?;
                    std::fs::write(to, s)
                        .with_context(|| format!("Failed to write {}", to.display()))?;
                    println!(
                        "Exported {} AI history entries to {} (JSON)",
                        records.len(),
                        to.display()
                    );
                }
                "jsonl" => {
                    let mut output = Vec::new();
                    for record in &records {
                        serde_json::to_writer(&mut output, record)
                            .with_context(|| "Failed to serialize record")?;
                        output.push(b'\n');
                    }
                    std::fs::write(to, output)
                        .with_context(|| format!("Failed to write {}", to.display()))?;
                    println!(
                        "Exported {} AI history entries to {} (JSONL)",
                        records.len(),
                        to.display()
                    );
                }
                "csv" => {
                    let mut wtr = csv::Writer::from_path(to)
                        .with_context(|| format!("Failed to open {} for CSV", to.display()))?;
                    wtr.write_record([
                        "ts",
                        "mode",
                        "working_directory",
                        "shell_kind",
                        "input",
                        "output",
                    ])?;
                    for rec in &records {
                        let ts = rec.get("ts").and_then(|v| v.as_str()).unwrap_or("");
                        let mode = rec.get("mode").and_then(|v| v.as_str()).unwrap_or("");
                        let wd =
                            rec.get("working_directory").and_then(|v| v.as_str()).unwrap_or("");
                        let sh = rec.get("shell_kind").and_then(|v| v.as_str()).unwrap_or("");
                        let input = rec.get("input").and_then(|v| v.as_str()).unwrap_or("");
                        let output = rec.get("output").and_then(|v| v.as_str()).unwrap_or("");
                        wtr.write_record([ts, mode, wd, sh, input, output])?;
                    }
                    wtr.flush()?;
                    println!(
                        "Exported {} AI history entries to {} (CSV)",
                        records.len(),
                        to.display()
                    );
                }
                other => {
                    return Err(anyhow!(format!("Unsupported export format: {}", other)));
                }
            }
            Ok(0)
        }

        AiCommand::HistoryPurge { keep_last } => {
            let base = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("openagent-terminal")
                .join("ai_history");
            let db_path = base.join("history.db");
            match Connection::open(&db_path) {
                Ok(conn) => {
                    // purge all but the last N by id
                    let k = *keep_last as i64;
                    if k <= 0 {
                        conn.execute("DELETE FROM conversations", [])?;
                    } else {
                        conn.execute(
                            "DELETE FROM conversations WHERE id NOT IN (
                                SELECT id FROM conversations ORDER BY id DESC LIMIT ?1
                             )",
                            [k],
                        )?;
                    }
                    println!("Purged AI history, kept last {} entries", keep_last);
                }
                Err(e) => {
                    tracing::warn!("No SQLite AI history found at {} ({}).", db_path.display(), e);
                    return Ok(2);
                }
            }
            // Also prune rotated JSONL files beyond default of 5 (reuse rotation convention)
            if let Ok(entries) = std::fs::read_dir(&base) {
                let mut rotated: Vec<std::fs::DirEntry> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        if let Some(name) = e.file_name().to_str() {
                            name.starts_with("history-") && name.ends_with(".jsonl")
                        } else {
                            false
                        }
                    })
                    .collect();
                rotated.sort_by_key(|e| e.file_name());
                let to_prune = rotated.len().saturating_sub(5);
                for e in rotated.into_iter().take(to_prune) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
            Ok(0)
        }

        AiCommand::HistoryStatus { json, verbose } => {
            use serde_json::json;
            let r = &config.ai.history_retention;
            let env_overrides = vec![
                (
                    "OPENAGENT_AI_HISTORY_MAX_BYTES",
                    std::env::var("OPENAGENT_AI_HISTORY_MAX_BYTES").ok(),
                ),
                (
                    "OPENAGENT_AI_HISTORY_ROTATED_KEEP",
                    std::env::var("OPENAGENT_AI_HISTORY_ROTATED_KEEP").ok(),
                ),
                (
                    "OPENAGENT_AI_HISTORY_JSONL_MAX_AGE_DAYS",
                    std::env::var("OPENAGENT_AI_HISTORY_JSONL_MAX_AGE_DAYS").ok(),
                ),
                (
                    "OPENAGENT_AI_HISTORY_SQLITE_MAX_ROWS",
                    std::env::var("OPENAGENT_AI_HISTORY_SQLITE_MAX_ROWS").ok(),
                ),
                (
                    "OPENAGENT_AI_HISTORY_SQLITE_MAX_AGE_DAYS",
                    std::env::var("OPENAGENT_AI_HISTORY_SQLITE_MAX_AGE_DAYS").ok(),
                ),
            ];
            let base = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("openagent-terminal")
                .join("ai_history");
            let jsonl = base.join("history.jsonl");
            let db_path = base.join("history.db");

            // JSONL stats
            let jsonl_size = std::fs::metadata(&jsonl).map(|m| m.len()).ok();
            let mut rotated_files: Vec<(String, u64, Option<String>)> = Vec::new();
            let mut rotated_total: u64 = 0;
            let mut oldest: Option<std::time::SystemTime> = None;
            let mut newest: Option<std::time::SystemTime> = None;
            if let Ok(entries) = std::fs::read_dir(&base) {
                for e in entries.flatten() {
                    if let Some(name) = e.file_name().to_str() {
                        if name.starts_with("history-") && name.ends_with(".jsonl") {
                            if let Ok(meta) = e.metadata() {
                                let size = meta.len();
                                rotated_total = rotated_total.saturating_add(size);
                                let mtime = meta.modified().ok();
                                if let Some(t) = mtime {
                                    oldest = Some(oldest.map_or(t, |o| o.min(t)));
                                    newest = Some(newest.map_or(t, |n| n.max(t)));
                                }
                                let ts_str = mtime.map(|t| {
                                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                                    dt.to_rfc3339()
                                });
                                rotated_files.push((name.to_string(), size, ts_str));
                            }
                        }
                    }
                }
            }
            rotated_files.sort_by(|a, b| a.0.cmp(&b.0));
            let oldest_s = oldest.map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
            let newest_s = newest.map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());

            // SQLite stats
            let (db_exists, db_size, row_count, min_ts, max_ts) = match Connection::open(&db_path) {
                Ok(conn) => {
                    let size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
                    let count: i64 = conn
                        .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
                        .unwrap_or(0);
                    let min_ts: Option<String> = conn
                        .query_row("SELECT MIN(ts) FROM conversations", [], |row| row.get(0))
                        .ok();
                    let max_ts: Option<String> = conn
                        .query_row("SELECT MAX(ts) FROM conversations", [], |row| row.get(0))
                        .ok();
                    (true, size, count as u64, min_ts, max_ts)
                }
                Err(_) => (false, 0, 0, None, None),
            };

            if *json {
                let ret = json!({
                    "retention": {
                        "ui_max_entries": r.ui_max_entries,
                        "ui_max_bytes": r.ui_max_bytes,
                        "conversation_jsonl_max_bytes": r.conversation_jsonl_max_bytes,
                        "conversation_rotated_keep": r.conversation_rotated_keep,
                        "conversation_max_rows": r.conversation_max_rows,
                        "conversation_max_age_days": r.conversation_max_age_days,
                    },
                    "env_overrides": env_overrides.iter().map(|(k,v)| json!({"var": k, "value": v})).collect::<Vec<_>>(),
                    "paths": {
                        "base": base.display().to_string(),
                        "jsonl": jsonl.display().to_string(),
                        "sqlite": db_path.display().to_string(),
                    },
                    "jsonl": {
                        "exists": jsonl_size.is_some(),
                        "size_bytes": jsonl_size.unwrap_or(0),
                        "rotated_count": rotated_files.len(),
                        "rotated_total_bytes": rotated_total,
                        "oldest_rotated_mtime": oldest_s,
                        "newest_rotated_mtime": newest_s,
                        "rotated_files": if *verbose { Some(rotated_files.iter().map(|(n,s,t)| json!({"name": n, "size_bytes": s, "mtime": t})).collect::<Vec<_>>()) } else { None },
                    },
                    "sqlite": {
                        "exists": db_exists,
                        "db_size_bytes": db_size,
                        "row_count": row_count,
                        "oldest_ts": min_ts,
                        "newest_ts": max_ts,
                    }
                });
                println!("{}", ret);
            } else {
                println!("AI History Status");
                println!("  Retention:");
                println!("    ui_max_entries: {}", r.ui_max_entries);
                println!("    ui_max_bytes:   {}", r.ui_max_bytes);
                println!("    jsonl_max_bytes:{}", r.conversation_jsonl_max_bytes);
                println!("    rotated_keep:   {}", r.conversation_rotated_keep);
                println!("    sqlite_rows:    {}", r.conversation_max_rows);
                println!("    max_age_days:   {}", r.conversation_max_age_days);
                let any_env = env_overrides.iter().any(|(_, v)| v.is_some());
                println!("  Env overrides active: {}", if any_env { "yes" } else { "no" });
                if *verbose {
                    for (k, v) in &env_overrides {
                        println!("    {} = {}", k, v.as_deref().unwrap_or("<unset>"));
                    }
                }
                println!("  Paths:");
                println!("    base:   {}", base.display());
                println!(
                    "    jsonl:  {} (exists: {}, size: {} bytes)",
                    jsonl.display(),
                    jsonl_size.is_some(),
                    jsonl_size.unwrap_or(0)
                );
                println!(
                    "    sqlite: {} (exists: {}, size: {} bytes)",
                    db_path.display(),
                    db_exists,
                    db_size
                );
                println!(
                    "  JSONL rotated: count={}, total_bytes={} oldest={} newest={}",
                    rotated_files.len(),
                    rotated_total,
                    oldest_s.as_deref().unwrap_or("-"),
                    newest_s.as_deref().unwrap_or("-")
                );
                if *verbose {
                    for (n, s, t) in &rotated_files {
                        println!(
                            "    - {} ({} bytes, mtime={})",
                            n,
                            s,
                            t.as_deref().unwrap_or("-")
                        );
                    }
                }
                if db_exists {
                    println!(
                        "  SQLite: rows={}, oldest_ts={}, newest_ts={}",
                        row_count,
                        min_ts.as_deref().unwrap_or("-"),
                        max_ts.as_deref().unwrap_or("-")
                    );
                }
            }
            Ok(0)
        }

        AiCommand::HistoryConfig { set, config_file, dry_run, json } => {
            use toml::Value as TomlValue;
            if set.is_empty() {
                eprintln!("No --set key=value provided. Keys: ui_max_entries, ui_max_bytes, conversation_jsonl_max_bytes, conversation_rotated_keep, conversation_max_rows, conversation_max_age_days");
                return Ok(2);
            }
            // Determine target config path
            let path = if let Some(p) = config_file {
                p.clone()
            } else if let Some(first) = config.config_paths.first() {
                first.clone()
            } else {
                eprintln!("No loaded config file path available. Use --config-file to specify a target, or add the section manually to your TOML.");
                return Ok(2);
            };
            // Load or create TOML
            let mut doc: TomlValue = if path.exists() {
                let s = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                toml::from_str(&s)
                    .with_context(|| format!("Failed to parse TOML at {}", path.display()))?
            } else {
                TomlValue::Table(toml::map::Map::new())
            };
            // Ensure [ai] and [ai.history_retention]
            if !doc.is_table() {
                doc = TomlValue::Table(toml::map::Map::new());
            }
            let tbl = doc.as_table_mut().unwrap();
            let ai = tbl.entry("ai").or_insert_with(|| TomlValue::Table(toml::map::Map::new()));
            if !ai.is_table() {
                *ai = TomlValue::Table(toml::map::Map::new());
            }
            let ai_tbl = ai.as_table_mut().unwrap();
            let hr = ai_tbl
                .entry("history_retention")
                .or_insert_with(|| TomlValue::Table(toml::map::Map::new()));
            if !hr.is_table() {
                *hr = TomlValue::Table(toml::map::Map::new());
            }
            let hr_tbl = hr.as_table_mut().unwrap();

            // Apply sets
            let mut changes: Vec<(String, String)> = Vec::new();
            for kv in set {
                let (k, vstr) = match kv.split_once('=') {
                    Some((k, v)) => (k.trim().to_string(), v.trim().to_string()),
                    None => {
                        return Err(anyhow!(format!("Invalid --set '{}', expected key=value", kv)));
                    }
                };
                let key_ok = matches!(
                    k.as_str(),
                    "ui_max_entries"
                        | "ui_max_bytes"
                        | "conversation_jsonl_max_bytes"
                        | "conversation_rotated_keep"
                        | "conversation_max_rows"
                        | "conversation_max_age_days"
                );
                if !key_ok {
                    return Err(anyhow!(format!("Unsupported key '{}'", k)));
                }
                let intval: i64 = vstr
                    .parse::<i64>()
                    .map_err(|e| anyhow!(format!("Invalid integer for {}: {}", k, e)))?;
                hr_tbl.insert(k.clone(), TomlValue::Integer(intval));
                changes.push((k, vstr));
            }

            if *dry_run {
                if *json {
                    let out = serde_json::json!({"config_file": path.display().to_string(), "changes": changes});
                    println!("{}", out);
                } else {
                    println!("Would update {} with:", path.display());
                    println!("[ai.history_retention]");
                    for (k, v) in &changes {
                        println!("{} = {}", k, v);
                    }
                }
                return Ok(0);
            }

            // Write back
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed creating parent dir {}", parent.display()))?;
            }
            let s = toml::to_string_pretty(&doc).with_context(|| "Failed to serialize TOML")?;
            std::fs::write(&path, s)
                .with_context(|| format!("Failed to write {}", path.display()))?;
            if *json {
                let out = serde_json::json!({"ok": true, "config_file": path.display().to_string(), "changes": changes});
                println!("{}", out);
            } else {
                println!("Updated {}:", path.display());
                println!("[ai.history_retention]");
                for (k, v) in &changes {
                    println!("{} = {}", k, v);
                }
            }
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::UiConfig;
    use std::fs;
    use tempfile::tempdir;

    fn write_jsonl_entry(dir: &std::path::Path) {
        let ai_dir = dir.join("openagent-terminal").join("ai_history");
        fs::create_dir_all(&ai_dir).unwrap();
        let jsonl = ai_dir.join("history.jsonl");
        let mut f = fs::OpenOptions::new().create(true).append(true).open(jsonl).unwrap();
        writeln!(
            f,
            "{}",
            serde_json::json!({
                "ts": "2025-09-17T08:00:00Z",
                "mode": "prompt",
                "working_directory": "/tmp",
                "shell_kind": "zsh",
                "input": "echo hi",
                "output": "hi"
            })
        )
        .unwrap();
    }

    #[test]
    fn history_export_jsonl_fallback_json() {
        let tmp = tempdir().unwrap();
        std::env::set_var("XDG_DATA_HOME", tmp.path());
        write_jsonl_entry(tmp.path());

        let opts = AiOptions {
            command: AiCommand::HistoryExport {
                format: "json".to_string(),
                to: tmp.path().join("out.json"),
            },
        };
        let cfg = UiConfig::default();
        let code = run_ai_cli(&opts, &cfg).unwrap();
        assert_eq!(code, 0);
        let content = fs::read_to_string(tmp.path().join("out.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(v.as_array().is_some());
        assert_eq!(v.as_array().unwrap().len(), 1);
    }

    #[test]
    fn history_export_jsonl_fallback_csv() {
        let tmp = tempdir().unwrap();
        std::env::set_var("XDG_DATA_HOME", tmp.path());
        write_jsonl_entry(tmp.path());

        let opts = AiOptions {
            command: AiCommand::HistoryExport {
                format: "csv".to_string(),
                to: tmp.path().join("out.csv"),
            },
        };
        let cfg = UiConfig::default();
        let code = run_ai_cli(&opts, &cfg).unwrap();
        assert_eq!(code, 0);
        let content = fs::read_to_string(tmp.path().join("out.csv")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines.len() >= 2); // header + at least one row
    }

    #[test]
    fn history_export_jsonl_fallback_jsonl() {
        let tmp = tempdir().unwrap();
        std::env::set_var("XDG_DATA_HOME", tmp.path());
        write_jsonl_entry(tmp.path());

        let opts = AiOptions {
            command: AiCommand::HistoryExport {
                format: "jsonl".to_string(),
                to: tmp.path().join("out.jsonl"),
            },
        };
        let cfg = UiConfig::default();
        let code = run_ai_cli(&opts, &cfg).unwrap();
        assert_eq!(code, 0);
        let content = fs::read_to_string(tmp.path().join("out.jsonl")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines.len() >= 1);
        // Validate that it's valid JSON per line
        for l in lines {
            if l.trim().is_empty() { continue; }
            let _: serde_json::Value = serde_json::from_str(l).unwrap();
        }
    }
}

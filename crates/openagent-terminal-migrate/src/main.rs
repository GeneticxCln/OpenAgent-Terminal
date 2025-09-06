use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;

mod cli;
mod config;
mod detectors;
mod generators;
mod parsers;
mod validation;

use cli::*;
use config::MigrationConfig;

#[derive(Parser)]
#[command(name = "openagent-migrate")]
#[command(about = "OpenAgent Terminal Configuration Migration Tool")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Auto-detect terminal configs and migrate them
    Auto {
        /// Preview migration without applying changes
        #[arg(short, long)]
        preview: bool,
        /// Force overwrite existing config
        #[arg(short, long)]
        force: bool,
        /// Output directory for migrated config
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Migrate from a specific terminal
    From {
        /// Terminal type to migrate from
        #[arg(value_enum)]
        terminal: TerminalType,
        /// Path to source configuration file
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Preview migration without applying changes
        #[arg(short, long)]
        preview: bool,
        /// Force overwrite existing config
        #[arg(short, long)]
        force: bool,
        /// Output directory for migrated config
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// List supported terminals and their config locations
    List,
    /// Validate an existing OpenAgent Terminal configuration
    Validate {
        /// Path to config file to validate
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Auto { preview, force, output } => {
            println!("{}", "🔍 Auto-detecting terminal configurations...".cyan());

            let detected = detectors::auto_detect_configs()?;

            if detected.is_empty() {
                println!("{}", "❌ No terminal configurations found.".red());
                return Ok(());
            }

            println!(
                "{}",
                format!("✅ Found {} terminal configuration(s)", detected.len()).green()
            );

            for config in detected {
                println!(
                    "  {} {} ({})",
                    "→".blue(),
                    config.terminal_type.to_string().yellow(),
                    config.config_path.display().to_string().dimmed()
                );

                if preview {
                    show_migration_preview(&config)?;
                } else {
                    perform_migration(&config, force, output.as_ref())?;
                }
            }
        },
        Commands::From { terminal, config, preview, force, output } => {
            let config_path = if let Some(path) = config {
                path
            } else {
                // Try to detect default location for this terminal
                detectors::get_default_config_path(&terminal)?
            };

            let migration_config = MigrationConfig {
                terminal_type: terminal,
                config_path,
                detected_automatically: false,
            };

            if preview {
                show_migration_preview(&migration_config)?;
            } else {
                perform_migration(&migration_config, force, output.as_ref())?;
            }
        },
        Commands::List => {
            show_supported_terminals();
        },
        Commands::Validate { config } => {
            let config_path = config.unwrap_or_else(|| {
                dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("openagent-terminal")
                    .join("openagent-terminal.toml")
            });

            validation::validate_config(&config_path)?;
        },
    }

    Ok(())
}

fn show_migration_preview(config: &MigrationConfig) -> Result<()> {
    println!(
        "{}",
        format!("📋 Preview migration from {}", config.terminal_type.to_string()).cyan()
    );

    let parsed = parsers::parse_config(config)?;
    let generated = generators::generate_openagent_config(&parsed)?;

    println!("{}", "Generated configuration:".yellow());
    println!("{}", "─".repeat(50).dimmed());
    println!("{}", generated);
    println!("{}", "─".repeat(50).dimmed());

    Ok(())
}

fn perform_migration(
    config: &MigrationConfig,
    force: bool,
    output_dir: Option<&PathBuf>,
) -> Result<()> {
    println!(
        "{}",
        format!("🔄 Migrating from {} configuration", config.terminal_type.to_string()).cyan()
    );

    let parsed = parsers::parse_config(config)?;
    let generated = generators::generate_openagent_config(&parsed)?;

    let output_path = determine_output_path(output_dir)?;

    if output_path.exists() && !force {
        println!(
            "{}",
            format!("❌ Configuration file already exists: {}", output_path.display()).red()
        );
        println!("    Use --force to overwrite or specify a different --output directory");
        return Ok(());
    }

    // Ensure directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&output_path, generated)?;

    println!(
        "{}",
        format!("✅ Migration complete! Configuration written to: {}", output_path.display())
            .green()
    );
    println!("    You can now use this configuration with OpenAgent Terminal");

    // Run validation on the generated config
    validation::validate_config(&output_path)?;

    Ok(())
}

fn determine_output_path(output_dir: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = output_dir {
        Ok(dir.join("openagent-terminal.toml"))
    } else {
        // Use default OpenAgent Terminal config location
        let config_dir =
            dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("openagent-terminal");

        Ok(config_dir.join("openagent-terminal.toml"))
    }
}

fn show_supported_terminals() {
    println!("{}", "📱 Supported terminals and their configuration locations:".cyan());
    println!();

    for terminal in TerminalType::all() {
        println!("{} {}", "▶".green(), terminal.to_string().yellow().bold());

        if let Ok(paths) = detectors::get_typical_config_locations(&terminal) {
            for path in paths {
                let exists = path.exists();
                let status = if exists { "✓".green() } else { "✗".red() };
                println!("  {} {}", status, path.display().to_string().dimmed());
            }
        }
        println!();
    }

    println!("{}", "Usage examples:".yellow());
    println!(
        "  {}",
        "openagent-migrate auto                    # Auto-detect and migrate".dimmed()
    );
    println!("  {}", "openagent-migrate from alacritty          # Migrate from Alacritty".dimmed());
    println!(
        "  {}",
        "openagent-migrate from iterm2 --preview   # Preview iTerm2 migration".dimmed()
    );
}

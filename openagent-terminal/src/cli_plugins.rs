use clap::{Args, Subcommand};
use std::path::PathBuf;

#[cfg(feature = "plugins")]
use crate::plugins_api::PluginEvent;

#[derive(Args, Debug, Clone)]
pub struct PluginsOptions {
    #[clap(subcommand)]
    pub command: PluginsCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum PluginsCommand {
    /// List loaded plugins (name and path)
    List,
    /// Discover plugins in standard directories
    Discover,
    /// Load a plugin from a WASM file path
    Load { path: PathBuf },
    /// Unload a plugin by ID/name
    Unload { id: String },
    /// Send a simple event to a plugin (JSON data optional)
    Event {
        id: String,
        #[clap(long = "type")]
        event_type: String,
        #[clap(long)]
        data: Option<String>,
    },
}

pub async fn run_plugins_cli(opts: &PluginsOptions) -> anyhow::Result<i32> {
    // Build policy toggles similar to components_init
    let enforce_signatures = true;
    let require_all_default = !cfg!(debug_assertions);
    let hot_reload_default = cfg!(debug_assertions);
    let require_all = std::env::var("OPENAGENT_PLUGINS_REQUIRE_ALL")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(require_all_default);
    let hot_reload = std::env::var("OPENAGENT_PLUGINS_HOT_RELOAD")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(hot_reload_default);
    let require_system = true;
    let require_user = std::env::var("OPENAGENT_PLUGINS_USER_REQUIRE_SIGNED")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let require_project = std::env::var("OPENAGENT_PLUGINS_PROJECT_REQUIRE_SIGNED")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let plugins_dir = if let Some(data) = dirs::data_dir() {
        data.join("openagent-terminal").join("plugins")
    } else {
        PathBuf::from("./.openagent-terminal/plugins")
    };

    let pm = crate::components_init::initialize_plugin_manager(
        plugins_dir,
        enforce_signatures,
        require_all,
        require_system,
        require_user,
        require_project,
        hot_reload,
    )
    .await?;

    match &opts.command {
        PluginsCommand::List => {
            let loaded = pm.loaded_names_and_paths().await;
            for (name, path) in loaded {
                println!("{}\t{}", name, path.display());
            }
            Ok(0)
        }
        PluginsCommand::Discover => match pm.discover_plugins().await {
            Ok(paths) => {
                for p in paths {
                    println!("{}", p.display());
                }
                Ok(0)
            }
            Err(e) => {
                eprintln!("discover failed: {}", e);
                Ok(1)
            }
        },
        PluginsCommand::Load { path } => match pm.load_plugin(path).await {
            Ok(id) => {
                println!("loaded: {}", id);
                Ok(0)
            }
            Err(e) => {
                eprintln!("load failed: {}", e);
                Ok(1)
            }
        },
        PluginsCommand::Unload { id } => match pm.unload_plugin(id).await {
            Ok(()) => {
                println!("unloaded: {}", id);
                Ok(0)
            }
            Err(e) => {
                eprintln!("unload failed: {}", e);
                Ok(1)
            }
        },
        PluginsCommand::Event { id, event_type, data } => {
            let json = match data {
                Some(s) => {
                    serde_json::from_str::<serde_json::Value>(s).unwrap_or(serde_json::json!({}))
                }
                None => serde_json::json!({}),
            };
            let evt = PluginEvent { event_type: event_type.clone(), data: json, timestamp: 0 };
            match pm.send_event_to_plugin(id, &evt).await {
                Ok(resp) => {
                    println!("{}", serde_json::to_string_pretty(&resp).unwrap_or("{}".to_string()));
                    Ok(if resp.success { 0 } else { 2 })
                }
                Err(e) => {
                    eprintln!("event failed: {}", e);
                    Ok(1)
                }
            }
        }
    }
}

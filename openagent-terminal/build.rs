use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;

// Build-time timestamping (avoid external deps in minimal builds)

#[allow(unexpected_cfgs)]
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=extra/");
    println!("cargo:rerun-if-env-changed=OPENAGENT_BUILD_CONFIG");

    // Build configuration
    let build_config = BuildConfig::new();
    build_config.print_status();

    // Generate version information
    generate_version_info(&build_config);

    // OpenGL backend removed: no GL bindings generation.

    // Generate feature flags configuration
    generate_feature_config(&build_config);

    // Bundle assets based on configuration
    bundle_assets(&build_config);

    // Generate platform-specific configuration
    generate_platform_config(&build_config);

    // Handle Windows-specific resources
    #[cfg(windows)]
    handle_windows_resources();

    // Generate runtime configuration
    generate_runtime_config(&build_config);

    // Validate feature combinations
    validate_feature_combinations(&build_config);
}

#[derive(Debug)]
struct BuildConfig {
    target_os: String,
    target_arch: String,
    profile: String,
    features: HashSet<String>,
    version: String,
    commit_hash: Option<String>,
    build_timestamp: String,
    is_release: bool,
    optimization_level: String,
}

impl BuildConfig {
    fn new() -> Self {
        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
        let profile = env::var("PROFILE").unwrap_or_default();
        let is_release = profile == "release";

        let features = collect_enabled_features();

        let mut version = env!("CARGO_PKG_VERSION").to_string();
        let commit_hash = commit_hash();
        if let Some(ref hash) = commit_hash {
            version = format!("{version} ({hash})");
        }

        let build_timestamp = {
            use std::time::{SystemTime, UNIX_EPOCH};
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            secs.to_string()
        };

        let optimization_level = env::var("OPT_LEVEL").unwrap_or_else(|_| {
            if is_release {
                "3".to_string()
            } else {
                "0".to_string()
            }
        });

        Self {
            target_os,
            target_arch,
            profile,
            features,
            version,
            commit_hash,
            build_timestamp,
            is_release,
            optimization_level,
        }
    }

    fn print_status(&self) {
        // Avoid emitting cargo warnings by default; enable verbose build status only if requested.
        let verbose = std::env::var("OPENAGENT_VERBOSE_BUILD")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        if !verbose {
            return;
        }
        println!("cargo:warning=Building OpenAgent Terminal v{}", self.version);
        println!("cargo:warning=Target: {}-{}", self.target_arch, self.target_os);
        println!("cargo:warning=Profile: {} (opt-level={})", self.profile, self.optimization_level);
        if !self.features.is_empty() {
            let feature_list: Vec<&String> = self.features.iter().collect();
            println!("cargo:warning=Features: {feature_list:?}");
        }
    }

    fn has_feature(&self, feature: &str) -> bool {
        self.features.contains(feature)
    }

    fn has_any_security_lens(&self) -> bool {
        self.has_feature("security-lens")
            || self.has_feature("security-lens-extended")
            || self.has_feature("security-lens-platform")
            || self.has_feature("security-lens-advanced")
            || self.has_feature("security-lens-full")
            || self.has_feature("security-lens-dev")
    }
}

fn generate_version_info(config: &BuildConfig) {
    let dest = env::var("OUT_DIR").unwrap();
    let version_file = Path::new(&dest).join("version.rs");

    let mut file = File::create(&version_file).unwrap();
    writeln!(file, "// Auto-generated version information").unwrap();
    writeln!(file, "pub const VERSION: &str = \"{}\";", config.version).unwrap();
    writeln!(file, "pub const PKG_VERSION: &str = \"{}\";", env!("CARGO_PKG_VERSION")).unwrap();

    if let Some(ref hash) = config.commit_hash {
        writeln!(file, "pub const COMMIT_HASH: Option<&str> = Some(\"{hash}\");").unwrap();
    } else {
        writeln!(file, "pub const COMMIT_HASH: Option<&str> = None;").unwrap();
    }

    writeln!(file, "pub const BUILD_TIMESTAMP: &str = \"{}\";", config.build_timestamp).unwrap();
    writeln!(
        file,
        "pub const BUILD_TARGET: &str = \"{}-{}\";",
        config.target_arch, config.target_os
    )
    .unwrap();
    writeln!(file, "pub const BUILD_PROFILE: &str = \"{}\";", config.profile).unwrap();
    writeln!(file, "pub const OPTIMIZATION_LEVEL: &str = \"{}\";", config.optimization_level)
        .unwrap();

    println!("cargo:rustc-env=VERSION={}", config.version);
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", config.build_timestamp);
    println!("cargo:rustc-env=BUILD_TARGET={}-{}", config.target_arch, config.target_os);
}

fn collect_enabled_features() -> HashSet<String> {
    let mut set = HashSet::new();
    for (key, _val) in env::vars() {
        if let Some(stripped) = key.strip_prefix("CARGO_FEATURE_") {
            // Convert ENV_STYLE to cargo feature style (env upper snake -> kebab)
            let feat = stripped.to_ascii_lowercase().replace('_', "-");
            set.insert(feat);
        }
    }
    set
}

fn generate_feature_config(config: &BuildConfig) {
    let dest = env::var("OUT_DIR").unwrap();
    let feature_file = Path::new(&dest).join("features.rs");

    let mut file = File::create(&feature_file).unwrap();
    writeln!(file, "// Auto-generated feature configuration").unwrap();
    writeln!(file, "use std::collections::HashSet;").unwrap();
    writeln!(file).unwrap();

    // Core feature flags
    writeln!(file, "pub const HAS_AI: bool = {};", config.has_feature("ai")).unwrap();
    writeln!(file, "pub const HAS_SYNC: bool = {};", config.has_feature("sync")).unwrap();
    writeln!(file, "pub const HAS_WORKFLOW: bool = {};", config.has_feature("workflow")).unwrap();
    writeln!(file, "pub const HAS_PLUGINS: bool = {};", config.has_feature("plugins")).unwrap();
    writeln!(file, "pub const HAS_BLOCKS: bool = {};", config.has_feature("blocks")).unwrap();
    writeln!(file, "pub const HAS_IDE: bool = {};", config.has_feature("ide")).unwrap();
    writeln!(
        file,
        "pub const HAS_WEBVIEW_EDITORS: bool = {};",
        config.has_feature("webview-editors")
    )
    .unwrap();

    // Rendering backends
    writeln!(file, "pub const HAS_WGPU: bool = {};", config.has_feature("wgpu")).unwrap();
    writeln!(file, "pub const HAS_OPENGL: bool = false;").unwrap();

    // Platform support
    writeln!(file, "pub const HAS_WAYLAND: bool = {};", config.has_feature("wayland")).unwrap();
    writeln!(file, "pub const HAS_X11: bool = {};", config.has_feature("x11")).unwrap();

    // Security Lens features
    writeln!(file, "pub const HAS_SECURITY_LENS: bool = {};", config.has_any_security_lens())
        .unwrap();
    writeln!(
        file,
        "pub const HAS_SECURITY_LENS_CORE: bool = {};",
        config.has_feature("security-lens")
    )
    .unwrap();
    writeln!(
        file,
        "pub const HAS_SECURITY_LENS_EXTENDED: bool = {};",
        config.has_feature("security-lens-extended")
    )
    .unwrap();
    writeln!(
        file,
        "pub const HAS_SECURITY_LENS_PLATFORM: bool = {};",
        config.has_feature("security-lens-platform")
    )
    .unwrap();
    writeln!(
        file,
        "pub const HAS_SECURITY_LENS_ADVANCED: bool = {};",
        config.has_feature("security-lens-advanced")
    )
    .unwrap();
    writeln!(
        file,
        "pub const HAS_SECURITY_LENS_FULL: bool = {};",
        config.has_feature("security-lens-full")
    )
    .unwrap();
    writeln!(
        file,
        "pub const HAS_SECURITY_LENS_DEV: bool = {};",
        config.has_feature("security-lens-dev")
    )
    .unwrap();

    // AI provider features
    writeln!(file, "pub const HAS_AI_OLLAMA: bool = {};", config.has_feature("ai-ollama")).unwrap();
    writeln!(file, "pub const HAS_AI_OPENAI: bool = {};", config.has_feature("ai-openai")).unwrap();
    writeln!(file, "pub const HAS_AI_ANTHROPIC: bool = {};", config.has_feature("ai-anthropic"))
        .unwrap();

    // Text shaping
    writeln!(file, "pub const HAS_HARFBUZZ: bool = {};", config.has_feature("harfbuzz")).unwrap();

    // Generate feature list function
    writeln!(file).unwrap();
    writeln!(file, "pub fn enabled_features() -> HashSet<&'static str> {{").unwrap();
    writeln!(file, "    let mut features = HashSet::new();").unwrap();

    for feature in &config.features {
        writeln!(file, "    features.insert(\"{feature}\");").unwrap();
    }

    writeln!(file, "    features").unwrap();
    writeln!(file, "}}").unwrap();

    // Generate build info function
    writeln!(file).unwrap();
    writeln!(file, "pub fn build_info() -> &'static str {{").unwrap();
    writeln!(
        file,
        "    \"OpenAgent Terminal {} built on {} for {}-{} (profile: {}, opt-level: {})\"",
        config.version,
        config.build_timestamp,
        config.target_arch,
        config.target_os,
        config.profile,
        config.optimization_level
    )
    .unwrap();
    writeln!(file, "}}").unwrap();
}

fn bundle_assets(config: &BuildConfig) {
    let dest = env::var("OUT_DIR").unwrap();
    let assets_file = Path::new(&dest).join("assets.rs");

    let mut file = File::create(&assets_file).unwrap();
    writeln!(file, "// Auto-generated asset bundle").unwrap();
    writeln!(file, "use std::collections::HashMap;").unwrap();
    writeln!(file).unwrap();

    // Bundle themes if available
    if let Ok(themes_dir) = fs::read_dir("extra/themes") {
        writeln!(file, "pub mod themes {{").unwrap();
        writeln!(file, "    use super::*;").unwrap();
        writeln!(file, "    ").unwrap();

        for entry in themes_dir.flatten() {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("toml") {
                if let Some(name) = entry.path().file_stem().and_then(|s| s.to_str()) {
                    let safe_name = name.replace(['-', '.'], "_");
                    writeln!(
                        file,
                        "    pub const {}: &str = \
                         include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \
                         \"/extra/themes/{}\"));",
                        safe_name.to_uppercase(),
                        entry.file_name().to_string_lossy()
                    )
                    .unwrap();
                }
            }
        }

        writeln!(file, "    ").unwrap();
        writeln!(file, "    pub fn get_all_themes() -> HashMap<&'static str, &'static str> {{")
            .unwrap();
        writeln!(file, "        let mut themes = HashMap::new();").unwrap();

        if let Ok(themes_dir) = fs::read_dir("extra/themes") {
            for entry in themes_dir.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("toml") {
                    if let Some(name) = entry.path().file_stem().and_then(|s| s.to_str()) {
                        let safe_name = name.replace(['-', '.'], "_");
                        writeln!(
                            file,
                            "        themes.insert(\"{}\", {});",
                            name,
                            safe_name.to_uppercase()
                        )
                        .unwrap();
                    }
                }
            }
        }

        writeln!(file, "        themes").unwrap();
        writeln!(file, "    }}").unwrap();
        writeln!(file, "}}").unwrap();
        writeln!(file).unwrap();
    }

    // Bundle fonts if available
    if Path::new("extra/fonts").exists() {
        writeln!(file, "pub mod fonts {{").unwrap();
        writeln!(file, "    // Font assets would be embedded here for release builds").unwrap();
        writeln!(file, "    pub const FONTS_DIR: &str = \"extra/fonts\";").unwrap();
        writeln!(file, "}}").unwrap();
        writeln!(file).unwrap();
    }

    // Bundle icons if available
    if Path::new("extra/icons").exists() {
        writeln!(file, "pub mod icons {{").unwrap();
        writeln!(file, "    pub const ICONS_DIR: &str = \"extra/icons\";").unwrap();
        writeln!(file, "}}").unwrap();
        writeln!(file).unwrap();
    }

    // Security Lens patterns
    if config.has_any_security_lens() {
        writeln!(file, "pub mod security_patterns {{").unwrap();
        writeln!(file, "    // Security Lens pattern definitions").unwrap();
        writeln!(file, "    pub const PATTERNS_AVAILABLE: bool = true;").unwrap();
        writeln!(file, "}}").unwrap();
        writeln!(file).unwrap();
    }
}

fn generate_platform_config(config: &BuildConfig) {
    let dest = env::var("OUT_DIR").unwrap();
    let platform_file = Path::new(&dest).join("platform.rs");

    let mut file = File::create(&platform_file).unwrap();
    writeln!(file, "// Auto-generated platform configuration").unwrap();
    writeln!(file).unwrap();

    writeln!(file, "pub const TARGET_OS: &str = \"{}\";", config.target_os).unwrap();
    writeln!(file, "pub const TARGET_ARCH: &str = \"{}\";", config.target_arch).unwrap();

    // Platform-specific configuration
    match config.target_os.as_str() {
        "linux" => {
            writeln!(file, "pub const IS_LINUX: bool = true;").unwrap();
            writeln!(file, "pub const IS_MACOS: bool = false;").unwrap();
            writeln!(file, "pub const IS_WINDOWS: bool = false;").unwrap();
            writeln!(file, "pub const IS_FREEBSD: bool = false;").unwrap();

            writeln!(file, "pub const DEFAULT_SHELL: &str = \"/bin/bash\";").unwrap();
            writeln!(file, "pub const CONFIG_DIR_NAME: &str = \".config/openagent-terminal\";")
                .unwrap();
        }
        "macos" => {
            writeln!(file, "pub const IS_LINUX: bool = false;").unwrap();
            writeln!(file, "pub const IS_MACOS: bool = true;").unwrap();
            writeln!(file, "pub const IS_WINDOWS: bool = false;").unwrap();
            writeln!(file, "pub const IS_FREEBSD: bool = false;").unwrap();

            writeln!(file, "pub const DEFAULT_SHELL: &str = \"/bin/zsh\";").unwrap();
            writeln!(
                file,
                "pub const CONFIG_DIR_NAME: &str = \"Library/Application \
                 Support/openagent-terminal\";"
            )
            .unwrap();
        }
        "windows" => {
            writeln!(file, "pub const IS_LINUX: bool = false;").unwrap();
            writeln!(file, "pub const IS_MACOS: bool = false;").unwrap();
            writeln!(file, "pub const IS_WINDOWS: bool = true;").unwrap();
            writeln!(file, "pub const IS_FREEBSD: bool = false;").unwrap();

            writeln!(file, "pub const DEFAULT_SHELL: &str = \"powershell.exe\";").unwrap();
            writeln!(
                file,
                "pub const CONFIG_DIR_NAME: &str = \"AppData/Roaming/openagent-terminal\";"
            )
            .unwrap();
        }
        "freebsd" => {
            writeln!(file, "pub const IS_LINUX: bool = false;").unwrap();
            writeln!(file, "pub const IS_MACOS: bool = false;").unwrap();
            writeln!(file, "pub const IS_WINDOWS: bool = false;").unwrap();
            writeln!(file, "pub const IS_FREEBSD: bool = true;").unwrap();

            writeln!(file, "pub const DEFAULT_SHELL: &str = \"/bin/sh\";").unwrap();
            writeln!(file, "pub const CONFIG_DIR_NAME: &str = \".config/openagent-terminal\";")
                .unwrap();
        }
        _ => {
            writeln!(file, "pub const IS_LINUX: bool = false;").unwrap();
            writeln!(file, "pub const IS_MACOS: bool = false;").unwrap();
            writeln!(file, "pub const IS_WINDOWS: bool = false;").unwrap();
            writeln!(file, "pub const IS_FREEBSD: bool = false;").unwrap();

            writeln!(file, "pub const DEFAULT_SHELL: &str = \"/bin/sh\";").unwrap();
            writeln!(file, "pub const CONFIG_DIR_NAME: &str = \".config/openagent-terminal\";")
                .unwrap();
        }
    }

    // Windowing system configuration
    writeln!(file).unwrap();
    writeln!(
        file,
        "pub const SUPPORTS_WAYLAND: bool = {};",
        config.has_feature("wayland")
            && (config.target_os == "linux" || config.target_os == "freebsd")
    )
    .unwrap();
    writeln!(
        file,
        "pub const SUPPORTS_X11: bool = {};",
        config.has_feature("x11") && (config.target_os == "linux" || config.target_os == "freebsd")
    )
    .unwrap();
    writeln!(file, "pub const SUPPORTS_WIN32: bool = {};", config.target_os == "windows").unwrap();
    writeln!(file, "pub const SUPPORTS_COCOA: bool = {};", config.target_os == "macos").unwrap();
}

#[cfg(windows)]
fn handle_windows_resources() {
    if Path::new("windows/openagent-terminal.rc").exists() {
        embed_resource::compile("windows/openagent-terminal.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}

fn generate_runtime_config(config: &BuildConfig) {
    let dest = env::var("OUT_DIR").unwrap();
    let runtime_file = Path::new(&dest).join("runtime.rs");

    let mut file = File::create(&runtime_file).unwrap();
    writeln!(file, "// Auto-generated runtime configuration").unwrap();
    writeln!(file).unwrap();

    // Performance settings based on build profile
    if config.is_release {
        writeln!(file, "pub const PERFORMANCE_MODE: &str = \"release\";").unwrap();
        writeln!(file, "pub const ENABLE_PROFILING: bool = false;").unwrap();
        writeln!(file, "pub const ENABLE_DEBUG_LOGGING: bool = false;").unwrap();
        writeln!(file, "pub const FRAME_RATE_CAP: Option<u32> = Some(120); // Cap FPS in release")
            .unwrap();
    } else {
        writeln!(file, "pub const PERFORMANCE_MODE: &str = \"debug\";").unwrap();
        writeln!(file, "pub const ENABLE_PROFILING: bool = true;").unwrap();
        writeln!(file, "pub const ENABLE_DEBUG_LOGGING: bool = true;").unwrap();
        writeln!(file, "pub const FRAME_RATE_CAP: Option<u32> = None; // Unlimited FPS in debug")
            .unwrap();
    }

    // Security Lens runtime settings
    if config.has_any_security_lens() {
        writeln!(file, "pub const SECURITY_LENS_ENABLED: bool = true;").unwrap();
        if config.has_feature("security-lens-dev") {
            writeln!(file, "pub const SECURITY_LENS_DEV_MODE: bool = true;").unwrap();
        } else {
            writeln!(file, "pub const SECURITY_LENS_DEV_MODE: bool = false;").unwrap();
        }
    } else {
        writeln!(file, "pub const SECURITY_LENS_ENABLED: bool = false;").unwrap();
        writeln!(file, "pub const SECURITY_LENS_DEV_MODE: bool = false;").unwrap();
    }

    // Memory settings
    match config.target_arch.as_str() {
        "x86_64" | "aarch64" => {
            writeln!(file, "pub const DEFAULT_SCROLLBACK_LINES: usize = 100_000;").unwrap();
            writeln!(file, "pub const MAX_TEXTURE_SIZE: u32 = 8192;").unwrap();
        }
        _ => {
            writeln!(file, "pub const DEFAULT_SCROLLBACK_LINES: usize = 50_000;").unwrap();
            writeln!(file, "pub const MAX_TEXTURE_SIZE: u32 = 4096;").unwrap();
        }
    }
}

fn validate_feature_combinations(config: &BuildConfig) {
    // Validate AI provider combinations
    let ai_providers = ["ai-ollama", "ai-openai", "ai-anthropic"];
    let enabled_providers: Vec<_> =
        ai_providers.iter().filter(|&&provider| config.has_feature(provider)).collect();

    if enabled_providers.len() > 1 {
        println!("cargo:warning=Multiple AI providers enabled: {enabled_providers:?}. This is supported but may increase binary size.");
    }

    // Validate rendering backend combinations
    if config.has_feature("wgpu") && !config.is_release {
        let verbose = std::env::var("OPENAGENT_VERBOSE_BUILD")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        if verbose {
            println!(
                "cargo:warning=WGPU backend enabled in debug mode. Performance may be degraded."
            );
        }
    }

    // Validate Security Lens combinations
    if config.has_feature("security-lens-dev") && config.is_release {
        println!(
            "cargo:warning=Security Lens dev features enabled in release build. Consider using \
             security-lens-full instead."
        );
    }

    // Validate platform-specific features
    match config.target_os.as_str() {
        "windows" => {
            if config.has_feature("wayland") || config.has_feature("x11") {
                println!(
                    "cargo:warning=Wayland/X11 features enabled on Windows. These will be ignored."
                );
            }
        }
        "macos" => {
            if config.has_feature("wayland") || config.has_feature("x11") {
                println!(
                    "cargo:warning=Wayland/X11 features enabled on macOS. These will be ignored."
                );
            }
        }
        "linux" | "freebsd" => {
            if !config.has_feature("wayland") && !config.has_feature("x11") {
                println!(
                    "cargo:warning=Neither Wayland nor X11 enabled on {}. Terminal may not \
                     display.",
                    config.target_os
                );
            }
        }
        _ => {
            println!("cargo:warning=Building for unsupported platform: {}", config.target_os);
        }
    }
}

fn commit_hash() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|hash| hash.trim().to_string())
}

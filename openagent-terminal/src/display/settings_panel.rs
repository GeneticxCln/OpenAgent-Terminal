// Settings Panel UI: state and rendering for in-app configuration
// Minimal MVP: edit AI provider secrets (API key, model, endpoint) and persist to a secure file

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

use unicode_width::UnicodeWidthStr;

use crate::config::{Action as BindingAction, BindingKey, KeyBinding, UiConfig};
#[cfg(not(feature = "ai"))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AiRoutingMode {
    Auto,
    Agent,
    Provider,
}
#[cfg(not(feature = "ai"))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AiApplyJoinStrategy {
    AndThen,
    Lines,
}
use crate::display::Display;
use crate::renderer::rects::RenderRect;
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::index::{Column, Point};
use winit::keyboard::ModifiersState;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SettingsCategory {
    Ai,
    Theme,
    General,
    Workspace,
    Keybindings,
}

impl SettingsCategory {
    fn as_str(&self) -> &'static str {
        match self {
            SettingsCategory::Ai => "AI",
            SettingsCategory::Theme => "Theme",
            SettingsCategory::General => "General",
            SettingsCategory::Workspace => "Workspace",
            SettingsCategory::Keybindings => "Keybindings",
        }
    }

    fn all() -> &'static [SettingsCategory] {
        const ALL: &[SettingsCategory] = &[
            SettingsCategory::Ai,
            SettingsCategory::Theme,
            SettingsCategory::General,
            SettingsCategory::Workspace,
            SettingsCategory::Keybindings,
        ];
        ALL
    }
}

#[derive(Clone, Debug)]
pub struct SettingsPanelState {
    pub active: bool,
    pub category: SettingsCategory,
    // AI page
    pub provider: String,
    pub selected_field: Field,
    pub ai_enabled: bool,
    pub routing: AiRoutingMode,
    pub apply_joiner: AiApplyJoinStrategy,
    pub privacy_strip_sensitive: bool,
    pub privacy_strip_cwd: bool,
    // Context toggles
    pub ctx_enabled: bool,
    pub ctx_env: bool,
    pub ctx_git: bool,
    pub ctx_file_tree: bool,
    pub api_key: String,
    pub model: String,
    pub endpoint: String,
    // Theme page
    pub theme_name: String,
    pub theme_reduce_motion: bool,
    pub theme_rounded_corners: bool,
    pub theme_corner_radius_px: f32,
    // General page
    pub general_live_reload: bool,
    pub general_working_directory: String,
    pub general_default_shell: String,
    pub general_selected: GeneralField,

    // Workspace page (pane drag & highlights)
    pub ws_highlight_color: Option<[u8; 3]>,
    /// Warp-style completions overlay toggle
    pub ws_completions_enabled: bool,
    pub ws_highlight_alpha_base: f32,
    pub ws_highlight_alpha_hover: f32,
    pub ws_tab_highlight_alpha_base: f32,
    pub ws_tab_highlight_alpha_hover: f32,
    pub ws_new_tab_highlight_alpha_base: f32,
    pub ws_new_tab_highlight_alpha_hover: f32,
    pub ws_tab_drop_snap_px: f32,
    pub ws_new_tab_snap_extra_px: f32,
    // Message area
    pub message: Option<String>,

    // Keybindings page (scaffold)
    pub kb_items: Vec<KbItem>,
    pub kb_filter: String,
    pub kb_selected: usize,
    pub kb_capture_mode: bool,
}

#[derive(Clone, Debug)]
pub struct KbItem {
    pub action: String,
    pub binding: String,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum GeneralField {
    #[default]
    LiveReload,
    WorkingDirectory,
    DefaultShell,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Field {
    #[default]
    Provider,
    AiEnabled,
    PrivacyStripSensitive,
    PrivacyStripCwd,
    CtxEnabled,
    CtxEnv,
    CtxGit,
    CtxFileTree,
    Routing,
    ApplyJoiner,
    ApiKey,
    Model,
    Endpoint,
}

impl Default for SettingsPanelState {
    fn default() -> Self {
        SettingsPanelState {
            active: false,
            category: SettingsCategory::Ai,
            provider: "openrouter".into(),
            selected_field: Field::Provider,
            ai_enabled: false,
            routing: AiRoutingMode::Auto,
            apply_joiner: AiApplyJoinStrategy::AndThen,
            privacy_strip_sensitive: true,
            privacy_strip_cwd: true,
            ctx_enabled: true,
            ctx_env: true,
            ctx_git: true,
            ctx_file_tree: true,
            api_key: String::new(),
            model: String::new(),
            endpoint: String::new(),
            theme_name: String::new(),
            theme_reduce_motion: false,
            theme_rounded_corners: false,
            theme_corner_radius_px: 12.0,
            general_live_reload: true,
            general_working_directory: String::new(),
            general_default_shell: String::new(),
            general_selected: GeneralField::LiveReload,
            message: None,
            // Workspace defaults get filled on open()
            ws_highlight_color: None,
            ws_completions_enabled: true,
            ws_highlight_alpha_base: 0.15,
            ws_highlight_alpha_hover: 0.5,
            ws_tab_highlight_alpha_base: 0.12,
            ws_tab_highlight_alpha_hover: 0.4,
            ws_new_tab_highlight_alpha_base: 0.10,
            ws_new_tab_highlight_alpha_hover: 0.45,
            ws_tab_drop_snap_px: 6.0,
            ws_new_tab_snap_extra_px: 24.0,
            kb_items: Vec::new(),
            kb_filter: String::new(),
            kb_selected: 0,
            kb_capture_mode: false,
        }
    }
}

impl SettingsPanelState {
    pub fn new() -> Self {
        Self { category: SettingsCategory::Ai, ..Default::default() }
    }

    pub fn open(&mut self, config: &UiConfig) {
        self.active = true;
        self.category = SettingsCategory::Ai;
        self.selected_field = Field::Provider;
        self.provider = current_provider_from_config(config);
        self.load_ai_from_secrets();
        // AI enabled and privacy options
        {
            self.ai_enabled = config.ai.enabled;
            self.routing = config.ai.routing;
            self.apply_joiner = config.ai.apply_joiner;
            // Context toggles from config
            self.ctx_enabled = config.ai.context.enabled;
            let providers = &config.ai.context.providers;
            self.ctx_env = providers.iter().any(|p| p == "env");
            self.ctx_git = providers.iter().any(|p| p == "git");
            self.ctx_file_tree = providers.iter().any(|p| p == "file_tree");
        }
        #[cfg(not(feature = "ai"))]
        {
            self.ai_enabled = false;
        }
        // Read privacy opts from secrets/env, default to true
        let secrets = read_secrets_file();
        self.privacy_strip_sensitive =
            secrets.get("OPENAGENT_AI_STRIP_SENSITIVE").map(|v| v != "0").unwrap_or(true);
        self.privacy_strip_cwd =
            secrets.get("OPENAGENT_AI_STRIP_CWD").map(|v| v != "0").unwrap_or(true);
        // Theme
        self.theme_name = config.theme.name.clone().unwrap_or_else(|| "dark".into());
        self.theme_reduce_motion = config.theme.reduce_motion;
        self.theme_rounded_corners = config.theme.rounded_corners;
        self.theme_corner_radius_px = config.theme.corner_radius_px;
        // General
        self.general_live_reload = config.general.live_config_reload;
        self.general_working_directory = config
            .general
            .working_directory
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        self.general_default_shell = config
            .terminal
            .shell
            .as_ref()
            .map(|p| match p {
                crate::config::ui_config::Program::Just(s) => s.clone(),
                crate::config::ui_config::Program::WithArgs { program, args } => {
                    if args.is_empty() {
                        program.clone()
                    } else {
                        format!("{} {}", program, args.join(" "))
                    }
                }
            })
            .unwrap_or_default();
        self.general_selected = GeneralField::LiveReload;

        // Workspace (drag & highlights)
        self.ws_completions_enabled = config.workspace.completions_enabled;
        let dcfg = &config.workspace.drag;
        self.ws_highlight_color = dcfg.highlight_color.get().map(|c| [c.r, c.g, c.b]);
        self.ws_highlight_alpha_base = dcfg.highlight_alpha_base;
        self.ws_highlight_alpha_hover = dcfg.highlight_alpha_hover;
        self.ws_tab_highlight_alpha_base = dcfg.tab_highlight_alpha_base;
        self.ws_tab_highlight_alpha_hover = dcfg.tab_highlight_alpha_hover;
        self.ws_new_tab_highlight_alpha_base = dcfg.new_tab_highlight_alpha_base;
        self.ws_new_tab_highlight_alpha_hover = dcfg.new_tab_highlight_alpha_hover;
        self.ws_tab_drop_snap_px = dcfg.tab_drop_snap_px;
        self.ws_new_tab_snap_extra_px = dcfg.new_tab_snap_extra_px;

        // Keybindings snapshot
        self.load_keybindings_from(config);
        self.kb_filter.clear();
        self.kb_selected = 0;
        self.kb_capture_mode = false;
        self.message = None;
    }

    fn load_ai_from_secrets(&mut self) {
        let secrets = read_secrets_file();
        let envs = provider_env_names(&self.provider);
        self.api_key.clear();
        self.model.clear();
        self.endpoint.clear();
        if let Some(api_var) = envs.api_key_env {
            if let Some(val) = secrets.get(api_var) {
                self.api_key = val.clone();
            }
        }
        if let Some(model_var) = envs.model_env {
            if let Some(val) = secrets.get(model_var) {
                self.model = val.clone();
            }
        }
        if let Some(endpoint_var) = envs.endpoint_env {
            if let Some(val) = secrets.get(endpoint_var) {
                self.endpoint = val.clone();
            }
        }
        if self.endpoint.is_empty() {
            if let Some(def) = envs.default_endpoint {
                self.endpoint = def.to_string();
            }
        }
    }

    pub fn cycle_apply_joiner(&mut self, forward: bool) {
        let options = [AiApplyJoinStrategy::AndThen, AiApplyJoinStrategy::Lines];
        let idx = options.iter().position(|m| *m == self.apply_joiner).unwrap_or(0);
        let next = if forward {
            (idx + 1) % options.len()
        } else {
            (idx + options.len() - 1) % options.len()
        };
        self.apply_joiner = options[next];
    }

    pub fn cycle_routing(&mut self, forward: bool) {
        let options = [AiRoutingMode::Auto, AiRoutingMode::Agent, AiRoutingMode::Provider];
        let idx = options.iter().position(|m| *m == self.routing).unwrap_or(0);
        let next = if forward {
            (idx + 1) % options.len()
        } else {
            (idx + options.len() - 1) % options.len()
        };
        self.routing = options[next];
    }

    pub fn close(&mut self) {
        self.active = false;
        self.message = None;
    }

    pub fn cycle_provider(&mut self, forward: bool) {
        let options = ["openrouter", "openai", "anthropic", "ollama"];
        if let Some(idx) = options.iter().position(|p| *p == self.provider) {
            let next = if forward {
                (idx + 1) % options.len()
            } else {
                (idx + options.len() - 1) % options.len()
            };
            self.provider = options[next].to_string();
            // Reload defaults for new provider
            self.load_ai_from_secrets();
        }
    }

    pub fn next_field(&mut self) {
        match self.category {
            SettingsCategory::Ai => {
                self.selected_field = match self.selected_field {
                    Field::AiEnabled => Field::PrivacyStripSensitive,
                    Field::PrivacyStripSensitive => Field::PrivacyStripCwd,
                    Field::PrivacyStripCwd => Field::CtxEnabled,
                    Field::CtxEnabled => Field::CtxEnv,
                    Field::CtxEnv => Field::CtxGit,
                    Field::CtxGit => Field::CtxFileTree,
                    Field::CtxFileTree => Field::Routing,
                    Field::Routing => Field::ApplyJoiner,
                    Field::ApplyJoiner => Field::Provider,
                    Field::Provider => Field::ApiKey,
                    Field::ApiKey => Field::Model,
                    Field::Model => Field::Endpoint,
                    Field::Endpoint => Field::AiEnabled,
                };
            }
            SettingsCategory::General => {
                self.general_selected = match self.general_selected {
                    GeneralField::LiveReload => GeneralField::WorkingDirectory,
                    GeneralField::WorkingDirectory => GeneralField::DefaultShell,
                    GeneralField::DefaultShell => GeneralField::LiveReload,
                };
            }
            SettingsCategory::Workspace => {
                // No focused subfield tracking yet; kept simple
            }
            SettingsCategory::Keybindings => {}
            SettingsCategory::Theme => {}
        }
    }

    pub fn prev_field(&mut self) {
        match self.category {
            SettingsCategory::Ai => {
                self.selected_field = match self.selected_field {
                    Field::Provider => Field::ApplyJoiner,
                    Field::ApplyJoiner => Field::Routing,
                    Field::Routing => Field::CtxFileTree,
                    Field::AiEnabled => Field::Endpoint,
                    Field::PrivacyStripSensitive => Field::AiEnabled,
                    Field::PrivacyStripCwd => Field::PrivacyStripSensitive,
                    Field::CtxEnabled => Field::PrivacyStripCwd,
                    Field::CtxEnv => Field::CtxEnabled,
                    Field::CtxGit => Field::CtxEnv,
                    Field::CtxFileTree => Field::CtxGit,
                    Field::ApiKey => Field::Provider,
                    Field::Model => Field::ApiKey,
                    Field::Endpoint => Field::Model,
                };
            }
            SettingsCategory::General => {
                self.general_selected = match self.general_selected {
                    GeneralField::LiveReload => GeneralField::DefaultShell,
                    GeneralField::WorkingDirectory => GeneralField::LiveReload,
                    GeneralField::DefaultShell => GeneralField::WorkingDirectory,
                };
            }
            _ => {}
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        if self.category == SettingsCategory::Ai {
            // Toggles for AI-enabled and privacy options
            match self.selected_field {
                Field::AiEnabled => {
                    if ch == ' ' {
                        self.ai_enabled = !self.ai_enabled;
                    }
                    return;
                }
                Field::PrivacyStripSensitive => {
                    if ch == ' ' {
                        self.privacy_strip_sensitive = !self.privacy_strip_sensitive;
                    }
                    return;
                }
                Field::PrivacyStripCwd => {
                    if ch == ' ' {
                        self.privacy_strip_cwd = !self.privacy_strip_cwd;
                    }
                    return;
                }
                Field::CtxEnabled => {
                    if ch == ' ' {
                        self.ctx_enabled = !self.ctx_enabled;
                    }
                    return;
                }
                Field::CtxEnv => {
                    if ch == ' ' {
                        self.ctx_env = !self.ctx_env;
                    }
                    return;
                }
                Field::CtxGit => {
                    if ch == ' ' {
                        self.ctx_git = !self.ctx_git;
                    }
                    return;
                }
                Field::CtxFileTree => {
                    if ch == ' ' {
                        self.ctx_file_tree = !self.ctx_file_tree;
                    }
                    return;
                }
                _ => {}
            }
            if ch.is_control() {
                return;
            }
            match self.selected_field {
                Field::Provider => {} // provider is cycled via arrows
                Field::ApiKey => self.api_key.push(ch),
                Field::Model => self.model.push(ch),
                Field::Endpoint => self.endpoint.push(ch),
                _ => {}
            }
        } else if self.category == SettingsCategory::Theme {
            match ch {
                ' ' => {
                    self.theme_rounded_corners = !self.theme_rounded_corners;
                }
                'm' | 'M' => {
                    self.theme_reduce_motion = !self.theme_reduce_motion;
                }
                '+' => {
                    self.theme_corner_radius_px = (self.theme_corner_radius_px + 1.0).min(64.0);
                }
                '-' => {
                    self.theme_corner_radius_px = (self.theme_corner_radius_px - 1.0).max(0.0);
                }
                c if !c.is_control() => {
                    self.theme_name.push(c);
                }
                _ => {}
            }
        } else if self.category == SettingsCategory::General {
            match self.general_selected {
                GeneralField::WorkingDirectory => {
                    if !ch.is_control() {
                        self.general_working_directory.push(ch);
                    }
                }
                GeneralField::DefaultShell => {
                    if !ch.is_control() {
                        self.general_default_shell.push(ch);
                    }
                }
                _ => {}
            }
        } else if self.category == SettingsCategory::Workspace {
            // Provide basic editing gestures via single-letter commands on this page:
            // 'o' toggles Warp-style completions overlay
            if ch == 'o' || ch == 'O' {
                self.ws_completions_enabled = !self.ws_completions_enabled;
                return;
            }
            // h/H: adjust highlight_alpha_base/hover; t/T: tab alphas; n/N: new-tab alphas,
            // s: increase vertical snap, S: decrease, e: increase new-tab edge snap, E: decrease.
            // c: clear highlight_color (use theme), C: set placeholder color 120,180,255.
            match ch {
                'h' => {
                    self.ws_highlight_alpha_base = (self.ws_highlight_alpha_base + 0.01).min(1.0)
                }
                'H' => {
                    self.ws_highlight_alpha_hover = (self.ws_highlight_alpha_hover + 0.01).min(1.0)
                }
                't' => {
                    self.ws_tab_highlight_alpha_base =
                        (self.ws_tab_highlight_alpha_base + 0.01).min(1.0)
                }
                'T' => {
                    self.ws_tab_highlight_alpha_hover =
                        (self.ws_tab_highlight_alpha_hover + 0.01).min(1.0)
                }
                'n' => {
                    self.ws_new_tab_highlight_alpha_base =
                        (self.ws_new_tab_highlight_alpha_base + 0.01).min(1.0)
                }
                'N' => {
                    self.ws_new_tab_highlight_alpha_hover =
                        (self.ws_new_tab_highlight_alpha_hover + 0.01).min(1.0)
                }
                's' => self.ws_tab_drop_snap_px = (self.ws_tab_drop_snap_px + 1.0).min(64.0),
                'S' => self.ws_tab_drop_snap_px = (self.ws_tab_drop_snap_px - 1.0).max(0.0),
                'e' => {
                    self.ws_new_tab_snap_extra_px = (self.ws_new_tab_snap_extra_px + 2.0).min(128.0)
                }
                'E' => {
                    self.ws_new_tab_snap_extra_px = (self.ws_new_tab_snap_extra_px - 2.0).max(0.0)
                }
                'c' => self.ws_highlight_color = None,
                'C' => self.ws_highlight_color = Some([120, 180, 255]),
                _ => {}
            }
        }
    }

    pub fn backspace(&mut self) {
        if self.category == SettingsCategory::Ai {
            match self.selected_field {
                Field::Provider => {}
                Field::ApiKey => {
                    self.api_key.pop();
                }
                Field::Model => {
                    self.model.pop();
                }
                Field::Endpoint => {
                    self.endpoint.pop();
                }
                // Toggles do not backspace-edit
                Field::AiEnabled
                | Field::PrivacyStripSensitive
                | Field::PrivacyStripCwd
                | Field::CtxEnabled
                | Field::CtxEnv
                | Field::CtxGit
                | Field::CtxFileTree
                | Field::Routing
                | Field::ApplyJoiner => {}
            }
        } else if self.category == SettingsCategory::Theme {
            self.theme_name.pop();
        } else if self.category == SettingsCategory::General {
            match self.general_selected {
                GeneralField::WorkingDirectory => {
                    self.general_working_directory.pop();
                }
                GeneralField::DefaultShell => {
                    self.general_default_shell.pop();
                }
                _ => {}
            }
        }
    }

    pub fn save(&mut self) -> Result<(), String> {
        match self.category {
            SettingsCategory::Ai => {
                let envs = provider_env_names(&self.provider);
                let mut map = read_secrets_file();
                if let Some(api_var) = envs.api_key_env {
                    if !self.api_key.is_empty() {
                        map.insert(api_var.to_string(), self.api_key.clone());
                    }
                }
                if let Some(model_var) = envs.model_env {
                    if !self.model.is_empty() {
                        map.insert(model_var.to_string(), self.model.clone());
                    }
                }
                if let Some(endpoint_var) = envs.endpoint_env {
                    if !self.endpoint.is_empty() {
                        map.insert(endpoint_var.to_string(), self.endpoint.clone());
                    }
                }
                // Privacy toggles as env flags
                map.insert(
                    "OPENAGENT_AI_STRIP_SENSITIVE".to_string(),
                    if self.privacy_strip_sensitive { "1".into() } else { "0".into() },
                );
                map.insert(
                    "OPENAGENT_AI_STRIP_CWD".to_string(),
                    if self.privacy_strip_cwd { "1".into() } else { "0".into() },
                );
                write_secrets_file(&map).map_err(|e| format!("Failed to save secrets: {}", e))?;
                // Write chosen provider, ai.enabled and ai.context providers into main config
                save_ai_provider_to_config(&self.provider)
                    .map_err(|e| format!("Failed to save provider to config: {}", e))?;
                save_ai_enabled_to_config(self.ai_enabled)
                    .map_err(|e| format!("Failed to save AI enabled: {}", e))?;
                save_ai_context_to_config(
                    self.ctx_enabled,
                    self.ctx_env,
                    self.ctx_git,
                    self.ctx_file_tree,
                )
                .map_err(|e| format!("Failed to save AI context: {}", e))?;
                save_ai_routing_to_config(self.routing)
                    .map_err(|e| format!("Failed to save AI routing: {}", e))?;
                save_ai_apply_joiner_to_config(self.apply_joiner)
                    .map_err(|e| format!("Failed to save AI apply joiner: {}", e))?;
                self.message = Some("Saved successfully".to_string());
                Ok(())
            }
            SettingsCategory::Theme => save_theme_to_config(
                &self.theme_name,
                self.theme_reduce_motion,
                self.theme_rounded_corners,
                self.theme_corner_radius_px,
            )
            .map(|_| self.message = Some("Saved theme".to_string()))
            .map_err(|e| format!("Failed to save theme: {}", e)),
            SettingsCategory::General => save_general_to_config(
                self.general_live_reload,
                if self.general_working_directory.trim().is_empty() {
                    None
                } else {
                    Some(self.general_working_directory.clone())
                },
                if self.general_default_shell.trim().is_empty() {
                    None
                } else {
                    Some(self.general_default_shell.clone())
                },
            )
            .map(|_| self.message = Some("Saved general settings".to_string()))
            .map_err(|e| format!("Failed to save general: {}", e)),
            SettingsCategory::Keybindings => {
                self.message = Some("Saved".to_string());
                Ok(())
            }
            SettingsCategory::Workspace => save_workspace_drag_to_config(
                self.ws_highlight_color.map(|c| (c[0], c[1], c[2])),
                self.ws_highlight_alpha_base,
                self.ws_highlight_alpha_hover,
                self.ws_tab_highlight_alpha_base,
                self.ws_tab_highlight_alpha_hover,
                self.ws_new_tab_highlight_alpha_base,
                self.ws_new_tab_highlight_alpha_hover,
                self.ws_tab_drop_snap_px,
                self.ws_new_tab_snap_extra_px,
            )
            .and_then(|_| save_workspace_completions_to_config(self.ws_completions_enabled))
            .map(|_| self.message = Some("Saved workspace settings".into()))
            .map_err(|e| format!("Failed to save workspace: {}", e)),
        }
    }

    pub fn switch_category(&mut self, forward: bool) {
        let cats = SettingsCategory::all();
        if let Some(idx) = cats.iter().position(|c| *c == self.category) {
            let next =
                if forward { (idx + 1) % cats.len() } else { (idx + cats.len() - 1) % cats.len() };
            self.category = cats[next];
        }
    }

    pub fn test_connection(&mut self, _config: &UiConfig) {
        // Validate credentials for the selected provider using secure loader
        let provider = self.provider.to_ascii_lowercase();
        {
            let prov_cfg = _config.ai.providers.get(&provider).cloned().unwrap_or_else(|| {
                // Fallback to defaults if not present
                crate::config::ai_providers::get_default_provider_configs()
                    .remove(&provider)
                    .unwrap_or_default()
            });
            match crate::config::ai_providers::ProviderCredentials::from_config(
                &provider, &prov_cfg,
            ) {
                Ok(creds) => {
                    // Basic checks
                    let mut ok = true;
                    if provider != "ollama" && creds.api_key.is_none() {
                        ok = false;
                    }
                    if creds.model.is_none() {
                        ok = false;
                    }
                    if creds.endpoint.is_none() {
                        ok = false;
                    }
                    if ok {
                        self.message = Some(format!("Credentials OK for '{}'.", provider));
                    } else {
                        self.message = Some(format!("Incomplete credentials for '{}'.", provider));
                    }
                }
                Err(e) => {
                    self.message = Some(format!("Credential error: {}", e));
                }
            }
        }
        #[cfg(not(feature = "ai"))]
        {
            self.message = Some("AI is not enabled in this build".to_string());
        }
    }
}

// Provider env var names used by the secure store
struct ProviderEnvs<'a> {
    api_key_env: Option<&'a str>,
    model_env: Option<&'a str>,
    endpoint_env: Option<&'a str>,
    default_endpoint: Option<&'a str>,
}

fn provider_env_names(provider: &str) -> ProviderEnvs<'_> {
    match provider.to_ascii_lowercase().as_str() {
        "openrouter" => ProviderEnvs {
            api_key_env: Some("OPENAGENT_OPENROUTER_API_KEY"),
            model_env: Some("OPENAGENT_OPENROUTER_MODEL"),
            endpoint_env: Some("OPENAGENT_OPENROUTER_ENDPOINT"),
            default_endpoint: Some("https://openrouter.ai/api/v1"),
        },
        "openai" => ProviderEnvs {
            api_key_env: Some("OPENAGENT_OPENAI_API_KEY"),
            model_env: Some("OPENAGENT_OPENAI_MODEL"),
            endpoint_env: Some("OPENAGENT_OPENAI_ENDPOINT"),
            default_endpoint: Some("https://api.openai.com/v1"),
        },
        "anthropic" => ProviderEnvs {
            api_key_env: Some("OPENAGENT_ANTHROPIC_API_KEY"),
            model_env: Some("OPENAGENT_ANTHROPIC_MODEL"),
            endpoint_env: Some("OPENAGENT_ANTHROPIC_ENDPOINT"),
            default_endpoint: Some("https://api.anthropic.com/v1"),
        },
        "ollama" => ProviderEnvs {
            api_key_env: None,
            model_env: Some("OPENAGENT_OLLAMA_MODEL"),
            endpoint_env: Some("OPENAGENT_OLLAMA_ENDPOINT"),
            default_endpoint: Some("http://localhost:11434"),
        },
        _ => ProviderEnvs {
            api_key_env: None,
            model_env: None,
            endpoint_env: None,
            default_endpoint: None,
        },
    }
}

fn config_path() -> PathBuf {
    if let Some(path) = crate::config::installed_config("toml") {
        return path;
    }
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("openagent-terminal").join("openagent-terminal.toml")
}

fn secrets_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("openagent-terminal").join("secrets.toml")
}

fn read_secrets_file() -> HashMap<String, String> {
    let path = secrets_path();
    let mut map = HashMap::new();
    if let Ok(mut f) = fs::File::open(&path) {
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            if let Ok(val) = toml::from_str::<toml::Value>(&s) {
                if let Some(tbl) = val.get("secrets").and_then(|v| v.as_table()) {
                    for (k, v) in tbl {
                        if let Some(sv) = v.as_str() {
                            map.insert(k.clone(), sv.to_string());
                        }
                    }
                }
            }
        }
    }
    map
}

fn write_secrets_file(map: &HashMap<String, String>) -> std::io::Result<()> {
    let path = secrets_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    // Build TOML
    let mut tbl = toml::value::Table::new();
    for (k, v) in map {
        tbl.insert(k.clone(), toml::Value::String(v.clone()));
    }
    let mut root = toml::value::Table::new();
    root.insert("secrets".to_string(), toml::Value::Table(tbl));
    let s = toml::to_string_pretty(&toml::Value::Table(root)).unwrap_or_default();

    let mut f = fs::File::create(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = f.metadata()?.permissions();
        perm.set_mode(0o600);
        fs::set_permissions(&path, perm)?;
    }
    f.write_all(s.as_bytes())
}

fn save_theme_to_config(
    name: &str,
    reduce_motion: bool,
    rounded_corners: bool,
    corner_radius_px: f32,
) -> std::io::Result<()> {
    let path = config_path();
    let mut root = if let Ok(text) = fs::read_to_string(&path) {
        toml::from_str::<toml::Value>(&text)
            .unwrap_or(toml::Value::Table(toml::value::Table::new()))
    } else {
        toml::Value::Table(toml::value::Table::new())
    };
    if !root.is_table() {
        root = toml::Value::Table(toml::value::Table::new());
    }
    let tbl = root.as_table_mut().unwrap();
    let theme_tbl =
        tbl.entry("theme").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let theme = theme_tbl.as_table_mut().unwrap();
    theme.insert("name".to_string(), toml::Value::String(name.to_string()));
    theme.insert("reduce_motion".to_string(), toml::Value::Boolean(reduce_motion));
    theme.insert("rounded_corners".to_string(), toml::Value::Boolean(rounded_corners));
    theme.insert("corner_radius_px".to_string(), toml::Value::Float(corner_radius_px as f64));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let s = toml::to_string_pretty(&root).unwrap_or_default();
    fs::write(&path, s)
}

fn save_ai_provider_to_config(provider: &str) -> std::io::Result<()> {
    let path = config_path();
    let mut root = if let Ok(text) = fs::read_to_string(&path) {
        toml::from_str::<toml::Value>(&text)
            .unwrap_or(toml::Value::Table(toml::value::Table::new()))
    } else {
        toml::Value::Table(toml::value::Table::new())
    };
    if !root.is_table() {
        root = toml::Value::Table(toml::value::Table::new());
    }
    let tbl = root.as_table_mut().unwrap();
    let ai_tbl = tbl.entry("ai").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let ai = ai_tbl.as_table_mut().unwrap();
    ai.insert("provider".to_string(), toml::Value::String(provider.to_string()));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let s = toml::to_string_pretty(&root).unwrap_or_default();
    fs::write(&path, s)
}

fn save_ai_routing_to_config(mode: AiRoutingMode) -> std::io::Result<()> {
    let path = config_path();
    let mut root = if let Ok(text) = fs::read_to_string(&path) {
        toml::from_str::<toml::Value>(&text)
            .unwrap_or(toml::Value::Table(toml::value::Table::new()))
    } else {
        toml::Value::Table(toml::value::Table::new())
    };
    if !root.is_table() {
        root = toml::Value::Table(toml::value::Table::new());
    }
    let tbl = root.as_table_mut().unwrap();
    let ai_tbl = tbl.entry("ai").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let ai = ai_tbl.as_table_mut().unwrap();
    let val = match mode {
        AiRoutingMode::Auto => "auto",
        AiRoutingMode::Agent => "agent",
        AiRoutingMode::Provider => "provider",
    };
    ai.insert("routing".to_string(), toml::Value::String(val.to_string()));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let s = toml::to_string_pretty(&root).unwrap_or_default();
    fs::write(&path, s)
}

fn save_ai_apply_joiner_to_config(mode: AiApplyJoinStrategy) -> std::io::Result<()> {
    let path = config_path();
    let mut root = if let Ok(text) = fs::read_to_string(&path) {
        toml::from_str::<toml::Value>(&text)
            .unwrap_or(toml::Value::Table(toml::value::Table::new()))
    } else {
        toml::Value::Table(toml::value::Table::new())
    };
    if !root.is_table() {
        root = toml::Value::Table(toml::value::Table::new());
    }
    let tbl = root.as_table_mut().unwrap();
    let ai_tbl = tbl.entry("ai").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let ai = ai_tbl.as_table_mut().unwrap();
    let val = match mode {
        AiApplyJoinStrategy::AndThen => "andthen",
        AiApplyJoinStrategy::Lines => "lines",
    };
    ai.insert("apply_joiner".to_string(), toml::Value::String(val.to_string()));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let s = toml::to_string_pretty(&root).unwrap_or_default();
    fs::write(&path, s)
}

fn save_ai_enabled_to_config(enabled: bool) -> std::io::Result<()> {
    let path = config_path();
    let mut root = if let Ok(text) = fs::read_to_string(&path) {
        toml::from_str::<toml::Value>(&text)
            .unwrap_or(toml::Value::Table(toml::value::Table::new()))
    } else {
        toml::Value::Table(toml::value::Table::new())
    };
    if !root.is_table() {
        root = toml::Value::Table(toml::value::Table::new());
    }
    let tbl = root.as_table_mut().unwrap();
    let ai_tbl = tbl.entry("ai").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let ai = ai_tbl.as_table_mut().unwrap();
    ai.insert("enabled".to_string(), toml::Value::Boolean(enabled));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let s = toml::to_string_pretty(&root).unwrap_or_default();
    fs::write(&path, s)
}

fn save_ai_context_to_config(
    ctx_enabled: bool,
    env_on: bool,
    git_on: bool,
    file_tree_on: bool,
) -> std::io::Result<()> {
    let path = config_path();
    let mut root = if let Ok(text) = fs::read_to_string(&path) {
        toml::from_str::<toml::Value>(&text)
            .unwrap_or(toml::Value::Table(toml::value::Table::new()))
    } else {
        toml::Value::Table(toml::value::Table::new())
    };
    if !root.is_table() {
        root = toml::Value::Table(toml::value::Table::new());
    }
    let tbl = root.as_table_mut().unwrap();

    let ai_tbl = tbl.entry("ai").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let ai = ai_tbl.as_table_mut().unwrap();

    let ctx_tbl =
        ai.entry("context").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let ctx = ctx_tbl.as_table_mut().unwrap();
    ctx.insert("enabled".into(), toml::Value::Boolean(ctx_enabled));

    // Build providers array from toggles
    let mut provs = Vec::new();
    if env_on {
        provs.push(toml::Value::String("env".into()));
    }
    if git_on {
        provs.push(toml::Value::String("git".into()));
    }
    if file_tree_on {
        provs.push(toml::Value::String("file_tree".into()));
    }
    ctx.insert("providers".into(), toml::Value::Array(provs));

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let s = toml::to_string_pretty(&root).unwrap_or_default();
    fs::write(&path, s)
}

fn save_general_to_config(
    live_reload: bool,
    working_directory: Option<String>,
    default_shell: Option<String>,
) -> std::io::Result<()> {
    let path = config_path();
    let mut root = if let Ok(text) = fs::read_to_string(&path) {
        toml::from_str::<toml::Value>(&text)
            .unwrap_or(toml::Value::Table(toml::value::Table::new()))
    } else {
        toml::Value::Table(toml::value::Table::new())
    };
    if !root.is_table() {
        root = toml::Value::Table(toml::value::Table::new());
    }
    let tbl = root.as_table_mut().unwrap();

    // general table
    let general_tbl =
        tbl.entry("general").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let general = general_tbl.as_table_mut().unwrap();
    general.insert("live_config_reload".to_string(), toml::Value::Boolean(live_reload));
    match working_directory {
        Some(dir) if !dir.trim().is_empty() => {
            general.insert("working_directory".to_string(), toml::Value::String(dir));
        }
        _ => {
            general.remove("working_directory");
        }
    }

    // terminal.shell
    if let Some(shell) = default_shell.filter(|s| !s.trim().is_empty()) {
        let term_tbl =
            tbl.entry("terminal").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
        let term = term_tbl.as_table_mut().unwrap();
        // Store as simple string for Program::Just
        term.insert("shell".to_string(), toml::Value::String(shell));
    }

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let s = toml::to_string_pretty(&root).unwrap_or_default();
    fs::write(&path, s)
}

fn current_provider_from_config(_config: &UiConfig) -> String {
    {
        _config.ai.provider.as_deref().unwrap_or("openrouter").to_string()
    }
    #[cfg(not(feature = "ai"))]
    {
        "null".to_string()
    }
}

impl SettingsPanelState {
    fn load_keybindings_from(&mut self, config: &UiConfig) {
        self.kb_items.clear();
        for kb in config.key_bindings() {
            let action = format_action(&kb.action);
            let binding = format_binding_for_display(kb);
            self.kb_items.push(KbItem { action, binding });
        }
    }

    pub fn move_kb_selection(&mut self, delta: isize) {
        let filtered_len = self
            .kb_items
            .iter()
            .filter(|it| {
                it.action.contains(&self.kb_filter) || it.binding.contains(&self.kb_filter)
            })
            .count();
        if filtered_len == 0 {
            self.kb_selected = 0;
            return;
        }
        let cur = self.kb_selected as isize;
        let mut next = cur + delta;
        if next < 0 {
            next = 0;
        }
        if next as usize >= filtered_len {
            next = filtered_len as isize - 1;
        }
        self.kb_selected = next as usize;
    }

    pub fn begin_kb_capture(&mut self) {
        self.kb_capture_mode = true;
        self.message = Some("Press new key combo (Esc to cancel)".into());
    }

    pub fn cancel_kb_capture(&mut self) {
        self.kb_capture_mode = false;
        self.message = Some("Capture canceled".into());
    }

    pub fn is_kb_capturing(&self) -> bool {
        self.kb_capture_mode
    }

    pub fn capture_kb_binding(
        &mut self,
        config: &UiConfig,
        key: winit::keyboard::Key<String>,
        mods: ModifiersState,
    ) -> Result<(), String> {
        // Determine selected action under current filter
        let filtered: Vec<&KbItem> = self
            .kb_items
            .iter()
            .filter(|it| {
                it.action.contains(&self.kb_filter) || it.binding.contains(&self.kb_filter)
            })
            .collect();
        if filtered.is_empty() {
            return Err("No selection".into());
        }
        let target = filtered[self.kb_selected].action.clone();

        // Conflict detection: disallow if any existing binding has same mods+trigger
        let key_str = key_string_for_config_from_key(&key);
        let mods_str = mods_string_for_config(mods);
        for kb in config.key_bindings() {
            let exist_key = key_string_for_config_from_binding(&kb.trigger);
            let exist_mods = mods_string_for_config(kb.mods);
            if exist_key == key_str && exist_mods == mods_str {
                return Err(format!(
                    "Conflict with existing binding for action {}",
                    format_action(&kb.action)
                ));
            }
        }

        // Persist into main config under [[keyboard.bindings]] as an additional binding
        save_keybinding_override_to_config(&target, &key_str, &mods_str)
            .map_err(|e| format!("Failed to save keybinding: {}", e))?;

        self.kb_capture_mode = false;
        self.message = Some(format!("Added binding {} {} for {}", mods_str, key_str, target));
        Ok(())
    }
}

impl Display {
    pub fn draw_settings_panel_overlay(&mut self, config: &UiConfig, st: &SettingsPanelState) {
        if !st.active {
            return;
        }
        let size_info = self.size_info;
        let theme =
            config.resolved_theme.as_ref().cloned().unwrap_or_else(|| config.theme.resolve());
        let tokens = theme.tokens;

        // Panel sizing: 40% of viewport height, min 8 lines
        let num_lines = size_info.screen_lines();
        let target_lines = ((num_lines as f32 * 0.40).round() as usize).clamp(8, num_lines);
        let start_line = num_lines.saturating_sub(target_lines);
        let panel_y = start_line as f32 * size_info.cell_height();
        let panel_h = target_lines as f32 * size_info.cell_height();

        // Backdrop + panel
        let rects = vec![
            RenderRect::new(0.0, 0.0, size_info.width(), size_info.height(), tokens.overlay, 0.18),
            RenderRect::new(0.0, panel_y, size_info.width(), panel_h, tokens.surface_muted, 0.96),
        ];
        let metrics = self.glyph_cache.font_metrics();
        let size_copy = self.size_info;
        self.renderer_draw_rects(&size_copy, &metrics, rects);

        let num_cols = size_info.columns();
        let fg = tokens.text;
        let bg = tokens.surface_muted;
        let mut line = start_line;

        // Header with categories
        let mut header = String::from("Settings — ");
        header.push_str(st.category.as_str());
        self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &header, num_cols - 2);
        // Draw category chips inline
        let mut ccol = 2 + header.width() + 2;
        for cat in SettingsCategory::all() {
            let name = cat.as_str();
            let label = if *cat == st.category { format!("[{}]", name) } else { name.to_string() };
            let color = if *cat == st.category { tokens.accent } else { tokens.text_muted };
            if ccol < num_cols {
                self.draw_ai_text(
                    Point::new(line, Column(ccol)),
                    color,
                    bg,
                    &label,
                    num_cols - ccol,
                );
            }
            ccol += label.width() + 2;
        }
        line += 2;

        match st.category {
            SettingsCategory::Ai => {
                // AI enabled toggle
                let enabled_lbl = "AI enabled: ";
                let enabled_val = if st.ai_enabled { "on" } else { "off" };
                let enabled_row = format!("{}{} (Space to toggle)", enabled_lbl, enabled_val);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &enabled_row, num_cols - 2);
                if st.selected_field == Field::AiEnabled {
                    let cur_col = 2 + enabled_lbl.width();
                    self.draw_ai_text(Point::new(line, Column(cur_col)), bg, fg, " ", 1);
                }
                line += 1;

                // Privacy toggles
                let ps_lbl = "Privacy: strip sensitive: ";
                let ps_val = if st.privacy_strip_sensitive { "on" } else { "off" };
                let ps_row = format!("{}{} (Space)", ps_lbl, ps_val);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &ps_row, num_cols - 2);
                if st.selected_field == Field::PrivacyStripSensitive {
                    let cur_col = 2 + ps_lbl.width();
                    self.draw_ai_text(Point::new(line, Column(cur_col)), bg, fg, " ", 1);
                }
                line += 1;

                let pc_lbl = "Privacy: strip cwd: ";
                let pc_val = if st.privacy_strip_cwd { "on" } else { "off" };
                let pc_row = format!("{}{} (Space)", pc_lbl, pc_val);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &pc_row, num_cols - 2);
                if st.selected_field == Field::PrivacyStripCwd {
                    let cur_col = 2 + pc_lbl.width();
                    self.draw_ai_text(Point::new(line, Column(cur_col)), bg, fg, " ", 1);
                }
                line += 1;

                // Context toggles
                let ctx_en_lbl = "Context: enabled: ";
                let ctx_en_val = if st.ctx_enabled { "on" } else { "off" };
                let ctx_en_row = format!("{}{} (Space)", ctx_en_lbl, ctx_en_val);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &ctx_en_row, num_cols - 2);
                if st.selected_field == Field::CtxEnabled {
                    let cur_col = 2 + ctx_en_lbl.width();
                    self.draw_ai_text(Point::new(line, Column(cur_col)), bg, fg, " ", 1);
                }
                line += 1;

                let provs_row = format!(
                    "Providers: env [{}]  git [{}]  file_tree [{}]  (Space to toggle)",
                    if st.ctx_env { "x" } else { " " },
                    if st.ctx_git { "x" } else { " " },
                    if st.ctx_file_tree { "x" } else { " " }
                );
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &provs_row, num_cols - 2);
                // Cursor for the first of provider toggles when selected
                if matches!(st.selected_field, Field::CtxEnv | Field::CtxGit | Field::CtxFileTree) {
                    self.draw_ai_text(
                        Point::new(line, Column(2 + "Providers: env [".width())),
                        bg,
                        fg,
                        " ",
                        1,
                    );
                }
                line += 1;

                // Routing mode row
                let routing_lbl = "Routing: ";
                let routing_val = match st.routing {
                    AiRoutingMode::Auto => "auto",
                    AiRoutingMode::Agent => "agent",
                    AiRoutingMode::Provider => "provider",
                };
                let routing_row = format!("{}[{}] (Left/Right)", routing_lbl, routing_val);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &routing_row, num_cols - 2);
                if st.selected_field == Field::Routing {
                    let cur_col = 2 + routing_lbl.width();
                    self.draw_ai_text(Point::new(line, Column(cur_col)), bg, fg, " ", 1);
                }
                line += 1;

                // Apply multi-command joiner
                let join_lbl = "Apply multi-command: ";
                let join_val = match st.apply_joiner {
                    AiApplyJoinStrategy::AndThen => "and_then",
                    AiApplyJoinStrategy::Lines => "lines",
                };
                let join_row = format!("{}[{}] (Left/Right)", join_lbl, join_val);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &join_row, num_cols - 2);
                if st.selected_field == Field::ApplyJoiner {
                    let cur_col = 2 + join_lbl.width();
                    self.draw_ai_text(Point::new(line, Column(cur_col)), bg, fg, " ", 1);
                }
                line += 1;

                // Provider selection row
                let provider_row = format!("Provider: [{}]", st.provider);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &provider_row, num_cols - 2);
                if st.selected_field == Field::Provider {
                    // simple cursor box
                    let cur_col = 2 + "Provider: [".width();
                    self.draw_ai_text(Point::new(line, Column(cur_col)), bg, fg, " ", 1);
                }
                line += 2;

                // API Key
                let api_lbl = "API Key: ";
                let masked = if st.api_key.is_empty() {
                    "".to_string()
                } else {
                    "••••••••".to_string()
                };
                let api_row = format!("{}{}", api_lbl, masked);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &api_row, num_cols - 2);
                if st.selected_field == Field::ApiKey {
                    let cur_col = 2 + api_lbl.width() + masked.width();
                    self.draw_ai_text(
                        Point::new(line, Column(cur_col.min(num_cols.saturating_sub(1)))),
                        bg,
                        fg,
                        " ",
                        1,
                    );
                }
                line += 1;

                // Model
                let mdl_lbl = "Model: ";
                let mdl_row = format!("{}{}", mdl_lbl, st.model);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &mdl_row, num_cols - 2);
                if st.selected_field == Field::Model {
                    let cur_col = 2 + mdl_lbl.width() + st.model.width();
                    self.draw_ai_text(
                        Point::new(line, Column(cur_col.min(num_cols.saturating_sub(1)))),
                        bg,
                        fg,
                        " ",
                        1,
                    );
                }
                line += 1;

                // Endpoint
                let ep_lbl = "Endpoint: ";
                let ep_row = format!("{}{}", ep_lbl, st.endpoint);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &ep_row, num_cols - 2);
                if st.selected_field == Field::Endpoint {
                    let cur_col = 2 + ep_lbl.width() + st.endpoint.width();
                    self.draw_ai_text(
                        Point::new(line, Column(cur_col.min(num_cols.saturating_sub(1)))),
                        bg,
                        fg,
                        " ",
                        1,
                    );
                }
                line += 2;

                // Footer / message
                if let Some(msg) = &st.message {
                    let m = msg.to_string();
                    self.draw_ai_text(
                        Point::new(line, Column(2)),
                        tokens.success,
                        bg,
                        &m,
                        num_cols - 2,
                    );
                } else {
                    let hint = "Enter: Save  •  Esc: Close  •  Tab/Shift+Tab: Next/Prev field  •  Ctrl+Left/Right: Switch category  •  Left/Right: Cycle provider  •  T: Test Connection";
                    self.draw_ai_text(
                        Point::new(line, Column(2)),
                        tokens.text_muted,
                        bg,
                        hint,
                        num_cols - 2,
                    );
                }
            }
            SettingsCategory::Theme => {
                // Theme Name
                let name_lbl = "Theme name: ";
                let name_row = format!("{}{}", name_lbl, st.theme_name);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &name_row, num_cols - 2);
                line += 1;
                // Reduce motion
                let rm_lbl = "Reduce motion: ";
                let rm_val = if st.theme_reduce_motion { "on" } else { "off" };
                let rm_row = format!("{}{} (press 'm' to toggle)", rm_lbl, rm_val);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &rm_row, num_cols - 2);
                line += 1;
                // Rounded corners
                let rc_lbl = "Rounded corners: ";
                let rc_val = if st.theme_rounded_corners { "on" } else { "off" };
                let rc_row = format!("{}{} (Space to toggle)", rc_lbl, rc_val);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &rc_row, num_cols - 2);
                line += 1;
                // Corner radius
                let cr_lbl = "Corner radius px: ";
                let cr_row = format!("{}{:.1} (use +/-)", cr_lbl, st.theme_corner_radius_px);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &cr_row, num_cols - 2);
                line += 2;
                if let Some(msg) = &st.message {
                    self.draw_ai_text(
                        Point::new(line, Column(2)),
                        tokens.success,
                        bg,
                        msg,
                        num_cols - 2,
                    );
                } else {
                    let hint = "Enter: Save  •  Esc: Close  •  Ctrl+Left/Right: Switch category  •  Type to edit theme name";
                    self.draw_ai_text(
                        Point::new(line, Column(2)),
                        tokens.text_muted,
                        bg,
                        hint,
                        num_cols - 2,
                    );
                }
            }
            SettingsCategory::General => {
                // Live reload
                let lr_lbl = "Live config reload: ";
                let lr_val = if st.general_live_reload { "on" } else { "off" };
                let lr_row = format!("{}{} (Space to toggle)", lr_lbl, lr_val);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &lr_row, num_cols - 2);
                if st.general_selected == GeneralField::LiveReload {
                    let cur_col = 2 + lr_lbl.width();
                    self.draw_ai_text(Point::new(line, Column(cur_col)), bg, fg, " ", 1);
                }
                line += 1;

                // Working directory
                let wd_lbl = "Working directory: ";
                let wd_row = format!("{}{}", wd_lbl, st.general_working_directory);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &wd_row, num_cols - 2);
                if st.general_selected == GeneralField::WorkingDirectory {
                    let cur_col = 2 + wd_lbl.width() + st.general_working_directory.width();
                    self.draw_ai_text(
                        Point::new(line, Column(cur_col.min(num_cols.saturating_sub(1)))),
                        bg,
                        fg,
                        " ",
                        1,
                    );
                }
                line += 1;

                // Default shell
                let sh_lbl = "Default shell: ";
                let sh_row = format!("{}{}", sh_lbl, st.general_default_shell);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &sh_row, num_cols - 2);
                if st.general_selected == GeneralField::DefaultShell {
                    let cur_col = 2 + sh_lbl.width() + st.general_default_shell.width();
                    self.draw_ai_text(
                        Point::new(line, Column(cur_col.min(num_cols.saturating_sub(1)))),
                        bg,
                        fg,
                        " ",
                        1,
                    );
                }
                line += 2;

                if let Some(msg) = &st.message {
                    self.draw_ai_text(
                        Point::new(line, Column(2)),
                        tokens.success,
                        bg,
                        msg,
                        num_cols - 2,
                    );
                } else {
                    let hint = "Enter: Save  •  Esc: Close  •  Tab/Shift+Tab: Next/Prev field  •  Ctrl+Left/Right: Switch category";
                    self.draw_ai_text(
                        Point::new(line, Column(2)),
                        tokens.text_muted,
                        bg,
                        hint,
                        num_cols - 2,
                    );
                }
            }
            SettingsCategory::Workspace => {
                // Workspace drag/highlight settings
                let title = "Workspace — Pane drag & highlights";
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, title, num_cols - 2);
                line += 1;

                // Highlight color (RGB)
                let hc_lbl = "Highlight color (RGB): ";
                let hc_val = st
                    .ws_highlight_color
                    .map(|c| format!("{:>3},{:>3},{:>3}", c[0], c[1], c[2]))
                    .unwrap_or_else(|| "—".into());
                let hc_row = format!("{}{}", hc_lbl, hc_val);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &hc_row, num_cols - 2);
                line += 1;

                // Alpha ramps
                let a1 = format!(
                    "Split highlight alpha (base/hover): {:.2} / {:.2}",
                    st.ws_highlight_alpha_base, st.ws_highlight_alpha_hover
                );
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &a1, num_cols - 2);
                line += 1;
                let a2 = format!(
                    "Tab highlight alpha (base/hover): {:.2} / {:.2}",
                    st.ws_tab_highlight_alpha_base, st.ws_tab_highlight_alpha_hover
                );
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &a2, num_cols - 2);
                line += 1;
                let a3 = format!(
                    "New Tab highlight alpha (base/hover): {:.2} / {:.2}",
                    st.ws_new_tab_highlight_alpha_base, st.ws_new_tab_highlight_alpha_hover
                );
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &a3, num_cols - 2);
                line += 1;

                // Snapping margins
                let s1 = format!("Tab bar vertical snap px: {:.1}", st.ws_tab_drop_snap_px);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &s1, num_cols - 2);
                line += 1;
                let s2 =
                    format!("New Tab extra right-edge snap px: {:.1}", st.ws_new_tab_snap_extra_px);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &s2, num_cols - 2);
                line += 2;

                // Completions toggle row
                let comp_lbl = "Completions overlay (Warp): ";
                let comp_val = if st.ws_completions_enabled { "on" } else { "off" };
                let comp_row = format!("{}{} (press 'o' to toggle)", comp_lbl, comp_val);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &comp_row, num_cols - 2);
                line += 2;

                // Save hint
                let hint = "Enter: Save  •  Esc: Close  •  Ctrl+Left/Right: Switch category";
                self.draw_ai_text(
                    Point::new(line, Column(2)),
                    tokens.text_muted,
                    bg,
                    hint,
                    num_cols - 2,
                );
            }
            SettingsCategory::Keybindings => {
                // Filter input
                let filter_lbl = "Filter: ";
                let filter_row = format!("{}{}", filter_lbl, st.kb_filter);
                self.draw_ai_text(Point::new(line, Column(2)), fg, bg, &filter_row, num_cols - 2);
                line += 1;

                // List entries (filtered)
                let max_rows = (size_info.screen_lines() - (line + 2)).max(3);
                let filtered: Vec<&KbItem> = st
                    .kb_items
                    .iter()
                    .filter(|it| {
                        it.action.contains(&st.kb_filter) || it.binding.contains(&st.kb_filter)
                    })
                    .collect();
                for (shown, (i, it)) in filtered.iter().enumerate().enumerate() {
                    if shown >= max_rows {
                        break;
                    }
                    let sel = i == st.kb_selected;
                    let prefix = if sel { "> " } else { "  " };
                    let row = format!("{}{:30} — {}", prefix, it.action, it.binding);
                    let color = if sel { tokens.accent } else { fg };
                    self.draw_ai_text(Point::new(line, Column(2)), color, bg, &row, num_cols - 2);
                    line += 1;
                }
                line += 1;

                if st.kb_capture_mode {
                    let hint = "Capturing: press new key combo (Esc to cancel)";
                    self.draw_ai_text(
                        Point::new(line, Column(2)),
                        tokens.warning,
                        bg,
                        hint,
                        num_cols - 2,
                    );
                } else if let Some(msg) = &st.message {
                    self.draw_ai_text(
                        Point::new(line, Column(2)),
                        tokens.success,
                        bg,
                        msg,
                        num_cols - 2,
                    );
                } else {
                    let hint = "c: Capture new binding  •  ↑/↓: Move  •  Enter: Save not required (applies on capture)  •  Esc: Close";
                    self.draw_ai_text(
                        Point::new(line, Column(2)),
                        tokens.text_muted,
                        bg,
                        hint,
                        num_cols - 2,
                    );
                }
            }
        }
    }
}

// ---------- Keybindings helpers ----------
fn format_action(action: &BindingAction) -> String {
    format!("{:?}", action)
}

fn format_binding_for_display(kb: &KeyBinding) -> String {
    let mods = mods_string_for_display(kb.mods);
    let key = key_string_for_display(&kb.trigger);
    if mods.is_empty() {
        key
    } else {
        format!("{} + {}", mods, key)
    }
}

fn mods_string_for_display(mods: ModifiersState) -> String {
    let mut parts = Vec::new();
    if mods.contains(ModifiersState::CONTROL) {
        parts.push("Ctrl");
    }
    if mods.contains(ModifiersState::SHIFT) {
        parts.push("Shift");
    }
    if mods.contains(ModifiersState::ALT) {
        parts.push("Alt");
    }
    if mods.contains(ModifiersState::SUPER) {
        parts.push("Super");
    }
    parts.join("+")
}

fn key_string_for_display(trigger: &BindingKey) -> String {
    match trigger {
        BindingKey::Keycode { key, .. } => match key {
            winit::keyboard::Key::Character(s) => s.to_string(),
            winit::keyboard::Key::Named(named) => format!("{:?}", named),
            _ => "Key".into(),
        },
        BindingKey::Scancode(_) => "Scancode".into(),
    }
}

fn mods_string_for_config(mods: ModifiersState) -> String {
    let mut parts = Vec::new();
    if mods.contains(ModifiersState::CONTROL) {
        parts.push("Control");
    }
    if mods.contains(ModifiersState::SHIFT) {
        parts.push("Shift");
    }
    if mods.contains(ModifiersState::ALT) {
        parts.push("Alt");
    }
    if mods.contains(ModifiersState::SUPER) {
        parts.push("Super");
    }
    if parts.is_empty() {
        "None".into()
    } else {
        parts.join("|")
    }
}

fn key_string_for_config_from_binding(trigger: &BindingKey) -> String {
    match trigger {
        BindingKey::Keycode { key, .. } => match key {
            winit::keyboard::Key::Character(s) => s.to_string(),
            winit::keyboard::Key::Named(named) => match named {
                winit::keyboard::NamedKey::Enter => "Enter".into(),
                winit::keyboard::NamedKey::Backspace => "Back".into(),
                winit::keyboard::NamedKey::ArrowUp => "Up".into(),
                winit::keyboard::NamedKey::ArrowDown => "Down".into(),
                winit::keyboard::NamedKey::ArrowLeft => "Left".into(),
                winit::keyboard::NamedKey::ArrowRight => "Right".into(),
                winit::keyboard::NamedKey::Tab => "Tab".into(),
                // Common punctuation mapped in deserialize
                _ => format!("{:?}", named),
            },
            _ => "".into(),
        },
        BindingKey::Scancode(_) => "".into(),
    }
}

fn key_string_for_config_from_key(key: &winit::keyboard::Key<String>) -> String {
    match key {
        winit::keyboard::Key::Character(s) => s.clone(),
        winit::keyboard::Key::Named(named) => match named {
            winit::keyboard::NamedKey::Enter => "Enter".into(),
            winit::keyboard::NamedKey::Backspace => "Back".into(),
            winit::keyboard::NamedKey::ArrowUp => "Up".into(),
            winit::keyboard::NamedKey::ArrowDown => "Down".into(),
            winit::keyboard::NamedKey::ArrowLeft => "Left".into(),
            winit::keyboard::NamedKey::ArrowRight => "Right".into(),
            winit::keyboard::NamedKey::Tab => "Tab".into(),
            _ => format!("{:?}", named),
        },
        _ => "".into(),
    }
}

fn save_keybinding_override_to_config(action: &str, key: &str, mods: &str) -> std::io::Result<()> {
    let path = config_path();
    let mut root = if let Ok(text) = fs::read_to_string(&path) {
        toml::from_str::<toml::Value>(&text)
            .unwrap_or(toml::Value::Table(toml::value::Table::new()))
    } else {
        toml::Value::Table(toml::value::Table::new())
    };
    if !root.is_table() {
        root = toml::Value::Table(toml::value::Table::new());
    }
    let tbl = root.as_table_mut().unwrap();
    let keyboard_tbl =
        tbl.entry("keyboard").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let kb = keyboard_tbl.as_table_mut().unwrap();
    let arr = kb.entry("bindings").or_insert_with(|| toml::Value::Array(Vec::new()));
    let arr_mut = arr.as_array_mut().unwrap();
    let mut entry = toml::value::Table::new();
    entry.insert("key".into(), toml::Value::String(key.into()));
    if mods != "None" {
        entry.insert("mods".into(), toml::Value::String(mods.into()));
    }
    entry.insert("action".into(), toml::Value::String(action.into()));
    arr_mut.push(toml::Value::Table(entry));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let s = toml::to_string_pretty(&root).unwrap_or_default();
    fs::write(&path, s)
}

#[allow(clippy::too_many_arguments)]
fn workspace_drag_write_to_toml(
    root: &mut toml::Value,
    color_rgb: Option<(u8, u8, u8)>,
    hl_base: f32,
    hl_hover: f32,
    tab_base: f32,
    tab_hover: f32,
    new_base: f32,
    new_hover: f32,
    snap_v: f32,
    snap_new: f32,
) {
    let tbl = root.as_table_mut().unwrap();
    let ws_tbl =
        tbl.entry("workspace").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let ws = ws_tbl.as_table_mut().unwrap();
    let drag_tbl =
        ws.entry("drag").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let drag = drag_tbl.as_table_mut().unwrap();
    if let Some((r, g, b)) = color_rgb {
        drag.insert(
            "highlight_color".into(),
            toml::Value::Array(vec![
                toml::Value::Integer(r as i64),
                toml::Value::Integer(g as i64),
                toml::Value::Integer(b as i64),
            ]),
        );
    } else {
        drag.remove("highlight_color");
    }
    drag.insert("highlight_alpha_base".into(), toml::Value::Float(hl_base as f64));
    drag.insert("highlight_alpha_hover".into(), toml::Value::Float(hl_hover as f64));
    drag.insert("tab_highlight_alpha_base".into(), toml::Value::Float(tab_base as f64));
    drag.insert("tab_highlight_alpha_hover".into(), toml::Value::Float(tab_hover as f64));
    drag.insert("new_tab_highlight_alpha_base".into(), toml::Value::Float(new_base as f64));
    drag.insert("new_tab_highlight_alpha_hover".into(), toml::Value::Float(new_hover as f64));
    drag.insert("tab_drop_snap_px".into(), toml::Value::Float(snap_v as f64));
    drag.insert("new_tab_snap_extra_px".into(), toml::Value::Float(snap_new as f64));
}

#[allow(clippy::too_many_arguments)]
fn save_workspace_drag_to_config(
    color_rgb: Option<(u8, u8, u8)>,
    hl_base: f32,
    hl_hover: f32,
    tab_base: f32,
    tab_hover: f32,
    new_base: f32,
    new_hover: f32,
    snap_v: f32,
    snap_new: f32,
) -> std::io::Result<()> {
    let path = config_path();
    let mut root = if let Ok(text) = fs::read_to_string(&path) {
        toml::from_str::<toml::Value>(&text)
            .unwrap_or(toml::Value::Table(toml::value::Table::new()))
    } else {
        toml::Value::Table(toml::value::Table::new())
    };
    if !root.is_table() {
        root = toml::Value::Table(toml::value::Table::new());
    }
    workspace_drag_write_to_toml(
        &mut root, color_rgb, hl_base, hl_hover, tab_base, tab_hover, new_base, new_hover, snap_v,
        snap_new,
    );
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let s = toml::to_string_pretty(&root).unwrap_or_default();
    fs::write(&path, s)
}

fn save_workspace_completions_to_config(enabled: bool) -> std::io::Result<()> {
    let path = config_path();
    let mut root = if let Ok(text) = fs::read_to_string(&path) {
        toml::from_str::<toml::Value>(&text)
            .unwrap_or(toml::Value::Table(toml::value::Table::new()))
    } else {
        toml::Value::Table(toml::value::Table::new())
    };
    if !root.is_table() {
        root = toml::Value::Table(toml::value::Table::new());
    }
    let tbl = root.as_table_mut().unwrap();
    let ws_tbl =
        tbl.entry("workspace").or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
    let ws = ws_tbl.as_table_mut().unwrap();
    ws.insert("completions_enabled".into(), toml::Value::Boolean(enabled));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let s = toml::to_string_pretty(&root).unwrap_or_default();
    fs::write(&path, s)
}

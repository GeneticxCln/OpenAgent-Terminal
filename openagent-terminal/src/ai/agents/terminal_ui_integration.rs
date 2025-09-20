use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::advanced_conversation_features::AdvancedConversationFeatures;
use super::conversation_manager::ConversationManager;
use super::privacy_content_filter::PrivacyContentFilter;
use super::*;

/// Terminal UI integration for rich conversation visualization
pub struct TerminalUIIntegration {
    id: String,
    conversation_manager: Option<Arc<ConversationManager>>,
    advanced_features: Option<Arc<AdvancedConversationFeatures>>,
    privacy_filter: Option<Arc<PrivacyContentFilter>>,
    ui_components: Arc<RwLock<HashMap<String, UIComponent>>>,
    layout_manager: Arc<LayoutManager>,
    theme_manager: Arc<ThemeManager>,
    input_handler: Arc<RwLock<InputHandler>>,
    display_buffer: Arc<RwLock<DisplayBuffer>>,
    config: TerminalUIConfig,
    is_initialized: bool,
}

/// UI component for terminal display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIComponent {
    pub id: String,
    pub component_type: ComponentType,
    pub position: Position,
    pub dimensions: Dimensions,
    pub content: ComponentContent,
    pub style: ComponentStyle,
    pub is_visible: bool,
    pub is_focused: bool,
    pub z_index: i32,
    pub update_frequency: UpdateFrequency,
    pub last_updated: DateTime<Utc>,
}

/// Types of UI components
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ComponentType {
    ConversationView, // Main conversation display
    BranchTree,       // Conversation branch visualization
    GoalTracker,      // Goal progress tracking
    PrivacyIndicator, // Privacy/security status
    SessionManager,   // Multi-session management
    CommandPalette,   // Command input/selection
    StatusBar,        // System status information
    SummaryPanel,     // Conversation summaries
    CompliancePanel,  // Privacy compliance dashboard
    NotificationArea, // Alerts and notifications
    Custom(String),   // Custom component types
}

/// Position in terminal coordinates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

/// Component dimensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimensions {
    pub width: u16,
    pub height: u16,
    pub min_width: u16,
    pub min_height: u16,
    pub max_width: Option<u16>,
    pub max_height: Option<u16>,
}

/// Component content structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentContent {
    pub title: String,
    pub body: ContentBody,
    pub footer: Option<String>,
    pub scrollable: bool,
    pub scroll_position: u16,
}

/// Different types of component content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentBody {
    Text(String),
    List(Vec<ListItem>),
    Table(TableData),
    Tree(TreeNode),
    Chart(ChartData),
    Form(FormData),
    Custom(serde_json::Value),
}

/// List item for list components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItem {
    pub id: String,
    pub text: String,
    pub icon: Option<String>,
    pub color: Option<Color>,
    pub is_selected: bool,
    pub metadata: HashMap<String, String>,
}

/// Table data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableData {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub column_widths: Vec<u16>,
    pub sortable: bool,
    pub sort_column: Option<usize>,
    pub sort_direction: SortDirection,
}

/// Tree node for hierarchical display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: String,
    pub text: String,
    pub children: Vec<TreeNode>,
    pub is_expanded: bool,
    pub is_selected: bool,
    pub icon: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Chart data for visualizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub chart_type: ChartType,
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub data_series: Vec<DataSeries>,
    pub legend_enabled: bool,
}

/// Types of charts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChartType {
    LineChart,
    BarChart,
    PieChart,
    Histogram,
    Sparkline,
    Custom(String),
}

/// Data series for charts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSeries {
    pub name: String,
    pub data: Vec<DataPoint>,
    pub color: Color,
    pub style: LineStyle,
}

/// Individual data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub x: f64,
    pub y: f64,
    pub label: Option<String>,
}

/// Form data for input components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormData {
    pub fields: Vec<FormField>,
    pub submit_button: String,
    pub cancel_button: Option<String>,
}

/// Individual form field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub id: String,
    pub label: String,
    pub field_type: FieldType,
    pub value: String,
    pub placeholder: Option<String>,
    pub validation: Option<ValidationRule>,
    pub is_required: bool,
    pub is_readonly: bool,
}

/// Types of form fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldType {
    Text,
    Password,
    Number,
    Email,
    Select(Vec<String>),
    MultiSelect(Vec<String>),
    Textarea,
    Checkbox,
    Radio(Vec<String>),
    Custom(String),
}

/// Validation rule for form fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub rule_type: ValidationType,
    pub parameters: HashMap<String, String>,
    pub error_message: String,
}

/// Types of field validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationType {
    Required,
    MinLength(usize),
    MaxLength(usize),
    Pattern(String),
    Email,
    Number,
    Range(f64, f64),
    Custom(String),
}

/// Component styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStyle {
    pub border: BorderStyle,
    pub background_color: Option<Color>,
    pub foreground_color: Option<Color>,
    pub highlight_color: Option<Color>,
    pub padding: Padding,
    pub font_style: FontStyle,
    pub alignment: Alignment,
}

/// Border styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderStyle {
    pub border_type: BorderType,
    pub color: Option<Color>,
    pub thickness: u8,
    pub rounded_corners: bool,
}

/// Types of borders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BorderType {
    None,
    Single,
    Double,
    Thick,
    Dashed,
    Dotted,
    Custom(String),
}

/// Component padding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Padding {
    pub top: u8,
    pub right: u8,
    pub bottom: u8,
    pub left: u8,
}

/// Font styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

/// Content alignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Justify,
}

/// Colors for styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Rgb(u8, u8, u8),
    Indexed(u8),
}

/// Line styles for charts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LineStyle {
    Solid,
    Dashed,
    Dotted,
    DashDot,
}

/// Sort direction for tables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Update frequency for components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateFrequency {
    Never,
    OnChange,
    Periodic(Duration),
    RealTime,
}

/// Layout manager for component positioning
pub struct LayoutManager {
    layout_engine: LayoutEngine,
    responsive_rules: RwLock<Vec<ResponsiveRule>>,
    terminal_size: RwLock<(u16, u16)>, // (width, height)
}

/// Layout calculation engine
#[derive(Debug, Clone)]
pub enum LayoutEngine {
    Fixed,   // Fixed positioning
    Grid,    // Grid-based layout
    Flexbox, // Flexible box layout
    Stack,   // Stacked layout
    Custom(String),
}

/// Responsive design rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsiveRule {
    pub condition: ResponsiveCondition,
    pub layout_changes: Vec<LayoutChange>,
}

/// Conditions for responsive layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponsiveCondition {
    MinWidth(u16),
    MaxWidth(u16),
    MinHeight(u16),
    MaxHeight(u16),
    AspectRatio(f32),
    Custom(String),
}

/// Layout changes for responsive design
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutChange {
    pub component_id: String,
    pub property: LayoutProperty,
    pub value: LayoutValue,
}

/// Layout properties that can be changed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutProperty {
    Position,
    Size,
    Visibility,
    ZIndex,
    Custom(String),
}

/// Values for layout properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutValue {
    Position(Position),
    Dimensions(Dimensions),
    Boolean(bool),
    Integer(i32),
    Custom(serde_json::Value),
}

/// Theme management system
pub struct ThemeManager {
    current_theme: RwLock<String>,
    themes: HashMap<String, Theme>,
    custom_styles: RwLock<HashMap<String, ComponentStyle>>,
}

/// Theme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub description: String,
    pub colors: ColorPalette,
    pub component_styles: HashMap<ComponentType, ComponentStyle>,
    pub global_styles: GlobalStyles,
}

/// Color palette for themes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPalette {
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub background: Color,
    pub foreground: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
}

/// Global styling options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalStyles {
    pub default_font: FontStyle,
    pub default_border: BorderStyle,
    pub default_padding: Padding,
    pub animation_speed: Duration,
    pub cursor_blink_rate: Duration,
}

/// Input handling system
pub struct InputHandler {
    key_bindings: HashMap<KeyCombination, Action>,
    mouse_enabled: bool,
    input_modes: Vec<InputMode>,
    current_mode: String,
    input_history: VecDeque<InputEvent>,
}

/// Key combination for input handling
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyCombination {
    pub key: Key,
    pub modifiers: Vec<Modifier>,
}

/// Key codes
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    Char(char),
    Function(u8),
    Arrow(Direction),
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Custom(String),
}

/// Direction for arrow keys
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Modifier keys
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
}

/// Actions triggered by input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Navigate(NavigationAction),
    Edit(EditAction),
    System(SystemAction),
    Custom(String, HashMap<String, serde_json::Value>),
}

/// Navigation actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NavigationAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    ScrollUp,
    ScrollDown,
    NextTab,
    PreviousTab,
    FocusNext,
    FocusPrevious,
    Custom(String),
}

/// Edit actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditAction {
    InsertChar(char),
    InsertString(String),
    DeleteChar,
    DeleteLine,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Custom(String),
}

/// System actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemAction {
    Quit,
    Save,
    Load,
    Refresh,
    ToggleFullscreen,
    ToggleTheme,
    ShowHelp,
    ShowSettings,
    Custom(String),
}

/// Input modes for different contexts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMode {
    pub name: String,
    pub description: String,
    pub key_bindings: HashMap<KeyCombination, Action>,
    pub is_modal: bool,
    pub escape_key: Option<KeyCombination>,
}

/// Input event tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: InputEventType,
    pub component_id: Option<String>,
    pub handled: bool,
}

/// Types of input events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEventType {
    KeyPress(KeyCombination),
    MouseClick(Position),
    MouseMove(Position),
    MouseScroll(ScrollDirection),
    Resize(u16, u16),
    Custom(String),
}

/// Scroll direction for mouse events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Display buffer for efficient rendering
pub struct DisplayBuffer {
    buffer: Vec<Vec<Cell>>,
    dirty_regions: Vec<Rectangle>,
    cursor_position: Position,
    cursor_visible: bool,
}

/// Individual cell in display buffer
#[derive(Debug, Clone)]
pub struct Cell {
    pub character: char,
    pub foreground_color: Color,
    pub background_color: Color,
    pub style: FontStyle,
}

/// Rectangle for dirty region tracking
#[derive(Debug, Clone)]
pub struct Rectangle {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// Configuration for terminal UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalUIConfig {
    pub default_theme: String,
    pub enable_mouse: bool,
    pub enable_animations: bool,
    pub refresh_rate: u64,
    pub buffer_size: usize,
    pub auto_scroll: bool,
    pub show_debug_info: bool,
    pub component_borders: bool,
    pub status_bar_enabled: bool,
    pub notifications_enabled: bool,
}

impl Default for TerminalUIConfig {
    fn default() -> Self {
        Self {
            default_theme: "default".to_string(),
            enable_mouse: true,
            enable_animations: true,
            refresh_rate: 60,
            buffer_size: 1000,
            auto_scroll: true,
            show_debug_info: false,
            component_borders: true,
            status_bar_enabled: true,
            notifications_enabled: true,
        }
    }
}

impl TerminalUIIntegration {
    pub fn new() -> Self {
        Self {
            id: "terminal-ui-integration".to_string(),
            conversation_manager: None,
            advanced_features: None,
            privacy_filter: None,
            ui_components: Arc::new(RwLock::new(HashMap::new())),
            layout_manager: Arc::new(LayoutManager::new()),
            theme_manager: Arc::new(ThemeManager::new()),
            input_handler: Arc::new(RwLock::new(InputHandler::new())),
            display_buffer: Arc::new(RwLock::new(DisplayBuffer::new())),
            config: TerminalUIConfig::default(),
            is_initialized: false,
        }
    }

    pub fn with_config(mut self, config: TerminalUIConfig) -> Self {
        self.config = config;
        self
    }

    /// Apply configuration to runtime-managed state (theme, input, etc.)
    pub async fn apply_config(&self) -> Result<()> {
        // Input settings
        {
            let mut ih = self.input_handler.write().await;
            ih.apply_config(&self.config);
        }
        // Theme selection
        {
            let mut current_theme = self.theme_manager.current_theme.write().await;
            *current_theme = self.config.default_theme.clone();
        }
        Ok(())
    }

    pub fn with_conversation_manager(mut self, manager: Arc<ConversationManager>) -> Self {
        self.conversation_manager = Some(manager);
        self
    }

    pub fn with_advanced_features(mut self, features: Arc<AdvancedConversationFeatures>) -> Self {
        self.advanced_features = Some(features);
        self
    }

    pub fn with_privacy_filter(mut self, filter: Arc<PrivacyContentFilter>) -> Self {
        self.privacy_filter = Some(filter);
        self
    }

    /// Create a UI component
    pub async fn create_component(
        &self,
        component_type: ComponentType,
        position: Position,
        dimensions: Dimensions,
    ) -> Result<String> {
        let component_id = format!("{:?}-{}", component_type, Uuid::new_v4());

        let component = UIComponent {
            id: component_id.clone(),
            component_type: component_type.clone(),
            position,
            dimensions,
            content: self.create_default_content(&component_type),
            style: self.get_default_style(&component_type).await,
            is_visible: true,
            is_focused: false,
            z_index: 0,
            update_frequency: UpdateFrequency::OnChange,
            last_updated: Utc::now(),
        };

        let mut components = self.ui_components.write().await;
        components.insert(component_id.clone(), component);

        // Update layout
        self.layout_manager.update_layout(&mut components).await?;

        tracing::info!("Created UI component: {} ({:?})", component_id, component_type);
        Ok(component_id)
    }

    /// Update component content
    pub async fn update_component_content(
        &self,
        component_id: &str,
        content: ComponentContent,
    ) -> Result<()> {
        let mut components = self.ui_components.write().await;
        if let Some(component) = components.get_mut(component_id) {
            component.content = content;
            component.last_updated = Utc::now();

            // Mark region as dirty for rendering
            let mut buffer = self.display_buffer.write().await;
            buffer.mark_dirty_region(Rectangle {
                x: component.position.x,
                y: component.position.y,
                width: component.dimensions.width,
                height: component.dimensions.height,
            });

            tracing::debug!("Updated content for component: {}", component_id);
            Ok(())
        } else {
            Err(anyhow!("Component not found: {}", component_id))
        }
    }

    /// Render all visible components
    pub async fn render(&self) -> Result<()> {
        let components = self.ui_components.read().await;
        let mut buffer = self.display_buffer.write().await;

        // Clear dirty regions
        let dirty_regions: Vec<_> = buffer.dirty_regions.clone();
        for region in dirty_regions {
            buffer.clear_region(&region);
        }

        // Sort components by z-index for proper layering
        let mut sorted_components: Vec<&UIComponent> =
            components.values().filter(|c| c.is_visible).collect();
        sorted_components.sort_by_key(|c| c.z_index);

        // Render each component
        for component in sorted_components {
            self.render_component(component, &mut buffer).await?;
        }

        // Render to terminal
        buffer.flush_to_terminal()?;
        buffer.dirty_regions.clear();

        Ok(())
    }

    /// Handle input events
    pub async fn handle_input(&self, event: InputEventType) -> Result<bool> {
        {
            let mut input_handler = self.input_handler.write().await;
            // Record input event
            input_handler.input_history.push_back(InputEvent {
                timestamp: Utc::now(),
                event_type: event.clone(),
                component_id: self.get_focused_component_id().await,
                handled: false,
            });
            // Keep input history manageable
            if input_handler.input_history.len() > 100 {
                input_handler.input_history.pop_front();
            }

            // Gate mouse events if disabled
            match event {
                InputEventType::MouseClick(_)
                | InputEventType::MouseMove(_)
                | InputEventType::MouseScroll(_) => {
                    if !input_handler.mouse_enabled {
                        return Ok(false);
                    }
                }
                _ => {}
            }
        }

        // Handle resize separately: update layout and buffer
        if let InputEventType::Resize(w, h) = event {
            self.layout_manager.set_terminal_size(w, h).await;
            {
                let mut buffer = self.display_buffer.write().await;
                buffer.resize(w as usize, h as usize);
            }
            let mut components = self.ui_components.write().await;
            self.layout_manager.update_layout(&mut components).await?;
            return Ok(true);
        }

        // Process input based on current mode/key bindings
        let handled =
            if let Some(action) = self.input_handler.read().await.get_action_for_input(&event) {
                self.execute_action(&action).await?
            } else {
                false
            };

        Ok(handled)
    }

    /// Show conversation in the main view
    pub async fn show_conversation(&self, session_id: Uuid) -> Result<()> {
        if let Some(conversation_manager) = &self.conversation_manager {
            // Get conversation summary
            let summary = conversation_manager
                .get_conversation_summary(session_id, 50)
                .await
                .unwrap_or_default();

            // Find or create conversation view component
            let component_id = self.find_or_create_conversation_view().await?;

            // Update content
            let content = ComponentContent {
                title: format!("Conversation - {}", session_id),
                body: ContentBody::Text(summary),
                footer: Some("Press 'h' for help".to_string()),
                scrollable: true,
                scroll_position: 0,
            };

            self.update_component_content(&component_id, content).await?;
        }

        Ok(())
    }

    /// Display conversation branch tree
    pub async fn show_branch_tree(&self, session_id: Uuid) -> Result<()> {
        if let Some(_advanced_features) = &self.advanced_features {
            // Create tree visualization of conversation branches
            let tree_data = self.build_branch_tree_data(session_id).await?;

            let component_id = self.find_or_create_component(ComponentType::BranchTree).await?;

            let content = ComponentContent {
                title: "Conversation Branches".to_string(),
                body: ContentBody::Tree(tree_data),
                footer: Some("Use arrow keys to navigate".to_string()),
                scrollable: true,
                scroll_position: 0,
            };

            self.update_component_content(&component_id, content).await?;
        }

        Ok(())
    }

    /// Display goal tracking information
    pub async fn show_goal_tracker(&self, session_id: Uuid) -> Result<()> {
        if let Some(_advanced_features) = &self.advanced_features {
            // Get goal tracking data
            let goals_data = self.build_goals_data(session_id).await?;

            let component_id = self.find_or_create_component(ComponentType::GoalTracker).await?;

            let content = ComponentContent {
                title: "Goal Progress".to_string(),
                body: ContentBody::Chart(goals_data),
                footer: Some("Goals updated in real-time".to_string()),
                scrollable: false,
                scroll_position: 0,
            };

            self.update_component_content(&component_id, content).await?;
        }

        Ok(())
    }

    /// Display privacy/security status
    pub async fn show_privacy_status(&self) -> Result<()> {
        if let Some(privacy_filter) = &self.privacy_filter {
            let status = privacy_filter.status().await;

            let component_id =
                self.find_or_create_component(ComponentType::PrivacyIndicator).await?;

            let privacy_info = format!(
                "Privacy Status: {}\nLast Activity: {}\nTask: {}",
                if status.is_healthy { "✅ Protected" } else { "❌ Issue" },
                status.last_activity.format("%H:%M:%S"),
                status.current_task.unwrap_or_default()
            );

            let content = ComponentContent {
                title: "Privacy & Security".to_string(),
                body: ContentBody::Text(privacy_info),
                footer: None,
                scrollable: false,
                scroll_position: 0,
            };

            self.update_component_content(&component_id, content).await?;
        }

        Ok(())
    }

    /// Switch to a different theme
    pub async fn set_theme(&self, theme_name: &str) -> Result<()> {
        let mut current_theme = self.theme_manager.current_theme.write().await;
        if self.theme_manager.themes.contains_key(theme_name) {
            *current_theme = theme_name.to_string();

            // Refresh all components with new theme
            self.refresh_all_components().await?;

            tracing::info!("Switched to theme: {}", theme_name);
            Ok(())
        } else {
            Err(anyhow!("Theme not found: {}", theme_name))
        }
    }

    // Helper methods

    fn create_default_content(&self, component_type: &ComponentType) -> ComponentContent {
        match component_type {
            ComponentType::ConversationView => ComponentContent {
                title: "Conversation".to_string(),
                body: ContentBody::Text("No conversation loaded".to_string()),
                footer: Some("Start a new conversation or load an existing one".to_string()),
                scrollable: true,
                scroll_position: 0,
            },
            ComponentType::StatusBar => ComponentContent {
                title: String::new(),
                body: ContentBody::Text("Ready".to_string()),
                footer: None,
                scrollable: false,
                scroll_position: 0,
            },
            _ => ComponentContent {
                title: format!("{:?}", component_type),
                body: ContentBody::Text("Loading...".to_string()),
                footer: None,
                scrollable: false,
                scroll_position: 0,
            },
        }
    }

    async fn get_default_style(&self, component_type: &ComponentType) -> ComponentStyle {
        // Check custom styles override first
        let key = format!("{:?}", component_type);
        if let Some(style) = self.theme_manager.custom_styles.read().await.get(&key) {
            return style.clone();
        }

        // Fallback to theme-provided style
        let theme_name = self.theme_manager.current_theme.read().await;
        if let Some(theme) = self.theme_manager.themes.get(&*theme_name) {
            if let Some(style) = theme.component_styles.get(component_type) {
                return style.clone();
            }
        }

        // Default style
        ComponentStyle {
            border: BorderStyle {
                border_type: BorderType::Single,
                color: Some(Color::White),
                thickness: 1,
                rounded_corners: false,
            },
            background_color: Some(Color::Black),
            foreground_color: Some(Color::White),
            highlight_color: Some(Color::Blue),
            padding: Padding { top: 1, right: 1, bottom: 1, left: 1 },
            font_style: FontStyle {
                bold: false,
                italic: false,
                underline: false,
                strikethrough: false,
            },
            alignment: Alignment::Left,
        }
    }

    async fn render_component(
        &self,
        component: &UIComponent,
        buffer: &mut DisplayBuffer,
    ) -> Result<()> {
        // This would implement the actual rendering logic
        // For now, just mark the region as dirty
        buffer.mark_dirty_region(Rectangle {
            x: component.position.x,
            y: component.position.y,
            width: component.dimensions.width,
            height: component.dimensions.height,
        });

        tracing::debug!(
            "Rendered component: {} at ({}, {})",
            component.id,
            component.position.x,
            component.position.y
        );
        Ok(())
    }

    async fn get_focused_component_id(&self) -> Option<String> {
        let components = self.ui_components.read().await;
        components.values().find(|c| c.is_focused).map(|c| c.id.clone())
    }

    async fn execute_action(&self, action: &Action) -> Result<bool> {
        match action {
            Action::System(SystemAction::Quit) => {
                // Handle quit action
                tracing::info!("Quit action triggered");
                return Ok(true);
            }
            Action::System(SystemAction::Refresh) => {
                self.render().await?;
                return Ok(true);
            }
            Action::Navigate(nav_action) => {
                self.handle_navigation_action(nav_action).await?;
                return Ok(true);
            }
            _ => {
                tracing::debug!("Unhandled action: {:?}", action);
                return Ok(false);
            }
        }
    }

    async fn handle_navigation_action(&self, action: &NavigationAction) -> Result<()> {
        match action {
            NavigationAction::FocusNext => {
                self.focus_next_component().await?;
            }
            NavigationAction::FocusPrevious => {
                self.focus_previous_component().await?;
            }
            NavigationAction::ScrollUp => {
                self.scroll_focused_component(-1).await?;
            }
            NavigationAction::ScrollDown => {
                self.scroll_focused_component(1).await?;
            }
            _ => {
                tracing::debug!("Unhandled navigation action: {:?}", action);
            }
        }
        Ok(())
    }

    async fn focus_next_component(&self) -> Result<()> {
        let mut components = self.ui_components.write().await;

        // Collect visible component IDs to avoid borrow checker issues
        let visible_component_ids: Vec<String> =
            components.values().filter(|c| c.is_visible).map(|c| c.id.clone()).collect();

        if let Some(focused_id) = components.values().find(|c| c.is_focused).map(|c| c.id.clone()) {
            // Clear current focus
            if let Some(focused) = components.get_mut(&focused_id) {
                focused.is_focused = false;
            }

            // Find next component
            if let Some(pos) = visible_component_ids.iter().position(|id| id == &focused_id) {
                let next_pos = (pos + 1) % visible_component_ids.len();
                let next_id = &visible_component_ids[next_pos];
                if let Some(next_component) = components.get_mut(next_id) {
                    next_component.is_focused = true;
                }
            }
        } else if !visible_component_ids.is_empty() {
            // No component focused, focus first visible
            let first_id = &visible_component_ids[0];
            if let Some(first_component) = components.get_mut(first_id) {
                first_component.is_focused = true;
            }
        }

        Ok(())
    }

    async fn focus_previous_component(&self) -> Result<()> {
        let mut components = self.ui_components.write().await;

        // Collect visible component IDs to avoid borrow checker issues
        let visible_component_ids: Vec<String> =
            components.values().filter(|c| c.is_visible).map(|c| c.id.clone()).collect();

        if let Some(focused_id) = components.values().find(|c| c.is_focused).map(|c| c.id.clone()) {
            // Clear current focus
            if let Some(focused) = components.get_mut(&focused_id) {
                focused.is_focused = false;
            }

            // Find previous component
            if let Some(pos) = visible_component_ids.iter().position(|id| id == &focused_id) {
                let prev_pos = if pos == 0 { visible_component_ids.len() - 1 } else { pos - 1 };
                let prev_id = &visible_component_ids[prev_pos];
                if let Some(prev_component) = components.get_mut(prev_id) {
                    prev_component.is_focused = true;
                }
            }
        }

        Ok(())
    }

    async fn scroll_focused_component(&self, delta: i32) -> Result<()> {
        let mut components = self.ui_components.write().await;
        if let Some(focused) =
            components.values_mut().find(|c| c.is_focused && c.content.scrollable)
        {
            let new_position = (focused.content.scroll_position as i32 + delta).max(0) as u16;
            focused.content.scroll_position = new_position;
            focused.last_updated = Utc::now();
        }
        Ok(())
    }

    async fn find_or_create_conversation_view(&self) -> Result<String> {
        self.find_or_create_component(ComponentType::ConversationView).await
    }

    async fn find_or_create_component(&self, component_type: ComponentType) -> Result<String> {
        let components = self.ui_components.read().await;

        // Look for existing component of this type
        if let Some(component) = components.values().find(|c| {
            std::mem::discriminant(&c.component_type) == std::mem::discriminant(&component_type)
        }) {
            return Ok(component.id.clone());
        }

        drop(components);

        // Create new component
        let position = Position { x: 0, y: 0 };
        let dimensions = Dimensions {
            width: 80,
            height: 24,
            min_width: 20,
            min_height: 5,
            max_width: None,
            max_height: None,
        };

        self.create_component(component_type, position, dimensions).await
    }

    async fn build_branch_tree_data(&self, _session_id: Uuid) -> Result<TreeNode> {
        // This would build actual branch tree data from advanced features
        Ok(TreeNode {
            id: "root".to_string(),
            text: "Main Conversation".to_string(),
            children: vec![
                TreeNode {
                    id: "branch1".to_string(),
                    text: "JWT Implementation".to_string(),
                    children: Vec::new(),
                    is_expanded: false,
                    is_selected: false,
                    icon: Some("🌿".to_string()),
                    metadata: HashMap::new(),
                },
                TreeNode {
                    id: "branch2".to_string(),
                    text: "OAuth Integration".to_string(),
                    children: Vec::new(),
                    is_expanded: false,
                    is_selected: false,
                    icon: Some("🌿".to_string()),
                    metadata: HashMap::new(),
                },
            ],
            is_expanded: true,
            is_selected: true,
            icon: Some("🌳".to_string()),
            metadata: HashMap::new(),
        })
    }

    async fn build_goals_data(&self, _session_id: Uuid) -> Result<ChartData> {
        // This would build actual goals data from advanced features
        Ok(ChartData {
            chart_type: ChartType::BarChart,
            title: "Goal Progress".to_string(),
            x_label: "Goals".to_string(),
            y_label: "Progress %".to_string(),
            data_series: vec![
                DataSeries {
                    name: "Authentication Goal".to_string(),
                    data: vec![DataPoint { x: 1.0, y: 75.0, label: Some("Auth".to_string()) }],
                    color: Color::Green,
                    style: LineStyle::Solid,
                },
                DataSeries {
                    name: "Testing Goal".to_string(),
                    data: vec![DataPoint { x: 2.0, y: 25.0, label: Some("Tests".to_string()) }],
                    color: Color::Blue,
                    style: LineStyle::Solid,
                },
            ],
            legend_enabled: true,
        })
    }

    async fn refresh_all_components(&self) -> Result<()> {
        let components = self.ui_components.read().await;
        let mut buffer = self.display_buffer.write().await;

        // Mark all components as dirty
        for component in components.values() {
            buffer.mark_dirty_region(Rectangle {
                x: component.position.x,
                y: component.position.y,
                width: component.dimensions.width,
                height: component.dimensions.height,
            });
        }

        Ok(())
    }
}

// Implementation for helper structs

impl LayoutManager {
    pub fn new() -> Self {
        Self {
            layout_engine: LayoutEngine::Grid,
            responsive_rules: RwLock::new(Vec::new()),
            terminal_size: RwLock::new((80, 24)),
        }
    }

    pub async fn update_layout(&self, components: &mut HashMap<String, UIComponent>) -> Result<()> {
        // Apply responsive rules based on current terminal size
        let (width, height) = *self.terminal_size.read().await;
        let rules = self.responsive_rules.read().await.clone();

        for rule in rules.iter() {
            let condition_met = match &rule.condition {
                ResponsiveCondition::MinWidth(w) => width >= *w,
                ResponsiveCondition::MaxWidth(w) => width <= *w,
                ResponsiveCondition::MinHeight(h) => height >= *h,
                ResponsiveCondition::MaxHeight(h) => height <= *h,
                ResponsiveCondition::AspectRatio(ratio) => {
                    let current = width as f32 / height.max(1) as f32;
                    (current - ratio).abs() <= 0.05
                }
                ResponsiveCondition::Custom(_) => false,
            };

            if condition_met {
                for change in &rule.layout_changes {
                    if let Some(component) = components.get_mut(&change.component_id) {
                        match (&change.property, &change.value) {
                            (LayoutProperty::Position, LayoutValue::Position(p)) => {
                                component.position = p.clone();
                            }
                            (LayoutProperty::Size, LayoutValue::Dimensions(d)) => {
                                component.dimensions = d.clone();
                            }
                            (LayoutProperty::Visibility, LayoutValue::Boolean(b)) => {
                                component.is_visible = *b;
                            }
                            (LayoutProperty::ZIndex, LayoutValue::Integer(z)) => {
                                component.z_index = *z;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        tracing::debug!("Layout updated: {}x{}", width, height);
        Ok(())
    }

    pub async fn set_terminal_size(&self, width: u16, height: u16) {
        let mut sz = self.terminal_size.write().await;
        *sz = (width, height);
    }

    pub async fn set_responsive_rules(&self, rules: Vec<ResponsiveRule>) {
        let mut guard = self.responsive_rules.write().await;
        *guard = rules;
    }
}

impl ThemeManager {
    pub fn new() -> Self {
        let mut themes = HashMap::new();

        // Default theme
        themes.insert(
            "default".to_string(),
            Theme {
                name: "Default".to_string(),
                description: "Default terminal theme".to_string(),
                colors: ColorPalette {
                    primary: Color::Blue,
                    secondary: Color::Cyan,
                    accent: Color::Yellow,
                    background: Color::Black,
                    foreground: Color::White,
                    success: Color::Green,
                    warning: Color::Yellow,
                    error: Color::Red,
                    info: Color::Cyan,
                },
                component_styles: HashMap::new(),
                global_styles: GlobalStyles {
                    default_font: FontStyle {
                        bold: false,
                        italic: false,
                        underline: false,
                        strikethrough: false,
                    },
                    default_border: BorderStyle {
                        border_type: BorderType::Single,
                        color: Some(Color::White),
                        thickness: 1,
                        rounded_corners: false,
                    },
                    default_padding: Padding { top: 1, right: 1, bottom: 1, left: 1 },
                    animation_speed: Duration::milliseconds(200),
                    cursor_blink_rate: Duration::milliseconds(500),
                },
            },
        );

        Self {
            current_theme: RwLock::new("default".to_string()),
            themes,
            custom_styles: RwLock::new(HashMap::new()),
        }
    }
}

impl InputHandler {
    pub fn new() -> Self {
        let mut key_bindings = HashMap::new();

        // Default key bindings
        key_bindings.insert(
            KeyCombination { key: Key::Char('q'), modifiers: vec![Modifier::Ctrl] },
            Action::System(SystemAction::Quit),
        );
        key_bindings.insert(
            KeyCombination { key: Key::Char('r'), modifiers: vec![Modifier::Ctrl] },
            Action::System(SystemAction::Refresh),
        );
        key_bindings.insert(
            KeyCombination { key: Key::Tab, modifiers: Vec::new() },
            Action::Navigate(NavigationAction::FocusNext),
        );

        Self {
            key_bindings,
            mouse_enabled: true,
            input_modes: Vec::new(),
            current_mode: "normal".to_string(),
            input_history: VecDeque::new(),
        }
    }

    pub fn apply_config(&mut self, cfg: &TerminalUIConfig) {
        self.mouse_enabled = cfg.enable_mouse;
        // Ensure a default input mode exists
        if self.input_modes.is_empty() {
            self.input_modes.push(InputMode {
                name: "normal".to_string(),
                description: "Default mode".to_string(),
                key_bindings: self.key_bindings.clone(),
                is_modal: false,
                escape_key: None,
            });
        }
        self.current_mode = "normal".to_string();
    }

    pub fn get_action_for_input(&self, event: &InputEventType) -> Option<Action> {
        match event {
            InputEventType::KeyPress(key_combo) => self.key_bindings.get(key_combo).cloned(),
            _ => None,
        }
    }
}

impl DisplayBuffer {
    pub fn new() -> Self {
        Self {
            buffer: vec![vec![Cell::default(); 80]; 24],
            dirty_regions: Vec::new(),
            cursor_position: Position { x: 0, y: 0 },
            cursor_visible: true,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        if height == 0 || width == 0 {
            return;
        }
        // Resize rows
        self.buffer.resize(height, vec![Cell::default(); width]);
        // Resize each row length
        for row in &mut self.buffer {
            row.resize(width, Cell::default());
        }
        // Clamp cursor
        self.cursor_position.x = self.cursor_position.x.min(width.saturating_sub(1) as u16);
        self.cursor_position.y = self.cursor_position.y.min(height.saturating_sub(1) as u16);
        // Mark full screen dirty
        self.mark_dirty_region(Rectangle {
            x: 0,
            y: 0,
            width: width as u16,
            height: height as u16,
        });
    }

    pub fn mark_dirty_region(&mut self, region: Rectangle) {
        self.dirty_regions.push(region);
    }

    pub fn clear_region(&mut self, _region: &Rectangle) {
        // This would clear the specified region
    }

    pub fn flush_to_terminal(&self) -> Result<()> {
        // This would render the buffer to the actual terminal
        print!("\x1B[2J\x1B[H"); // Clear screen and move cursor to top-left
        io::stdout().flush()?;
        Ok(())
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            character: ' ',
            foreground_color: Color::White,
            background_color: Color::Black,
            style: FontStyle { bold: false, italic: false, underline: false, strikethrough: false },
        }
    }
}

#[async_trait]
impl Agent for TerminalUIIntegration {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Terminal UI Integration"
    }

    fn description(&self) -> &str {
        "Rich terminal user interface for conversation management, privacy controls, and advanced features visualization"
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::ContextManagement,
            AgentCapability::Custom("UIRendering".to_string()),
            AgentCapability::Custom("InputHandling".to_string()),
            AgentCapability::Custom("ThemeManagement".to_string()),
            AgentCapability::Custom("LayoutManagement".to_string()),
            AgentCapability::Custom("ComponentSystem".to_string()),
        ]
    }

    async fn handle_request(&self, request: AgentRequest) -> Result<AgentResponse> {
        let mut response = AgentResponse {
            request_id: request.id,
            agent_id: self.id.clone(),
            success: false,
            payload: serde_json::json!({}),
            artifacts: Vec::new(),
            next_actions: Vec::new(),
            metadata: HashMap::new(),
        };

        match request.request_type {
            AgentRequestType::Custom(ref custom_type) => match custom_type.as_str() {
                "ShowConversation" => {
                    if let Some(session_id) = request
                        .payload
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s).ok())
                    {
                        match self.show_conversation(session_id).await {
                            Ok(()) => {
                                response.success = true;
                                response.payload = serde_json::json!({
                                    "message": "Conversation displayed"
                                });
                            }
                            Err(e) => {
                                response.payload = serde_json::json!({
                                    "error": e.to_string()
                                });
                            }
                        }
                    }
                }
                "SetTheme" => {
                    if let Some(theme_name) = request.payload.get("theme").and_then(|v| v.as_str())
                    {
                        match self.set_theme(theme_name).await {
                            Ok(()) => {
                                response.success = true;
                                response.payload = serde_json::json!({
                                    "message": format!("Theme switched to {}", theme_name)
                                });
                            }
                            Err(e) => {
                                response.payload = serde_json::json!({
                                    "error": e.to_string()
                                });
                            }
                        }
                    }
                }
                "Render" => match self.render().await {
                    Ok(()) => {
                        response.success = true;
                        response.payload = serde_json::json!({
                            "message": "UI rendered successfully"
                        });
                    }
                    Err(e) => {
                        response.payload = serde_json::json!({
                            "error": e.to_string()
                        });
                    }
                },
                _ => {
                    return Err(anyhow!("Unknown terminal UI request: {}", custom_type));
                }
            },
            _ => {
                return Err(anyhow!(
                    "Terminal UI Integration cannot handle request type: {:?}",
                    request.request_type
                ));
            }
        }

        Ok(response)
    }

    fn can_handle(&self, request_type: &AgentRequestType) -> bool {
        matches!(request_type,
            AgentRequestType::Custom(custom_type)
            if custom_type == "ShowConversation"
            || custom_type == "SetTheme"
            || custom_type == "Render"
            || custom_type == "HandleInput"
            || custom_type == "CreateComponent"
        )
    }

    async fn status(&self) -> AgentStatus {
        let components = self.ui_components.read().await;
        let buffer = self.display_buffer.read().await;

        AgentStatus {
            is_healthy: self.is_initialized,
            is_busy: !buffer.dirty_regions.is_empty(),
            last_activity: Utc::now(),
            current_task: Some(format!(
                "Managing {} UI components, {} dirty regions",
                components.len(),
                buffer.dirty_regions.len()
            )),
            error_message: None,
        }
    }

    async fn initialize(&mut self, _config: AgentConfig) -> Result<()> {
        // Apply configuration to runtime state
        self.apply_config().await?;
        // Initialize default components
        self.create_default_layout().await?;

        self.is_initialized = true;
        tracing::info!("Terminal UI Integration initialized");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Clear display and restore terminal
        print!("\x1B[2J\x1B[H"); // Clear screen
        io::stdout().flush()?;

        self.is_initialized = false;
        tracing::info!("Terminal UI Integration shut down");
        Ok(())
    }
}

impl TerminalUIIntegration {
    async fn create_default_layout(&self) -> Result<()> {
        // Create main conversation view
        self.create_component(
            ComponentType::ConversationView,
            Position { x: 0, y: 0 },
            Dimensions {
                width: 60,
                height: 20,
                min_width: 40,
                min_height: 10,
                max_width: None,
                max_height: None,
            },
        )
        .await?;

        // Create status bar
        self.create_component(
            ComponentType::StatusBar,
            Position { x: 0, y: 23 },
            Dimensions {
                width: 80,
                height: 1,
                min_width: 80,
                min_height: 1,
                max_width: None,
                max_height: Some(1),
            },
        )
        .await?;

        // Create privacy indicator
        self.create_component(
            ComponentType::PrivacyIndicator,
            Position { x: 60, y: 0 },
            Dimensions {
                width: 20,
                height: 10,
                min_width: 15,
                min_height: 5,
                max_width: None,
                max_height: None,
            },
        )
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_terminal_ui_creation() {
        let ui = TerminalUIIntegration::new();
        assert_eq!(ui.id(), "terminal-ui-integration");
        assert_eq!(ui.name(), "Terminal UI Integration");
    }

    #[tokio::test]
    async fn test_component_creation() {
        let ui = TerminalUIIntegration::new();

        let component_id = ui
            .create_component(
                ComponentType::ConversationView,
                Position { x: 0, y: 0 },
                Dimensions {
                    width: 80,
                    height: 24,
                    min_width: 20,
                    min_height: 5,
                    max_width: None,
                    max_height: None,
                },
            )
            .await
            .unwrap();

        assert!(component_id.starts_with("ConversationView-"));

        let components = ui.ui_components.read().await;
        assert!(components.contains_key(&component_id));
    }

    #[test]
    fn test_key_combination_hash() {
        let key1 = KeyCombination { key: Key::Char('q'), modifiers: vec![Modifier::Ctrl] };
        let key2 = KeyCombination { key: Key::Char('q'), modifiers: vec![Modifier::Ctrl] };

        assert_eq!(key1, key2);
    }
}

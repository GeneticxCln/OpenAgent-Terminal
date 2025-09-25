//! Warp-style Command Explanation Overlay
//!
//! Provides Warp-inspired command explanations and interactive help:
//! - Hover tooltips with command explanations
//! - Risk assessment indicators
//! - Interactive command breakdown
//! - Suggestion panels

use std::collections::HashMap;
use std::time::Instant;

use openagent_terminal_core::index::{Column, Line, Point};
use openagent_terminal_core::term::{SizeInfo, Term};

use crate::ai::warp_integration::{
    ExplanationResult, RiskLevel, CommandPart, CommandPartType, CommandExample
};
use crate::config::UiConfig;
use crate::display::color::{Rgb, List};
use crate::renderer::rects::RenderRect;

/// Warp-style command explanation overlay
#[derive(Debug)]
pub struct WarpCommandOverlay {
    /// Current command being explained
    current_command: Option<String>,
    
    /// Explanation result
    explanation: Option<ExplanationResult>,
    
    /// Hover state
    hover_state: HoverState,
    
    /// Animation state
    animation_state: AnimationState,
    
    /// Risk indicator state
    risk_indicator: RiskIndicator,
    
    /// Interactive elements
    interactive_elements: Vec<InteractiveElement>,
    
    /// Suggestion panel
    suggestion_panel: SuggestionPanel,
    
    /// Performance metrics
    metrics: OverlayMetrics,
}

/// Hover state management
#[derive(Debug, Default)]
pub struct HoverState {
    /// Mouse position
    pub mouse_position: Option<Point>,
    
    /// Hovered command part
    pub hovered_part: Option<usize>,
    
    /// Hover start time
    pub hover_start: Option<Instant>,
    
    /// Hover delay threshold
    pub hover_delay: std::time::Duration,
    
    /// Tooltip visibility
    pub show_tooltip: bool,
    
    /// Tooltip position
    pub tooltip_position: Option<Point>,
    
    /// Tooltip size
    pub tooltip_size: Option<(u16, u16)>,
}

/// Animation state for smooth transitions
#[derive(Debug, Default)]
pub struct AnimationState {
    /// Fade in/out alpha
    pub alpha: f32,
    
    /// Target alpha
    pub target_alpha: f32,
    
    /// Animation start time
    pub animation_start: Option<Instant>,
    
    /// Animation duration
    pub animation_duration: std::time::Duration,
    
    /// Slide animation offset
    pub slide_offset: f32,
    
    /// Scale animation
    pub scale: f32,
}

/// Risk level indicator
#[derive(Debug)]
pub struct RiskIndicator {
    /// Current risk level
    pub risk_level: RiskLevel,
    
    /// Risk color
    pub risk_color: Rgb,
    
    /// Risk icon
    pub risk_icon: &'static str,
    
    /// Risk description
    pub risk_description: Option<String>,
    
    /// Pulsing animation for high risk
    pub pulse_phase: f32,
    
    /// Warning visibility
    pub show_warning: bool,
}

/// Interactive UI elements
#[derive(Debug, Clone)]
pub struct InteractiveElement {
    /// Element ID
    pub id: String,
    
    /// Element type
    pub element_type: ElementType,
    
    /// Position and size
    pub bounds: RenderRect,
    
    /// Hover state
    pub is_hovered: bool,
    
    /// Click handler
    pub action: ElementAction,
    
    /// Tooltip text
    pub tooltip: Option<String>,
}

/// Types of interactive elements
#[derive(Debug, Clone)]
pub enum ElementType {
    CommandPart {
        part_index: usize,
        part_type: CommandPartType,
    },
    RiskIndicator,
    SuggestionButton,
    ExampleCommand,
    SaferAlternative,
    RelatedCommand,
}

/// Actions for interactive elements
#[derive(Debug, Clone)]
pub enum ElementAction {
    ShowCommandPartDetail(usize),
    ShowRiskExplanation,
    ApplySuggestion(String),
    ShowExample(CommandExample),
    ShowAlternative(String),
    ShowRelatedCommand(String),
}

/// Suggestion panel for workflows and alternatives
#[derive(Debug, Default)]
pub struct SuggestionPanel {
    /// Panel visibility
    pub visible: bool,
    
    /// Panel position
    pub position: Point,
    
    /// Panel size
    pub size: (u16, u16),
    
    /// Current suggestions
    pub suggestions: Vec<Suggestion>,
    
    /// Selected suggestion
    pub selected: usize,
    
    /// Panel scroll position
    pub scroll_offset: usize,
}

/// Individual suggestion
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// Suggestion text
    pub text: String,
    
    /// Suggestion type
    pub suggestion_type: SuggestionType,
    
    /// Confidence score
    pub confidence: f32,
    
    /// Icon
    pub icon: &'static str,
    
    /// Description
    pub description: Option<String>,
    
    /// Action when selected
    pub action: String,
}

/// Types of suggestions
#[derive(Debug, Clone)]
pub enum SuggestionType {
    Completion,
    Alternative,
    Workflow,
    Example,
    Related,
    SaferOption,
}

/// Performance metrics for overlay
#[derive(Debug, Default)]
pub struct OverlayMetrics {
    /// Explanation request time
    pub explanation_time: Option<std::time::Duration>,
    
    /// Render time
    pub render_time: Option<std::time::Duration>,
    
    /// Cache hit count
    pub cache_hits: u32,
    
    /// User interactions
    pub interactions: u32,
    
    /// Hover events
    pub hover_events: u32,
}

impl WarpCommandOverlay {
    /// Create new command overlay
    pub fn new() -> Self {
        Self {
            current_command: None,
            explanation: None,
            hover_state: HoverState {
                hover_delay: std::time::Duration::from_millis(500),
                ..Default::default()
            },
            animation_state: AnimationState {
                animation_duration: std::time::Duration::from_millis(300),
                alpha: 0.0,
                target_alpha: 0.0,
                scale: 0.8,
                ..Default::default()
            },
            risk_indicator: RiskIndicator {
                risk_level: RiskLevel::Safe,
                risk_color: Rgb { r: 0, g: 255, b: 0 },
                risk_icon: "✓",
                risk_description: None,
                pulse_phase: 0.0,
                show_warning: false,
            },
            interactive_elements: Vec::new(),
            suggestion_panel: SuggestionPanel::default(),
            metrics: OverlayMetrics::default(),
        }
    }
    
    /// Update overlay with new explanation
    pub fn update_explanation(&mut self, command: String, explanation: ExplanationResult) {
        self.current_command = Some(command);
        self.explanation = Some(explanation.clone());
        
        // Update risk indicator
        self.update_risk_indicator(&explanation);
        
        // Generate interactive elements
        self.generate_interactive_elements(&explanation);
        
        // Update suggestions
        self.update_suggestions(&explanation);
        
        // Start fade in animation
        self.start_fade_in();
    }
    
    /// Handle mouse movement for hover effects
    pub fn handle_mouse_move(&mut self, position: Point) {
        let old_position = self.hover_state.mouse_position;
        self.hover_state.mouse_position = Some(position);
        
        // Check if we're hovering over a new element
        let new_hovered = self.find_hovered_element(position);
        
        if new_hovered != self.hover_state.hovered_part {
            self.hover_state.hovered_part = new_hovered;
            self.hover_state.hover_start = Some(Instant::now());
            self.hover_state.show_tooltip = false;
            self.metrics.hover_events += 1;
            
            // Update interactive element hover states
            self.update_element_hover_states(position);
        }
    }
    
    /// Handle mouse click
    pub fn handle_mouse_click(&mut self, position: Point) -> Option<ElementAction> {
        for element in &mut self.interactive_elements {
            if element.bounds.contains_point(position) {
                element.is_hovered = true;
                self.metrics.interactions += 1;
                return Some(element.action.clone());
            }
        }
        None
    }
    
    /// Handle keyboard input for suggestion panel
    pub fn handle_key(&mut self, key: &str) -> bool {
        if !self.suggestion_panel.visible {
            return false;
        }
        
        match key {
            "ArrowUp" => {
                if self.suggestion_panel.selected > 0 {
                    self.suggestion_panel.selected -= 1;
                }
                true
            }
            "ArrowDown" => {
                if self.suggestion_panel.selected < self.suggestion_panel.suggestions.len().saturating_sub(1) {
                    self.suggestion_panel.selected += 1;
                }
                true
            }
            "Enter" => {
                if let Some(suggestion) = self.suggestion_panel.suggestions.get(self.suggestion_panel.selected) {
                    // Apply the selected suggestion
                    self.apply_suggestion(suggestion.clone());
                }
                true
            }
            "Escape" => {
                self.hide_suggestions();
                true
            }
            _ => false
        }
    }
    
    /// Update animation state
    pub fn update_animation(&mut self, dt: std::time::Duration) {
        // Update fade animation
        if let Some(start) = self.animation_state.animation_start {
            let elapsed = start.elapsed();
            if elapsed < self.animation_state.animation_duration {
                let progress = elapsed.as_secs_f32() / self.animation_state.animation_duration.as_secs_f32();
                let eased_progress = self.ease_in_out_cubic(progress);
                
                self.animation_state.alpha = self.lerp(
                    if self.animation_state.target_alpha > self.animation_state.alpha { 0.0 } else { 1.0 },
                    self.animation_state.target_alpha,
                    eased_progress
                );
                
                self.animation_state.scale = self.lerp(0.8, 1.0, eased_progress);
                self.animation_state.slide_offset = (1.0 - eased_progress) * 20.0;
            } else {
                self.animation_state.alpha = self.animation_state.target_alpha;
                self.animation_state.scale = 1.0;
                self.animation_state.slide_offset = 0.0;
                self.animation_state.animation_start = None;
            }
        }
        
        // Update tooltip visibility based on hover time
        if let Some(hover_start) = self.hover_state.hover_start {
            if hover_start.elapsed() > self.hover_state.hover_delay && !self.hover_state.show_tooltip {
                self.hover_state.show_tooltip = true;
                self.calculate_tooltip_position();
            }
        }
        
        // Update risk indicator pulsing for high-risk commands
        if matches!(self.risk_indicator.risk_level, RiskLevel::High | RiskLevel::Critical) {
            self.risk_indicator.pulse_phase += dt.as_secs_f32() * 4.0;
            if self.risk_indicator.pulse_phase > 2.0 * std::f32::consts::PI {
                self.risk_indicator.pulse_phase -= 2.0 * std::f32::consts::PI;
            }
        }
    }
    
    /// Check if overlay should be visible
    pub fn should_render(&self) -> bool {
        self.animation_state.alpha > 0.01 && self.explanation.is_some()
    }
    
    /// Get render data for the overlay
    pub fn get_render_data(&self, size_info: &SizeInfo) -> OverlayRenderData {
        let mut render_data = OverlayRenderData::default();
        
        if let Some(ref explanation) = self.explanation {
            // Main explanation panel
            render_data.main_panel = Some(self.build_main_panel(explanation, size_info));
            
            // Risk indicator
            if self.risk_indicator.show_warning || !matches!(self.risk_indicator.risk_level, RiskLevel::Safe) {
                render_data.risk_indicator = Some(self.build_risk_indicator());
            }
            
            // Tooltip
            if self.hover_state.show_tooltip {
                render_data.tooltip = self.build_tooltip(explanation);
            }
            
            // Suggestion panel
            if self.suggestion_panel.visible {
                render_data.suggestion_panel = Some(self.build_suggestion_panel());
            }
            
            // Interactive highlights
            render_data.interactive_highlights = self.build_interactive_highlights();
        }
        
        render_data.alpha = self.animation_state.alpha;
        render_data.scale = self.animation_state.scale;
        render_data.slide_offset = self.animation_state.slide_offset;
        
        render_data
    }
    
    // Private helper methods
    
    fn update_risk_indicator(&mut self, explanation: &ExplanationResult) {
        self.risk_indicator.risk_level = explanation.risk_level;
        self.risk_indicator.risk_description = explanation.risk_explanation.clone();
        
        // Set color and icon based on risk level
        match explanation.risk_level {
            RiskLevel::Safe => {
                self.risk_indicator.risk_color = Rgb { r: 0, g: 255, b: 0 };
                self.risk_indicator.risk_icon = "✓";
                self.risk_indicator.show_warning = false;
            }
            RiskLevel::Low => {
                self.risk_indicator.risk_color = Rgb { r: 255, g: 255, b: 0 };
                self.risk_indicator.risk_icon = "⚠";
                self.risk_indicator.show_warning = false;
            }
            RiskLevel::Medium => {
                self.risk_indicator.risk_color = Rgb { r: 255, g: 165, b: 0 };
                self.risk_indicator.risk_icon = "⚠";
                self.risk_indicator.show_warning = true;
            }
            RiskLevel::High => {
                self.risk_indicator.risk_color = Rgb { r: 255, g: 69, b: 0 };
                self.risk_indicator.risk_icon = "⚡";
                self.risk_indicator.show_warning = true;
            }
            RiskLevel::Critical => {
                self.risk_indicator.risk_color = Rgb { r: 255, g: 0, b: 0 };
                self.risk_indicator.risk_icon = "💀";
                self.risk_indicator.show_warning = true;
            }
        }
    }
    
    fn generate_interactive_elements(&mut self, explanation: &ExplanationResult) {
        self.interactive_elements.clear();
        
        // Command parts
        for (i, part) in explanation.breakdown.iter().enumerate() {
            let element = InteractiveElement {
                id: format!("part_{}", i),
                element_type: ElementType::CommandPart {
                    part_index: i,
                    part_type: part.part_type.clone(),
                },
                bounds: RenderRect::new(i as f32 * 50.0, 0.0, 50.0, 20.0), // Placeholder bounds
                is_hovered: false,
                action: ElementAction::ShowCommandPartDetail(i),
                tooltip: Some(part.explanation.clone()),
            };
            self.interactive_elements.push(element);
        }
        
        // Risk indicator element
        if !matches!(self.risk_indicator.risk_level, RiskLevel::Safe) {
            let risk_element = InteractiveElement {
                id: "risk_indicator".to_string(),
                element_type: ElementType::RiskIndicator,
                bounds: RenderRect::new(0.0, 25.0, 100.0, 20.0), // Placeholder bounds
                is_hovered: false,
                action: ElementAction::ShowRiskExplanation,
                tooltip: self.risk_indicator.risk_description.clone(),
            };
            self.interactive_elements.push(risk_element);
        }
    }
    
    fn update_suggestions(&mut self, explanation: &ExplanationResult) {
        self.suggestion_panel.suggestions.clear();
        
        // Add safer alternatives
        for alternative in &explanation.safer_alternatives {
            self.suggestion_panel.suggestions.push(Suggestion {
                text: alternative.clone(),
                suggestion_type: SuggestionType::SaferOption,
                confidence: 0.8,
                icon: "🛡",
                description: Some("Safer alternative command".to_string()),
                action: alternative.clone(),
            });
        }
        
        // Add related commands
        for related in &explanation.related_commands {
            self.suggestion_panel.suggestions.push(Suggestion {
                text: related.clone(),
                suggestion_type: SuggestionType::Related,
                confidence: 0.6,
                icon: "🔗",
                description: Some("Related command".to_string()),
                action: related.clone(),
            });
        }
        
        // Add examples
        for example in &explanation.examples {
            self.suggestion_panel.suggestions.push(Suggestion {
                text: example.command.clone(),
                suggestion_type: SuggestionType::Example,
                confidence: 0.7,
                icon: "📖",
                description: Some(example.context.clone()),
                action: example.command.clone(),
            });
        }
    }
    
    fn start_fade_in(&mut self) {
        self.animation_state.target_alpha = 1.0;
        self.animation_state.animation_start = Some(Instant::now());
    }
    
    fn find_hovered_element(&self, position: Point) -> Option<usize> {
        for (i, element) in self.interactive_elements.iter().enumerate() {
            if element.bounds.contains_point(position) {
                return Some(i);
            }
        }
        None
    }
    
    fn update_element_hover_states(&mut self, position: Point) {
        for element in &mut self.interactive_elements {
            element.is_hovered = element.bounds.contains_point(position);
        }
    }
    
    fn calculate_tooltip_position(&mut self) {
        if let Some(mouse_pos) = self.hover_state.mouse_position {
            // Position tooltip relative to mouse, avoiding screen edges
            let tooltip_width = 200;
            let tooltip_height = 100;
            
            self.hover_state.tooltip_position = Some(Point::new(
                Column(mouse_pos.column.0 + 2),
                Line(mouse_pos.line.0.saturating_sub(3))
            ));
            self.hover_state.tooltip_size = Some((tooltip_width, tooltip_height));
        }
    }
    
    fn apply_suggestion(&mut self, suggestion: Suggestion) {
        // Apply the suggestion - this would interface with the terminal input
        // For now, just hide the suggestion panel
        self.hide_suggestions();
    }
    
    fn hide_suggestions(&mut self) {
        self.suggestion_panel.visible = false;
    }
    
    fn ease_in_out_cubic(&self, t: f32) -> f32 {
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
        }
    }
    
    fn lerp(&self, a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }
    
    // Render data builders
    
    fn build_main_panel(&self, explanation: &ExplanationResult, size_info: &SizeInfo) -> PanelRenderData {
        PanelRenderData {
            title: "Command Explanation".to_string(),
            content: explanation.explanation.clone(),
            bounds: RenderRect::new(
                10.0,
                size_info.height() as f32 - 200.0,
                400.0,
                150.0
            ),
            background_color: Rgb { r: 40, g: 44, b: 52 },
            border_color: Rgb { r: 60, g: 64, b: 72 },
            text_color: Rgb { r: 255, g: 255, b: 255 },
        }
    }
    
    fn build_risk_indicator(&self) -> RiskIndicatorRenderData {
        RiskIndicatorRenderData {
            icon: self.risk_indicator.risk_icon,
            color: self.risk_indicator.risk_color,
            pulse_alpha: if matches!(self.risk_indicator.risk_level, RiskLevel::High | RiskLevel::Critical) {
                0.5 + 0.5 * self.risk_indicator.pulse_phase.sin()
            } else {
                1.0
            },
            bounds: RenderRect::new(420.0, 10.0, 40.0, 40.0),
            description: self.risk_indicator.risk_description.clone(),
        }
    }
    
    fn build_tooltip(&self, explanation: &ExplanationResult) -> Option<TooltipRenderData> {
        if let (Some(position), Some(size), Some(hovered_idx)) = (
            self.hover_state.tooltip_position,
            self.hover_state.tooltip_size,
            self.hover_state.hovered_part
        ) {
            if let Some(element) = self.interactive_elements.get(hovered_idx) {
                return Some(TooltipRenderData {
                    text: element.tooltip.clone().unwrap_or_default(),
                    position,
                    size,
                    background_color: Rgb { r: 30, g: 30, b: 30 },
                    text_color: Rgb { r: 255, g: 255, b: 255 },
                    border_color: Rgb { r: 80, g: 80, b: 80 },
                });
            }
        }
        None
    }
    
    fn build_suggestion_panel(&self) -> SuggestionPanelRenderData {
        SuggestionPanelRenderData {
            bounds: RenderRect::new(
                self.suggestion_panel.position.column.0 as f32,
                self.suggestion_panel.position.line.0 as f32,
                self.suggestion_panel.size.0 as f32,
                self.suggestion_panel.size.1 as f32,
            ),
            suggestions: self.suggestion_panel.suggestions.clone(),
            selected: self.suggestion_panel.selected,
            scroll_offset: self.suggestion_panel.scroll_offset,
            background_color: Rgb { r: 35, g: 39, b: 47 },
            selected_color: Rgb { r: 60, g: 120, b: 200 },
            text_color: Rgb { r: 255, g: 255, b: 255 },
        }
    }
    
    fn build_interactive_highlights(&self) -> Vec<HighlightRenderData> {
        self.interactive_elements
            .iter()
            .filter(|element| element.is_hovered)
            .map(|element| HighlightRenderData {
                bounds: element.bounds,
                color: match element.element_type {
                    ElementType::CommandPart { .. } => Rgb { r: 100, g: 149, b: 237 },
                    ElementType::RiskIndicator => self.risk_indicator.risk_color,
                    _ => Rgb { r: 128, g: 128, b: 128 },
                },
                alpha: 0.3,
            })
            .collect()
    }
}

impl Default for WarpCommandOverlay {
    fn default() -> Self {
        Self::new()
    }
}

// Render data structures for the display system

/// Complete overlay render data
#[derive(Debug, Default)]
pub struct OverlayRenderData {
    pub main_panel: Option<PanelRenderData>,
    pub risk_indicator: Option<RiskIndicatorRenderData>,
    pub tooltip: Option<TooltipRenderData>,
    pub suggestion_panel: Option<SuggestionPanelRenderData>,
    pub interactive_highlights: Vec<HighlightRenderData>,
    pub alpha: f32,
    pub scale: f32,
    pub slide_offset: f32,
}

/// Panel render data
#[derive(Debug)]
pub struct PanelRenderData {
    pub title: String,
    pub content: String,
    pub bounds: RenderRect,
    pub background_color: Rgb,
    pub border_color: Rgb,
    pub text_color: Rgb,
}

/// Risk indicator render data
#[derive(Debug)]
pub struct RiskIndicatorRenderData {
    pub icon: &'static str,
    pub color: Rgb,
    pub pulse_alpha: f32,
    pub bounds: RenderRect,
    pub description: Option<String>,
}

/// Tooltip render data
#[derive(Debug)]
pub struct TooltipRenderData {
    pub text: String,
    pub position: Point,
    pub size: (u16, u16),
    pub background_color: Rgb,
    pub text_color: Rgb,
    pub border_color: Rgb,
}

/// Suggestion panel render data
#[derive(Debug)]
pub struct SuggestionPanelRenderData {
    pub bounds: RenderRect,
    pub suggestions: Vec<Suggestion>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub background_color: Rgb,
    pub selected_color: Rgb,
    pub text_color: Rgb,
}

/// Highlight render data
#[derive(Debug)]
pub struct HighlightRenderData {
    pub bounds: RenderRect,
    pub color: Rgb,
    pub alpha: f32,
}

// Extension trait for RenderRect to add point containment
impl RenderRect {
    fn contains_point(&self, point: Point) -> bool {
        let x = point.column.0 as f32;
        let y = point.line.0 as f32;
        
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}
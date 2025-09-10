// Native Keyboard/Mouse Integration for OpenAgent Terminal
//
// Provides immediate input handling for blocks, tabs, and splits with no lazy fallbacks.
// Features real-time key capture, mouse interaction, gesture recognition, and context-aware
// shortcuts.
//
// #![allow(dead_code)]
//
// use std::collections::{HashMap, HashSet, VecDeque};
// use std::sync::Arc;
// use std::time::{Duration, Instant};
//
// use anyhow::Result;
// use bitflags::bitflags;
// use crossterm::event::{
// Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
// };
// use serde::{Deserialize, Serialize};
// use tokio::sync::mpsc;
// use tracing::{debug, error, info, warn};
//
// use crate::blocks_v2::{BlockId, Block};
// use crate::native_renderer::RenderEvent;
//
// Native input integration manager
// pub struct InputIntegration {
// Keyboard handler for immediate key processing
// keyboard_handler: KeyboardHandler,
//
// Mouse handler for immediate mouse processing
// mouse_handler: MouseHandler,
//
// Gesture recognizer for advanced interactions
// gesture_recognizer: GestureRecognizer,
//
// Shortcut manager for context-aware bindings
// shortcut_manager: ShortcutManager,
//
// Focus manager for input routing
// focus_manager: FocusManager,
//
// Input state tracker
// state_tracker: InputStateTracker,
//
// Event callbacks for immediate responses
// event_callbacks: Vec<Box<dyn Fn(&InputEvent) + Send + Sync>>,
//
// Performance statistics
// stats: InputStats,
// }
//
// Input events for immediate feedback
// #[derive(Debug, Clone)]
// pub enum InputEvent {
// KeyPressed {
// key: KeyInput,
// target: InputTarget,
// timestamp: Instant,
// },
// KeyReleased {
// key: KeyInput,
// target: InputTarget,
// timestamp: Instant,
// },
// MouseClicked {
// button: MouseButton,
// position: Position,
// target: InputTarget,
// timestamp: Instant,
// },
// MouseMoved {
// position: Position,
// target: Option<InputTarget>,
// timestamp: Instant,
// },
// MouseScrolled {
// direction: ScrollDirection,
// position: Position,
// target: InputTarget,
// timestamp: Instant,
// },
// GestureDetected {
// gesture: Gesture,
// target: InputTarget,
// timestamp: Instant,
// },
// ShortcutTriggered {
// shortcut: Shortcut,
// target: InputTarget,
// timestamp: Instant,
// },
// FocusChanged {
// from: Option<InputTarget>,
// to: InputTarget,
// timestamp: Instant,
// },
// }
//
// Keyboard handler for immediate key processing
// #[derive(Debug)]
// pub struct KeyboardHandler {
// Currently pressed keys
// pressed_keys: HashSet<KeyInput>,
//
// Key repeat handling
// repeat_handler: KeyRepeatHandler,
//
// Key sequence detector
// sequence_detector: KeySequenceDetector,
//
// Context-aware key mappings
// key_mappings: HashMap<InputContext, HashMap<KeyInput, KeyAction>>,
//
// Key timing for performance analysis
// key_timings: VecDeque<KeyTiming>,
//
// Statistics
// total_keys: usize,
// keys_per_second: f64,
// last_update: Instant,
// }
//
// Mouse handler for immediate mouse processing
// #[derive(Debug)]
// pub struct MouseHandler {
// Current mouse position
// current_position: Position,
//
// Mouse button states
// button_states: HashMap<MouseButton, ButtonState>,
//
// Click detection and tracking
// click_detector: ClickDetector,
//
// Drag and drop handling
// drag_handler: DragHandler,
//
// Hover detection
// hover_detector: HoverDetector,
//
// Mouse timing for performance analysis
// mouse_timings: VecDeque<MouseTiming>,
//
// Statistics
// total_clicks: usize,
// total_moves: usize,
// last_update: Instant,
// }
//
// Gesture recognizer for advanced interactions
// #[derive(Debug)]
// pub struct GestureRecognizer {
// Active gesture tracking
// active_gestures: HashMap<GestureId, ActiveGesture>,
//
// Gesture patterns for recognition
// gesture_patterns: Vec<GesturePattern>,
//
// Gesture history for learning
// gesture_history: VecDeque<CompletedGesture>,
//
// Learning system for adaptive recognition
// learning_system: GestureLearning,
//
// Configuration
// sensitivity: f32,
// timeout: Duration,
// }
//
// Shortcut manager for context-aware bindings
// #[derive(Debug)]
// pub struct ShortcutManager {
// Global shortcuts (always active)
// global_shortcuts: HashMap<KeyCombination, Shortcut>,
//
// Context-specific shortcuts
// context_shortcuts: HashMap<InputContext, HashMap<KeyCombination, Shortcut>>,
//
// Dynamic shortcuts (runtime created)
// dynamic_shortcuts: HashMap<String, DynamicShortcut>,
//
// Shortcut history and usage tracking
// usage_tracker: ShortcutUsageTracker,
//
// Conflict resolution
// conflict_resolver: ShortcutConflictResolver,
// }
//
// Focus manager for input routing
// #[derive(Debug)]
// pub struct FocusManager {
// Current focus target
// current_focus: Option<InputTarget>,
//
// Focus history for navigation
// focus_history: VecDeque<FocusChange>,
//
// Focus tree for hierarchical navigation
// focus_tree: FocusTree,
//
// Tab navigation order
// tab_order: Vec<InputTarget>,
//
// Focus policies
// focus_policies: HashMap<InputContext, FocusPolicy>,
// }
//
// Input state tracker
// #[derive(Debug, Default)]
// pub struct InputStateTracker {
// Modifier key states
// modifiers: KeyModifiers,
//
// Active input modes
// active_modes: HashSet<InputMode>,
//
// Input context stack
// context_stack: Vec<InputContext>,
//
// State change history
// state_history: VecDeque<StateChange>,
//
// Performance tracking
// state_transitions: usize,
// last_transition: Instant,
// }
//
// Key input representation
// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
// pub struct KeyInput {
// pub code: KeyCode,
// pub modifiers: KeyModifiers,
// }
//
// Input target identification
// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
// pub enum InputTarget {
// Block(BlockId),
// Tab(String),
// Split(String),
// Terminal,
// SearchBar,
// CommandPalette,
// StatusBar,
// Sidebar,
// Global,
// }
//
// Screen position
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub struct Position {
// pub x: u16,
// pub y: u16,
// }
//
// Scroll direction
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum ScrollDirection {
// Up,
// Down,
// Left,
// Right,
// }
//
// Gesture types
// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
// pub enum Gesture {
// Swipe { direction: SwipeDirection, distance: u16 },
// Pinch { scale: f32 },
// Rotate { angle: f32 },
// TwoFingerTap,
// ThreeFingerTap,
// LongPress { duration: Duration },
// DoubleClick,
// TripleClick,
// Custom(String),
// }
//
// Swipe directions
// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// pub enum SwipeDirection {
// Up,
// Down,
// Left,
// Right,
// UpLeft,
// UpRight,
// DownLeft,
// DownRight,
// }
//
// Shortcut definition
// #[derive(Debug, Clone)]
// pub struct Shortcut {
// pub id: String,
// pub name: String,
// pub combination: KeyCombination,
// pub action: ShortcutAction,
// pub context: Option<InputContext>,
// pub description: String,
// pub enabled: bool,
// }
//
// Key combination for shortcuts
// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
// pub struct KeyCombination {
// pub keys: Vec<KeyInput>,
// pub sequence: bool, // true for sequences like "Ctrl+K, Ctrl+S"
// }
//
// Shortcut actions
// #[derive(Debug, Clone)]
// pub enum ShortcutAction {
// Command(String),
// Function(String),
// Script(String),
// Internal(InternalAction),
// Custom(Box<dyn Fn() + Send + Sync>),
// }
//
// Internal actions
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum InternalAction {
// NewBlock,
// CloseBlock,
// SwitchTab,
// SplitHorizontal,
// SplitVertical,
// FocusNext,
// FocusPrevious,
// ToggleFullscreen,
// ShowCommandPalette,
// Search,
// Copy,
// Paste,
// Undo,
// Redo,
// }
//
// Input contexts for context-aware handling
// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// pub enum InputContext {
// Terminal,
// Editor,
// Search,
// CommandPalette,
// Settings,
// FileExplorer,
// Git,
// Debug,
// Global,
// }
//
// Input modes
// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// pub enum InputMode {
// Normal,
// Insert,
// Visual,
// Command,
// Search,
// Navigation,
// }
//
// Key actions for mappings
// #[derive(Debug, Clone)]
// pub enum KeyAction {
// Insert(char),
// Movement(MovementAction),
// Edit(EditAction),
// Navigation(NavigationAction),
// System(SystemAction),
// Custom(String),
// }
//
// Movement actions
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum MovementAction {
// Up,
// Down,
// Left,
// Right,
// Home,
// End,
// PageUp,
// PageDown,
// WordLeft,
// WordRight,
// }
//
// Edit actions
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum EditAction {
// Backspace,
// Delete,
// Cut,
// Copy,
// Paste,
// Undo,
// Redo,
// SelectAll,
// }
//
// Navigation actions
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum NavigationAction {
// NextTab,
// PreviousTab,
// NextPane,
// PreviousPane,
// FirstTab,
// LastTab,
// }
//
// System actions
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum SystemAction {
// Quit,
// Minimize,
// Maximize,
// ToggleFullscreen,
// ShowHelp,
// ShowSettings,
// }
//
// Key repeat handling
// #[derive(Debug)]
// pub struct KeyRepeatHandler {
// pub initial_delay: Duration,
// pub repeat_rate: Duration,
// pub active_repeats: HashMap<KeyInput, KeyRepeat>,
// }
//
// Active key repeat tracking
// #[derive(Debug)]
// pub struct KeyRepeat {
// pub key: KeyInput,
// pub start_time: Instant,
// pub last_repeat: Instant,
// pub repeat_count: usize,
// }
//
// Key sequence detection
// #[derive(Debug, Default)]
// pub struct KeySequenceDetector {
// pub active_sequences: Vec<PartialSequence>,
// pub sequence_timeout: Duration,
// pub max_sequence_length: usize,
// }
//
// Partial key sequence
// #[derive(Debug)]
// pub struct PartialSequence {
// pub keys: Vec<KeyInput>,
// pub start_time: Instant,
// pub last_key: Instant,
// pub potential_matches: Vec<KeyCombination>,
// }
//
// Button state tracking
// #[derive(Debug, Clone)]
// pub struct ButtonState {
// pub pressed: bool,
// pub press_time: Option<Instant>,
// pub press_position: Option<Position>,
// pub click_count: usize,
// pub last_click: Option<Instant>,
// }
//
// Click detection and tracking
// #[derive(Debug)]
// pub struct ClickDetector {
// pub double_click_threshold: Duration,
// pub triple_click_threshold: Duration,
// pub click_distance_threshold: u16,
// pub recent_clicks: VecDeque<Click>,
// }
//
// Click information
// #[derive(Debug, Clone)]
// pub struct Click {
// pub button: MouseButton,
// pub position: Position,
// pub timestamp: Instant,
// pub count: usize,
// }
//
// Drag and drop handling
// #[derive(Debug)]
// pub struct DragHandler {
// pub active_drags: HashMap<MouseButton, ActiveDrag>,
// pub drag_threshold: u16,
// pub drag_data: HashMap<String, DragData>,
// }
//
// Active drag operation
// #[derive(Debug)]
// pub struct ActiveDrag {
// pub start_position: Position,
// pub current_position: Position,
// pub start_time: Instant,
// pub target: InputTarget,
// pub data: Option<DragData>,
// }
//
// Drag data
// #[derive(Debug, Clone)]
// pub struct DragData {
// pub data_type: String,
// pub content: Vec<u8>,
// pub mime_type: Option<String>,
// }
//
// Hover detection
// #[derive(Debug, Default)]
// pub struct HoverDetector {
// pub current_hover: Option<HoverInfo>,
// pub hover_threshold: Duration,
// pub hover_tolerance: u16,
// }
//
// Hover information
// #[derive(Debug)]
// pub struct HoverInfo {
// pub target: InputTarget,
// pub position: Position,
// pub start_time: Instant,
// pub tooltip_shown: bool,
// }
//
// Gesture identification
// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// pub struct GestureId(u64);
//
// impl GestureId {
// pub fn new() -> Self {
// use std::sync::atomic::{AtomicU64, Ordering};
// static COUNTER: AtomicU64 = AtomicU64::new(0);
// Self(COUNTER.fetch_add(1, Ordering::Relaxed))
// }
// }
//
// Active gesture tracking
// #[derive(Debug)]
// pub struct ActiveGesture {
// pub id: GestureId,
// pub gesture_type: GestureType,
// pub start_time: Instant,
// pub positions: Vec<(Position, Instant)>,
// pub properties: HashMap<String, f32>,
// }
//
// Gesture types for recognition
// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum GestureType {
// Tap,
// Swipe,
// Pinch,
// Rotate,
// LongPress,
// Custom(String),
// }
//
// Gesture pattern for recognition
// #[derive(Debug, Clone)]
// pub struct GesturePattern {
// pub name: String,
// pub gesture_type: GestureType,
// pub constraints: Vec<GestureConstraint>,
// pub confidence_threshold: f32,
// }
//
// Gesture constraints
// #[derive(Debug, Clone)]
// pub enum GestureConstraint {
// MinDistance(u16),
// MaxDistance(u16),
// MinDuration(Duration),
// MaxDuration(Duration),
// DirectionConstraint(SwipeDirection, f32), // direction and tolerance
// VelocityConstraint { min: f32, max: f32 },
// PositionConstraint { region: Rectangle },
// }
//
// Rectangle for position constraints
// #[derive(Debug, Clone)]
// pub struct Rectangle {
// pub x: u16,
// pub y: u16,
// pub width: u16,
// pub height: u16,
// }
//
// Completed gesture
// #[derive(Debug, Clone)]
// pub struct CompletedGesture {
// pub gesture: Gesture,
// pub target: InputTarget,
// pub timestamp: Instant,
// pub confidence: f32,
// pub properties: HashMap<String, f32>,
// }
//
// Gesture learning system
// #[derive(Debug, Default)]
// pub struct GestureLearning {
// pub enabled: bool,
// pub learning_rate: f32,
// pub gesture_memory: HashMap<String, GestureMemory>,
// pub adaptation_threshold: f32,
// }
//
// Gesture memory for learning
// #[derive(Debug, Default)]
// pub struct GestureMemory {
// pub successful_patterns: Vec<GesturePattern>,
// pub failed_attempts: usize,
// pub success_rate: f32,
// pub last_updated: Instant,
// }
//
// Dynamic shortcut
// #[derive(Debug, Clone)]
// pub struct DynamicShortcut {
// pub shortcut: Shortcut,
// pub creator: String,
// pub creation_time: Instant,
// pub usage_count: usize,
// pub last_used: Option<Instant>,
// }
//
// Shortcut usage tracking
// #[derive(Debug, Default)]
// pub struct ShortcutUsageTracker {
// pub usage_stats: HashMap<String, ShortcutUsage>,
// pub frequent_shortcuts: Vec<String>,
// pub last_analysis: Instant,
// }
//
// Shortcut usage statistics
// #[derive(Debug, Default)]
// pub struct ShortcutUsage {
// pub count: usize,
// pub last_used: Instant,
// pub average_response_time: Duration,
// pub contexts: HashSet<InputContext>,
// }
//
// Shortcut conflict resolution
// #[derive(Debug, Default)]
// pub struct ShortcutConflictResolver {
// pub conflicts: Vec<ShortcutConflict>,
// pub resolution_strategy: ConflictResolution,
// pub user_overrides: HashMap<String, String>,
// }
//
// Shortcut conflict
// #[derive(Debug)]
// pub struct ShortcutConflict {
// pub combination: KeyCombination,
// pub shortcuts: Vec<String>,
// pub contexts: Vec<InputContext>,
// pub severity: ConflictSeverity,
// }
//
// Conflict severity
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum ConflictSeverity {
// Low,
// Medium,
// High,
// Critical,
// }
//
// Conflict resolution strategy
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum ConflictResolution {
// ContextPriority,
// UserChoice,
// LastDefined,
// MostUsed,
// Disabled,
// }
//
// Focus change tracking
// #[derive(Debug, Clone)]
// pub struct FocusChange {
// pub from: Option<InputTarget>,
// pub to: InputTarget,
// pub timestamp: Instant,
// pub trigger: FocusTrigger,
// }
//
// Focus trigger
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum FocusTrigger {
// Mouse,
// Keyboard,
// Tab,
// Programmatic,
// User,
// }
//
// Focus tree for hierarchical navigation
// #[derive(Debug, Default)]
// pub struct FocusTree {
// pub root: Option<FocusNode>,
// pub current: Option<InputTarget>,
// pub navigation_cache: HashMap<InputTarget, Vec<InputTarget>>,
// }
//
// Focus tree node
// #[derive(Debug)]
// pub struct FocusNode {
// pub target: InputTarget,
// pub parent: Option<InputTarget>,
// pub children: Vec<InputTarget>,
// pub focusable: bool,
// pub tab_index: Option<i32>,
// }
//
// Focus policy
// #[derive(Debug, Clone)]
// pub struct FocusPolicy {
// pub auto_focus: bool,
// pub trap_focus: bool,
// pub restore_focus: bool,
// pub focus_order: Vec<InputTarget>,
// }
//
// State change tracking
// #[derive(Debug, Clone)]
// pub struct StateChange {
// pub from_state: InputState,
// pub to_state: InputState,
// pub timestamp: Instant,
// pub trigger: StateTrigger,
// }
//
// Input state snapshot
// #[derive(Debug, Clone)]
// pub struct InputState {
// pub modifiers: KeyModifiers,
// pub modes: HashSet<InputMode>,
// pub context: InputContext,
// pub focus: Option<InputTarget>,
// }
//
// State change trigger
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum StateTrigger {
// KeyPress,
// KeyRelease,
// MouseEvent,
// Focus,
// Mode,
// Context,
// }
//
// Key timing for performance analysis
// #[derive(Debug, Clone)]
// pub struct KeyTiming {
// pub key: KeyInput,
// pub timestamp: Instant,
// pub processing_time: Duration,
// pub context: InputContext,
// }
//
// Mouse timing for performance analysis
// #[derive(Debug, Clone)]
// pub struct MouseTiming {
// pub event_type: MouseEventType,
// pub position: Position,
// pub timestamp: Instant,
// pub processing_time: Duration,
// }
//
// Mouse event types for timing
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum MouseEventType {
// Click,
// Move,
// Scroll,
// Drag,
// Hover,
// }
//
// Performance statistics
// #[derive(Debug, Default, Clone)]
// pub struct InputStats {
// pub keys_processed: usize,
// pub mouse_events_processed: usize,
// pub gestures_recognized: usize,
// pub shortcuts_triggered: usize,
// pub focus_changes: usize,
// pub average_key_processing_time: Duration,
// pub average_mouse_processing_time: Duration,
// pub events_per_second: f64,
// pub last_reset: Instant,
// }
//
// bitflags! {
// Input processing flags
// pub struct InputFlags: u32 {
// const CAPTURE_ALL = 0b00000001;
// const BLOCK_PROPAGATION = 0b00000010;
// const IMMEDIATE_PROCESS = 0b00000100;
// const LOG_EVENTS = 0b00001000;
// const ENABLE_GESTURES = 0b00010000;
// const ENABLE_SHORTCUTS = 0b00100000;
// const ENABLE_SEQUENCES = 0b01000000;
// const ENABLE_LEARNING = 0b10000000;
// }
// }
//
// impl InputIntegration {
// Create new input integration with immediate capabilities
// pub fn new() -> Self {
// let mut integration = Self {
// keyboard_handler: KeyboardHandler::new(),
// mouse_handler: MouseHandler::new(),
// gesture_recognizer: GestureRecognizer::new(),
// shortcut_manager: ShortcutManager::new(),
// focus_manager: FocusManager::new(),
// state_tracker: InputStateTracker::default(),
// event_callbacks: Vec::new(),
// stats: InputStats {
// last_reset: Instant::now(),
// ..Default::default()
// },
// };
//
// Setup default key mappings immediately
// integration.setup_default_mappings();
//
// Setup default shortcuts immediately
// integration.setup_default_shortcuts();
//
// integration
// }
//
// Register event callback for immediate responses
// pub fn register_event_callback<F>(&mut self, callback: F)
// where
// F: Fn(&InputEvent) + Send + Sync + 'static,
// {
// self.event_callbacks.push(Box::new(callback));
// }
//
// Emit input event immediately
// fn emit_event(&self, event: InputEvent) {
// for callback in &self.event_callbacks {
// callback(&event);
// }
// }
//
// Process keyboard event immediately
// pub fn process_keyboard_event(&mut self, event: KeyEvent) -> Result<bool> {
// let start_time = Instant::now();
// let key_input = KeyInput {
// code: event.code,
// modifiers: event.modifiers,
// };
//
// Update modifier state immediately
// self.state_tracker.modifiers = event.modifiers;
//
// Determine target immediately
// let target = self.focus_manager.current_focus.clone()
// .unwrap_or(InputTarget::Terminal);
//
// match event.kind {
// crossterm::event::KeyEventKind::Press => {
// Track key press immediately
// self.keyboard_handler.pressed_keys.insert(key_input);
//
// Check for shortcuts first
// if let Some(shortcut) = self.shortcut_manager.check_shortcut(&key_input,
// &self.get_current_context()) { self.execute_shortcut(shortcut, &target)?;
// self.emit_event(InputEvent::ShortcutTriggered {
// shortcut: shortcut.clone(),
// target: target.clone(),
// timestamp: start_time,
// });
// return Ok(true); // Event handled
// }
//
// Check for key sequences
// if let Some(sequence_result) = self.keyboard_handler.sequence_detector.process_key(&key_input) {
// if let Some(shortcut) = self.shortcut_manager.check_sequence(&sequence_result) {
// self.execute_shortcut(shortcut, &target)?;
// return Ok(true);
// }
// }
//
// Handle key repeat
// self.keyboard_handler.repeat_handler.start_repeat(key_input);
//
// Get key action for current context
// let context = self.get_current_context();
// if let Some(action) = self.keyboard_handler.get_key_action(&key_input, &context) {
// self.execute_key_action(action, &target)?;
// }
//
// Emit key pressed event
// self.emit_event(InputEvent::KeyPressed {
// key: key_input,
// target: target.clone(),
// timestamp: start_time,
// });
// },
// crossterm::event::KeyEventKind::Release => {
// Track key release immediately
// self.keyboard_handler.pressed_keys.remove(&key_input);
//
// Stop key repeat
// self.keyboard_handler.repeat_handler.stop_repeat(&key_input);
//
// Emit key released event
// self.emit_event(InputEvent::KeyReleased {
// key: key_input,
// target: target.clone(),
// timestamp: start_time,
// });
// },
// _ => {}, // Other key event types
// }
//
// Update statistics immediately
// let processing_time = start_time.elapsed();
// self.keyboard_handler.key_timings.push_back(KeyTiming {
// key: key_input,
// timestamp: start_time,
// processing_time,
// context: self.get_current_context(),
// });
//
// Limit timing history
// if self.keyboard_handler.key_timings.len() > 1000 {
// self.keyboard_handler.key_timings.pop_front();
// }
//
// self.stats.keys_processed += 1;
// self.update_performance_stats();
//
// Ok(false) // Event not fully handled, allow propagation
// }
//
// Process mouse event immediately
// pub fn process_mouse_event(&mut self, event: MouseEvent) -> Result<bool> {
// let start_time = Instant::now();
// let position = Position { x: event.column, y: event.row };
//
// Update current mouse position immediately
// self.mouse_handler.current_position = position;
//
// Determine target immediately
// let target = self.get_target_at_position(position);
//
// match event.kind {
// MouseEventKind::Down(button) => {
// Update button state immediately
// let button_state = self.mouse_handler.button_states.entry(button).or_default();
// button_state.pressed = true;
// button_state.press_time = Some(start_time);
// button_state.press_position = Some(position);
//
// Start potential drag operation
// self.mouse_handler.drag_handler.start_potential_drag(button, position, target.clone());
//
// Emit mouse clicked event
// self.emit_event(InputEvent::MouseClicked {
// button,
// position,
// target: target.clone(),
// timestamp: start_time,
// });
// },
// MouseEventKind::Up(button) => {
// Update button state immediately
// if let Some(button_state) = self.mouse_handler.button_states.get_mut(&button) {
// button_state.pressed = false;
//
// Detect clicks immediately
// if let Some(press_time) = button_state.press_time {
// let click_duration = start_time.duration_since(press_time);
// if click_duration < Duration::from_millis(500) {
// let click = self.mouse_handler.click_detector.register_click(button, position, start_time);
// self.handle_click(click, &target)?;
// }
// }
//
// button_state.press_time = None;
// button_state.press_position = None;
// }
//
// End drag operation if active
// self.mouse_handler.drag_handler.end_drag(button, position);
// },
// MouseEventKind::Moved => {
// Update hover detection immediately
// self.mouse_handler.hover_detector.update_hover(position, target.clone(), start_time);
//
// Update active drags immediately
// self.mouse_handler.drag_handler.update_drags(position);
//
// Emit mouse moved event
// self.emit_event(InputEvent::MouseMoved {
// position,
// target: Some(target),
// timestamp: start_time,
// });
// },
// MouseEventKind::ScrollDown => {
// self.handle_scroll(ScrollDirection::Down, position, target.clone(), start_time)?;
// },
// MouseEventKind::ScrollUp => {
// self.handle_scroll(ScrollDirection::Up, position, target.clone(), start_time)?;
// },
// MouseEventKind::ScrollLeft => {
// self.handle_scroll(ScrollDirection::Left, position, target.clone(), start_time)?;
// },
// MouseEventKind::ScrollRight => {
// self.handle_scroll(ScrollDirection::Right, position, target.clone(), start_time)?;
// },
// _ => {}, // Other mouse event types
// }
//
// Update statistics immediately
// let processing_time = start_time.elapsed();
// self.mouse_handler.mouse_timings.push_back(MouseTiming {
// event_type: match event.kind {
// MouseEventKind::Down(_) | MouseEventKind::Up(_) => MouseEventType::Click,
// MouseEventKind::Moved => MouseEventType::Move,
// MouseEventKind::ScrollDown | MouseEventKind::ScrollUp |
// MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => MouseEventType::Scroll,
// _ => MouseEventType::Move,
// },
// position,
// timestamp: start_time,
// processing_time,
// });
//
// Limit timing history
// if self.mouse_handler.mouse_timings.len() > 1000 {
// self.mouse_handler.mouse_timings.pop_front();
// }
//
// self.stats.mouse_events_processed += 1;
// self.update_performance_stats();
//
// Ok(false) // Allow event propagation
// }
//
// Set focus target immediately
// pub fn set_focus(&mut self, target: InputTarget) -> Result<()> {
// let old_focus = self.focus_manager.current_focus.clone();
//
// if old_focus.as_ref() != Some(&target) {
// Update focus immediately
// self.focus_manager.current_focus = Some(target.clone());
//
// Record focus change
// let focus_change = FocusChange {
// from: old_focus.clone(),
// to: target.clone(),
// timestamp: Instant::now(),
// trigger: FocusTrigger::Programmatic,
// };
//
// self.focus_manager.focus_history.push_back(focus_change);
// if self.focus_manager.focus_history.len() > 100 {
// self.focus_manager.focus_history.pop_front();
// }
//
// Update context if needed
// self.update_context_for_target(&target);
//
// Emit focus changed event immediately
// self.emit_event(InputEvent::FocusChanged {
// from: old_focus,
// to: target,
// timestamp: Instant::now(),
// });
//
// self.stats.focus_changes += 1;
// }
//
// Ok(())
// }
//
// Add custom shortcut immediately
// pub fn add_shortcut(&mut self, shortcut: Shortcut) -> Result<()> {
// Check for conflicts immediately
// let conflicts = self.shortcut_manager.conflict_resolver.check_conflicts(&shortcut.combination);
//
// if !conflicts.is_empty() {
// Handle conflicts based on resolution strategy
// self.shortcut_manager.conflict_resolver.resolve_conflicts(&shortcut, conflicts)?;
// }
//
// Add shortcut to appropriate collection
// if let Some(context) = shortcut.context {
// self.shortcut_manager.context_shortcuts
// .entry(context)
// .or_insert_with(HashMap::new)
// .insert(shortcut.combination.clone(), shortcut);
// } else {
// self.shortcut_manager.global_shortcuts
// .insert(shortcut.combination.clone(), shortcut);
// }
//
// Ok(())
// }
//
// Remove shortcut immediately
// pub fn remove_shortcut(&mut self, combination: &KeyCombination, context: Option<InputContext>) ->
// Result<bool> { let removed = if let Some(ctx) = context {
// self.shortcut_manager.context_shortcuts
// .get_mut(&ctx)
// .map(|shortcuts| shortcuts.remove(combination).is_some())
// .unwrap_or(false)
// } else {
// self.shortcut_manager.global_shortcuts.remove(combination).is_some()
// };
//
// Ok(removed)
// }
//
// Get input statistics
// pub fn get_stats(&self) -> InputStats {
// self.stats.clone()
// }
//
// Setup default key mappings
// fn setup_default_mappings(&mut self) {
// Terminal context mappings
// let mut terminal_mappings = HashMap::new();
//
// Navigation
// terminal_mappings.insert(
// KeyInput { code: KeyCode::Up, modifiers: KeyModifiers::NONE },
// KeyAction::Movement(MovementAction::Up)
// );
// terminal_mappings.insert(
// KeyInput { code: KeyCode::Down, modifiers: KeyModifiers::NONE },
// KeyAction::Movement(MovementAction::Down)
// );
// terminal_mappings.insert(
// KeyInput { code: KeyCode::Left, modifiers: KeyModifiers::NONE },
// KeyAction::Movement(MovementAction::Left)
// );
// terminal_mappings.insert(
// KeyInput { code: KeyCode::Right, modifiers: KeyModifiers::NONE },
// KeyAction::Movement(MovementAction::Right)
// );
//
// Editing
// terminal_mappings.insert(
// KeyInput { code: KeyCode::Backspace, modifiers: KeyModifiers::NONE },
// KeyAction::Edit(EditAction::Backspace)
// );
// terminal_mappings.insert(
// KeyInput { code: KeyCode::Delete, modifiers: KeyModifiers::NONE },
// KeyAction::Edit(EditAction::Delete)
// );
//
// self.keyboard_handler.key_mappings.insert(InputContext::Terminal, terminal_mappings);
// }
//
// Setup default shortcuts
// fn setup_default_shortcuts(&mut self) {
// Global shortcuts
// self.add_shortcut(Shortcut {
// id: "new_block".to_string(),
// name: "New Block".to_string(),
// combination: KeyCombination {
// keys: vec![KeyInput {
// code: KeyCode::Char('n'),
// modifiers: KeyModifiers::CONTROL
// }],
// sequence: false,
// },
// action: ShortcutAction::Internal(InternalAction::NewBlock),
// context: None,
// description: "Create a new block".to_string(),
// enabled: true,
// }).ok();
//
// self.add_shortcut(Shortcut {
// id: "close_block".to_string(),
// name: "Close Block".to_string(),
// combination: KeyCombination {
// keys: vec![KeyInput {
// code: KeyCode::Char('w'),
// modifiers: KeyModifiers::CONTROL
// }],
// sequence: false,
// },
// action: ShortcutAction::Internal(InternalAction::CloseBlock),
// context: None,
// description: "Close current block".to_string(),
// enabled: true,
// }).ok();
//
// Add more default shortcuts...
// }
//
// Get current input context
// fn get_current_context(&self) -> InputContext {
// self.state_tracker.context_stack.last()
// .copied()
// .unwrap_or(InputContext::Global)
// }
//
// Get target at position
// fn get_target_at_position(&self, position: Position) -> InputTarget {
// This would integrate with the renderer to determine what's at the position
// For now, return the current focus or terminal
// self.focus_manager.current_focus.clone()
// .unwrap_or(InputTarget::Terminal)
// }
//
// Execute shortcut action
// fn execute_shortcut(&mut self, shortcut: &Shortcut, target: &InputTarget) -> Result<()> {
// Update usage statistics immediately
// self.shortcut_manager.usage_tracker.update_usage(&shortcut.id);
//
// match &shortcut.action {
// ShortcutAction::Internal(action) => {
// self.execute_internal_action(*action, target)?;
// },
// ShortcutAction::Command(cmd) => {
// Execute external command
// debug!("Executing command: {}", cmd);
// },
// ShortcutAction::Function(func) => {
// Execute function
// debug!("Executing function: {}", func);
// },
// _ => {
// warn!("Shortcut action not implemented: {:?}", shortcut.action);
// },
// }
//
// self.stats.shortcuts_triggered += 1;
// Ok(())
// }
//
// Execute internal action
// fn execute_internal_action(&mut self, action: InternalAction, target: &InputTarget) -> Result<()>
// { match action {
// InternalAction::NewBlock => {
// info!("Creating new block");
// Integrate with blocks system
// },
// InternalAction::CloseBlock => {
// info!("Closing block");
// Integrate with blocks system
// },
// InternalAction::SwitchTab => {
// info!("Switching tab");
// self.focus_manager.navigate_next_tab()?;
// },
// InternalAction::FocusNext => {
// self.focus_manager.focus_next()?;
// },
// InternalAction::FocusPrevious => {
// self.focus_manager.focus_previous()?;
// },
// _ => {
// debug!("Internal action not implemented: {:?}", action);
// },
// }
// Ok(())
// }
//
// Execute key action
// fn execute_key_action(&mut self, action: &KeyAction, target: &InputTarget) -> Result<()> {
// match action {
// KeyAction::Movement(movement) => {
// self.execute_movement_action(*movement, target)?;
// },
// KeyAction::Edit(edit) => {
// self.execute_edit_action(*edit, target)?;
// },
// KeyAction::Navigation(nav) => {
// self.execute_navigation_action(*nav, target)?;
// },
// KeyAction::System(system) => {
// self.execute_system_action(*system, target)?;
// },
// _ => {
// debug!("Key action not implemented: {:?}", action);
// },
// }
// Ok()
// }
//
// Execute movement action
// fn execute_movement_action(&mut self, action: MovementAction, target: &InputTarget) -> Result<()>
// { match action {
// MovementAction::Up | MovementAction::Down |
// MovementAction::Left | MovementAction::Right => {
// Send to terminal or active block
// debug!("Movement: {:?} for target: {:?}", action, target);
// },
// _ => {
// debug!("Movement action not implemented: {:?}", action);
// },
// }
// Ok(())
// }
//
// Execute edit action
// fn execute_edit_action(&mut self, action: EditAction, target: &InputTarget) -> Result<()> {
// match action {
// EditAction::Copy | EditAction::Cut | EditAction::Paste => {
// Handle clipboard operations
// debug!("Edit: {:?} for target: {:?}", action, target);
// },
// _ => {
// debug!("Edit action not implemented: {:?}", action);
// },
// }
// Ok(())
// }
//
// Execute navigation action
// fn execute_navigation_action(&mut self, action: NavigationAction, target: &InputTarget) ->
// Result<()> { match action {
// NavigationAction::NextTab => {
// self.focus_manager.navigate_next_tab()?;
// },
// NavigationAction::PreviousTab => {
// self.focus_manager.navigate_previous_tab()?;
// },
// _ => {
// debug!("Navigation action not implemented: {:?}", action);
// },
// }
// Ok(())
// }
//
// Execute system action
// fn execute_system_action(&mut self, action: SystemAction, target: &InputTarget) -> Result<()> {
// match action {
// SystemAction::Quit => {
// info!("Quit requested");
// Send quit signal
// },
// SystemAction::ToggleFullscreen => {
// info!("Toggle fullscreen");
// Send fullscreen toggle
// },
// _ => {
// debug!("System action not implemented: {:?}", action);
// },
// }
// Ok(())
// }
//
// Handle click event
// fn handle_click(&mut self, click: Click, target: &InputTarget) -> Result<()> {
// Set focus if needed
// if self.focus_manager.current_focus.as_ref() != Some(target) {
// self.set_focus(target.clone())?;
// }
//
// Handle special click types
// match click.count {
// 2 => {
// Double click - select word or similar
// debug!("Double click at {:?}", click.position);
// },
// 3 => {
// Triple click - select line or similar
// debug!("Triple click at {:?}", click.position);
// },
// _ => {
// Single click
// debug!("Single click at {:?}", click.position);
// },
// }
//
// Ok(())
// }
//
// Handle scroll event
// fn handle_scroll(&mut self, direction: ScrollDirection, position: Position, target: InputTarget,
// timestamp: Instant) -> Result<()> { Emit scroll event immediately
// self.emit_event(InputEvent::MouseScrolled {
// direction,
// position,
// target: target.clone(),
// timestamp,
// });
//
// Handle scroll based on target
// match target {
// InputTarget::Block(_) => {
// Send scroll to block
// debug!("Scrolling block: {:?}", direction);
// },
// InputTarget::Terminal => {
// Send scroll to terminal
// debug!("Scrolling terminal: {:?}", direction);
// },
// _ => {
// debug!("Scroll not handled for target: {:?}", target);
// },
// }
//
// Ok(())
// }
//
// Update context for target
// fn update_context_for_target(&mut self, target: &InputTarget) {
// let new_context = match target {
// InputTarget::Terminal => InputContext::Terminal,
// InputTarget::SearchBar => InputContext::Search,
// InputTarget::CommandPalette => InputContext::CommandPalette,
// _ => InputContext::Global,
// };
//
// if self.state_tracker.context_stack.last() != Some(&new_context) {
// self.state_tracker.context_stack.push(new_context);
//
// Limit context stack depth
// if self.state_tracker.context_stack.len() > 10 {
// self.state_tracker.context_stack.remove(0);
// }
// }
// }
//
// Update performance statistics
// fn update_performance_stats(&mut self) {
// let now = Instant::now();
// let elapsed = now.duration_since(self.stats.last_reset);
//
// if elapsed >= Duration::from_secs(1) {
// let total_events = self.stats.keys_processed + self.stats.mouse_events_processed;
// self.stats.events_per_second = total_events as f64 / elapsed.as_secs_f64();
//
// Update average processing times
// if !self.keyboard_handler.key_timings.is_empty() {
// let total_time: Duration = self.keyboard_handler.key_timings
// .iter()
// .map(|t| t.processing_time)
// .sum();
// self.stats.average_key_processing_time =
// total_time / self.keyboard_handler.key_timings.len() as u32;
// }
//
// if !self.mouse_handler.mouse_timings.is_empty() {
// let total_time: Duration = self.mouse_handler.mouse_timings
// .iter()
// .map(|t| t.processing_time)
// .sum();
// self.stats.average_mouse_processing_time =
// total_time / self.mouse_handler.mouse_timings.len() as u32;
// }
// }
// }
// }
//
// Implementation for helper structs
// impl KeyboardHandler {
// fn new() -> Self {
// Self {
// pressed_keys: HashSet::new(),
// repeat_handler: KeyRepeatHandler::new(),
// sequence_detector: KeySequenceDetector::default(),
// key_mappings: HashMap::new(),
// key_timings: VecDeque::new(),
// total_keys: 0,
// keys_per_second: 0.0,
// last_update: Instant::now(),
// }
// }
//
// fn get_key_action(&self, key: &KeyInput, context: &InputContext) -> Option<&KeyAction> {
// self.key_mappings.get(context)?.get(key)
// }
// }
//
// impl MouseHandler {
// fn new() -> Self {
// Self {
// current_position: Position { x: 0, y: 0 },
// button_states: HashMap::new(),
// click_detector: ClickDetector::new(),
// drag_handler: DragHandler::new(),
// hover_detector: HoverDetector::default(),
// mouse_timings: VecDeque::new(),
// total_clicks: 0,
// total_moves: 0,
// last_update: Instant::now(),
// }
// }
// }
//
// impl Default for ButtonState {
// fn default() -> Self {
// Self {
// pressed: false,
// press_time: None,
// press_position: None,
// click_count: 0,
// last_click: None,
// }
// }
// }
//
// impl ClickDetector {
// fn new() -> Self {
// Self {
// double_click_threshold: Duration::from_millis(500),
// triple_click_threshold: Duration::from_millis(300),
// click_distance_threshold: 5,
// recent_clicks: VecDeque::new(),
// }
// }
//
// fn register_click(&mut self, button: MouseButton, position: Position, timestamp: Instant) ->
// Click { Find recent clicks for multi-click detection
// let mut click_count = 1;
//
// for recent_click in self.recent_clicks.iter().rev() {
// if recent_click.button == button &&
// timestamp.duration_since(recent_click.timestamp) < self.double_click_threshold &&
// self.distance(position, recent_click.position) < self.click_distance_threshold {
// click_count = recent_click.count + 1;
// break;
// }
// }
//
// let click = Click {
// button,
// position,
// timestamp,
// count: click_count,
// };
//
// self.recent_clicks.push_back(click.clone());
//
// Limit recent clicks history
// if self.recent_clicks.len() > 10 {
// self.recent_clicks.pop_front();
// }
//
// click
// }
//
// fn distance(&self, p1: Position, p2: Position) -> u16 {
// let dx = (p1.x as i32 - p2.x as i32).abs() as u16;
// let dy = (p1.y as i32 - p2.y as i32).abs() as u16;
// ((dx * dx + dy * dy) as f64).sqrt() as u16
// }
// }
//
// impl DragHandler {
// fn new() -> Self {
// Self {
// active_drags: HashMap::new(),
// drag_threshold: 5,
// drag_data: HashMap::new(),
// }
// }
//
// fn start_potential_drag(&mut self, button: MouseButton, position: Position, target: InputTarget)
// { let drag = ActiveDrag {
// start_position: position,
// current_position: position,
// start_time: Instant::now(),
// target,
// data: None,
// };
// self.active_drags.insert(button, drag);
// }
//
// fn update_drags(&mut self, position: Position) {
// for drag in self.active_drags.values_mut() {
// drag.current_position = position;
// }
// }
//
// fn end_drag(&mut self, button: MouseButton, position: Position) {
// if let Some(drag) = self.active_drags.remove(&button) {
// let distance = self.distance(drag.start_position, position);
// if distance > self.drag_threshold {
// This was a drag operation
// debug!("Drag completed: {:?} to {:?}", drag.start_position, position);
// }
// }
// }
//
// fn distance(&self, p1: Position, p2: Position) -> u16 {
// let dx = (p1.x as i32 - p2.x as i32).abs() as u16;
// let dy = (p1.y as i32 - p2.y as i32).abs() as u16;
// ((dx * dx + dy * dy) as f64).sqrt() as u16
// }
// }
//
// impl HoverDetector {
// fn update_hover(&mut self, position: Position, target: InputTarget, timestamp: Instant) {
// if let Some(ref mut hover) = self.current_hover {
// if hover.target == target && self.distance(hover.position, position) < self.hover_tolerance {
// Still hovering over same target
// return;
// }
// }
//
// New hover target
// self.current_hover = Some(HoverInfo {
// target,
// position,
// start_time: timestamp,
// tooltip_shown: false,
// });
// }
//
// fn distance(&self, p1: Position, p2: Position) -> u16 {
// let dx = (p1.x as i32 - p2.x as i32).abs() as u16;
// let dy = (p1.y as i32 - p2.y as i32).abs() as u16;
// ((dx * dx + dy * dy) as f64).sqrt() as u16
// }
// }
//
// impl GestureRecognizer {
// fn new() -> Self {
// Self {
// active_gestures: HashMap::new(),
// gesture_patterns: Vec::new(),
// gesture_history: VecDeque::new(),
// learning_system: GestureLearning::default(),
// sensitivity: 0.7,
// timeout: Duration::from_secs(2),
// }
// }
// }
//
// impl ShortcutManager {
// fn new() -> Self {
// Self {
// global_shortcuts: HashMap::new(),
// context_shortcuts: HashMap::new(),
// dynamic_shortcuts: HashMap::new(),
// usage_tracker: ShortcutUsageTracker::default(),
// conflict_resolver: ShortcutConflictResolver::default(),
// }
// }
//
// fn check_shortcut(&self, key: &KeyInput, context: &InputContext) -> Option<&Shortcut> {
// let combination = KeyCombination {
// keys: vec![*key],
// sequence: false,
// };
//
// Check context-specific shortcuts first
// if let Some(shortcuts) = self.context_shortcuts.get(context) {
// if let Some(shortcut) = shortcuts.get(&combination) {
// if shortcut.enabled {
// return Some(shortcut);
// }
// }
// }
//
// Check global shortcuts
// if let Some(shortcut) = self.global_shortcuts.get(&combination) {
// if shortcut.enabled {
// return Some(shortcut);
// }
// }
//
// None
// }
//
// fn check_sequence(&self, combination: &KeyCombination) -> Option<&Shortcut> {
// Check all shortcuts for sequence match
// for shortcut in self.global_shortcuts.values() {
// if shortcut.combination == *combination && shortcut.enabled {
// return Some(shortcut);
// }
// }
//
// for shortcuts in self.context_shortcuts.values() {
// for shortcut in shortcuts.values() {
// if shortcut.combination == *combination && shortcut.enabled {
// return Some(shortcut);
// }
// }
// }
//
// None
// }
// }
//
// impl ShortcutUsageTracker {
// fn update_usage(&mut self, shortcut_id: &str) {
// let usage = self.usage_stats.entry(shortcut_id.to_string()).or_default();
// usage.count += 1;
// usage.last_used = Instant::now();
// }
// }
//
// impl ShortcutConflictResolver {
// fn check_conflicts(&self, combination: &KeyCombination) -> Vec<ShortcutConflict> {
// This would check for conflicts and return them
// Vec::new() // Simplified for now
// }
//
// fn resolve_conflicts(&mut self, shortcut: &Shortcut, conflicts: Vec<ShortcutConflict>) ->
// Result<()> { This would resolve conflicts based on strategy
// Ok(())
// }
// }
//
// impl FocusManager {
// fn new() -> Self {
// Self {
// current_focus: None,
// focus_history: VecDeque::new(),
// focus_tree: FocusTree::default(),
// tab_order: Vec::new(),
// focus_policies: HashMap::new(),
// }
// }
//
// fn navigate_next_tab(&mut self) -> Result<()> {
// if !self.tab_order.is_empty() {
// let current_index = self.current_focus.as_ref()
// .and_then(|focus| self.tab_order.iter().position(|t| t == focus))
// .unwrap_or(0);
//
// let next_index = (current_index + 1) % self.tab_order.len();
// let next_target = self.tab_order[next_index].clone();
//
// This would be integrated with the main focus setting
// info!("Navigating to next tab: {:?}", next_target);
// }
// Ok(())
// }
//
// fn navigate_previous_tab(&mut self) -> Result<()> {
// if !self.tab_order.is_empty() {
// let current_index = self.current_focus.as_ref()
// .and_then(|focus| self.tab_order.iter().position(|t| t == focus))
// .unwrap_or(0);
//
// let prev_index = if current_index == 0 {
// self.tab_order.len() - 1
// } else {
// current_index - 1
// };
// let prev_target = self.tab_order[prev_index].clone();
//
// info!("Navigating to previous tab: {:?}", prev_target);
// }
// Ok(())
// }
//
// fn focus_next(&mut self) -> Result<()> {
// Navigate to next focusable element in tree
// debug!("Focusing next element");
// Ok(())
// }
//
// fn focus_previous(&mut self) -> Result<()> {
// Navigate to previous focusable element in tree
// debug!("Focusing previous element");
// Ok(())
// }
// }
//
// impl KeyRepeatHandler {
// fn new() -> Self {
// Self {
// initial_delay: Duration::from_millis(500),
// repeat_rate: Duration::from_millis(50),
// active_repeats: HashMap::new(),
// }
// }
//
// fn start_repeat(&mut self, key: KeyInput) {
// let repeat = KeyRepeat {
// key,
// start_time: Instant::now(),
// last_repeat: Instant::now(),
// repeat_count: 0,
// };
// self.active_repeats.insert(key, repeat);
// }
//
// fn stop_repeat(&mut self, key: &KeyInput) {
// self.active_repeats.remove(key);
// }
// }
//
// impl KeySequenceDetector {
// fn process_key(&mut self, key: &KeyInput) -> Option<KeyCombination> {
// This would process key sequences and return completed combinations
// None // Simplified for now
// }
// }
//
// impl Default for InputIntegration {
// fn default() -> Self {
// Self::new()
// }
// }
//
// #[cfg(test)]
// mod tests {
// use super::*;
//
// #[test]
// fn test_key_input_creation() {
// let key = KeyInput {
// code: KeyCode::Char('a'),
// modifiers: KeyModifiers::CONTROL,
// };
// assert_eq!(key.code, KeyCode::Char('a'));
// assert_eq!(key.modifiers, KeyModifiers::CONTROL);
// }
//
// #[test]
// fn test_position_creation() {
// let pos = Position { x: 10, y: 20 };
// assert_eq!(pos.x, 10);
// assert_eq!(pos.y, 20);
// }
//
// #[test]
// fn test_focus_manager() {
// let mut focus_manager = FocusManager::new();
// assert!(focus_manager.current_focus.is_none());
//
// Test would be more comprehensive with actual focus setting
// }
//
// #[test]
// fn test_click_detection() {
// let mut detector = ClickDetector::new();
// let click = detector.register_click(
// MouseButton::Left,
// Position { x: 10, y: 10 },
// Instant::now()
// );
// assert_eq!(click.count, 1);
// }
// }
//
//! Native Input Integration for OpenAgent Terminal
//!
//! This module provides immediate keyboard and mouse input handling for command blocks,
//! tabs, and splits with no lazy event processing or deferred operations.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use anyhow::Result;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::blocks_v2::BlockId;
use crate::workspace::split_manager::PaneId;
use crate::workspace::TabId;

/// Native input manager for immediate input processing
pub struct NativeInput {
    /// Keyboard state for immediate key handling
    keyboard_state: KeyboardState,

    /// Mouse state for immediate mouse handling
    mouse_state: MouseState,

    /// Input event callbacks for immediate responses
    event_callbacks: Vec<Box<dyn Fn(&InputEvent) + Send + Sync>>,

    /// Hotkey bindings for immediate activation
    hotkey_bindings: HashMap<HotkeyCombo, InputAction>,

    /// Mouse gesture recognizer
    gesture_recognizer: GestureRecognizer,

    /// Focus management for immediate focus changes
    focus_manager: FocusManager,

    /// Input context for context-sensitive shortcuts
    input_context: InputContext,
}

/// Input events for immediate processing
#[derive(Debug, Clone)]
pub enum InputEvent {
    // Keyboard events
    KeyPressed {
        key: Key,
        modifiers: ModifiersState,
        context: InputContext,
    },
    KeyReleased {
        key: Key,
        modifiers: ModifiersState,
    },
    HotkeyTriggered {
        hotkey: HotkeyCombo,
        action: InputAction,
    },

    // Mouse events
    MousePressed {
        button: MouseButton,
        position: (f64, f64),
        context: MouseContext,
    },
    MouseReleased {
        button: MouseButton,
        position: (f64, f64),
    },
    MouseMoved {
        position: (f64, f64),
        delta: (f64, f64),
    },
    MouseWheel {
        delta: (f64, f64),
        position: (f64, f64),
    },

    // Gesture events
    GestureStarted {
        gesture: Gesture,
        position: (f64, f64),
    },
    GestureUpdated {
        gesture: Gesture,
        position: (f64, f64),
        progress: f32,
    },
    GestureCompleted {
        gesture: Gesture,
        position: (f64, f64),
    },
    GestureCancelled {
        gesture: Gesture,
    },

    // Focus events
    FocusChanged {
        from: Option<FocusTarget>,
        to: FocusTarget,
    },
    FocusLost,
}

/// Input actions for immediate execution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAction {
    // Block actions
    CreateBlock,
    DeleteBlock(BlockId),
    ToggleBlockCollapse(BlockId),
    FocusBlock(BlockId),
    SearchBlocks,

    // Tab actions
    CreateTab,
    CloseTab(Option<TabId>),
    SwitchTab(TabDirection),
    MoveTab(TabId, i32),
    RenameTab(TabId),

    // Split actions
    SplitHorizontal,
    SplitVertical,
    ClosePane(Option<PaneId>),
    FocusPane(PaneDirection),
    ResizePane(ResizeDirection, i32),
    ToggleZoom,

    // Application actions
    ToggleSearch,
    ShowHelp,
    ShowSettings,
    Quit,

    // Custom actions
    Custom(String),
}

/// Hotkey combinations for immediate recognition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeyCombo {
    pub modifiers: ModifiersState,
    pub key: Key,
    pub context: Option<InputContext>,
}

/// Input contexts for context-sensitive shortcuts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputContext {
    Global,
    Terminal,
    BlockSearch,
    TabBar,
    SplitPane,
    CommandBlock,
    Settings,
}

/// Mouse contexts for context-sensitive actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseContext {
    Terminal,
    TabBar(Option<TabId>),
    BlockArea(Option<BlockId>),
    SplitDivider(PaneId),
    ScrollBar,
    Unknown,
}

/// Focus targets for immediate focus management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Terminal,
    Tab(TabId),
    Block(BlockId),
    Pane(PaneId),
    SearchBox,
    Settings,
}

/// Tab navigation directions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabDirection {
    Next,
    Previous,
    First,
    Last,
    Index(usize),
}

/// Pane navigation directions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneDirection {
    Left,
    Right,
    Up,
    Down,
    Next,
    Previous,
}

/// Resize directions for immediate pane resizing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Mouse gestures for immediate recognition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    None,
    Drag,
    RightClickDrag,
    DoubleClick,
    TripleClick,
    Swipe(SwipeDirection),
    Pinch,
    TwoFingerScroll,
}

/// Swipe directions for gesture recognition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Keyboard state for immediate key tracking
#[derive(Debug, Default)]
pub struct KeyboardState {
    pub pressed_keys: HashSet<Key>,
    pub modifiers: ModifiersState,
    pub last_key_time: Option<Instant>,
    pub repeat_key: Option<Key>,
    pub repeat_count: usize,
}

/// Mouse state for immediate mouse tracking
#[derive(Debug, Default)]
pub struct MouseState {
    pub position: (f64, f64),
    pub pressed_buttons: HashSet<MouseButton>,
    pub last_click_time: Option<Instant>,
    pub last_click_position: (f64, f64),
    pub click_count: usize,
    pub drag_start: Option<(f64, f64)>,
    pub drag_threshold: f64,
}

/// Gesture recognizer for immediate gesture detection
#[derive(Debug)]
pub struct GestureRecognizer {
    pub active_gesture: Gesture,
    pub gesture_start_time: Option<Instant>,
    pub gesture_start_position: (f64, f64),
    pub gesture_current_position: (f64, f64),
    pub gesture_threshold: f64,
    pub double_click_time: Duration,
    pub gesture_timeout: Duration,
}

/// Focus manager for immediate focus handling
#[derive(Debug)]
pub struct FocusManager {
    pub current_focus: Option<FocusTarget>,
    pub previous_focus: Option<FocusTarget>,
    pub focus_history: Vec<FocusTarget>,
    pub max_history: usize,
}

impl NativeInput {
    /// Create new native input manager with immediate capabilities
    pub fn new() -> Self {
        let mut input = Self {
            keyboard_state: KeyboardState::default(),
            mouse_state: MouseState {
                drag_threshold: 5.0, // 5 pixels
                ..Default::default()
            },
            event_callbacks: Vec::new(),
            hotkey_bindings: HashMap::new(),
            gesture_recognizer: GestureRecognizer {
                active_gesture: Gesture::None,
                gesture_start_time: None,
                gesture_start_position: (0.0, 0.0),
                gesture_current_position: (0.0, 0.0),
                gesture_threshold: 10.0, // 10 pixels
                double_click_time: Duration::from_millis(500),
                gesture_timeout: Duration::from_secs(2),
            },
            focus_manager: FocusManager {
                current_focus: None,
                previous_focus: None,
                focus_history: Vec::new(),
                max_history: 20,
            },
            input_context: InputContext::Global,
        };

        // Register default hotkeys immediately
        input.register_default_hotkeys();

        input
    }

    /// Register input event callback for immediate responses
    pub fn register_event_callback<F>(&mut self, callback: F)
    where
        F: Fn(&InputEvent) + Send + Sync + 'static,
    {
        self.event_callbacks.push(Box::new(callback));
    }

    /// Emit input event immediately
    fn emit_event(&self, event: InputEvent) {
        for callback in &self.event_callbacks {
            callback(&event);
        }
    }

    /// Handle keyboard input immediately
    pub fn handle_keyboard_input(
        &mut self,
        event: KeyEvent,
        modifiers: ModifiersState,
    ) -> Result<()> {
        self.keyboard_state.modifiers = modifiers;

        let now = Instant::now();

        match event.state {
            ElementState::Pressed => {
                self.keyboard_state
                    .pressed_keys
                    .insert(event.logical_key.clone());

                // Check for key repeat
                if Some(&event.logical_key) == self.keyboard_state.repeat_key.as_ref() {
                    self.keyboard_state.repeat_count += 1;
                } else {
                    self.keyboard_state.repeat_key = Some(event.logical_key.clone());
                    self.keyboard_state.repeat_count = 1;
                }

                self.keyboard_state.last_key_time = Some(now);

                // Check for hotkey matches immediately
                let hotkey = HotkeyCombo {
                    modifiers,
                    key: event.logical_key.clone(),
                    context: Some(self.input_context),
                };

                if let Some(action) = self.hotkey_bindings.get(&hotkey) {
                    self.emit_event(InputEvent::HotkeyTriggered {
                        hotkey,
                        action: action.clone(),
                    });

                    // Execute action immediately
                    self.execute_action(action.clone())?;
                } else {
                    // Emit key pressed event
                    self.emit_event(InputEvent::KeyPressed {
                        key: event.logical_key,
                        modifiers,
                        context: self.input_context,
                    });
                }
            }
            ElementState::Released => {
                self.keyboard_state.pressed_keys.remove(&event.logical_key);

                // Reset repeat state
                if Some(&event.logical_key) == self.keyboard_state.repeat_key.as_ref() {
                    self.keyboard_state.repeat_key = None;
                    self.keyboard_state.repeat_count = 0;
                }

                self.emit_event(InputEvent::KeyReleased {
                    key: event.logical_key,
                    modifiers,
                });
            }
        }

        Ok(())
    }

    /// Handle mouse input immediately
    pub fn handle_mouse_input(
        &mut self,
        button: MouseButton,
        state: ElementState,
        position: (f64, f64),
    ) -> Result<()> {
        self.mouse_state.position = position;
        let now = Instant::now();

        match state {
            ElementState::Pressed => {
                self.mouse_state.pressed_buttons.insert(button);

                // Detect multi-clicks immediately
                if let Some(last_click) = self.mouse_state.last_click_time {
                    let time_diff = now.duration_since(last_click);
                    let pos_diff = (
                        (position.0 - self.mouse_state.last_click_position.0).abs(),
                        (position.1 - self.mouse_state.last_click_position.1).abs(),
                    );

                    if time_diff < self.gesture_recognizer.double_click_time
                        && pos_diff.0 < self.gesture_recognizer.gesture_threshold
                        && pos_diff.1 < self.gesture_recognizer.gesture_threshold
                    {
                        self.mouse_state.click_count += 1;
                    } else {
                        self.mouse_state.click_count = 1;
                    }
                } else {
                    self.mouse_state.click_count = 1;
                }

                self.mouse_state.last_click_time = Some(now);
                self.mouse_state.last_click_position = position;

                // Start potential drag
                if button == MouseButton::Left {
                    self.mouse_state.drag_start = Some(position);
                }

                // Detect gesture start immediately
                if self.mouse_state.click_count == 2 {
                    self.start_gesture(Gesture::DoubleClick, position);
                } else if self.mouse_state.click_count == 3 {
                    self.start_gesture(Gesture::TripleClick, position);
                }

                // Determine mouse context
                let context = self.determine_mouse_context(position);

                self.emit_event(InputEvent::MousePressed {
                    button,
                    position,
                    context,
                });
            }
            ElementState::Released => {
                self.mouse_state.pressed_buttons.remove(&button);

                // End drag if active
                if button == MouseButton::Left {
                    if let Some(drag_start) = self.mouse_state.drag_start.take() {
                        let drag_distance = (
                            (position.0 - drag_start.0).abs(),
                            (position.1 - drag_start.1).abs(),
                        );

                        if drag_distance.0 > self.mouse_state.drag_threshold
                            || drag_distance.1 > self.mouse_state.drag_threshold
                        {
                            self.complete_gesture(Gesture::Drag, position);
                        }
                    }
                }

                self.emit_event(InputEvent::MouseReleased { button, position });
            }
        }

        Ok(())
    }

    /// Handle mouse movement immediately
    pub fn handle_mouse_move(&mut self, position: (f64, f64)) -> Result<()> {
        let old_position = self.mouse_state.position;
        self.mouse_state.position = position;

        let delta = (position.0 - old_position.0, position.1 - old_position.1);

        // Update active drag gesture
        if let Some(drag_start) = self.mouse_state.drag_start {
            let drag_distance = (
                (position.0 - drag_start.0).abs(),
                (position.1 - drag_start.1).abs(),
            );

            if drag_distance.0 > self.mouse_state.drag_threshold
                || drag_distance.1 > self.mouse_state.drag_threshold
            {
                if self.gesture_recognizer.active_gesture == Gesture::None {
                    self.start_gesture(Gesture::Drag, drag_start);
                }

                self.update_gesture(Gesture::Drag, position);
            }
        }

        self.emit_event(InputEvent::MouseMoved { position, delta });

        Ok(())
    }

    /// Handle mouse wheel immediately
    pub fn handle_mouse_wheel(&mut self, delta: (f64, f64), position: (f64, f64)) -> Result<()> {
        // Check for gesture patterns
        if delta.0.abs() > delta.1.abs() {
            // Horizontal scroll - potential swipe
            if delta.0 > 0.0 {
                self.start_gesture(Gesture::Swipe(SwipeDirection::Right), position);
            } else {
                self.start_gesture(Gesture::Swipe(SwipeDirection::Left), position);
            }
        } else {
            // Vertical scroll
            if delta.1 > 0.0 {
                self.start_gesture(Gesture::Swipe(SwipeDirection::Up), position);
            } else {
                self.start_gesture(Gesture::Swipe(SwipeDirection::Down), position);
            }
        }

        self.emit_event(InputEvent::MouseWheel { delta, position });

        Ok(())
    }

    /// Start gesture immediately
    fn start_gesture(&mut self, gesture: Gesture, position: (f64, f64)) {
        self.gesture_recognizer.active_gesture = gesture;
        self.gesture_recognizer.gesture_start_time = Some(Instant::now());
        self.gesture_recognizer.gesture_start_position = position;
        self.gesture_recognizer.gesture_current_position = position;

        self.emit_event(InputEvent::GestureStarted { gesture, position });
    }

    /// Update gesture immediately
    fn update_gesture(&mut self, gesture: Gesture, position: (f64, f64)) {
        if self.gesture_recognizer.active_gesture == gesture {
            self.gesture_recognizer.gesture_current_position = position;

            // Calculate progress
            let start_pos = self.gesture_recognizer.gesture_start_position;
            let distance =
                ((position.0 - start_pos.0).powi(2) + (position.1 - start_pos.1).powi(2)).sqrt();
            let progress = ((distance / 100.0).clamp(0.0, 1.0)) as f32; // Normalize to 0-1

            self.emit_event(InputEvent::GestureUpdated {
                gesture,
                position,
                progress,
            });
        }
    }

    /// Complete gesture immediately
    fn complete_gesture(&mut self, gesture: Gesture, position: (f64, f64)) {
        if self.gesture_recognizer.active_gesture == gesture {
            self.gesture_recognizer.active_gesture = Gesture::None;
            self.gesture_recognizer.gesture_start_time = None;

            self.emit_event(InputEvent::GestureCompleted { gesture, position });
        }
    }

    /// Execute input action immediately
    fn execute_action(&mut self, action: InputAction) -> Result<()> {
        match action {
            InputAction::CreateBlock => {
                // Implementation would trigger block creation
            }
            InputAction::CreateTab => {
                // Implementation would trigger tab creation
            }
            InputAction::SwitchTab(direction) => {
                // Implementation would switch tabs immediately
            }
            InputAction::SplitHorizontal => {
                // Implementation would create horizontal split
            }
            InputAction::SplitVertical => {
                // Implementation would create vertical split
            }
            InputAction::ToggleSearch => {
                // Implementation would toggle search immediately
                self.set_input_context(InputContext::BlockSearch);
            }
            InputAction::Quit => {
                // Implementation would quit application
            }
            _ => {
                // Handle other actions
            }
        }

        Ok(())
    }

    /// Register hotkey binding immediately
    pub fn register_hotkey(&mut self, combo: HotkeyCombo, action: InputAction) {
        self.hotkey_bindings.insert(combo, action);
    }

    /// Register default hotkeys immediately
    fn register_default_hotkeys(&mut self) {
        // Block shortcuts
        self.register_hotkey(
            HotkeyCombo {
                modifiers: ModifiersState::CONTROL,
                key: Key::Character("b".into()),
                context: Some(InputContext::Global),
            },
            InputAction::CreateBlock,
        );

        // Tab shortcuts
        self.register_hotkey(
            HotkeyCombo {
                modifiers: ModifiersState::CONTROL,
                key: Key::Character("t".into()),
                context: Some(InputContext::Global),
            },
            InputAction::CreateTab,
        );

        self.register_hotkey(
            HotkeyCombo {
                modifiers: ModifiersState::CONTROL,
                key: Key::Named(NamedKey::Tab),
                context: Some(InputContext::Global),
            },
            InputAction::SwitchTab(TabDirection::Next),
        );

        self.register_hotkey(
            HotkeyCombo {
                modifiers: ModifiersState::CONTROL | ModifiersState::SHIFT,
                key: Key::Named(NamedKey::Tab),
                context: Some(InputContext::Global),
            },
            InputAction::SwitchTab(TabDirection::Previous),
        );

        // Split shortcuts
        self.register_hotkey(
            HotkeyCombo {
                modifiers: ModifiersState::CONTROL | ModifiersState::SHIFT,
                key: Key::Character("h".into()),
                context: Some(InputContext::Global),
            },
            InputAction::SplitHorizontal,
        );

        self.register_hotkey(
            HotkeyCombo {
                modifiers: ModifiersState::CONTROL | ModifiersState::SHIFT,
                key: Key::Character("v".into()),
                context: Some(InputContext::Global),
            },
            InputAction::SplitVertical,
        );

        // Search shortcuts
        self.register_hotkey(
            HotkeyCombo {
                modifiers: ModifiersState::CONTROL,
                key: Key::Character("f".into()),
                context: Some(InputContext::Global),
            },
            InputAction::ToggleSearch,
        );

        // Application shortcuts
        self.register_hotkey(
            HotkeyCombo {
                modifiers: ModifiersState::CONTROL,
                key: Key::Character("q".into()),
                context: Some(InputContext::Global),
            },
            InputAction::Quit,
        );

        // Help shortcut
        self.register_hotkey(
            HotkeyCombo {
                modifiers: ModifiersState::empty(),
                key: Key::Named(NamedKey::F1),
                context: Some(InputContext::Global),
            },
            InputAction::ShowHelp,
        );
    }

    /// Set input context immediately
    pub fn set_input_context(&mut self, context: InputContext) {
        self.input_context = context;
    }

    /// Set focus immediately
    pub fn set_focus(&mut self, target: FocusTarget) {
        let old_focus = self.focus_manager.current_focus;

        // Update focus history
        if let Some(current) = self.focus_manager.current_focus {
            self.focus_manager.previous_focus = Some(current);
            self.focus_manager.focus_history.push(current);

            // Limit history size
            if self.focus_manager.focus_history.len() > self.focus_manager.max_history {
                self.focus_manager.focus_history.remove(0);
            }
        }

        self.focus_manager.current_focus = Some(target);

        // Update input context based on focus
        self.input_context = match target {
            FocusTarget::Terminal => InputContext::Terminal,
            FocusTarget::Tab(_) => InputContext::TabBar,
            FocusTarget::Block(_) => InputContext::CommandBlock,
            FocusTarget::Pane(_) => InputContext::SplitPane,
            FocusTarget::SearchBox => InputContext::BlockSearch,
            FocusTarget::Settings => InputContext::Settings,
        };

        self.emit_event(InputEvent::FocusChanged {
            from: old_focus,
            to: target,
        });
    }

    /// Get current focus
    pub fn get_focus(&self) -> Option<FocusTarget> {
        self.focus_manager.current_focus
    }

    /// Determine mouse context from position
    fn determine_mouse_context(&self, position: (f64, f64)) -> MouseContext {
        // This would be implemented to check hit-testing against UI elements
        // For now, return a default context
        MouseContext::Terminal
    }

    /// Check if key is currently pressed
    pub fn is_key_pressed(&self, key: &Key) -> bool {
        self.keyboard_state.pressed_keys.contains(key)
    }

    /// Check if mouse button is currently pressed
    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_state.pressed_buttons.contains(&button)
    }

    /// Get current mouse position
    pub fn get_mouse_position(&self) -> (f64, f64) {
        self.mouse_state.position
    }

    /// Get current modifiers state
    pub fn get_modifiers(&self) -> ModifiersState {
        self.keyboard_state.modifiers
    }
}

impl Default for NativeInput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_input_creation() {
        let input = NativeInput::new();
        assert_eq!(input.input_context, InputContext::Global);
        assert!(input.hotkey_bindings.len() > 0); // Should have default hotkeys
    }

    #[test]
    fn test_hotkey_registration() {
        let mut input = NativeInput::new();
        let combo = HotkeyCombo {
            modifiers: ModifiersState::CONTROL,
            key: Key::Character("x".into()),
            context: Some(InputContext::Global),
        };
        let action = InputAction::Custom("test".to_string());

        input.register_hotkey(combo.clone(), action.clone());
        assert_eq!(input.hotkey_bindings.get(&combo), Some(&action));
    }

    #[test]
    fn test_focus_management() {
        let mut input = NativeInput::new();

        // Initially no focus
        assert_eq!(input.get_focus(), None);

        // Set focus to tab
        let tab_target = FocusTarget::Tab(TabId(1));
        input.set_focus(tab_target);

        assert_eq!(input.get_focus(), Some(tab_target));
        assert_eq!(input.input_context, InputContext::TabBar);
    }

    #[test]
    fn test_gesture_recognition() {
        let mut input = NativeInput::new();

        // Start drag gesture
        input.start_gesture(Gesture::Drag, (10.0, 10.0));
        assert_eq!(input.gesture_recognizer.active_gesture, Gesture::Drag);

        // Update gesture
        input.update_gesture(Gesture::Drag, (20.0, 20.0));
        assert_eq!(
            input.gesture_recognizer.gesture_current_position,
            (20.0, 20.0)
        );

        // Complete gesture
        input.complete_gesture(Gesture::Drag, (30.0, 30.0));
        assert_eq!(input.gesture_recognizer.active_gesture, Gesture::None);
    }
}

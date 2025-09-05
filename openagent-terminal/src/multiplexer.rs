//! Terminal multiplexing system for OpenAgent Terminal
//!
//! This module provides the core multiplexing functionality that allows multiple
//! terminal instances (panes) to be managed within a single window.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use openagent_terminal_core::event::Event as TerminalEvent;
use openagent_terminal_core::event_loop::{EventLoop as PtyEventLoop, Msg, Notifier};
use openagent_terminal_core::grid::Dimensions;
use openagent_terminal_core::sync::FairMutex;
use openagent_terminal_core::term::{Config as TermConfig, Term};
use openagent_terminal_core::tty::{self, Pty};

use crate::config::UiConfig;
use crate::display::SizeInfo;
use crate::event::EventProxy;
use crate::workspace::PaneId;
use crate::workspace::split_manager::SplitLayout;

/// Represents a single terminal pane with its associated PTY and state
pub struct Pane {
    /// The terminal emulator for this pane
    pub terminal: Arc<FairMutex<Term<EventProxy>>>,
    
    /// The PTY process for this pane
    pub pty_notifier: Notifier,
    
    /// Working directory for this pane
    pub working_directory: PathBuf,
    
    /// Title of this pane
    pub title: String,
    
    /// Whether this pane is currently focused
    pub is_focused: bool,
    
    /// Last known size of this pane
    pub size: SizeInfo,
    
    /// Process ID of the shell running in this pane
    #[cfg(not(windows))]
    pub shell_pid: u32,
    
    /// Master file descriptor for the PTY
    #[cfg(not(windows))]
    pub master_fd: std::os::unix::io::RawFd,
}

impl Pane {
    /// Create a new pane with the given configuration
    pub fn new(
        pane_id: PaneId,
        config: &UiConfig,
        size_info: SizeInfo,
        working_directory: Option<PathBuf>,
        title: String,
        event_proxy: EventProxy,
        window_id: winit::window::WindowId,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Create terminal configuration
        let term_config = config.term_options();
        
        // Create the terminal instance
        let terminal = Term::new(term_config, &size_info, event_proxy.clone());
        let terminal = Arc::new(FairMutex::new(terminal));
        
        // Create PTY configuration
        let mut pty_config = config.pty_config();
        if let Some(ref wd) = working_directory {
            pty_config.working_directory = Some(wd.clone());
        }
        
        // Create the PTY
        let pty = tty::new(&pty_config, size_info.into(), window_id.into())?;
        
        #[cfg(not(windows))]
        let master_fd = {
            use std::os::unix::io::AsRawFd;
            pty.file().as_raw_fd()
        };
        
        #[cfg(not(windows))]
        let shell_pid = pty.child().id();
        
        // Create the PTY event loop
        let event_loop = PtyEventLoop::new(
            Arc::clone(&terminal),
            event_proxy.clone(),
            pty,
            pty_config.drain_on_exit,
            config.debug.ref_test,
        )?;
        
        let pty_notifier = Notifier(event_loop.channel());
        
        // Start the I/O thread
        let _io_thread = event_loop.spawn();
        
        let working_directory = working_directory.unwrap_or_else(|| {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
        });
        
        Ok(Pane {
            terminal,
            pty_notifier,
            working_directory,
            title,
            is_focused: false,
            size: size_info,
            #[cfg(not(windows))]
            shell_pid,
            #[cfg(not(windows))]
            master_fd,
        })
    }
    
    /// Resize this pane to the new size
    pub fn resize(&mut self, size_info: SizeInfo) {
        self.size = size_info;
        self.terminal.lock().resize(size_info);
        
        // Send resize notification to PTY
        let _ = self.pty_notifier.0.send(Msg::Resize(size_info.into()));
    }
    
    /// Set focus state for this pane
    pub fn set_focused(&mut self, focused: bool) {
        if self.is_focused != focused {
            self.is_focused = focused;
            // Focus notifications are no longer sent via PTY Notifier.
        }
    }
    
    /// Write input to this pane's PTY
    pub fn write_input(&mut self, data: &[u8]) {
        let _ = self.pty_notifier.0.send(Msg::Input(data.to_vec().into()));
    }
    
    /// Shutdown this pane
    pub fn shutdown(&mut self) {
        let _ = self.pty_notifier.0.send(Msg::Shutdown);
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Manages multiple terminal panes within a window
pub struct PaneManager {
    /// Map of pane ID to pane instance
    panes: HashMap<PaneId, Pane>,
    
    /// Counter for generating unique pane IDs
    next_pane_id: usize,
    
    /// Currently focused pane
    focused_pane: Option<PaneId>,
    
    /// Configuration reference
    config: std::rc::Rc<UiConfig>,
    
    /// Default size for new panes
    default_size: SizeInfo,
    
    /// Window ID for PTY creation
    window_id: winit::window::WindowId,
    
    /// Event proxy for terminal events
    event_proxy: EventProxy,
}

impl PaneManager {
    /// Create a new pane manager
    pub fn new(
        config: std::rc::Rc<UiConfig>,
        default_size: SizeInfo,
        window_id: winit::window::WindowId,
        event_proxy: EventProxy,
    ) -> Self {
        Self {
            panes: HashMap::new(),
            next_pane_id: 1,
            focused_pane: None,
            config,
            default_size,
            window_id,
            event_proxy,
        }
    }
    
    /// Get terminal Arc for a given pane
    pub fn get_terminal(&self, pane_id: PaneId) -> Option<Arc<FairMutex<Term<EventProxy>>>> {
        self.panes.get(&pane_id).map(|p| p.terminal.clone())
    }
    
    /// Create a new pane with an auto-generated ID and return its ID
    pub fn create_pane(
        &mut self,
        working_directory: Option<PathBuf>,
        title: String,
        size_info: Option<SizeInfo>,
    ) -> Result<PaneId, Box<dyn std::error::Error>> {
        let pane_id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;
        
        let size = size_info.unwrap_or(self.default_size);
        
        let pane = Pane::new(
            pane_id,
            &self.config,
            size,
            working_directory,
            title,
            self.event_proxy.clone(),
            self.window_id,
        )?;
        
        self.panes.insert(pane_id, pane);
        
        // If this is the first pane, focus it
        if self.focused_pane.is_none() {
            self.focus_pane(pane_id);
        }
        
        Ok(pane_id)
    }

    /// Create a new pane with a specific PaneId. If the ID already exists this returns Ok(existing_id).
    pub fn create_pane_with_id(
        &mut self,
        pane_id: PaneId,
        working_directory: Option<PathBuf>,
        title: String,
        size_info: Option<SizeInfo>,
    ) -> Result<PaneId, Box<dyn std::error::Error>> {
        if self.panes.contains_key(&pane_id) {
            return Ok(pane_id);
        }
        let size = size_info.unwrap_or(self.default_size);
        let pane = Pane::new(
            pane_id,
            &self.config,
            size,
            working_directory,
            title,
            self.event_proxy.clone(),
            self.window_id,
        )?;
        self.panes.insert(pane_id, pane);
        if self.focused_pane.is_none() {
            self.focus_pane(pane_id);
        }
        Ok(pane_id)
    }
    
    /// Remove a pane and clean up its resources
    pub fn remove_pane(&mut self, pane_id: PaneId) -> bool {
        if let Some(mut pane) = self.panes.remove(&pane_id) {
            pane.shutdown();
            
            // If this was the focused pane, focus another one
            if self.focused_pane == Some(pane_id) {
                self.focused_pane = None;
                
                // Focus the first available pane
                if let Some((&new_focus, _)) = self.panes.iter().next() {
                    self.focus_pane(new_focus);
                }
            }
            
            true
        } else {
            false
        }
    }
    
    /// Focus a specific pane
    pub fn focus_pane(&mut self, pane_id: PaneId) -> bool {
        if !self.panes.contains_key(&pane_id) {
            return false;
        }
        
        // Unfocus current pane
        if let Some(current_id) = self.focused_pane {
            if let Some(current_pane) = self.panes.get_mut(&current_id) {
                current_pane.set_focused(false);
            }
        }
        
        // Focus new pane
        if let Some(new_pane) = self.panes.get_mut(&pane_id) {
            new_pane.set_focused(true);
            self.focused_pane = Some(pane_id);
            return true;
        }
        
        false
    }
    
    /// Get the currently focused pane
    pub fn focused_pane(&self) -> Option<PaneId> {
        self.focused_pane
    }
    
    /// Get a reference to a pane
    pub fn get_pane(&self, pane_id: PaneId) -> Option<&Pane> {
        self.panes.get(&pane_id)
    }
    
    /// Get a mutable reference to a pane
    pub fn get_pane_mut(&mut self, pane_id: PaneId) -> Option<&mut Pane> {
        self.panes.get_mut(&pane_id)
    }
    
    /// Get all pane IDs
    pub fn pane_ids(&self) -> impl Iterator<Item = PaneId> + '_ {
        self.panes.keys().copied()
    }
    
    /// Get the number of panes
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }
    
    /// Resize a specific pane
    pub fn resize_pane(&mut self, pane_id: PaneId, size_info: SizeInfo) -> bool {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.resize(size_info);
            true
        } else {
            false
        }
    }
    
    /// Write input to the currently focused pane
    pub fn write_to_focused(&mut self, data: &[u8]) -> bool {
        if let Some(pane_id) = self.focused_pane {
            if let Some(pane) = self.panes.get_mut(&pane_id) {
                pane.write_input(data);
                return true;
            }
        }
        false
    }
    
    /// Write input to a specific pane
    pub fn write_to_pane(&mut self, pane_id: PaneId, data: &[u8]) -> bool {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.write_input(data);
            true
        } else {
            false
        }
    }
    
    /// Update the default size for new panes
    pub fn update_default_size(&mut self, size_info: SizeInfo) {
        self.default_size = size_info;
    }
    
    /// Get the terminals for all panes (for rendering)
    pub fn get_all_terminals(&self) -> HashMap<PaneId, Arc<FairMutex<Term<EventProxy>>>> {
        self.panes
            .iter()
            .map(|(&id, pane)| (id, Arc::clone(&pane.terminal)))
            .collect()
    }
    
    /// Shutdown all panes
    pub fn shutdown_all(&mut self) {
        for (_, pane) in self.panes.iter_mut() {
            pane.shutdown();
        }
        self.panes.clear();
        self.focused_pane = None;
    }
}

impl Drop for PaneManager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}

/// Compute pane rectangles for rendering based on split layout
pub fn compute_pane_rectangles(
    layout: &SplitLayout,
    container: crate::workspace::split_manager::PaneRect,
) -> HashMap<PaneId, crate::workspace::split_manager::PaneRect> {
    let mut rectangles = HashMap::new();
    compute_pane_rectangles_recursive(layout, container, &mut rectangles);
    rectangles
}

fn compute_pane_rectangles_recursive(
    layout: &SplitLayout,
    container: crate::workspace::split_manager::PaneRect,
    rectangles: &mut HashMap<PaneId, crate::workspace::split_manager::PaneRect>,
) {
    match layout {
        SplitLayout::Single(pane_id) => {
            rectangles.insert(*pane_id, container);
        }
        SplitLayout::Horizontal { left, right, ratio } => {
            let (left_rect, right_rect) = container.split_horizontal(*ratio);
            compute_pane_rectangles_recursive(left, left_rect, rectangles);
            compute_pane_rectangles_recursive(right, right_rect, rectangles);
        }
        SplitLayout::Vertical { top, bottom, ratio } => {
            let (top_rect, bottom_rect) = container.split_vertical(*ratio);
            compute_pane_rectangles_recursive(top, top_rect, rectangles);
            compute_pane_rectangles_recursive(bottom, bottom_rect, rectangles);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::UiConfig;
    use crate::display::SizeInfo;
    use crate::workspace::split_manager::{SplitLayout, PaneRect};
    use std::rc::Rc;
    use winit::event_loop::EventLoop;

    fn create_test_size_info() -> SizeInfo {
        SizeInfo::new(800.0, 600.0, 10.0, 20.0, 5.0, 5.0, false)
    }

    fn create_test_event_proxy() -> EventProxy {
        let event_loop = EventLoop::with_user_event().build().unwrap();
        let proxy = event_loop.create_proxy();
        EventProxy::new(proxy, winit::window::WindowId::from(0))
    }

    #[test]
    fn test_compute_pane_rectangles() {
        let container = PaneRect::new(0.0, 0.0, 1000.0, 600.0);
        let p1 = PaneId(1);
        let p2 = PaneId(2);
        let p3 = PaneId(3);
        let layout = SplitLayout::Horizontal {
            left: Box::new(SplitLayout::Single(p1)),
            right: Box::new(SplitLayout::Vertical {
                top: Box::new(SplitLayout::Single(p2)),
                bottom: Box::new(SplitLayout::Single(p3)),
                ratio: 0.5,
            }),
            ratio: 0.5,
        };
        let rects = compute_pane_rectangles(&layout, container);
        assert_eq!(rects.len(), 3);
        let r1 = rects.get(&p1).unwrap();
        assert!((r1.width - 500.0).abs() < 1e-3);
        assert!((r1.height - 600.0).abs() < 1e-3);
        let r2 = rects.get(&p2).unwrap();
        assert!((r2.x - 500.0).abs() < 1e-3);
        assert!((r2.width - 500.0).abs() < 1e-3);
        assert!((r2.height - 300.0).abs() < 1e-3);
        let r3 = rects.get(&p3).unwrap();
        assert!((r3.y - 300.0).abs() < 1e-3);
        assert!((r3.width - 500.0).abs() < 1e-3);
    }

    #[test]
    fn test_pane_creation() {
        let config = Rc::new(UiConfig::default());
        let size_info = create_test_size_info();
        let window_id = winit::window::WindowId::from(0);
        let event_proxy = create_test_event_proxy();
        
        let mut manager = PaneManager::new(config, size_info, window_id, event_proxy);
        
        let pane_id = manager
            .create_pane(None, "Test Pane".to_string(), None)
            .expect("Failed to create pane");
        
        assert_eq!(manager.pane_count(), 1);
        assert_eq!(manager.focused_pane(), Some(pane_id));
        assert!(manager.get_pane(pane_id).is_some());
    }

    #[test]
    fn test_pane_focus_switching() {
        let config = Rc::new(UiConfig::default());
        let size_info = create_test_size_info();
        let window_id = winit::window::WindowId::from(0);
        let event_proxy = create_test_event_proxy();
        
        let mut manager = PaneManager::new(config, size_info, window_id, event_proxy);
        
        let pane1 = manager
            .create_pane(None, "Pane 1".to_string(), None)
            .expect("Failed to create pane 1");
        let pane2 = manager
            .create_pane(None, "Pane 2".to_string(), None)
            .expect("Failed to create pane 2");
        
        assert_eq!(manager.focused_pane(), Some(pane1));
        
        assert!(manager.focus_pane(pane2));
        assert_eq!(manager.focused_pane(), Some(pane2));
        
        assert!(manager.get_pane(pane1).map(|p| !p.is_focused).unwrap_or(false));
        assert!(manager.get_pane(pane2).map(|p| p.is_focused).unwrap_or(false));
    }

    #[test]
    fn test_pane_removal() {
        let config = Rc::new(UiConfig::default());
        let size_info = create_test_size_info();
        let window_id = winit::window::WindowId::from(0);
        let event_proxy = create_test_event_proxy();
        
        let mut manager = PaneManager::new(config, size_info, window_id, event_proxy);
        
        let pane1 = manager
            .create_pane(None, "Pane 1".to_string(), None)
            .expect("Failed to create pane 1");
        let pane2 = manager
            .create_pane(None, "Pane 2".to_string(), None)
            .expect("Failed to create pane 2");
        
        assert_eq!(manager.pane_count(), 2);
        
        assert!(manager.remove_pane(pane1));
        assert_eq!(manager.pane_count(), 1);
        assert!(manager.get_pane(pane1).is_none());
        assert!(manager.get_pane(pane2).is_some());
        assert_eq!(manager.focused_pane(), Some(pane2));
    }
}

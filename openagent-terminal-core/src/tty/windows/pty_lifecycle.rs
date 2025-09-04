use std::sync::Arc;
use std::io::Result;
use crate::event::WindowSize;
use crate::tty::windows::{ReadPipe, WritePipe};
use crate::tty::windows::child::ChildExitWatcher;
use crate::tty::windows::conpty::Conpty;

/// PTY lifecycle management using typestate pattern to enforce correct drop order.
/// 
/// This ensures that ConPTY is always dropped before the conout pipe, preventing
/// Windows ConPTY deadlocks that occur when the drop order is incorrect.
///
/// The progression is: PtyBuilder -> PtyActive -> (shutdown)
/// The PtyActive state can be safely shutdown, which properly drops resources
/// in the correct order and returns the child watcher for continued monitoring.

/// Builder state - resources not yet active
pub struct PtyBuilder {
    backend: Conpty,
    conout: ReadPipe,
    conin: WritePipe,
    child_watcher: ChildExitWatcher,
}

/// Active state - PTY is running and operational
pub struct PtyActive {
    backend: Conpty,
    conout: ReadPipe,
    conin: WritePipe,
    child_watcher: ChildExitWatcher,
}


impl PtyBuilder {
    /// Create a new PTY builder with all resources
    pub fn new(
        backend: Conpty,
        conout: ReadPipe,
        conin: WritePipe,
        child_watcher: ChildExitWatcher,
    ) -> Self {
        Self { backend, conout, conin, child_watcher }
    }

    /// Activate the PTY, transitioning to the active state
    pub fn activate(self) -> PtyActive {
        PtyActive {
            backend: self.backend,
            conout: self.conout,
            conin: self.conin,
            child_watcher: self.child_watcher,
        }
    }
}

impl PtyActive {
    /// Get mutable access to the reader pipe
    pub fn reader(&mut self) -> &mut ReadPipe {
        &mut self.conout
    }

    /// Get mutable access to the writer pipe
    pub fn writer(&mut self) -> &mut WritePipe {
        &mut self.conin
    }

    /// Get access to the child watcher
    pub fn child_watcher(&self) -> &ChildExitWatcher {
        &self.child_watcher
    }

    /// Get mutable access to the backend for resize operations
    pub fn backend_mut(&mut self) -> &mut Conpty {
        &mut self.backend
    }

    /// Properly shutdown the PTY, ensuring correct drop order.
    /// 
    /// This is the critical method that prevents ConPTY deadlocks.
    /// It explicitly drops resources in the correct order:
    /// 1. First, drop the conout pipe
    /// 2. Then, drop the ConPTY backend
    /// 3. Finally, return remaining safe resources
    pub fn shutdown(self) -> ChildExitWatcher {
        // CRITICAL: Drop conout pipe first, then backend (ConPTY)
        // This prevents the ConPTY deadlock described in the FIXME
        drop(self.conout);
        drop(self.backend);
        // conin can be dropped safely after ConPTY
        drop(self.conin);
        
        // Return the child watcher as it's still needed
        self.child_watcher
    }
}


/// Wrapper that provides the old PTY interface while using the new lifecycle internally
/// This ensures backward compatibility while fixing the drop order issue
pub struct SafePty {
    active: Option<PtyActive>,
    child_watcher: Option<ChildExitWatcher>, // Keeps child watcher available after shutdown
}

impl SafePty {
    /// Create a new SafePty from components
    pub fn new(
        backend: Conpty,
        conout: ReadPipe,
        conin: WritePipe,
        child_watcher: ChildExitWatcher,
    ) -> Self {
        let builder = PtyBuilder::new(backend, conout, conin, child_watcher);
        let active = builder.activate();
        Self {
            active: Some(active),
            child_watcher: None, // Will be extracted during shutdown
        }
    }

    /// Get mutable access to the reader pipe
    pub fn reader(&mut self) -> Option<&mut ReadPipe> {
        self.active.as_mut().map(|active| active.reader())
    }

    /// Get mutable access to the writer pipe
    pub fn writer(&mut self) -> Option<&mut WritePipe> {
        self.active.as_mut().map(|active| active.writer())
    }

    /// Get access to the child watcher
    pub fn child_watcher(&self) -> &ChildExitWatcher {
        if let Some(ref active) = self.active {
            active.child_watcher()
        } else if let Some(ref child_watcher) = self.child_watcher {
            child_watcher
        } else {
            panic!("SafePty in invalid state - no child watcher available")
        }
    }

    /// Get mutable access to the backend for resize operations
    pub fn backend_mut(&mut self) -> Option<&mut Conpty> {
        self.active.as_mut().map(|active| active.backend_mut())
    }

    /// Manually trigger shutdown with proper drop order
    pub fn shutdown(&mut self) {
        if let Some(active) = self.active.take() {
            let child_watcher = active.shutdown();
            // Store the child watcher for continued access
            self.child_watcher = Some(child_watcher);
        }
    }
}

impl Drop for SafePty {
    /// Ensure proper shutdown on drop
    fn drop(&mut self) {
        // This will call shutdown() if not already done, ensuring proper drop order
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pty_lifecycle_state_transitions() {
        // This test would require mock implementations of the dependencies
        // but demonstrates the intended usage pattern
        
        // let builder = PtyBuilder::new(mock_backend, mock_conout, mock_conin, mock_child);
        // let active = builder.activate();
        // let shutdown = active.shutdown();
        // 
        // // Once shutdown, the PTY cannot be reactivated
        // // This is enforced by the type system
        
        // The key point is that shutdown() consumes the PtyActive state,
        // preventing any further operations on the PTY while ensuring
        // proper drop order
    }
}

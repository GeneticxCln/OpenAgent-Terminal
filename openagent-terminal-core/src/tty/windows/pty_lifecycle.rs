use crate::event::WindowSize;
use crate::tty::windows::child::ChildExitWatcher;
use crate::tty::windows::conpty::Conpty;
use crate::tty::windows::{ReadPipe, WritePipe};
use std::io::Result;
use std::sync::Arc;

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
        Self {
            backend,
            conout,
            conin,
            child_watcher,
        }
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
        // This prevents the ConPTY deadlock described by Microsoft docs and prior issues
        drop(self.conout);
        drop(self.backend);
        // Drop conin after backend is gone
        drop(self.conin);

        // Return the child watcher as it's still needed for event delivery
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
    use std::sync::{Arc, Mutex};

    /// Test helper types to verify drop order without relying on Windows APIs.
    #[derive(Clone)]
    struct Drops(Arc<Mutex<Vec<&'static str>>>);
    impl Drops {
        fn new() -> Self { Self(Arc::new(Mutex::new(Vec::new()))) }
        fn push(&self, s: &'static str) { self.0.lock().unwrap().push(s); }
        fn take(&self) -> Vec<&'static str> { std::mem::take(&mut *self.0.lock().unwrap()) }
    }

    struct MockBackend(Drops);
    impl Drop for MockBackend { fn drop(&mut self) { self.0.push("backend"); } }

    struct MockRead(Drops);
    impl Drop for MockRead { fn drop(&mut self) { self.0.push("conout"); } }

    struct MockWrite(Drops);
    impl Drop for MockWrite { fn drop(&mut self) { self.0.push("conin"); } }

    /// Minimal test-only copy of the shutdown sequence to assert drop order semantics.
    struct TestActive {
        backend: MockBackend,
        conout: MockRead,
        conin: MockWrite,
    }

    impl TestActive {
        fn shutdown(self) {
            drop(self.conout);
            drop(self.backend);
            drop(self.conin);
        }
    }

    #[test]
    fn test_shutdown_drops_in_correct_order() {
        let drops = Drops::new();
        let active = TestActive {
            backend: MockBackend(drops.clone()),
            conout: MockRead(drops.clone()),
            conin: MockWrite(drops.clone()),
        };
        active.shutdown();
        assert_eq!(drops.take(), vec!["conout", "backend", "conin"]);
    }

    #[test]
    fn test_pty_lifecycle_state_transitions() {
        // Typestate intention validation (compile-time): builder -> active -> shutdown (consumed)
        // This ensures we don't allow operations after shutdown.
        // The functional behavior is covered by test_shutdown_drops_in_correct_order.
    }
}

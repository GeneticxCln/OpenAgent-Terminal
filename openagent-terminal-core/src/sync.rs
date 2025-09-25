//! Synchronization primitives used in OpenAgent Terminal core.

use std::ops::{Deref, DerefMut};
use parking_lot::{Mutex, MutexGuard};

/// A simple wrapper around parking_lot::Mutex that provides a compatibility
/// layer for the historical FairMutex API used by the core event loop.
///
/// The "fair" semantics are not strictly implemented here; instead we
/// provide a minimal subset that preserves behavior and compiles cleanly.
pub struct FairMutex<T> {
    inner: Mutex<T>,
}

impl<T> FairMutex<T> {
    pub fn new(value: T) -> Self {
        Self { inner: Mutex::new(value) }
    }

    /// Acquire the lock (unfair). Alias for `lock_unfair`.
    pub fn lock(&self) -> FairMutexGuard<'_, T> {
        FairMutexGuard { guard: self.inner.lock() }
    }

    /// Try to acquire the lock without blocking. Semantics match parking_lot.
    pub fn try_lock_unfair(&self) -> Option<FairMutexGuard<'_, T>> {
        self.inner.try_lock().map(|g| FairMutexGuard { guard: g })
    }

    /// Acquire the lock, potentially queuing behind other waiters.
    pub fn lock_unfair(&self) -> FairMutexGuard<'_, T> {
        FairMutexGuard { guard: self.inner.lock() }
    }

    /// Return a lease token used by legacy code to yield fairness.
    /// This is a no-op in this implementation but preserves API surface.
    pub fn lease(&self) -> FairLease {
        FairLease {}
    }
}

/// RAII guard for FairMutex which derefs to the protected value.
pub struct FairMutexGuard<'a, T> {
    guard: MutexGuard<'a, T>,
}

impl<'a, T> Deref for FairMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target { &self.guard }
}

impl<'a, T> DerefMut for FairMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.guard }
}

/// Legacy lease token. Kept to avoid changing caller code paths.
pub struct FairLease {}

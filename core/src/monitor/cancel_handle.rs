//! The stop button for a running sweep.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Cancels an in-flight sweep. Cheap to clone and safe to hold across
/// threads, so a host can wire it straight to a Stop button.
#[derive(Clone, Default)]
pub struct MonitorHandle(Arc<AtomicBool>);

impl MonitorHandle {
    /// Ask the sweep to stop. Each worker finishes the save it has in flight
    /// and persists it, so cancelling never discards a completed check.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Clear the flag so the same handle can drive another sweep.
    pub(super) fn reset(&self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

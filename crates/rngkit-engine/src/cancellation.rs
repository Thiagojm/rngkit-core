//! Cooperative cancellation for the collection loop.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Shared cancellation flag.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    inner: Arc<AtomicBool>,
}

impl CancelToken {
    /// Creates an unset token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.inner.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }
}

/// Wait ended because the token was cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

/// Monotonic clock and cancellable wait used by the engine.
///
/// Tests inject a fake clock so they do not sleep real interval durations.
pub trait Clock: Send {
    /// Current monotonic instant.
    fn now(&self) -> Instant;
    /// Waits up to `duration` or until `cancel` is set.
    ///
    /// # Errors
    ///
    /// Returns [`Cancelled`] when the token is set before the wait completes.
    fn wait(&self, duration: Duration, cancel: &CancelToken) -> Result<(), Cancelled>;
}

/// Production clock using [`Instant`] and a chunked sleep.
#[derive(Debug, Default, Clone, Copy)]
pub struct StdClock;

impl Clock for StdClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn wait(&self, duration: Duration, cancel: &CancelToken) -> Result<(), Cancelled> {
        if duration.is_zero() {
            return if cancel.is_cancelled() {
                Err(Cancelled)
            } else {
                Ok(())
            };
        }
        let deadline = Instant::now() + duration;
        loop {
            if cancel.is_cancelled() {
                return Err(Cancelled);
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(());
            }
            let remaining = deadline.saturating_duration_since(now);
            thread::sleep(remaining.min(Duration::from_millis(10)));
        }
    }
}

/// Fake clock that advances without sleeping.
#[derive(Debug)]
pub struct FakeClock {
    origin: Instant,
    offset: std::sync::Mutex<Duration>,
}

impl FakeClock {
    /// Starts at a captured origin.
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
            offset: std::sync::Mutex::new(Duration::ZERO),
        }
    }

    /// Advances the fake clock.
    pub fn advance(&self, duration: Duration) {
        let mut offset = self.offset.lock().expect("fake clock mutex");
        *offset = offset.saturating_add(duration);
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        let offset = self.offset.lock().expect("fake clock mutex");
        self.origin + *offset
    }

    fn wait(&self, duration: Duration, cancel: &CancelToken) -> Result<(), Cancelled> {
        if cancel.is_cancelled() {
            return Err(Cancelled);
        }
        self.advance(duration);
        if cancel.is_cancelled() {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}

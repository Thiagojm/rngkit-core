//! Typed engine events.

use std::time::Duration;

use rngkit_analysis::Snapshot;
use rngkit_core::SampleRecord;
use rngkit_recording::SessionStem;

use crate::error::EngineError;

/// Events emitted after the corresponding state transition.
#[derive(Debug)]
#[non_exhaustive]
pub enum EngineEvent {
    /// Native bundle initialized.
    SessionStarted {
        /// Session stem.
        stem: SessionStem,
    },
    /// Sample committed, analyzed, and ready for the caller.
    SampleCommitted {
        /// Durable sample metadata.
        record: SampleRecord,
        /// Incremental descriptive snapshot.
        snapshot: Snapshot,
    },
    /// A complete cycle met or exceeded the interval.
    TimingOverrun {
        /// Measured cycle duration.
        cycle: Duration,
        /// Configured interval.
        interval: Duration,
    },
    /// Cancellation and clean finalization.
    SessionStopped {
        /// Committed samples.
        committed: u64,
        /// Overrun count.
        overruns: u64,
    },
    /// Terminal failure after best-effort failed-manifest finalization.
    SessionFailed {
        /// Committed samples before failure.
        committed: u64,
        /// Failure kind label.
        kind: &'static str,
        /// Diagnostic.
        diagnostic: String,
    },
}

/// Receives engine events. Sink failure is terminal.
pub trait EventSink {
    /// Handles one event.
    ///
    /// # Errors
    ///
    /// Returning an error stops the session.
    fn emit(&mut self, event: EngineEvent) -> Result<(), EngineError>;
}

/// Collects events for tests.
#[derive(Debug, Default)]
pub struct VecSink {
    events: Vec<EngineEvent>,
}

impl VecSink {
    /// Typed events in emission order.
    #[must_use]
    pub fn events(&self) -> &[EngineEvent] {
        &self.events
    }

    /// Count of committed samples.
    #[must_use]
    pub fn committed_count(&self) -> usize {
        self.events
            .iter()
            .filter(|event| matches!(event, EngineEvent::SampleCommitted { .. }))
            .count()
    }
}

impl EventSink for VecSink {
    fn emit(&mut self, event: EngineEvent) -> Result<(), EngineError> {
        self.events.push(event);
        Ok(())
    }
}

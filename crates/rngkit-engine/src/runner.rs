//! Blocking single-source collection loop.

use std::cell::Cell;
use std::path::PathBuf;

use rngkit_analysis::Accumulator;
use rngkit_core::{EntropySource, IntervalSeconds, SampleBits, SourceDescriptor, UtcTimestamp};
use rngkit_recording::{FailPoint, SessionStem, SessionWriter, now_local};
use time::UtcOffset;

use crate::cancellation::{CancelToken, Cancelled, Clock, StdClock};
use crate::error::EngineError;
use crate::event::{EngineEvent, EventSink};

/// Configuration for one collection session.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Output root. Session directory is created underneath.
    pub output_root: PathBuf,
    /// Sample size. Invalid values fail before artifacts exist.
    pub sample_bits: SampleBits,
    /// Interval, minimum one second.
    pub interval: IntervalSeconds,
}

/// Outcome of a session that stopped cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionOutcome {
    /// Session directory.
    pub directory: PathBuf,
    /// Committed samples.
    pub committed: u64,
    /// Overrun count.
    pub overruns: u64,
}

/// Runs a cancellable session until stop or terminal failure.
///
/// # Errors
///
/// Invalid configuration fails before creating files. Source, recording,
/// analysis, and sink errors are terminal.
pub fn run_session<S, K>(
    source: &mut S,
    config: EngineConfig,
    cancel: &CancelToken,
    sink: &mut K,
) -> Result<SessionOutcome, EngineError>
where
    S: EntropySource,
    K: EventSink,
{
    run_session_with_clock(source, config, cancel, sink, &StdClock)
}

/// Clock-injectable entry point used by tests.
pub fn run_session_with_clock<S, K, C>(
    source: &mut S,
    config: EngineConfig,
    cancel: &CancelToken,
    sink: &mut K,
    clock: &C,
) -> Result<SessionOutcome, EngineError>
where
    S: EntropySource,
    K: EventSink,
    C: Clock,
{
    let (local, offset) = now_local()?;
    run_session_inner(source, config, cancel, sink, clock, local, offset)
}

/// Test entry that supplies a local datetime.
pub fn run_session_at<S, K, C>(
    source: &mut S,
    config: EngineConfig,
    cancel: &CancelToken,
    sink: &mut K,
    clock: &C,
    local: time::OffsetDateTime,
    offset: UtcOffset,
) -> Result<SessionOutcome, EngineError>
where
    S: EntropySource,
    K: EventSink,
    C: Clock,
{
    run_session_inner(source, config, cancel, sink, clock, local, offset)
}

fn run_session_inner<S, K, C>(
    source: &mut S,
    config: EngineConfig,
    cancel: &CancelToken,
    sink: &mut K,
    clock: &C,
    local: time::OffsetDateTime,
    offset: UtcOffset,
) -> Result<SessionOutcome, EngineError>
where
    S: EntropySource,
    K: EventSink,
    C: Clock,
{
    let descriptor: SourceDescriptor = source.descriptor().clone();
    let stem = SessionStem::new(
        local,
        descriptor.id().clone(),
        config.sample_bits,
        config.interval,
        descriptor.fold(),
    )?;
    let started_at = UtcTimestamp::now();
    let mut writer = SessionWriter::create(
        &config.output_root,
        stem.clone(),
        &descriptor,
        started_at,
        offset,
    )?;
    TEST_WRITER_FAIL.with(|point| {
        if let Some(point) = point.get() {
            writer.set_fail_point(Some(point));
        }
    });
    if let Err(err) = sink.emit(EngineEvent::SessionStarted { stem }) {
        return fail(writer, sink, err);
    }

    let mut analysis = Accumulator::new(config.sample_bits);
    let interval = config.interval.duration();
    let session_start = clock.now();

    loop {
        if cancel.is_cancelled() {
            return stop(writer, sink);
        }

        let cycle_start = clock.now();
        let acquire_start = cycle_start;
        let bytes = match source.read_bits(config.sample_bits) {
            Ok(bytes) => bytes,
            Err(err) => return fail(writer, sink, EngineError::Source(err)),
        };
        let acquire_end = clock.now();
        let captured_at = UtcTimestamp::now();
        let acquisition = acquire_end.saturating_duration_since(acquire_start);
        let elapsed = acquire_end.saturating_duration_since(session_start);

        let record = match writer.commit_sample(&bytes, captured_at, elapsed, acquisition) {
            Ok(record) => record,
            Err(err) => return fail(writer, sink, EngineError::Recording(err)),
        };
        let snapshot = match analysis.push(record.ones) {
            Ok(snapshot) => snapshot,
            Err(err) => return fail(writer, sink, EngineError::Analysis(err)),
        };
        if let Err(err) = sink.emit(EngineEvent::SampleCommitted { record, snapshot }) {
            return fail(writer, sink, err);
        }

        if cancel.is_cancelled() {
            return stop(writer, sink);
        }

        let cycle_end = clock.now();
        let cycle = cycle_end.saturating_duration_since(cycle_start);
        if cycle >= interval {
            writer.add_overrun();
            if let Err(err) = sink.emit(EngineEvent::TimingOverrun { cycle, interval }) {
                return fail(writer, sink, err);
            }
            continue;
        }
        let remaining = interval.saturating_sub(cycle);
        match clock.wait(remaining, cancel) {
            Ok(()) => {}
            Err(Cancelled) => return stop(writer, sink),
        }
    }
}

fn stop<K: EventSink>(
    mut writer: SessionWriter,
    sink: &mut K,
) -> Result<SessionOutcome, EngineError> {
    let committed = writer.committed();
    let overruns = writer.overruns();
    if let Err(err) = writer.finalize_completed() {
        return fail(writer, sink, EngineError::Recording(err));
    }
    match sink.emit(EngineEvent::SessionStopped {
        committed,
        overruns,
    }) {
        Ok(()) => Ok(SessionOutcome {
            directory: writer.into_directory(),
            committed,
            overruns,
        }),
        Err(err) => fail(writer, sink, err),
    }
}

fn fail<K: EventSink>(
    mut writer: SessionWriter,
    sink: &mut K,
    err: EngineError,
) -> Result<SessionOutcome, EngineError> {
    let committed = writer.committed();
    let kind = err.kind_label();
    let diagnostic = err.to_string();
    let _ = writer.finalize_failed(kind, &diagnostic);
    let _ = sink.emit(EngineEvent::SessionFailed {
        committed,
        kind,
        diagnostic,
    });
    Err(err)
}

thread_local! {
    static TEST_WRITER_FAIL: Cell<Option<FailPoint>> = const { Cell::new(None) };
}

/// Installs a writer fail point for the duration of `body`.
///
/// Used by tests to inject completed-manifest and failed-manifest write
/// failures after [`SessionWriter::create`].
#[doc(hidden)]
pub fn with_writer_fail_point<R>(point: FailPoint, body: impl FnOnce() -> R) -> R {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_WRITER_FAIL.with(|cell| cell.set(None));
        }
    }
    TEST_WRITER_FAIL.with(|cell| cell.set(Some(point)));
    let _reset = Reset;
    body()
}

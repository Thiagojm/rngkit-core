//! Deterministic timing and overrun tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rngkit_core::{
    EntropySource, IntervalSeconds, SampleBits, SourceDescriptor, SourceError, SourceId,
};
use rngkit_engine::{CancelToken, EngineConfig, EngineEvent, FakeClock, VecSink, run_session_at};
use tempfile::tempdir;
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

struct Counting {
    descriptor: SourceDescriptor,
    inflight: Arc<AtomicUsize>,
    max_inflight: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    clock: Arc<FakeClock>,
    cancel: CancelToken,
    limit: usize,
}

impl EntropySource for Counting {
    fn descriptor(&self) -> &SourceDescriptor {
        &self.descriptor
    }

    fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, SourceError> {
        let now = self.inflight.fetch_add(1, Ordering::SeqCst) + 1;
        let mut max = self.max_inflight.load(Ordering::SeqCst);
        while now > max {
            match self
                .max_inflight
                .compare_exchange(max, now, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => break,
                Err(actual) => max = actual,
            }
        }
        self.clock.advance(Duration::from_secs(2));
        self.inflight.fetch_sub(1, Ordering::SeqCst);
        let n = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
        if n >= self.limit {
            self.cancel.cancel();
        }
        Ok(vec![0u8; bits.bytes().unwrap()])
    }
}

#[test]
fn overrun_starts_next_read_immediately_without_overlap() {
    let root = tempdir().unwrap();
    let cancel = CancelToken::new();
    let clock = Arc::new(FakeClock::new());
    let inflight = Arc::new(AtomicUsize::new(0));
    let max_inflight = Arc::new(AtomicUsize::new(0));
    let reads = Arc::new(AtomicUsize::new(0));
    let mut source = Counting {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None).unwrap(),
        inflight,
        max_inflight: max_inflight.clone(),
        reads: reads.clone(),
        clock: clock.clone(),
        cancel: cancel.clone(),
        limit: 3,
    };
    let mut sink = VecSink::default();
    let date = Date::from_calendar_date(2026, Month::August, 21).unwrap();
    let time = Time::from_hms(18, 30, 0).unwrap();
    run_session_at(
        &mut source,
        EngineConfig {
            output_root: root.path().to_path_buf(),
            sample_bits: SampleBits::new(8).unwrap(),
            interval: IntervalSeconds::new(1).unwrap(),
        },
        &cancel,
        &mut sink,
        clock.as_ref(),
        PrimitiveDateTime::new(date, time).assume_utc(),
        UtcOffset::UTC,
    )
    .unwrap();
    assert_eq!(max_inflight.load(Ordering::SeqCst), 1);
    let overruns = sink
        .events()
        .iter()
        .filter(|e| matches!(e, EngineEvent::TimingOverrun { .. }))
        .count();
    assert!(overruns >= 2);
}

#[test]
fn cancel_during_wait_is_prompt() {
    let root = tempdir().unwrap();
    let cancel = CancelToken::new();
    let clock = Arc::new(FakeClock::new());
    struct OneThenWait {
        descriptor: SourceDescriptor,
        reads: usize,
        cancel: CancelToken,
        clock: Arc<FakeClock>,
    }
    impl EntropySource for OneThenWait {
        fn descriptor(&self) -> &SourceDescriptor {
            &self.descriptor
        }
        fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, SourceError> {
            self.reads += 1;
            self.clock.advance(Duration::from_millis(1));
            if self.reads >= 1 {
                self.cancel.cancel();
            }
            Ok(vec![0u8; bits.bytes().unwrap()])
        }
    }
    let mut source = OneThenWait {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None).unwrap(),
        reads: 0,
        cancel: cancel.clone(),
        clock: clock.clone(),
    };
    let mut sink = VecSink::default();
    let date = Date::from_calendar_date(2026, Month::August, 21).unwrap();
    let time = Time::from_hms(18, 31, 0).unwrap();
    let outcome = run_session_at(
        &mut source,
        EngineConfig {
            output_root: root.path().to_path_buf(),
            sample_bits: SampleBits::new(8).unwrap(),
            interval: IntervalSeconds::new(1).unwrap(),
        },
        &cancel,
        &mut sink,
        clock.as_ref(),
        PrimitiveDateTime::new(date, time).assume_utc(),
        UtcOffset::UTC,
    )
    .unwrap();
    assert_eq!(outcome.committed, 1);
    assert_eq!(source.reads, 1);
}

//! Mock pipeline: engine → native recording → reader → analysis.

use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use rngkit_analysis::analyze_records;
use rngkit_core::{
    EntropySource, IntervalSeconds, SampleBits, SourceDescriptor, SourceError, SourceId,
};
use rngkit_engine::{CancelToken, EngineConfig, EngineEvent, FakeClock, VecSink, run_session_at};
use rngkit_recording::NativeSession;
use tempfile::tempdir;
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

struct Scripted {
    descriptor: SourceDescriptor,
    samples: Vec<Vec<u8>>,
    index: usize,
    cancel: CancelToken,
    clock: Arc<FakeClock>,
    read_duration: Duration,
    on_read: Option<Arc<Barrier>>,
}

impl EntropySource for Scripted {
    fn descriptor(&self) -> &SourceDescriptor {
        &self.descriptor
    }

    fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, SourceError> {
        self.clock.advance(self.read_duration);
        if let Some(barrier) = &self.on_read {
            barrier.wait();
        }
        if self.index >= self.samples.len() {
            self.cancel.cancel();
            let n = bits.bytes().unwrap();
            return Ok(vec![0; n]);
        }
        let sample = self.samples[self.index].clone();
        self.index += 1;
        if self.index >= self.samples.len() {
            self.cancel.cancel();
        }
        Ok(sample)
    }
}

fn local() -> (time::OffsetDateTime, UtcOffset) {
    let date = Date::from_calendar_date(2026, Month::August, 21).unwrap();
    let time = Time::from_hms(18, 30, 0).unwrap();
    (
        PrimitiveDateTime::new(date, time).assume_utc(),
        UtcOffset::UTC,
    )
}

#[test]
fn mock_session_writes_bundle_and_matches_batch() {
    let root = tempdir().unwrap();
    let bits = SampleBits::new(16).unwrap();
    let cancel = CancelToken::new();
    let clock = Arc::new(FakeClock::new());
    let mut source = Scripted {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None).unwrap(),
        samples: vec![vec![0xFF, 0x00], vec![0x00, 0xFF], vec![0xAA, 0x55]],
        index: 0,
        cancel: cancel.clone(),
        clock: clock.clone(),
        read_duration: Duration::from_millis(1),
        on_read: None,
    };
    let mut sink = VecSink::default();
    let (when, offset) = local();
    let outcome = run_session_at(
        &mut source,
        EngineConfig {
            output_root: root.path().to_path_buf(),
            sample_bits: bits,
            interval: IntervalSeconds::new(1).unwrap(),
        },
        &cancel,
        &mut sink,
        clock.as_ref(),
        when,
        offset,
    )
    .unwrap();
    assert_eq!(outcome.committed, 3);
    let entries: Vec<_> = std::fs::read_dir(&outcome.directory)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert!(entries.iter().any(|n| n.ends_with(".bin")));
    assert!(entries.iter().any(|n| n.ends_with(".csv")));
    assert!(entries.iter().any(|n| n == "manifest.json"));
    assert_eq!(entries.iter().filter(|n| n.ends_with(".bin")).count(), 1);

    let native = NativeSession::open(&outcome.directory).unwrap();
    let inc: Vec<_> = sink
        .events()
        .iter()
        .filter_map(|e| match e {
            EngineEvent::SampleCommitted { snapshot, .. } => Some(snapshot.z),
            _ => None,
        })
        .collect();
    let batch = analyze_records(bits, native.records().to_vec()).unwrap();
    assert_eq!(inc.len(), batch.len());
    for (a, b) in inc.iter().zip(batch.iter()) {
        assert!((a - b.z).abs() < 1e-12);
    }

    let xlsx_path = rngkit_xlsx::native_report_path(&outcome.directory, native.manifest().stem());
    rngkit_xlsx::write_report(
        &native.normalized(),
        &xlsx_path,
        rngkit_xlsx::Overwrite::ErrorIfExists,
    )
    .unwrap();
    assert!(xlsx_path.exists());
}

#[test]
fn cancel_during_read_commits_one_sample() {
    let root = tempdir().unwrap();
    let bits = SampleBits::new(8).unwrap();
    let cancel = CancelToken::new();
    let clock = Arc::new(FakeClock::new());
    let barrier = Arc::new(Barrier::new(2));
    let mut source = Scripted {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None).unwrap(),
        samples: vec![vec![0xFF], vec![0x00]],
        index: 0,
        cancel: cancel.clone(),
        clock: clock.clone(),
        read_duration: Duration::from_millis(1),
        on_read: Some(barrier.clone()),
    };
    let mut sink = VecSink::default();
    let (when, offset) = local();
    let cfg = EngineConfig {
        output_root: root.path().to_path_buf(),
        sample_bits: bits,
        interval: IntervalSeconds::new(1).unwrap(),
    };
    let handle = thread::spawn({
        let cancel = cancel.clone();
        move || {
            barrier.wait();
            cancel.cancel();
        }
    });
    let outcome = run_session_at(
        &mut source,
        cfg,
        &cancel,
        &mut sink,
        clock.as_ref(),
        when,
        offset,
    )
    .unwrap();
    handle.join().unwrap();
    assert_eq!(outcome.committed, 1);
    assert_eq!(sink.committed_count(), 1);
}

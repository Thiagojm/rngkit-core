//! Mock pipeline: engine → native recording → reader → analysis.

use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use rngkit_analysis::analyze_records;
use rngkit_core::{
    EntropySource, IntervalSeconds, SampleBits, SourceDescriptor, SourceError, SourceId,
};
use rngkit_engine::{
    CancelToken, EngineConfig, EngineError, EngineEvent, EventSink, FakeClock, VecSink,
    run_session_at, with_writer_fail_point,
};
use rngkit_recording::{FailPoint, ManifestStatus, NativeSession};
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

    let xlsx_path =
        rngkit_xlsx::native_report_path(&outcome.directory, native.session_stem()).unwrap();
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

fn session_dir(root: &std::path::Path) -> std::path::PathBuf {
    std::fs::read_dir(root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}

#[derive(Clone, Copy)]
enum SinkFailAt {
    Started,
    Committed,
    Overrun,
    Stopped,
    Failed,
}

struct FailSink {
    at: SinkFailAt,
    names: Vec<&'static str>,
}

impl FailSink {
    fn new(at: SinkFailAt) -> Self {
        Self {
            at,
            names: Vec::new(),
        }
    }

    fn names(&self) -> &[&'static str] {
        &self.names
    }
}

fn event_name(event: &EngineEvent) -> &'static str {
    match event {
        EngineEvent::SessionStarted { .. } => "started",
        EngineEvent::SampleCommitted { .. } => "committed",
        EngineEvent::TimingOverrun { .. } => "overrun",
        EngineEvent::SessionStopped { .. } => "stopped",
        EngineEvent::SessionFailed { .. } => "failed",
        _ => "other",
    }
}

impl EventSink for FailSink {
    fn emit(&mut self, event: EngineEvent) -> Result<(), EngineError> {
        let name = event_name(&event);
        let fail = matches!(
            (self.at, &event),
            (SinkFailAt::Started, EngineEvent::SessionStarted { .. })
                | (SinkFailAt::Committed, EngineEvent::SampleCommitted { .. })
                | (SinkFailAt::Overrun, EngineEvent::TimingOverrun { .. })
                | (SinkFailAt::Stopped, EngineEvent::SessionStopped { .. })
                | (SinkFailAt::Failed, EngineEvent::SessionFailed { .. })
        );
        self.names.push(name);
        if fail {
            Err(EngineError::Sink(format!("injected {name} failure")))
        } else {
            Ok(())
        }
    }
}

fn assert_primary_sink(err: EngineError, needle: &str) {
    match err {
        EngineError::Sink(msg) => assert!(msg.contains(needle), "unexpected sink message {msg}"),
        other => panic!("expected primary sink error, got {other}"),
    }
}

#[test]
fn sink_failure_on_session_started_finalizes_failed_manifest() {
    let root = tempdir().unwrap();
    let bits = SampleBits::new(8).unwrap();
    let cancel = CancelToken::new();
    let clock = Arc::new(FakeClock::new());
    let mut source = Scripted {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None).unwrap(),
        samples: vec![vec![0xFF]],
        index: 0,
        cancel: cancel.clone(),
        clock: clock.clone(),
        read_duration: Duration::from_millis(1),
        on_read: None,
    };
    let mut sink = FailSink::new(SinkFailAt::Started);
    let (when, offset) = local();
    let err = run_session_at(
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
    .unwrap_err();
    assert_primary_sink(err, "started");
    assert_eq!(sink.names(), &["started", "failed"]);
    let dir = session_dir(root.path());
    let native = NativeSession::open(&dir).unwrap();
    assert_eq!(native.manifest().status(), ManifestStatus::Failed);
    assert_eq!(native.manifest().committed_samples(), 0);
    assert_eq!(native.manifest().failure_kind(), Some("sink"));
}

#[test]
fn sink_failure_on_sample_committed_keeps_durable_sample() {
    let root = tempdir().unwrap();
    let bits = SampleBits::new(8).unwrap();
    let cancel = CancelToken::new();
    let clock = Arc::new(FakeClock::new());
    let mut source = Scripted {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None).unwrap(),
        samples: vec![vec![0xFF]],
        index: 0,
        cancel: cancel.clone(),
        clock: clock.clone(),
        read_duration: Duration::from_millis(1),
        on_read: None,
    };
    let mut sink = FailSink::new(SinkFailAt::Committed);
    let (when, offset) = local();
    let err = run_session_at(
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
    .unwrap_err();
    assert_primary_sink(err, "committed");
    assert_eq!(sink.names(), &["started", "committed", "failed"]);
    let dir = session_dir(root.path());
    let native = NativeSession::open(&dir).unwrap();
    assert_eq!(native.manifest().status(), ManifestStatus::Failed);
    assert_eq!(native.manifest().committed_samples(), 1);
    assert_eq!(native.records().len(), 1);
    assert_eq!(native.manifest().failure_kind(), Some("sink"));
}

#[test]
fn sink_failure_on_timing_overrun_preserves_counts() {
    let root = tempdir().unwrap();
    let bits = SampleBits::new(8).unwrap();
    let cancel = CancelToken::new();
    let clock = Arc::new(FakeClock::new());
    struct OverrunSource {
        descriptor: SourceDescriptor,
        clock: Arc<FakeClock>,
    }
    impl EntropySource for OverrunSource {
        fn descriptor(&self) -> &SourceDescriptor {
            &self.descriptor
        }
        fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, SourceError> {
            self.clock.advance(Duration::from_secs(2));
            Ok(vec![0u8; bits.bytes().unwrap()])
        }
    }
    let mut source = OverrunSource {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None).unwrap(),
        clock: clock.clone(),
    };
    let mut sink = FailSink::new(SinkFailAt::Overrun);
    let (when, offset) = local();
    let err = run_session_at(
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
    .unwrap_err();
    assert_primary_sink(err, "overrun");
    assert_eq!(sink.names(), &["started", "committed", "overrun", "failed"]);
    let dir = session_dir(root.path());
    let native = NativeSession::open(&dir).unwrap();
    assert_eq!(native.manifest().status(), ManifestStatus::Failed);
    assert_eq!(native.manifest().committed_samples(), 1);
    assert_eq!(native.manifest().overrun_count(), 1);
    assert_eq!(native.manifest().failure_kind(), Some("sink"));
}

#[test]
fn sink_failure_on_session_stopped_finalizes_failed_manifest() {
    let root = tempdir().unwrap();
    let bits = SampleBits::new(8).unwrap();
    let cancel = CancelToken::new();
    let clock = Arc::new(FakeClock::new());
    let mut source = Scripted {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None).unwrap(),
        samples: vec![vec![0xAA]],
        index: 0,
        cancel: cancel.clone(),
        clock: clock.clone(),
        read_duration: Duration::from_millis(1),
        on_read: None,
    };
    let mut sink = FailSink::new(SinkFailAt::Stopped);
    let (when, offset) = local();
    let err = run_session_at(
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
    .unwrap_err();
    assert_primary_sink(err, "stopped");
    assert_eq!(sink.names(), &["started", "committed", "stopped", "failed"]);
    let dir = session_dir(root.path());
    let native = NativeSession::open(&dir).unwrap();
    assert_eq!(native.manifest().status(), ManifestStatus::Failed);
    assert_eq!(native.manifest().committed_samples(), 1);
    assert_eq!(native.records().len(), 1);
    assert_eq!(native.manifest().failure_kind(), Some("sink"));
}

#[test]
fn complete_manifest_failure_falls_back_to_failed_manifest() {
    let root = tempdir().unwrap();
    let bits = SampleBits::new(8).unwrap();
    let cancel = CancelToken::new();
    let clock = Arc::new(FakeClock::new());
    let mut source = Scripted {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None).unwrap(),
        samples: vec![vec![0xAA]],
        index: 0,
        cancel: cancel.clone(),
        clock: clock.clone(),
        read_duration: Duration::from_millis(1),
        on_read: None,
    };
    let mut sink = VecSink::default();
    let (when, offset) = local();
    let err = with_writer_fail_point(FailPoint::CompleteManifest, || {
        run_session_at(
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
    })
    .unwrap_err();
    match err {
        EngineError::Recording(inner) => {
            let text = inner.to_string();
            assert!(
                text.contains("complete-manifest"),
                "expected complete-manifest primary, got {text}"
            );
        }
        other => panic!("expected recording error, got {other}"),
    }
    let dir = session_dir(root.path());
    let native = NativeSession::open(&dir).unwrap();
    assert_eq!(native.manifest().status(), ManifestStatus::Failed);
    assert_eq!(native.manifest().committed_samples(), 1);
    assert_eq!(native.manifest().failure_kind(), Some("recording"));
    assert!(
        sink.events()
            .iter()
            .any(|event| matches!(event, EngineEvent::SessionFailed { .. }))
    );
}

#[test]
fn complete_and_fail_manifest_keeps_primary_completion_error() {
    let root = tempdir().unwrap();
    let bits = SampleBits::new(8).unwrap();
    let cancel = CancelToken::new();
    let clock = Arc::new(FakeClock::new());
    let mut source = Scripted {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None).unwrap(),
        samples: vec![vec![0xAA]],
        index: 0,
        cancel: cancel.clone(),
        clock: clock.clone(),
        read_duration: Duration::from_millis(1),
        on_read: None,
    };
    let mut sink = VecSink::default();
    let (when, offset) = local();
    let err = with_writer_fail_point(FailPoint::CompleteAndFailManifest, || {
        run_session_at(
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
    })
    .unwrap_err();
    match err {
        EngineError::Recording(inner) => {
            let text = inner.to_string();
            assert!(
                text.contains("complete-manifest"),
                "secondary fail-manifest must not replace primary, got {text}"
            );
            assert!(
                !text.contains("fail-manifest"),
                "primary must remain the completion error, got {text}"
            );
        }
        other => panic!("expected recording error, got {other}"),
    }
    let dir = session_dir(root.path());
    let native = NativeSession::open(&dir).unwrap();
    assert_eq!(native.manifest().status(), ManifestStatus::Recording);
    assert_eq!(native.records().len(), 1);
}

#[test]
fn session_failed_sink_failure_preserves_primary_source_error() {
    let root = tempdir().unwrap();
    let bits = SampleBits::new(8).unwrap();
    let cancel = CancelToken::new();
    let clock = Arc::new(FakeClock::new());
    struct Boom {
        descriptor: SourceDescriptor,
    }
    impl EntropySource for Boom {
        fn descriptor(&self) -> &SourceDescriptor {
            &self.descriptor
        }
        fn read_bits(&mut self, _bits: SampleBits) -> Result<Vec<u8>, SourceError> {
            Err(SourceError::new(
                rngkit_core::SourceErrorKind::Disconnected,
                "injected source failure",
            ))
        }
    }
    let mut source = Boom {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None).unwrap(),
    };
    let mut sink = FailSink::new(SinkFailAt::Failed);
    let (when, offset) = local();
    let err = run_session_at(
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
    .unwrap_err();
    match err {
        EngineError::Source(inner) => {
            assert!(inner.message().contains("injected source failure"));
        }
        other => panic!("expected primary source error, got {other}"),
    }
    assert_eq!(sink.names(), &["started", "failed"]);
    let dir = session_dir(root.path());
    let native = NativeSession::open(&dir).unwrap();
    assert_eq!(native.manifest().status(), ManifestStatus::Failed);
    assert_eq!(native.manifest().committed_samples(), 0);
    assert_eq!(native.manifest().failure_kind(), Some("source"));
}

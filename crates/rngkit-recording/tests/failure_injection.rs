//! Commit-boundary failure injection.

use std::time::Duration;

use rngkit_core::{IntervalSeconds, SampleBits, SourceDescriptor, SourceId, UtcTimestamp};
use rngkit_recording::{FailPoint, NativeSession, RecordingError, SessionStem, SessionWriter};
use tempfile::tempdir;
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

fn writer_at(point: Option<FailPoint>) -> (tempfile::TempDir, SessionWriter) {
    let root = tempdir().unwrap();
    let date = Date::from_calendar_date(2026, Month::August, 21).unwrap();
    let time = Time::from_hms(18, 30, 0).unwrap();
    let local = PrimitiveDateTime::new(date, time).assume_utc();
    let stem = SessionStem::new(
        local,
        SourceId::pseudo(),
        SampleBits::new(8).unwrap(),
        IntervalSeconds::new(1).unwrap(),
        None,
    )
    .unwrap();
    let desc = SourceDescriptor::new(SourceId::pseudo(), "PseudoRNG", None, None).unwrap();
    let mut writer = SessionWriter::create(
        root.path(),
        stem,
        &desc,
        UtcTimestamp::now(),
        UtcOffset::UTC,
    )
    .unwrap();
    writer.set_fail_point(point);
    (root, writer)
}

fn inject(point: FailPoint) {
    let (_root, mut writer) = writer_at(Some(point));
    let err = writer
        .commit_sample(
            &[0xFF],
            UtcTimestamp::now(),
            Duration::from_millis(1),
            Duration::from_millis(1),
        )
        .unwrap_err();
    assert!(matches!(err, RecordingError::Commit { .. }));
    let dir = writer.directory().to_path_buf();
    drop(writer);
    match NativeSession::open(&dir) {
        Ok(session) => match point {
            FailPoint::AfterCsvAppend => {
                // The CSV crate may flush the row before our explicit sync.
                // A visible CSV row is the commit marker only if BIN contains
                // the matching bytes; the writer still returned Err.
                for item in session.raw_samples() {
                    item.expect("csv row must not outrank missing bin bytes");
                }
            }
            FailPoint::AfterValidate | FailPoint::AfterBinAppend | FailPoint::AfterBinSync => {
                assert_eq!(
                    session.consistency().committed_samples,
                    0,
                    "injected {point:?} must not yield a committed csv row"
                );
            }
            FailPoint::CompleteManifest
            | FailPoint::FailManifest
            | FailPoint::CompleteAndFailManifest => {
                panic!("commit injection used a finalization fail point {point:?}")
            }
        },
        Err(RecordingError::Corrupt { .. }) => {}
        Err(other) => panic!("unexpected {other}"),
    }
}

#[test]
fn fail_after_validate() {
    inject(FailPoint::AfterValidate);
}

#[test]
fn fail_after_bin_append() {
    inject(FailPoint::AfterBinAppend);
}

#[test]
fn fail_after_bin_sync() {
    inject(FailPoint::AfterBinSync);
}

#[test]
fn fail_after_csv_append() {
    inject(FailPoint::AfterCsvAppend);
}

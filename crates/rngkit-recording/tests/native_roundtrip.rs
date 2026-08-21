//! Native BIN/CSV round-trip and consistency tests.

use std::time::Duration;

use rngkit_core::{
    IntervalSeconds, SampleBits, SourceDescriptor, SourceId, UtcTimestamp, count_ones,
};
use rngkit_recording::{
    ConsistencyWarning, NATIVE_CSV_COLUMNS, NativeSession, SessionStem, SessionWriter,
};
use tempfile::tempdir;
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

fn setup() -> (
    tempfile::TempDir,
    SessionStem,
    SourceDescriptor,
    UtcTimestamp,
    UtcOffset,
) {
    let dir = tempdir().unwrap();
    let date = Date::from_calendar_date(2026, Month::August, 21).unwrap();
    let time = Time::from_hms(18, 30, 0).unwrap();
    let local = PrimitiveDateTime::new(date, time).assume_utc();
    let stem = SessionStem::new(
        local,
        SourceId::pseudo(),
        SampleBits::new(16).unwrap(),
        IntervalSeconds::new(1).unwrap(),
        None,
    )
    .unwrap();
    let desc = SourceDescriptor::new(SourceId::pseudo(), "PseudoRNG", None, None).unwrap();
    (dir, stem, desc, UtcTimestamp::now(), UtcOffset::UTC)
}

#[test]
fn round_trip_preserves_bytes_and_csv() {
    let (root, stem, desc, started, offset) = setup();
    let mut writer =
        SessionWriter::create(root.path(), stem.clone(), &desc, started, offset).unwrap();
    let sample = [0xAAu8, 0x55];
    let record = writer
        .commit_sample(
            &sample,
            UtcTimestamp::now(),
            Duration::from_millis(10),
            Duration::from_millis(3),
        )
        .unwrap();
    assert_eq!(record.ones, 8);
    let dir = writer.complete().unwrap();
    let session = NativeSession::open(&dir).unwrap();
    assert_eq!(session.consistency().committed_samples, 1);
    assert!(session.consistency().warnings.is_empty());
    let raw = session.raw_samples().next().unwrap().unwrap();
    assert_eq!(raw.1, sample);
    assert_eq!(raw.0.ones, count_ones(&sample).unwrap());
    let csv = std::fs::read_to_string(session.csv_path()).unwrap();
    let header = csv.lines().next().unwrap();
    assert_eq!(header, NATIVE_CSV_COLUMNS.join(","));
}

#[test]
fn collision_does_not_overwrite() {
    let (root, stem, desc, started, offset) = setup();
    let writer = SessionWriter::create(root.path(), stem.clone(), &desc, started, offset).unwrap();
    let _ = writer.complete().unwrap();
    let err = SessionWriter::create(root.path(), stem, &desc, started, offset).unwrap_err();
    assert!(matches!(
        err,
        rngkit_recording::RecordingError::AlreadyExists { .. }
    ));
}

#[test]
fn bin_tail_is_warning() {
    let (root, stem, desc, started, offset) = setup();
    let mut writer =
        SessionWriter::create(root.path(), stem.clone(), &desc, started, offset).unwrap();
    writer
        .commit_sample(
            &[0xFF, 0x00],
            UtcTimestamp::now(),
            Duration::from_millis(1),
            Duration::from_millis(1),
        )
        .unwrap();
    let dir = writer.complete().unwrap();
    let bin_path = dir.join(format!("{}.bin", stem.as_str()));
    let mut bytes = std::fs::read(&bin_path).unwrap();
    bytes.extend_from_slice(&[0x11, 0x22]);
    std::fs::write(&bin_path, bytes).unwrap();
    let session = NativeSession::open(&dir).unwrap();
    assert_eq!(session.consistency().committed_samples, 1);
    assert!(matches!(
        session.consistency().warnings.as_slice(),
        [ConsistencyWarning::UncommittedBinaryTail { .. }]
    ));
}

#[test]
fn csv_beyond_bin_is_corrupt() {
    let (root, stem, desc, started, offset) = setup();
    let mut writer =
        SessionWriter::create(root.path(), stem.clone(), &desc, started, offset).unwrap();
    writer
        .commit_sample(
            &[0x00, 0x00],
            UtcTimestamp::now(),
            Duration::from_millis(1),
            Duration::from_millis(1),
        )
        .unwrap();
    let dir = writer.complete().unwrap();
    let bin_path = dir.join(format!("{}.bin", stem.as_str()));
    std::fs::write(&bin_path, []).unwrap();
    let err = NativeSession::open(&dir).unwrap_err();
    assert!(matches!(
        err,
        rngkit_recording::RecordingError::Corrupt { .. }
    ));
}

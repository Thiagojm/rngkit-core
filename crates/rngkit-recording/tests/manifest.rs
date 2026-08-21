//! Manifest round-trip and replacement tests.

use rngkit_core::{IntervalSeconds, SampleBits, SourceDescriptor, SourceId, UtcTimestamp};
use rngkit_recording::{Manifest, ManifestStatus, SCHEMA_VERSION, SessionStem};
use tempfile::tempdir;
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

fn stem() -> SessionStem {
    let date = Date::from_calendar_date(2026, Month::August, 21).unwrap();
    let time = Time::from_hms(18, 30, 0).unwrap();
    let local = PrimitiveDateTime::new(date, time).assume_utc();
    SessionStem::new(
        local,
        SourceId::trng(),
        SampleBits::new(2048).unwrap(),
        IntervalSeconds::new(1).unwrap(),
        None,
    )
    .unwrap()
}

#[test]
fn round_trip_without_selectors() {
    let dir = tempdir().unwrap();
    let desc = SourceDescriptor::new(
        SourceId::trng(),
        "TrueRNG v1/v2/v3",
        Some("TrueRNG".into()),
        None,
    )
    .unwrap();
    let offset = UtcOffset::from_hms(-3, 0, 0).unwrap();
    let mut manifest = Manifest::recording(&stem(), &desc, UtcTimestamp::now(), offset);
    manifest.complete(3, 1, UtcTimestamp::now());
    manifest.write_to(dir.path()).unwrap();
    let loaded = Manifest::read_from(dir.path()).unwrap();
    assert_eq!(loaded.schema_version(), SCHEMA_VERSION);
    assert_eq!(loaded.status(), ManifestStatus::Completed);
    assert_eq!(loaded.committed_samples(), 3);
    assert_eq!(loaded.overrun_count(), 1);
    assert_eq!(loaded.local_utc_offset(), "-03:00");
    let json = std::fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    assert!(!json.contains("COM"));
    assert!(!json.contains("serial"));
    assert!(!json.contains("seed"));
    assert!(!json.to_lowercase().contains("chacha"));
}

#[test]
fn replace_leaves_complete_manifest() {
    let dir = tempdir().unwrap();
    let desc = SourceDescriptor::new(SourceId::pseudo(), "PseudoRNG", None, None).unwrap();
    let offset = UtcOffset::UTC;
    let first = Manifest::recording(&stem(), &desc, UtcTimestamp::now(), offset);
    first.write_to(dir.path()).unwrap();
    let mut second = first;
    second.complete(2, 0, UtcTimestamp::now());
    second.write_to(dir.path()).unwrap();
    let loaded = Manifest::read_from(dir.path()).unwrap();
    assert_eq!(loaded.status(), ManifestStatus::Completed);
    assert_eq!(loaded.committed_samples(), 2);
}

#[test]
fn rejects_unknown_schema() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("manifest.json"),
        r#"{"schema_version":2,"stem":"x"}"#,
    )
    .unwrap();
    let err = Manifest::read_from(dir.path()).unwrap_err();
    assert!(matches!(
        err,
        rngkit_recording::RecordingError::UnsupportedSchema { version: 2 }
            | rngkit_recording::RecordingError::Json(_)
    ));
}

//! Manifest round-trip and replacement tests.

use rngkit_core::{Fold, IntervalSeconds, SampleBits, SourceDescriptor, SourceId, UtcTimestamp};
use rngkit_recording::{
    Manifest, ManifestStatus, NativeSession, RecordingError, SCHEMA_VERSION, SessionStem,
    join_contained,
};
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
    let desc = SourceDescriptor::new(SourceId::trng(), "TrueRNG v1/v2/v3", None, None).unwrap();
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

fn write_session_bundle(dir: &std::path::Path, manifest: &Manifest) {
    manifest.write_to(dir).unwrap();
    let stem = manifest.stem();
    std::fs::write(dir.join(format!("{stem}.bin")), []).unwrap();
    std::fs::write(
        dir.join(format!("{stem}.csv")),
        "sample_index,captured_at_utc,elapsed_ms,acquisition_ms,ones,byte_offset,byte_length\n",
    )
    .unwrap();
}

fn replace_field(manifest_path: &std::path::Path, from: &str, to: &str) {
    let text = std::fs::read_to_string(manifest_path).unwrap();
    assert!(text.contains(from), "manifest missing {from}: {text}");
    std::fs::write(manifest_path, text.replace(from, to)).unwrap();
}

#[test]
fn tampered_artifact_paths_cannot_escape_session_dir() {
    let root = tempdir().unwrap();
    let session = root.path().join("session");
    std::fs::create_dir(&session).unwrap();
    let outside = root.path().join("outside.bin");
    std::fs::write(&outside, b"secret-bytes").unwrap();
    let outside_hash = std::fs::read(&outside).unwrap();

    let desc = SourceDescriptor::new(SourceId::trng(), "TrueRNG v1/v2/v3", None, None).unwrap();
    let manifest = Manifest::recording(&stem(), &desc, UtcTimestamp::now(), UtcOffset::UTC);
    write_session_bundle(&session, &manifest);
    let path = session.join("manifest.json");
    let stem_name = stem().to_string();

    replace_field(
        &path,
        &format!("\"bin_file\": \"{stem_name}.bin\""),
        "\"bin_file\": \"../outside.bin\"",
    );
    let err = Manifest::read_from(&session).unwrap_err();
    assert!(
        matches!(err, RecordingError::PathEscapesRoot { .. }),
        "expected path escape, got {err}"
    );
    assert!(NativeSession::open(&session).is_err());
    assert_eq!(std::fs::read(&outside).unwrap(), outside_hash);
    assert!(!root.path().join("outside.xlsx").exists());

    write_session_bundle(&session, &manifest);
    let abs = outside.to_str().unwrap().replace('\\', "\\\\");
    replace_field(
        &path,
        &format!("\"csv_file\": \"{stem_name}.csv\""),
        &format!("\"csv_file\": \"{abs}\""),
    );
    assert!(matches!(
        Manifest::read_from(&session),
        Err(RecordingError::PathEscapesRoot { .. })
    ));
    assert_eq!(std::fs::read(&outside).unwrap(), outside_hash);

    write_session_bundle(&session, &manifest);
    replace_field(
        &path,
        &format!("\"bin_file\": \"{stem_name}.bin\""),
        "\"bin_file\": \"other.bin\"",
    );
    assert!(matches!(
        Manifest::read_from(&session),
        Err(RecordingError::InvalidName { .. })
    ));

    write_session_bundle(&session, &manifest);
    replace_field(
        &path,
        &format!("\"bin_file\": \"{stem_name}.bin\""),
        &format!("\"bin_file\": \"{stem_name}.csv\""),
    );
    assert!(matches!(
        Manifest::read_from(&session),
        Err(RecordingError::InvalidName { .. })
    ));
    assert_eq!(std::fs::read(&outside).unwrap(), outside_hash);
}

#[test]
fn tampered_manifest_metadata_must_match_stem() {
    let dir = tempdir().unwrap();
    let desc = SourceDescriptor::new(SourceId::trng(), "TrueRNG v1/v2/v3", None, None).unwrap();
    let manifest = Manifest::recording(&stem(), &desc, UtcTimestamp::now(), UtcOffset::UTC);
    write_session_bundle(dir.path(), &manifest);
    let path = dir.path().join("manifest.json");

    replace_field(
        &path,
        "\"source_id\": \"trng\"",
        "\"source_id\": \"pseudo\"",
    );
    assert!(matches!(
        Manifest::read_from(dir.path()),
        Err(RecordingError::Corrupt { .. })
    ));

    write_session_bundle(dir.path(), &manifest);
    replace_field(&path, "\"sample_bits\": 2048", "\"sample_bits\": 16");
    assert!(matches!(
        Manifest::read_from(dir.path()),
        Err(RecordingError::Corrupt { .. })
    ));

    write_session_bundle(dir.path(), &manifest);
    replace_field(&path, "\"interval_seconds\": 1", "\"interval_seconds\": 2");
    assert!(matches!(
        Manifest::read_from(dir.path()),
        Err(RecordingError::Corrupt { .. })
    ));

    let bitb = SessionStem::new(
        stem().local_start(),
        SourceId::bitb(),
        SampleBits::new(2048).unwrap(),
        IntervalSeconds::new(1).unwrap(),
        Some(Fold::new(0).unwrap()),
    )
    .unwrap();
    let bitb_desc = SourceDescriptor::new(
        SourceId::bitb(),
        "BitBabbler",
        None,
        Some(Fold::new(0).unwrap()),
    )
    .unwrap();
    let bitb_manifest = Manifest::recording(&bitb, &bitb_desc, UtcTimestamp::now(), UtcOffset::UTC);
    write_session_bundle(dir.path(), &bitb_manifest);
    replace_field(&path, "\"fold\": 0", "\"fold\": 1");
    assert!(matches!(
        Manifest::read_from(dir.path()),
        Err(RecordingError::Corrupt { .. })
    ));
}

#[test]
fn valid_manifest_round_trip_stays_contained() {
    let dir = tempdir().unwrap();
    let desc = SourceDescriptor::new(SourceId::trng(), "TrueRNG v1/v2/v3", None, None).unwrap();
    let manifest = Manifest::recording(&stem(), &desc, UtcTimestamp::now(), UtcOffset::UTC);
    write_session_bundle(dir.path(), &manifest);
    let loaded = Manifest::read_from(dir.path()).unwrap();
    assert_eq!(loaded.session_stem().unwrap(), stem());
    let bin = join_contained(dir.path(), loaded.bin_file()).unwrap();
    let csv = join_contained(dir.path(), loaded.csv_file()).unwrap();
    let root = dir.path().canonicalize().unwrap();
    assert!(bin.starts_with(&root));
    assert!(csv.starts_with(&root));
    assert!(NativeSession::open(dir.path()).is_ok());
}

#[test]
fn join_contained_rejects_parent_and_absolute_names() {
    let dir = tempdir().unwrap();
    assert!(matches!(
        join_contained(dir.path(), "../outside.bin"),
        Err(RecordingError::PathEscapesRoot { .. })
    ));
    assert!(matches!(
        join_contained(dir.path(), "..\\outside.bin"),
        Err(RecordingError::PathEscapesRoot { .. })
    ));
    let abs = dir.path().join("outside.bin");
    assert!(matches!(
        join_contained(dir.path(), abs.to_str().unwrap()),
        Err(RecordingError::PathEscapesRoot { .. })
    ));
}

fn try_file_symlink(target: &std::path::Path, link: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).expect("unix file symlink");
        true
    }
    #[cfg(windows)]
    {
        match std::os::windows::fs::symlink_file(target, link) {
            Ok(()) => true,
            Err(err) => {
                eprintln!("skipping real file-symlink coverage: {err}");
                false
            }
        }
    }
}

fn isolated_session(root: &std::path::Path, name: &str, manifest: &Manifest) -> std::path::PathBuf {
    let session = root.join(name);
    std::fs::create_dir(&session).unwrap();
    write_session_bundle(&session, manifest);
    session
}

#[test]
fn exact_name_artifact_symlink_cannot_escape_session_dir() {
    let root = tempdir().unwrap();
    let outside = root.path().join("outside.bin");
    let secret = b"do-not-read-or-replace";
    std::fs::write(&outside, secret).unwrap();

    let desc = SourceDescriptor::new(SourceId::trng(), "TrueRNG v1/v2/v3", None, None).unwrap();
    let manifest = Manifest::recording(&stem(), &desc, UtcTimestamp::now(), UtcOffset::UTC);
    let stem_name = stem().to_string();

    let bin_session = isolated_session(root.path(), "session-bin", &manifest);
    let bin_link = bin_session.join(format!("{stem_name}.bin"));
    std::fs::remove_file(&bin_link).unwrap();
    if !try_file_symlink(&outside, &bin_link) {
        write_session_bundle(&bin_session, &manifest);
        assert!(NativeSession::open(&bin_session).is_ok());
        assert_eq!(std::fs::read(&outside).unwrap(), secret);
        return;
    }

    let err = NativeSession::open(&bin_session).unwrap_err();
    assert!(
        matches!(err, RecordingError::PathEscapesRoot { .. }),
        "expected contained-path error, got {err}"
    );
    assert_eq!(std::fs::read(&outside).unwrap(), secret);

    let csv_session = isolated_session(root.path(), "session-csv", &manifest);
    let csv_link = csv_session.join(format!("{stem_name}.csv"));
    std::fs::remove_file(&csv_link).unwrap();
    assert!(
        try_file_symlink(&outside, &csv_link),
        "csv symlink should succeed after bin symlink succeeded"
    );
    let err = NativeSession::open(&csv_session).unwrap_err();
    assert!(
        matches!(err, RecordingError::PathEscapesRoot { .. }),
        "expected csv link to be rejected, got {err}"
    );
    assert_eq!(std::fs::read(&outside).unwrap(), secret);

    let manifest_session = isolated_session(root.path(), "session-manifest", &manifest);
    let manifest_link = manifest_session.join("manifest.json");
    std::fs::remove_file(&manifest_link).unwrap();
    assert!(
        try_file_symlink(&outside, &manifest_link),
        "manifest symlink should succeed after bin symlink succeeded"
    );
    let err = Manifest::read_from(&manifest_session).unwrap_err();
    assert!(
        matches!(
            err,
            RecordingError::PathEscapesRoot { .. } | RecordingError::Json(_)
        ),
        "expected manifest link to be rejected, got {err}"
    );
    assert_eq!(std::fs::read(&outside).unwrap(), secret);
    assert!(NativeSession::open(&manifest_session).is_err());
}

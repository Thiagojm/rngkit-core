//! Derived concatenation failure-injection tests.

use std::path::{Path, PathBuf};

use rngkit_recording::{
    ConcatenationFailPoint, RecordingError, create_legacy_csv_concatenation_at,
    with_concatenation_fail_point, with_concatenation_inspect_hook,
    with_concatenation_promote_hook,
};
use tempfile::tempdir;
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

const FILE_A: &str = "20260821T18:30:00,8\n20260821T18:30:01,8\n";
const FILE_B: &str = "20260821T18:30:10,4\n20260821T18:30:11,4\n";
const STEM_A: &str = "20260821T183000_trng_s16_i1";
const STEM_B: &str = "20260821T183010_trng_s16_i1";
const CONCAT_STEM: &str = "20260821T183000_concat_trng_s16_i1";

fn local(offset_hours: i8) -> time::OffsetDateTime {
    let date = Date::from_calendar_date(2026, Month::August, 21).expect("date");
    let time = Time::from_hms(18, 30, 0).expect("time");
    let offset = UtcOffset::from_hms(offset_hours, 0, 0).expect("offset");
    PrimitiveDateTime::new(date, time).assume_offset(offset)
}

fn offset() -> UtcOffset {
    UtcOffset::from_hms(-3, 0, 0).unwrap()
}

fn write_csv(dir: &Path, stem: &str, body: &str) -> PathBuf {
    let path = dir.join(format!("{stem}.csv"));
    std::fs::write(&path, body).unwrap();
    path
}

fn leftover_staging(root: &Path) -> Vec<String> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".rngkit-concat-") && name.ends_with(".tmp"))
        .collect()
}

fn create_at(paths: &[PathBuf], output: &Path) -> Result<PathBuf, RecordingError> {
    create_legacy_csv_concatenation_at(paths, output, local(-3), offset())
}

fn inject(point: ConcatenationFailPoint) {
    let dir = tempdir().unwrap();
    let first = write_csv(dir.path(), STEM_A, FILE_A);
    let second = write_csv(dir.path(), STEM_B, FILE_B);
    let first_bytes = std::fs::read(&first).unwrap();
    let second_bytes = std::fs::read(&second).unwrap();
    let output = dir.path().join("out");
    std::fs::create_dir(&output).unwrap();
    let dest = output.join(CONCAT_STEM);

    let err = with_concatenation_fail_point(point, || {
        create_at(&[first.clone(), second.clone()], &output)
    })
    .unwrap_err();
    assert!(
        matches!(
            err,
            RecordingError::ConcatenationWrite { ref reason, .. } if reason == "injected failure"
        ),
        "unexpected {err:?}"
    );
    assert!(!dest.exists());
    assert!(leftover_staging(&output).is_empty());
    assert_eq!(std::fs::read(&first).unwrap(), first_bytes);
    assert_eq!(std::fs::read(&second).unwrap(), second_bytes);
}

#[test]
fn fail_after_inspect() {
    inject(ConcatenationFailPoint::AfterInspect);
}

#[test]
fn fail_after_csv_write() {
    inject(ConcatenationFailPoint::AfterCsvWrite);
}

#[test]
fn fail_after_csv_sync() {
    inject(ConcatenationFailPoint::AfterCsvSync);
}

#[test]
fn fail_after_manifest_write() {
    inject(ConcatenationFailPoint::AfterManifestWrite);
}

#[test]
fn fail_after_manifest_sync() {
    inject(ConcatenationFailPoint::AfterManifestSync);
}

#[test]
fn changed_after_preview_is_rejected_without_a_final_bundle() {
    let dir = tempdir().unwrap();
    let first = write_csv(dir.path(), STEM_A, FILE_A);
    let second = write_csv(dir.path(), STEM_B, FILE_B);
    let first_bytes = std::fs::read(&first).unwrap();
    let output = dir.path().join("out");
    std::fs::create_dir(&output).unwrap();
    let dest = output.join(CONCAT_STEM);
    let mutated = b"20260821T18:30:10,1\n20260821T18:30:11,1\n";
    let second_for_hook = second.clone();

    let err = with_concatenation_inspect_hook(
        move || {
            std::fs::write(&second_for_hook, mutated).unwrap();
        },
        || create_at(&[first.clone(), second.clone()], &output),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        RecordingError::ConcatenationInputChanged { .. }
    ));
    assert!(!dest.exists());
    assert!(leftover_staging(&output).is_empty());
    assert_eq!(std::fs::read(&first).unwrap(), first_bytes);
    assert_eq!(std::fs::read(&second).unwrap(), mutated);
}

#[test]
fn concurrent_destination_is_not_replaced() {
    let dir = tempdir().unwrap();
    let first = write_csv(dir.path(), STEM_A, FILE_A);
    let second = write_csv(dir.path(), STEM_B, FILE_B);
    let first_bytes = std::fs::read(&first).unwrap();
    let second_bytes = std::fs::read(&second).unwrap();
    let output = dir.path().join("out");
    std::fs::create_dir(&output).unwrap();
    let dest = output.join(CONCAT_STEM);

    let err = with_concatenation_promote_hook(
        |dest| {
            std::fs::create_dir(dest).unwrap();
            std::fs::write(dest.join("marker.txt"), b"concurrent").unwrap();
        },
        || create_at(&[first.clone(), second.clone()], &output),
    )
    .unwrap_err();
    assert!(matches!(err, RecordingError::AlreadyExists { .. }));
    assert_eq!(
        std::fs::read(dest.join("marker.txt")).unwrap(),
        b"concurrent"
    );
    assert!(!dest.join(format!("{CONCAT_STEM}.csv")).exists());
    assert!(!dest.join("manifest.json").exists());
    assert!(leftover_staging(&output).is_empty());
    assert_eq!(std::fs::read(&first).unwrap(), first_bytes);
    assert_eq!(std::fs::read(&second).unwrap(), second_bytes);
}

#[test]
fn concurrent_empty_destination_is_not_replaced() {
    let dir = tempdir().unwrap();
    let first = write_csv(dir.path(), STEM_A, FILE_A);
    let second = write_csv(dir.path(), STEM_B, FILE_B);
    let first_bytes = std::fs::read(&first).unwrap();
    let second_bytes = std::fs::read(&second).unwrap();
    let output = dir.path().join("out");
    std::fs::create_dir(&output).unwrap();
    let dest = output.join(CONCAT_STEM);

    let err = with_concatenation_promote_hook(
        |dest| std::fs::create_dir(dest).unwrap(),
        || create_at(&[first.clone(), second.clone()], &output),
    )
    .unwrap_err();
    assert!(matches!(err, RecordingError::AlreadyExists { .. }));
    assert!(dest.is_dir());
    assert!(std::fs::read_dir(&dest).unwrap().next().is_none());
    assert!(leftover_staging(&output).is_empty());
    assert_eq!(std::fs::read(&first).unwrap(), first_bytes);
    assert_eq!(std::fs::read(&second).unwrap(), second_bytes);
}

//! Derived concatenation bundle round-trip tests.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rngkit_core::{IntervalSeconds, SampleBits, SourceId};
use rngkit_recording::{
    CONCATENATION_KIND, ConcatenationManifest, ConcatenationStem, DERIVED_CSV_COLUMNS,
    create_legacy_csv_concatenation_at, inspect_legacy_csvs, open_concatenation,
};
use tempfile::tempdir;
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

const FILE_A: &str = "20260821T18:30:00,8\n20260821T18:30:01,8\n";
const FILE_A_SHA256: &str = "cbfaf6d58acd634aa8450ed77fe04623eb6762b261ef35ae7ab8eb2fed83d1a9";
const FILE_B: &str = "20260821T18:30:10,4\n20260821T18:30:11,4\n";
const FILE_B_SHA256: &str = "0305ec81971218d796aa819d7c29f0f1a1fc804b2ee49af1bcc97e02547b2405";
const STEM_A: &str = "20260821T183000_trng_s16_i1";
const STEM_B: &str = "20260821T183010_trng_s16_i1";
const CONCAT_STEM: &str = "20260821T183000_concat_trng_s16_i1";

fn bits() -> SampleBits {
    SampleBits::new(16).expect("bits")
}

fn interval() -> IntervalSeconds {
    IntervalSeconds::new(1).expect("interval")
}

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

fn assert_no_absolute_path(text: &str, dir: &Path) {
    let displayed = dir.display().to_string();
    assert!(
        !text.contains(&displayed),
        "absolute path {displayed} leaked in {text}"
    );
    if let Ok(canonical) = std::fs::canonicalize(dir) {
        let canonical = canonical.to_string_lossy();
        assert!(
            !text.contains(canonical.as_ref()),
            "canonical path {canonical} leaked in {text}"
        );
    }
    assert!(
        !text.contains("\\\\?\\"),
        "verbatim path prefix leaked in {text}"
    );
}

fn leftover_staging(root: &Path) -> Vec<String> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".rngkit-concat-") && name.ends_with(".tmp"))
        .collect()
}

fn bundle_names(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn create_and_open_preserves_ordered_rows_and_leaves_inputs_unchanged() {
    let dir = tempdir().unwrap();
    let later = write_csv(dir.path(), STEM_B, FILE_B);
    let earlier = write_csv(dir.path(), STEM_A, FILE_A);
    let earlier_bytes = std::fs::read(&earlier).unwrap();
    let later_bytes = std::fs::read(&later).unwrap();

    let output = dir.path().join("out");
    std::fs::create_dir(&output).unwrap();
    let bundle = create_legacy_csv_concatenation_at(
        &[later.clone(), earlier.clone()],
        &output,
        local(-3),
        offset(),
    )
    .unwrap();

    assert_eq!(
        bundle.file_name().and_then(|name| name.to_str()),
        Some(CONCAT_STEM)
    );
    let csv_name = format!("{CONCAT_STEM}.csv");
    assert_eq!(
        bundle_names(&bundle),
        BTreeSet::from(["manifest.json".to_owned(), csv_name.clone()])
    );
    assert!(!bundle.join(format!("{CONCAT_STEM}.bin")).exists());
    assert!(leftover_staging(&output).is_empty());

    let csv = std::fs::read_to_string(bundle.join(&csv_name)).unwrap();
    assert_eq!(csv.lines().next().unwrap(), DERIVED_CSV_COLUMNS.join(","));
    let data_rows: Vec<&str> = csv
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(data_rows.len(), 4);
    assert!(data_rows[0].starts_with("1,2026-08-21T18:30:00Z,8,1,1"));
    assert!(data_rows[1].starts_with("2,2026-08-21T18:30:01Z,8,1,2"));
    assert!(data_rows[2].starts_with("3,2026-08-21T18:30:10Z,4,2,1"));
    assert!(data_rows[3].starts_with("4,2026-08-21T18:30:11Z,4,2,2"));

    let manifest_bytes = std::fs::read(bundle.join("manifest.json")).unwrap();
    let manifest_text = String::from_utf8(manifest_bytes.clone()).unwrap();
    assert_no_absolute_path(&manifest_text, dir.path());
    assert_no_absolute_path(&manifest_text, &output);
    assert!(!manifest_text.contains("C:"));
    let manifest = ConcatenationManifest::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest.kind(), CONCATENATION_KIND);
    assert_eq!(manifest.total_rows(), 4);
    assert_eq!(manifest.inputs()[0].basename(), format!("{STEM_A}.csv"));
    assert_eq!(manifest.inputs()[0].sha256().as_str(), FILE_A_SHA256);
    assert_eq!(manifest.inputs()[1].basename(), format!("{STEM_B}.csv"));
    assert_eq!(manifest.inputs()[1].sha256().as_str(), FILE_B_SHA256);
    assert_eq!(manifest.csv_file(), csv_name);
    assert_eq!(manifest.local_utc_offset(), "-03:00");

    let session = open_concatenation(&bundle).unwrap();
    assert_eq!(session.meta().stem, CONCAT_STEM);
    assert_eq!(session.meta().source_id.as_str(), "trng");
    assert_eq!(session.records().len(), 4);
    assert_eq!(session.records()[0].ones, 8);
    assert_eq!(session.records()[0].index.get(), 1);
    assert_eq!(session.records()[1].ones, 8);
    assert_eq!(session.records()[2].ones, 4);
    assert_eq!(session.records()[3].ones, 4);
    assert_eq!(session.records()[3].index.get(), 4);

    let preview = inspect_legacy_csvs(&[later.clone(), earlier.clone()]).unwrap();
    assert_eq!(
        session.records()[0].timestamp,
        preview.inputs()[0].first_timestamp()
    );
    assert_eq!(
        session.records()[3].timestamp,
        preview.inputs()[1].last_timestamp()
    );

    assert_eq!(std::fs::read(&earlier).unwrap(), earlier_bytes);
    assert_eq!(std::fs::read(&later).unwrap(), later_bytes);
}

#[test]
fn create_rejects_existing_destination_without_replacing_it() {
    let dir = tempdir().unwrap();
    let input = write_csv(dir.path(), STEM_A, FILE_A);
    let input_bytes = std::fs::read(&input).unwrap();
    let output = dir.path().join("out");
    std::fs::create_dir(&output).unwrap();
    let dest = output.join(CONCAT_STEM);
    std::fs::create_dir(&dest).unwrap();
    std::fs::write(dest.join("keep.txt"), b"existing").unwrap();

    let err = create_legacy_csv_concatenation_at(
        std::slice::from_ref(&input),
        &output,
        local(-3),
        offset(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        rngkit_recording::RecordingError::AlreadyExists { .. }
    ));
    assert_eq!(std::fs::read(dest.join("keep.txt")).unwrap(), b"existing");
    assert!(!dest.join(format!("{CONCAT_STEM}.csv")).exists());
    assert!(!dest.join("manifest.json").exists());
    assert_eq!(std::fs::read(&input).unwrap(), input_bytes);
    assert!(leftover_staging(&output).is_empty());
}

#[test]
fn concat_stem_is_independent_of_session_stem() {
    let stem =
        ConcatenationStem::new(local(-3), SourceId::trng(), bits(), interval(), None).unwrap();
    assert_eq!(stem.as_str(), CONCAT_STEM);
    assert!(rngkit_recording::SessionStem::parse(stem.as_str()).is_err());
}

//! Derived concatenation naming, manifest, and inspection tests.

use std::path::{Path, PathBuf};

use rngkit_core::{Fold, IntervalSeconds, SampleBits, SourceId, UtcTimestamp};
use rngkit_recording::{
    CONCATENATION_KIND, CONCATENATION_SCHEMA_VERSION, ConcatenationCompatibilityField,
    ConcatenationManifest, ConcatenationStem, NATIVE_CSV_COLUMNS, RecordingError, SessionStem,
    inspect_legacy_csvs,
};
use tempfile::tempdir;
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

const FILE_A: &str = "20260821T18:30:00,8\n20260821T18:30:01,8\n";
const FILE_A_SHA256: &str = "cbfaf6d58acd634aa8450ed77fe04623eb6762b261ef35ae7ab8eb2fed83d1a9";
const FILE_B: &str = "20260821T18:30:10,4\n20260821T18:30:11,4\n";
const FILE_B_SHA256: &str = "0305ec81971218d796aa819d7c29f0f1a1fc804b2ee49af1bcc97e02547b2405";
const STEM_A: &str = "20260821T183000_trng_s16_i1";
const STEM_B: &str = "20260821T183010_trng_s16_i1";

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

fn valid_single_input_manifest_json() -> serde_json::Value {
    let dir = tempdir().unwrap();
    let input = write_csv(dir.path(), STEM_A, FILE_A);
    let preview = inspect_legacy_csvs(&[input]).unwrap();
    let stem =
        ConcatenationStem::new(local(-3), SourceId::trng(), bits(), interval(), None).unwrap();
    let manifest = ConcatenationManifest::new(
        &stem,
        UtcTimestamp::new(local(-3).to_offset(UtcOffset::UTC)),
        UtcOffset::from_hms(-3, 0, 0).unwrap(),
        preview.inputs().to_vec(),
    )
    .unwrap();
    serde_json::to_value(manifest).unwrap()
}

#[test]
fn concat_stem_round_trip_is_independent_of_session_stem() {
    let t = local(-3);
    let stem = ConcatenationStem::new(t, SourceId::trng(), bits(), interval(), None).unwrap();
    assert_eq!(stem.as_str(), "20260821T183000_concat_trng_s16_i1");
    assert_eq!(
        stem.csv_basename(),
        "20260821T183000_concat_trng_s16_i1.csv"
    );
    assert!(SessionStem::parse(stem.as_str()).is_err());
    assert!(ConcatenationStem::parse("20260821T183000_trng_s16_i1").is_err());
    let parsed = ConcatenationStem::parse(stem.as_str()).unwrap();
    assert_eq!(parsed.as_str(), stem.as_str());
    assert_eq!(parsed.source().as_str(), "trng");
}

#[test]
fn concat_stem_bitb_requires_fold_and_others_reject_fold() {
    let t = local(0);
    let bitb = ConcatenationStem::new(
        t,
        SourceId::bitb(),
        bits(),
        interval(),
        Some(Fold::new(4).unwrap()),
    )
    .unwrap();
    assert_eq!(bitb.as_str(), "20260821T183000_concat_bitb_s16_i1_f4");
    assert!(ConcatenationStem::new(t, SourceId::bitb(), bits(), interval(), None).is_err());
    assert!(
        ConcatenationStem::new(
            t,
            SourceId::trng(),
            bits(),
            interval(),
            Some(Fold::new(0).unwrap())
        )
        .is_err()
    );
}

#[test]
fn concat_stem_rejects_hyphenated_and_path_tokens() {
    let err = ConcatenationStem::parse("20260821-183000_concat_trng_s16_i1").unwrap_err();
    assert!(matches!(err, RecordingError::UnsupportedVersion { .. }));
    assert!(ConcatenationStem::parse("20260821T183000_concat_trng_s16_i1/../x").is_err());
}

#[test]
fn inspect_orders_valid_inputs_and_returns_stable_hashes() {
    let dir = tempdir().unwrap();
    let later = write_csv(dir.path(), STEM_B, FILE_B);
    let earlier = write_csv(dir.path(), STEM_A, FILE_A);

    let preview = inspect_legacy_csvs(&[later.clone(), earlier.clone()]).unwrap();
    assert_eq!(preview.source_id().as_str(), "trng");
    assert_eq!(preview.sample_bits().get(), 16);
    assert_eq!(preview.interval().get(), 1);
    assert_eq!(preview.fold(), None);
    assert_eq!(preview.total_rows(), 4);
    assert_eq!(preview.inputs().len(), 2);
    assert_eq!(preview.inputs()[0].basename(), format!("{STEM_A}.csv"));
    assert_eq!(preview.inputs()[0].sha256().as_str(), FILE_A_SHA256);
    assert_eq!(preview.inputs()[0].row_count(), 2);
    assert_eq!(preview.inputs()[0].output_start().get(), 1);
    assert_eq!(preview.inputs()[0].output_end().get(), 2);
    assert_eq!(preview.inputs()[1].basename(), format!("{STEM_B}.csv"));
    assert_eq!(preview.inputs()[1].sha256().as_str(), FILE_B_SHA256);
    assert_eq!(preview.inputs()[1].output_start().get(), 3);
    assert_eq!(preview.inputs()[1].output_end().get(), 4);

    let debug = format!("{preview:?}");
    let json = serde_json::to_string(&preview).unwrap();
    assert_no_absolute_path(&debug, dir.path());
    assert_no_absolute_path(&json, dir.path());
    assert!(!json.contains('\\'));
    assert!(!debug.contains('\\'));
}

#[test]
fn inspect_accepts_equal_timestamps_within_one_file() {
    let dir = tempdir().unwrap();
    let path = write_csv(
        dir.path(),
        STEM_A,
        "20260821T18:30:00,8\n20260821T18:30:00,9\n",
    );
    let preview = inspect_legacy_csvs(&[path]).unwrap();
    assert_eq!(preview.total_rows(), 2);
    assert_eq!(
        preview.inputs()[0].first_timestamp(),
        preview.inputs()[0].last_timestamp()
    );
}

#[test]
fn inspect_rejects_equal_boundaries_between_files() {
    let dir = tempdir().unwrap();
    let first = write_csv(dir.path(), STEM_A, FILE_A);
    let second = write_csv(
        dir.path(),
        STEM_B,
        "20260821T18:30:01,4\n20260821T18:30:11,4\n",
    );
    let err = inspect_legacy_csvs(&[first, second]).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::OverlappingConcatenationRanges { .. }
    ));
}

#[test]
fn inspect_allows_gaps_between_files() {
    let dir = tempdir().unwrap();
    let first = write_csv(dir.path(), STEM_A, FILE_A);
    let second = write_csv(
        dir.path(),
        STEM_B,
        "20260821T18:30:02,4\n20260821T18:30:11,4\n",
    );
    let preview = inspect_legacy_csvs(&[first, second]).unwrap();
    assert_eq!(preview.total_rows(), 4);
}

#[test]
fn inspect_rejects_empty_malformed_duplicate_and_native_inputs() {
    assert!(matches!(
        inspect_legacy_csvs(&[]),
        Err(RecordingError::EmptyConcatenationInputs)
    ));

    let dir = tempdir().unwrap();
    let empty = write_csv(dir.path(), STEM_A, "");
    assert!(matches!(
        inspect_legacy_csvs(&[empty]),
        Err(RecordingError::EmptyConcatenationInput { .. })
    ));

    let v2 = dir.path().join("20260821-183000_trng_s16_i1.csv");
    std::fs::write(&v2, FILE_A).unwrap();
    assert!(matches!(
        inspect_legacy_csvs(&[v2]),
        Err(RecordingError::UnsupportedVersion { .. })
    ));

    let bin = dir.path().join(format!("{STEM_A}.bin"));
    std::fs::write(&bin, FILE_A).unwrap();
    assert!(matches!(
        inspect_legacy_csvs(&[bin]),
        Err(RecordingError::ConcatenationInputNotCsv { .. })
    ));

    let valid = write_csv(dir.path(), STEM_A, FILE_A);
    assert!(matches!(
        inspect_legacy_csvs(&[valid.clone(), valid.clone()]),
        Err(RecordingError::DuplicateConcatenationInput { .. })
    ));
    let alias = dir.path().join(".").join(format!("{STEM_A}.csv"));
    assert!(matches!(
        inspect_legacy_csvs(&[valid.clone(), alias]),
        Err(RecordingError::DuplicateConcatenationInput { .. })
    ));

    let native = write_csv(
        dir.path(),
        "20260821T183100_trng_s16_i1",
        &format!("{}\n1,0,0,8,0,2\n", NATIVE_CSV_COLUMNS.join(",")),
    );
    assert!(matches!(
        inspect_legacy_csvs(&[native]),
        Err(RecordingError::NativeConcatenationInput { .. })
    ));
}

#[test]
fn inspect_rejects_decreasing_incompatible_and_one_count_errors() {
    let dir = tempdir().unwrap();
    let decreasing = write_csv(
        dir.path(),
        STEM_A,
        "20260821T18:30:01,8\n20260821T18:30:00,8\n",
    );
    assert!(matches!(
        inspect_legacy_csvs(&[decreasing]),
        Err(RecordingError::DecreasingConcatenationTimestamp { .. })
    ));

    let ones = write_csv(dir.path(), STEM_A, "20260821T18:30:00,17\n");
    assert!(matches!(
        inspect_legacy_csvs(&[ones]),
        Err(RecordingError::OnesExceedSampleBits {
            ones: 17,
            sample_bits: 16
        })
    ));

    let left = write_csv(dir.path(), STEM_A, FILE_A);
    let mixed = write_csv(dir.path(), "20260821T183010_pseudo_s16_i1", FILE_B);
    let err = inspect_legacy_csvs(&[left.clone(), mixed]).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::IncompatibleConcatenationInputs {
            field: ConcatenationCompatibilityField::Source,
            ..
        }
    ));

    let bits = write_csv(dir.path(), "20260821T183010_trng_s32_i1", FILE_B);
    let err = inspect_legacy_csvs(&[left.clone(), bits]).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::IncompatibleConcatenationInputs {
            field: ConcatenationCompatibilityField::SampleBits,
            ..
        }
    ));

    let interval = write_csv(dir.path(), "20260821T183010_trng_s16_i2", FILE_B);
    let err = inspect_legacy_csvs(&[left.clone(), interval]).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::IncompatibleConcatenationInputs {
            field: ConcatenationCompatibilityField::Interval,
            ..
        }
    ));

    let fold0 = write_csv(dir.path(), "20260821T183000_bitb_s16_i1_f0", FILE_A);
    let fold1 = write_csv(dir.path(), "20260821T183010_bitb_s16_i1_f1", FILE_B);
    let err = inspect_legacy_csvs(&[fold0, fold1]).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::IncompatibleConcatenationInputs {
            field: ConcatenationCompatibilityField::Fold,
            ..
        }
    ));

    let rdseed = write_csv(dir.path(), "20260821T183000_rdseed_s16_i1", FILE_A);
    assert!(matches!(
        inspect_legacy_csvs(&[rdseed]),
        Err(RecordingError::UnsupportedVersion { .. })
    ));
}

#[test]
fn inspect_error_debug_uses_basenames_not_absolute_paths() {
    let dir = tempdir().unwrap();
    let first = write_csv(dir.path(), STEM_A, FILE_A);
    let second = write_csv(
        dir.path(),
        STEM_B,
        "20260821T18:30:01,4\n20260821T18:30:11,4\n",
    );
    let err = inspect_legacy_csvs(&[first, second]).unwrap_err();
    let debug = format!("{err:?}");
    let display = err.to_string();
    assert_no_absolute_path(&debug, dir.path());
    assert_no_absolute_path(&display, dir.path());
    assert!(display.contains(&format!("{STEM_A}.csv")));
    assert!(display.contains(&format!("{STEM_B}.csv")));
}

#[test]
fn manifest_round_trip_has_no_absolute_paths() {
    let dir = tempdir().unwrap();
    let earlier = write_csv(dir.path(), STEM_A, FILE_A);
    let later = write_csv(dir.path(), STEM_B, FILE_B);
    let preview = inspect_legacy_csvs(&[later, earlier]).unwrap();
    let stem =
        ConcatenationStem::new(local(-3), SourceId::trng(), bits(), interval(), None).unwrap();
    let created = UtcTimestamp::new(local(-3).to_offset(UtcOffset::UTC));
    let manifest = ConcatenationManifest::new(
        &stem,
        created,
        UtcOffset::from_hms(-3, 0, 0).unwrap(),
        preview.inputs().to_vec(),
    )
    .unwrap();
    assert_eq!(manifest.schema_version(), CONCATENATION_SCHEMA_VERSION);
    assert_eq!(manifest.kind(), CONCATENATION_KIND);
    assert_eq!(manifest.total_rows(), 4);
    assert_eq!(manifest.local_utc_offset(), "-03:00");
    assert_eq!(manifest.csv_file(), stem.csv_basename());

    let json = serde_json::to_vec_pretty(&manifest).unwrap();
    let text = String::from_utf8(json.clone()).unwrap();
    assert_no_absolute_path(&text, dir.path());
    assert!(!text.contains("C:"));
    assert!(!text.to_lowercase().contains("appdata"));
    let loaded = ConcatenationManifest::from_slice(&json).unwrap();
    assert_eq!(loaded, manifest);
    assert_eq!(loaded.inputs()[0].sha256().as_str(), FILE_A_SHA256);
}

#[test]
fn manifest_rejects_unknown_schema_and_kind() {
    let mut unknown_schema = valid_single_input_manifest_json();
    unknown_schema["schema_version"] = serde_json::json!(2);
    let encoded = serde_json::to_vec(&unknown_schema).unwrap();
    let err = ConcatenationManifest::from_slice(&encoded).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::UnsupportedSchema { version: 2 }
    ));

    let mut unknown_kind = valid_single_input_manifest_json();
    unknown_kind["kind"] = serde_json::json!("native_session");
    let encoded = serde_json::to_vec(&unknown_kind).unwrap();
    let err = ConcatenationManifest::from_slice(&encoded).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::UnsupportedConcatenationKind { ref kind } if kind == "native_session"
    ));
}

#[test]
fn manifest_rejects_tampered_input_invariants() {
    let mut zero_rows = valid_single_input_manifest_json();
    zero_rows["inputs"][0]["row_count"] = serde_json::json!(0);
    zero_rows["total_rows"] = serde_json::json!(0);
    let encoded = serde_json::to_vec(&zero_rows).unwrap();
    let err = ConcatenationManifest::from_slice(&encoded).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::EmptyConcatenationInput { .. }
    ));

    let mut reversed_timestamps = valid_single_input_manifest_json();
    reversed_timestamps["inputs"][0]["first_timestamp"] = serde_json::json!("2026-08-21T18:30:10Z");
    reversed_timestamps["inputs"][0]["last_timestamp"] = serde_json::json!("2026-08-21T18:30:00Z");
    let encoded = serde_json::to_vec(&reversed_timestamps).unwrap();
    let err = ConcatenationManifest::from_slice(&encoded).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::InconsistentConcatenationRange
    ));

    let mut inconsistent_span = valid_single_input_manifest_json();
    inconsistent_span["inputs"][0]["output_end"] = serde_json::json!(99);
    let encoded = serde_json::to_vec(&inconsistent_span).unwrap();
    let err = ConcatenationManifest::from_slice(&encoded).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::InconsistentConcatenationRange
    ));
}

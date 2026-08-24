//! Format-neutral CSV concatenation tests.

use std::path::{Path, PathBuf};

use rngkit_recording::{
    CSV_CONCATENATION_KIND, CSV_CONCATENATION_SCHEMA_VERSION, DERIVED_CSV_COLUMNS,
    StandaloneInputFormat, create_csv_concatenation_at, inspect_csv_inputs, open_concatenation,
};
use tempfile::tempdir;
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

const HEADER: &str =
    "sample_index,captured_at_utc,elapsed_ms,acquisition_ms,ones,byte_offset,byte_length\n";

fn local() -> time::OffsetDateTime {
    PrimitiveDateTime::new(
        Date::from_calendar_date(2026, Month::August, 24).unwrap(),
        Time::from_hms(15, 0, 0).unwrap(),
    )
    .assume_offset(UtcOffset::UTC)
}

fn write(path: &Path, body: &str) -> PathBuf {
    std::fs::write(path, body).unwrap();
    path.to_path_buf()
}

#[test]
fn legacy_current_and_mixed_sets_preview_and_create_schema_two() {
    let dir = tempdir().unwrap();
    let legacy = write(
        &dir.path().join("20260824T145947_trng_s16_i1.csv"),
        "20260824T145948,8\n20260824T145949,7\n",
    );
    let current = write(
        &dir.path().join("20260824T145950_trng_s16_i1.csv"),
        &format!(
            "{HEADER}1,2026-08-24T14:59:50Z,1000,2,6,0,2\n2,2026-08-24T14:59:51Z,2000,2,5,2,2\n"
        ),
    );

    let preview = inspect_csv_inputs(&[current.clone(), legacy.clone()]).unwrap();
    assert_eq!(preview.total_rows(), 4);
    assert_eq!(preview.inputs().len(), 2);
    assert_eq!(
        preview.inputs()[0].format(),
        Some(StandaloneInputFormat::LegacyV3Csv)
    );
    assert_eq!(
        preview.inputs()[1].format(),
        Some(StandaloneInputFormat::CurrentCsv)
    );

    let output = dir.path().join("out");
    let bundle = create_csv_concatenation_at(
        &[current.clone(), legacy.clone()],
        &output,
        local(),
        UtcOffset::UTC,
    )
    .unwrap();
    let manifest_bytes = std::fs::read(bundle.join("manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(
        manifest["schema_version"],
        serde_json::json!(CSV_CONCATENATION_SCHEMA_VERSION)
    );
    assert_eq!(manifest["kind"], serde_json::json!(CSV_CONCATENATION_KIND));
    assert_eq!(
        manifest["inputs"][0]["format"],
        serde_json::json!("legacy_v3_csv")
    );
    assert_eq!(
        manifest["inputs"][1]["format"],
        serde_json::json!("current_csv")
    );
    let session = open_concatenation(&bundle).unwrap();
    assert_eq!(session.records().len(), 4);
    assert_eq!(session.records()[0].ones, 8);
    assert_eq!(session.records()[3].ones, 5);

    let output_csv = std::fs::read_to_string(bundle.join(format!(
        "{}.csv",
        bundle.file_name().unwrap().to_string_lossy()
    )))
    .unwrap();
    assert_eq!(
        output_csv.lines().next().unwrap(),
        DERIVED_CSV_COLUMNS.join(",")
    );
}

#[test]
fn generic_inspection_rejects_bin_and_overlapping_mixed_ranges() {
    let dir = tempdir().unwrap();
    let legacy = write(
        &dir.path().join("20260824T145947_trng_s16_i1.csv"),
        "20260824T145948,8\n20260824T145949,7\n",
    );
    let current = write(
        &dir.path().join("20260824T145950_trng_s16_i1.csv"),
        &format!("{HEADER}1,2026-08-24T14:59:49Z,1000,2,6,0,2\n"),
    );
    let bin_path = dir.path().join("20260824T145950_trng_s16_i1.bin");
    std::fs::write(&bin_path, [0u8, 0u8]).unwrap();
    assert!(matches!(
        inspect_csv_inputs(&[bin_path]),
        Err(rngkit_recording::RecordingError::ConcatenationInputNotCsv { .. })
    ));
    assert!(matches!(
        inspect_csv_inputs(&[legacy, current]),
        Err(rngkit_recording::RecordingError::OverlappingConcatenationRanges { .. })
    ));
}

#[test]
fn current_rdseed_concatenation_keeps_the_friendly_source_label() {
    let dir = tempdir().unwrap();
    let current = write(
        &dir.path().join("20260824T145950_rdseed_s16_i1.csv"),
        &format!("{HEADER}1,2026-08-24T14:59:50Z,1000,2,6,0,2\n"),
    );
    let bundle =
        create_csv_concatenation_at(&[current], &dir.path().join("out"), local(), UtcOffset::UTC)
            .unwrap();

    let session = open_concatenation(bundle).unwrap();
    assert_eq!(session.meta().source_label, "RDSEED");
}

//! Format-neutral CSV concatenation tests.

use std::path::{Path, PathBuf};

use rngkit_core::{Fold, SourceId};
use rngkit_recording::{
    CSV_CONCATENATION_KIND, CSV_CONCATENATION_SCHEMA_VERSION, ConcatenationCompatibilityField,
    ConcatenationManifest, DERIVED_CSV_COLUMNS, MIXED_CSV_CONCATENATION_SCHEMA_VERSION,
    MIXED_SOURCE_ID, MIXED_SOURCE_LABEL, RecordingError, StandaloneInputFormat,
    create_csv_concatenation_at, inspect_csv_inputs, inspect_legacy_csvs, open_concatenation,
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
    assert!(manifest["inputs"][0].get("source_id").is_none());
    assert!(manifest["inputs"][0].get("fold").is_none());
    assert!(manifest["inputs"][1].get("source_id").is_none());
    assert!(manifest["inputs"][1].get("fold").is_none());
    assert_eq!(manifest["source_id"], serde_json::json!("trng"));
    assert_eq!(
        preview.inputs()[0].source_id().map(SourceId::as_str),
        Some("trng")
    );
    assert_eq!(
        preview.inputs()[1].source_id().map(SourceId::as_str),
        Some("trng")
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
}

fn create_mixed_bundle(dir: &Path) -> (PathBuf, serde_json::Value) {
    let trng = write(
        &dir.join("20260824T145947_trng_s16_i1.csv"),
        "20260824T145948,8\n20260824T145949,7\n",
    );
    let pseudo = write(
        &dir.join("20260824T145950_pseudo_s16_i1.csv"),
        "20260824T145950,6\n20260824T145951,5\n",
    );
    let bundle =
        create_csv_concatenation_at(&[trng, pseudo], &dir.join("out"), local(), UtcOffset::UTC)
            .unwrap();
    let manifest_bytes = std::fs::read(bundle.join("manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    (bundle, manifest)
}

#[test]
fn mixed_source_inspect_and_create_preserves_values() {
    assert_eq!(SourceId::new(MIXED_SOURCE_ID).unwrap().as_str(), "mixed");

    let dir = tempdir().unwrap();
    let trng = write(
        &dir.path().join("20260824T145947_trng_s16_i1.csv"),
        "20260824T145948,8\n20260824T145949,7\n",
    );
    let pseudo = write(
        &dir.path().join("20260824T145950_pseudo_s16_i1.csv"),
        "20260824T145950,6\n20260824T145951,5\n",
    );

    let preview = inspect_csv_inputs(&[pseudo.clone(), trng.clone()]).unwrap();
    assert_eq!(preview.source_id().as_str(), MIXED_SOURCE_ID);
    assert_eq!(preview.fold(), None);
    assert_eq!(preview.total_rows(), 4);
    assert_eq!(
        preview.inputs()[0].source_id().map(SourceId::as_str),
        Some("trng")
    );
    assert_eq!(preview.inputs()[0].fold(), None);
    assert_eq!(
        preview.inputs()[1].source_id().map(SourceId::as_str),
        Some("pseudo")
    );

    let bundle = create_csv_concatenation_at(
        &[pseudo, trng],
        &dir.path().join("out"),
        local(),
        UtcOffset::UTC,
    )
    .unwrap();
    let stem = bundle.file_name().unwrap().to_string_lossy();
    assert!(stem.contains("_concat_mixed_"));
    assert!(!stem.contains("_f"));
    assert_eq!(stem.as_ref(), "20260824T150000_concat_mixed_s16_i1");

    let manifest_bytes = std::fs::read(bundle.join("manifest.json")).unwrap();
    let manifest_text = String::from_utf8(manifest_bytes.clone()).unwrap();
    assert_no_absolute_path(&manifest_text, dir.path());
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(
        manifest["schema_version"],
        serde_json::json!(MIXED_CSV_CONCATENATION_SCHEMA_VERSION)
    );
    assert_eq!(manifest["kind"], serde_json::json!(CSV_CONCATENATION_KIND));
    assert_eq!(manifest["source_id"], serde_json::json!(MIXED_SOURCE_ID));
    assert!(manifest.get("fold").is_none());
    assert_eq!(
        manifest["inputs"][0]["source_id"],
        serde_json::json!("trng")
    );
    assert_eq!(
        manifest["inputs"][1]["source_id"],
        serde_json::json!("pseudo")
    );

    let session = open_concatenation(&bundle).unwrap();
    assert_eq!(session.meta().source_id.as_str(), MIXED_SOURCE_ID);
    assert_eq!(session.meta().source_label, MIXED_SOURCE_LABEL);
    assert_eq!(session.meta().fold, None);
    assert_eq!(session.records().len(), 4);
    assert_eq!(session.records()[0].ones, 8);
    assert_eq!(session.records()[1].ones, 7);
    assert_eq!(session.records()[2].ones, 6);
    assert_eq!(session.records()[3].ones, 5);
    assert_eq!(
        session.records()[0].timestamp,
        preview.inputs()[0].first_timestamp()
    );
    assert_eq!(
        session.records()[3].timestamp,
        preview.inputs()[1].last_timestamp()
    );
}

#[test]
fn mixed_bitb_folds_preserve_per_input_fold() {
    let dir = tempdir().unwrap();
    let fold0 = write(
        &dir.path().join("20260824T145947_bitb_s16_i1_f0.csv"),
        "20260824T145948,8\n20260824T145949,7\n",
    );
    let fold1 = write(
        &dir.path().join("20260824T145950_bitb_s16_i1_f1.csv"),
        "20260824T145950,6\n20260824T145951,5\n",
    );

    let preview = inspect_csv_inputs(&[fold0.clone(), fold1.clone()]).unwrap();
    assert_eq!(preview.source_id().as_str(), MIXED_SOURCE_ID);
    assert_eq!(preview.fold(), None);
    assert_eq!(
        preview.inputs()[0].source_id().map(SourceId::as_str),
        Some("bitb")
    );
    assert_eq!(preview.inputs()[0].fold(), Some(Fold::new(0).unwrap()));
    assert_eq!(preview.inputs()[1].fold(), Some(Fold::new(1).unwrap()));

    let bundle = create_csv_concatenation_at(
        &[fold0, fold1],
        &dir.path().join("out"),
        local(),
        UtcOffset::UTC,
    )
    .unwrap();
    let stem = bundle.file_name().unwrap().to_string_lossy();
    assert_eq!(stem.as_ref(), "20260824T150000_concat_mixed_s16_i1");

    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(bundle.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["schema_version"],
        serde_json::json!(MIXED_CSV_CONCATENATION_SCHEMA_VERSION)
    );
    assert_eq!(manifest["inputs"][0]["fold"], serde_json::json!(0));
    assert_eq!(manifest["inputs"][1]["fold"], serde_json::json!(1));
    assert!(manifest.get("fold").is_none());

    let session = open_concatenation(&bundle).unwrap();
    assert_eq!(session.meta().source_label, MIXED_SOURCE_LABEL);
    assert_eq!(session.records()[0].ones, 8);
    assert_eq!(session.records()[3].ones, 5);
}

#[test]
fn format_neutral_rejects_unequal_bits_before_interval() {
    let dir = tempdir().unwrap();
    let left = write(
        &dir.path().join("20260824T145947_trng_s2048_i2.csv"),
        "20260824T145948,8\n",
    );
    let right = write(
        &dir.path().join("20260824T145950_trng_s1024_i1.csv"),
        "20260824T145950,8\n",
    );
    let err = inspect_csv_inputs(&[left, right]).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::IncompatibleConcatenationInputs {
            field: ConcatenationCompatibilityField::SampleBits,
            ..
        }
    ));
}

#[test]
fn format_neutral_rejects_unequal_interval_when_bits_match() {
    let dir = tempdir().unwrap();
    let left = write(
        &dir.path().join("20260824T145947_trng_s16_i1.csv"),
        "20260824T145948,8\n",
    );
    let right = write(
        &dir.path().join("20260824T145950_pseudo_s16_i2.csv"),
        "20260824T145950,8\n",
    );
    let err = inspect_csv_inputs(&[left, right]).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::IncompatibleConcatenationInputs {
            field: ConcatenationCompatibilityField::Interval,
            ..
        }
    ));
}

#[test]
fn format_neutral_still_rejects_duplicates_and_overlap() {
    let dir = tempdir().unwrap();
    let trng = write(
        &dir.path().join("20260824T145947_trng_s16_i1.csv"),
        "20260824T145948,8\n20260824T145949,7\n",
    );
    assert!(matches!(
        inspect_csv_inputs(&[trng.clone(), trng.clone()]),
        Err(RecordingError::DuplicateConcatenationInput { .. })
    ));

    let overlapping = write(
        &dir.path().join("20260824T145950_pseudo_s16_i1.csv"),
        "20260824T145949,6\n20260824T145951,5\n",
    );
    assert!(matches!(
        inspect_csv_inputs(&[trng, overlapping]),
        Err(RecordingError::OverlappingConcatenationRanges { .. })
    ));
}

#[test]
fn inspect_legacy_still_rejects_differing_source_and_fold() {
    let dir = tempdir().unwrap();
    let trng = write(
        &dir.path().join("20260824T145947_trng_s16_i1.csv"),
        "20260824T145948,8\n",
    );
    let pseudo = write(
        &dir.path().join("20260824T145950_pseudo_s16_i1.csv"),
        "20260824T145950,6\n",
    );
    assert!(matches!(
        inspect_legacy_csvs(&[trng, pseudo]),
        Err(RecordingError::IncompatibleConcatenationInputs {
            field: ConcatenationCompatibilityField::Source,
            ..
        })
    ));

    let fold0 = write(
        &dir.path().join("20260824T145947_bitb_s16_i1_f0.csv"),
        "20260824T145948,8\n",
    );
    let fold1 = write(
        &dir.path().join("20260824T145950_bitb_s16_i1_f1.csv"),
        "20260824T145950,6\n",
    );
    assert!(matches!(
        inspect_legacy_csvs(&[fold0, fold1]),
        Err(RecordingError::IncompatibleConcatenationInputs {
            field: ConcatenationCompatibilityField::Fold,
            ..
        })
    ));
}

#[test]
fn schema_three_rejects_masquerade_and_unsupported_schema() {
    let dir = tempdir().unwrap();
    let (_bundle, mixed) = create_mixed_bundle(dir.path());

    let mut schema2_mixed = mixed.clone();
    schema2_mixed["schema_version"] = serde_json::json!(CSV_CONCATENATION_SCHEMA_VERSION);
    let err = ConcatenationManifest::from_slice(&serde_json::to_vec(&schema2_mixed).unwrap())
        .unwrap_err();
    assert!(matches!(err, RecordingError::Corrupt { .. }));

    let mut not_mixed = mixed.clone();
    not_mixed["source_id"] = serde_json::json!("trng");
    not_mixed["stem"] = serde_json::json!("20260824T150000_concat_trng_s16_i1");
    not_mixed["csv_file"] = serde_json::json!("20260824T150000_concat_trng_s16_i1.csv");
    let err =
        ConcatenationManifest::from_slice(&serde_json::to_vec(&not_mixed).unwrap()).unwrap_err();
    assert!(matches!(err, RecordingError::Corrupt { .. }));

    let mut schema4 = mixed.clone();
    schema4["schema_version"] = serde_json::json!(4);
    let err =
        ConcatenationManifest::from_slice(&serde_json::to_vec(&schema4).unwrap()).unwrap_err();
    assert!(matches!(
        err,
        RecordingError::UnsupportedSchema { version: 4 }
    ));
}

#[test]
fn schema_two_rejects_per_input_provenance() {
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
    let bundle = create_csv_concatenation_at(
        &[current, legacy],
        &dir.path().join("out"),
        local(),
        UtcOffset::UTC,
    )
    .unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(bundle.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["schema_version"],
        serde_json::json!(CSV_CONCATENATION_SCHEMA_VERSION)
    );
    manifest["inputs"][0]["source_id"] = serde_json::json!("trng");
    let err =
        ConcatenationManifest::from_slice(&serde_json::to_vec(&manifest).unwrap()).unwrap_err();
    assert!(matches!(err, RecordingError::Corrupt { .. }));
}

#[test]
fn schema_three_rejects_provenance_and_identity_mismatches() {
    let dir = tempdir().unwrap();
    let (_bundle, mixed) = create_mixed_bundle(dir.path());

    let mut provenance = mixed.clone();
    provenance["inputs"][1]["source_id"] = serde_json::json!("trng");
    let err =
        ConcatenationManifest::from_slice(&serde_json::to_vec(&provenance).unwrap()).unwrap_err();
    assert!(matches!(err, RecordingError::Corrupt { .. }));

    let mut bits = mixed.clone();
    bits["inputs"][1]["basename"] = serde_json::json!("20260824T145950_pseudo_s32_i1.csv");
    let err = ConcatenationManifest::from_slice(&serde_json::to_vec(&bits).unwrap()).unwrap_err();
    assert!(matches!(err, RecordingError::Corrupt { .. }));

    let mut interval = mixed.clone();
    interval["inputs"][1]["basename"] = serde_json::json!("20260824T145950_pseudo_s16_i2.csv");
    let err =
        ConcatenationManifest::from_slice(&serde_json::to_vec(&interval).unwrap()).unwrap_err();
    assert!(matches!(err, RecordingError::Corrupt { .. }));

    let mut mixed_input = mixed.clone();
    mixed_input["inputs"][1]["source_id"] = serde_json::json!(MIXED_SOURCE_ID);
    mixed_input["inputs"][1]["basename"] = serde_json::json!("20260824T145950_mixed_s16_i1.csv");
    let err =
        ConcatenationManifest::from_slice(&serde_json::to_vec(&mixed_input).unwrap()).unwrap_err();
    assert!(matches!(err, RecordingError::Corrupt { .. }));

    let mut rdseed_legacy = mixed.clone();
    rdseed_legacy["inputs"][1]["source_id"] = serde_json::json!("rdseed");
    rdseed_legacy["inputs"][1]["basename"] = serde_json::json!("20260824T145950_rdseed_s16_i1.csv");
    rdseed_legacy["inputs"][1]["format"] = serde_json::json!("legacy_v3_csv");
    let err = ConcatenationManifest::from_slice(&serde_json::to_vec(&rdseed_legacy).unwrap())
        .unwrap_err();
    assert!(matches!(err, RecordingError::UnsupportedVersion { .. }));

    let mut homogeneous = mixed.clone();
    homogeneous["inputs"][1]["source_id"] = serde_json::json!("trng");
    homogeneous["inputs"][1]["basename"] = serde_json::json!("20260824T145950_trng_s16_i1.csv");
    let err =
        ConcatenationManifest::from_slice(&serde_json::to_vec(&homogeneous).unwrap()).unwrap_err();
    assert!(matches!(err, RecordingError::Corrupt { .. }));
}

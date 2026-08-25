//! Flat legacy concatenation CSV normalization tests.

use std::path::{Path, PathBuf};

use rngkit_core::TimestampProvenance;
use rngkit_recording::{
    ConcatenationStem, RecordingError, SessionStem, StandaloneInputFormat,
    open_flat_legacy_concatenation, open_standalone,
};
use tempfile::tempdir;

const CURRENT_HEADER: &str =
    "sample_index,captured_at_utc,elapsed_ms,acquisition_ms,ones,byte_offset,byte_length\n";

fn write(path: &Path, contents: impl AsRef<[u8]>) -> PathBuf {
    std::fs::write(path, contents).unwrap();
    path.to_path_buf()
}

#[test]
fn canonical_flat_concat_is_read_without_a_manifest() {
    let dir = tempdir().unwrap();
    let path = write(
        &dir.path()
            .join("20260824T145947_concat_bitb_s2048_i1_f0.csv"),
        "20260824T145948,1014\n20260824T145949,1001\n",
    );
    let original = std::fs::read(&path).unwrap();

    let session = open_flat_legacy_concatenation(&path).unwrap();

    assert_eq!(
        session.meta().stem,
        "20260824T145947_concat_bitb_s2048_i1_f0"
    );
    assert_eq!(session.meta().source_id.as_str(), "bitb");
    assert_eq!(session.meta().sample_bits.get(), 2048);
    assert_eq!(session.meta().provenance, TimestampProvenance::Recorded);
    assert_eq!(session.records().len(), 2);
    assert_eq!(
        session.meta().started_at,
        Some(session.records()[0].timestamp)
    );
    assert_eq!(
        session.meta().completed_at,
        Some(session.records()[1].timestamp)
    );
    assert_eq!(session.records()[0].index.get(), 1);
    assert_eq!(session.records()[1].ones, 1001);
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn standalone_entry_point_classifies_flat_concat_distinctly() {
    let dir = tempdir().unwrap();
    let path = write(
        &dir.path().join("20260824T145947_concat_trng_s16_i1.csv"),
        "20260824T145948,8\n",
    );

    let session = open_standalone(&path).unwrap();
    assert_eq!(session.records().len(), 1);
    assert_eq!(
        serde_json::to_string(&StandaloneInputFormat::FlatLegacyConcatenation).unwrap(),
        "\"flat_legacy_concatenation\""
    );
    assert!(SessionStem::parse("20260824T145947_concat_trng_s16_i1").is_err());
    assert!(ConcatenationStem::parse("20260824T145947_concat_trng_s16_i1").is_ok());
}

#[test]
fn flat_concat_rejects_current_header() {
    let dir = tempdir().unwrap();
    let path = write(
        &dir.path().join("20260824T145947_concat_trng_s16_i1.csv"),
        format!("{CURRENT_HEADER}1,2026-08-24T14:59:48Z,1000,2,8,0,2\n"),
    );

    assert!(matches!(
        open_flat_legacy_concatenation(&path),
        Err(RecordingError::InvalidNativeCsvHeader { .. })
    ));
}

#[test]
fn flat_concat_rejects_empty_decreasing_and_overflow_inputs() {
    let dir = tempdir().unwrap();
    let stem = "20260824T145947_concat_trng_s16_i1";
    let path = dir.path().join(format!("{stem}.csv"));

    write(&path, "\n");
    assert!(matches!(
        open_flat_legacy_concatenation(&path),
        Err(RecordingError::EmptyConcatenationInput { .. })
    ));

    write(&path, "20260824T145949,8\n20260824T145948,8\n");
    assert!(matches!(
        open_flat_legacy_concatenation(&path),
        Err(RecordingError::DecreasingConcatenationTimestamp { .. })
    ));

    write(&path, "20260824T145948,17\n");
    assert!(matches!(
        open_flat_legacy_concatenation(&path),
        Err(RecordingError::OnesExceedSampleBits { .. })
    ));
}

#[test]
fn flat_concat_rejects_noncanonical_extension_and_legacy_source() {
    let dir = tempdir().unwrap();
    let csv = dir.path().join("20260824T145947_concat_rdseed_s16_i1.csv");
    write(&csv, "20260824T145948,8\n");
    assert!(matches!(
        open_flat_legacy_concatenation(&csv),
        Err(RecordingError::UnsupportedVersion { .. })
    ));

    let bin = dir.path().join("20260824T145947_concat_trng_s16_i1.bin");
    write(&bin, [0u8; 2]);
    assert!(matches!(
        open_flat_legacy_concatenation(&bin),
        Err(RecordingError::InvalidName { .. })
    ));
}

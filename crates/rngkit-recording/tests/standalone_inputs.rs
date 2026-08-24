//! Standalone current/legacy CSV and BIN normalization tests.

use std::path::{Path, PathBuf};

use rngkit_core::TimestampProvenance;
use rngkit_recording::{RecordingError, StandaloneInputFormat, open_standalone};
use tempfile::tempdir;

const CURRENT_HEADER: &str =
    "sample_index,captured_at_utc,elapsed_ms,acquisition_ms,ones,byte_offset,byte_length\n";

fn current_csv(rows: &str) -> String {
    format!("{CURRENT_HEADER}{rows}")
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) -> PathBuf {
    std::fs::write(path, bytes).unwrap();
    path.to_path_buf()
}

#[test]
fn compact_legacy_csv_is_read_without_a_manifest() {
    let dir = tempdir().unwrap();
    let path = write(
        &dir.path().join("20260824T145947_bitb_s2048_i1_f0.csv"),
        "20260824T145948,1014\n20260824T145949,1001\n",
    );

    let session = open_standalone(&path).unwrap();
    assert_eq!(session.meta().source_id.as_str(), "bitb");
    assert_eq!(session.meta().sample_bits.get(), 2048);
    assert_eq!(session.records().len(), 2);
    assert_eq!(session.records()[0].ones, 1014);
    assert_eq!(session.meta().provenance, TimestampProvenance::Recorded);
}

#[test]
fn current_csv_supports_all_current_source_ids() {
    let dir = tempdir().unwrap();
    for stem in [
        "20260824T145947_bitb_s16_i1_f0",
        "20260824T145947_trng_s16_i1",
        "20260824T145947_rdseed_s16_i1",
        "20260824T145947_pseudo_s16_i1",
    ] {
        let path = write(
            &dir.path().join(format!("{stem}.csv")),
            current_csv("1,2026-08-24T14:59:48Z,1000,2,8,0,2\n"),
        );
        let session = open_standalone(&path).unwrap();
        assert_eq!(session.records().len(), 1);
        assert_eq!(session.records()[0].byte_length.unwrap().get(), 2);
    }
}

#[test]
fn current_bin_rdseed_is_estimated_and_rejects_partial_samples() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("20260824T145947_rdseed_s16_i1.bin");
    write(&path, [0xFF, 0x00, 0x01, 0x00]);
    let session = open_standalone(&path).unwrap();
    assert_eq!(session.records().len(), 2);
    assert_eq!(session.records()[0].ones, 8);
    assert_eq!(session.meta().provenance, TimestampProvenance::Estimated);

    write(&path, [0xFF]);
    assert!(matches!(
        open_standalone(&path),
        Err(RecordingError::Corrupt { .. })
    ));
}

#[test]
fn current_csv_pair_checks_bin_popcounts_and_offsets() {
    let dir = tempdir().unwrap();
    let stem = "20260824T145947_pseudo_s16_i1";
    let csv_path = write(
        &dir.path().join(format!("{stem}.csv")),
        current_csv("1,2026-08-24T14:59:48Z,1000,2,8,0,2\n"),
    );
    write(&dir.path().join(format!("{stem}.bin")), [0x00, 0x00]);
    assert!(matches!(
        open_standalone(&csv_path),
        Err(RecordingError::Corrupt { .. })
    ));
}

#[test]
fn current_csv_validation_is_fail_closed() {
    let dir = tempdir().unwrap();
    let stem = "20260824T145947_trng_s16_i1";
    let path = dir.path().join(format!("{stem}.csv"));

    write(
        &path,
        "sample_index,captured_at_utc,elapsed_ms,acquisition_ms,ones,byte_offset\n",
    );
    assert!(matches!(
        open_standalone(&path),
        Err(RecordingError::InvalidNativeCsvHeader { .. })
    ));

    write(&path, current_csv("2,2026-08-24T14:59:48Z,1000,2,8,0,2\n"));
    assert!(matches!(
        open_standalone(&path),
        Err(RecordingError::Corrupt { .. })
    ));

    write(&path, current_csv("1,2026-08-24T14:59:48Z,1000,2,17,0,2\n"));
    assert!(matches!(
        open_standalone(&path),
        Err(RecordingError::OnesExceedSampleBits { .. })
    ));
}

#[test]
fn format_enum_has_stable_serialized_labels() {
    assert_eq!(
        serde_json::to_string(&StandaloneInputFormat::CurrentCsv).unwrap(),
        "\"current_csv\""
    );
    assert_eq!(
        serde_json::to_string(&StandaloneInputFormat::LegacyV3Csv).unwrap(),
        "\"legacy_v3_csv\""
    );
}

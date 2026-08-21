//! Read-only version 3 import tests.

use std::io::Write;

use rngkit_core::{SOURCE_ID_TRNG, TimestampProvenance};
use rngkit_recording::{RecordingError, open_legacy};
use tempfile::tempdir;

fn write_pair(
    dir: &std::path::Path,
    stem: &str,
    csv: &str,
    bin: &[u8],
) -> (std::path::PathBuf, std::path::PathBuf) {
    let csv_path = dir.join(format!("{stem}.csv"));
    let bin_path = dir.join(format!("{stem}.bin"));
    std::fs::write(&csv_path, csv).unwrap();
    std::fs::write(&bin_path, bin).unwrap();
    (csv_path, bin_path)
}

fn hash(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

#[test]
fn csv_only_bin_only_and_paired() {
    let dir = tempdir().unwrap();
    let stem = "20260821T183000_trng_s16_i1";
    let sample = [0xFFu8, 0x00];
    let csv = "20260821T18:30:00,8\n";
    let (csv_path, bin_path) = write_pair(dir.path(), stem, csv, &sample);
    let before_csv = hash(&csv_path);
    let before_bin = hash(&bin_path);

    let paired = open_legacy(&csv_path).unwrap();
    assert_eq!(paired.meta().source_id.as_str(), SOURCE_ID_TRNG);
    assert_eq!(paired.records().len(), 1);
    assert_eq!(paired.records()[0].ones, 8);
    assert_eq!(paired.meta().provenance, TimestampProvenance::Recorded);

    std::fs::remove_file(&bin_path).unwrap();
    let csv_only = open_legacy(&csv_path).unwrap();
    assert_eq!(csv_only.records()[0].ones, 8);
    assert!(csv_only.records()[0].byte_offset.is_none());

    std::fs::write(&bin_path, sample).unwrap();
    std::fs::remove_file(&csv_path).unwrap();
    let bin_only = open_legacy(&bin_path).unwrap();
    assert_eq!(bin_only.meta().provenance, TimestampProvenance::Estimated);
    assert_eq!(bin_only.records()[0].ones, 8);

    std::fs::write(&csv_path, csv).unwrap();
    assert_eq!(hash(&csv_path), before_csv);
    assert_eq!(hash(&bin_path), before_bin);
}

#[test]
fn rejects_version_two_and_space_csv() {
    let dir = tempdir().unwrap();
    let v2 = dir.path().join("20260821-183000_trng_s16_i1.csv");
    std::fs::write(&v2, "18:30:00 8\n").unwrap();
    assert!(matches!(
        open_legacy(&v2),
        Err(RecordingError::UnsupportedVersion { .. })
    ));
    let stem = "20260821T183000_trng_s16_i1";
    let space = dir.path().join(format!("{stem}.csv"));
    std::fs::write(&space, "20260821T18:30:00 8\n").unwrap();
    assert!(matches!(
        open_legacy(&space),
        Err(RecordingError::UnsupportedVersion { .. })
    ));
}

#[test]
fn mismatched_pair_and_partial_bin_fail() {
    let dir = tempdir().unwrap();
    let stem = "20260821T183000_bitb_s16_i1_f0";
    let (csv_path, _) = write_pair(dir.path(), stem, "20260821T18:30:00,8\n", &[0x00, 0x00]);
    assert!(matches!(
        open_legacy(&csv_path),
        Err(RecordingError::Corrupt { .. })
    ));
    let partial = dir.path().join("20260821T183000_pseudo_s16_i1.bin");
    let mut f = std::fs::File::create(&partial).unwrap();
    f.write_all(&[0x00]).unwrap();
    assert!(matches!(
        open_legacy(&partial),
        Err(RecordingError::Corrupt { .. })
    ));
}

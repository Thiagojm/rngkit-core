//! Read-only version 3 import tests.

use std::io::Write;

use rngkit_core::{SOURCE_ID_TRNG, SampleBits, TimestampProvenance};
use rngkit_recording::legacy_v3::bin::LegacyBinReader;
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

#[test]
fn malformed_multibyte_timestamp_returns_error_without_panic() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("20260821T183000_trng_s16_i1.csv");
    std::fs::write(&path, "202é123T12:34:56,8\n").unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| open_legacy(&path)));
    assert!(result.is_ok(), "legacy import unwound on malformed UTF-8");
    assert!(matches!(
        result.unwrap(),
        Err(RecordingError::InvalidName { .. })
    ));
}

#[test]
fn csv_ones_boundaries_and_overflow() {
    let dir = tempdir().unwrap();
    let stem = "20260821T183000_trng_s16_i1";
    let csv_path = dir.path().join(format!("{stem}.csv"));

    std::fs::write(&csv_path, "20260821T18:30:00,0\n").unwrap();
    let zero = open_legacy(&csv_path).unwrap();
    assert_eq!(zero.records()[0].ones, 0);

    std::fs::write(&csv_path, "20260821T18:30:00,16\n").unwrap();
    let exact = open_legacy(&csv_path).unwrap();
    assert_eq!(exact.records()[0].ones, 16);

    std::fs::write(&csv_path, "20260821T18:30:00,17\n").unwrap();
    assert!(matches!(
        open_legacy(&csv_path),
        Err(RecordingError::OnesExceedSampleBits {
            ones: 17,
            sample_bits: 16
        })
    ));

    let (paired_csv, _) = write_pair(
        dir.path(),
        "20260821T183001_trng_s16_i1",
        "20260821T18:30:01,17\n",
        &[0xFF, 0xFF],
    );
    assert!(matches!(
        open_legacy(&paired_csv),
        Err(RecordingError::OnesExceedSampleBits {
            ones: 17,
            sample_bits: 16
        })
    ));
}

#[test]
fn bin_reader_reuses_one_sample_buffer() {
    let dir = tempdir().unwrap();
    let stem = "20260821T183000_pseudo_s16_i1";
    let path = dir.path().join(format!("{stem}.bin"));
    let mut payload = Vec::new();
    for i in 0..32u8 {
        payload.extend_from_slice(&[i, 0x00]);
    }
    std::fs::write(&path, &payload).unwrap();
    let bits = SampleBits::new(16).unwrap();
    let mut reader = LegacyBinReader::open(&path, bits).unwrap();
    let len = reader.raw_buffer_len();
    let cap = reader.raw_buffer_capacity();
    assert_eq!(len, 2);
    let mut count = 0u64;
    while let Some(meta) = reader.read_next().unwrap() {
        count += 1;
        assert_eq!(meta.index.get(), count);
        assert_eq!(reader.raw_buffer_len(), len);
        assert_eq!(reader.raw_buffer_capacity(), cap);
    }
    assert_eq!(count, 32);
    assert_eq!(reader.sample_count(), 32);
}

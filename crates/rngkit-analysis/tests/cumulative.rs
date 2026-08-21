//! Hand-calculated cumulative Z-score tests.

use rngkit_analysis::{Accumulator, AnalysisError, analyze_records};
use rngkit_core::{SampleBits, SampleIndex, SampleRecord, TimestampProvenance, UtcTimestamp};

const EPS: f64 = 1e-12;

fn bits(n: u32) -> SampleBits {
    SampleBits::new(n).expect("bits")
}

fn record(index: u64, ones: u64) -> SampleRecord {
    SampleRecord {
        index: SampleIndex::new(index).expect("index"),
        timestamp: UtcTimestamp::now(),
        provenance: TimestampProvenance::Recorded,
        elapsed: None,
        acquisition: None,
        ones,
        byte_offset: None,
        byte_length: None,
    }
}

#[test]
fn balanced_sample_has_zero_z() {
    let mut acc = Accumulator::new(bits(8));
    let snap = acc.push(4).expect("push");
    assert!((snap.z).abs() < EPS);
    assert!((snap.proportion - 0.5).abs() < EPS);
    assert!((snap.deviation).abs() < EPS);
}

#[test]
fn all_ones_is_positive_sqrt_n() {
    let mut acc = Accumulator::new(bits(8));
    let snap = acc.push(8).expect("push");
    let expected = (8.0_f64).sqrt();
    assert!((snap.z - expected).abs() < EPS);
    assert!(snap.z > 0.0);
    assert!((snap.proportion - 1.0).abs() < EPS);
    assert!((snap.deviation - 0.5).abs() < EPS);
}

#[test]
fn all_zeroes_is_negative_sqrt_n() {
    let mut acc = Accumulator::new(bits(8));
    let snap = acc.push(0).expect("push");
    let expected = -(8.0_f64).sqrt();
    assert!((snap.z - expected).abs() < EPS);
    assert!(snap.z < 0.0);
}

#[test]
fn positive_and_negative_are_symmetric() {
    let mut high = Accumulator::new(bits(16));
    let mut low = Accumulator::new(bits(16));
    let pos = high.push(12).expect("pos");
    let neg = low.push(4).expect("neg");
    assert!((pos.z + neg.z).abs() < EPS);
    assert!(pos.z > 0.0);
    assert!(neg.z < 0.0);
}

#[test]
fn two_sample_legacy_style_example() {
    // After first: C=6, N=8, Z = (12-8)/sqrt(8) = sqrt(2)
    // After second: C=8, N=16, Z = 0
    let mut acc = Accumulator::new(bits(8));
    let first = acc.push(6).expect("first");
    let expected_first = (2.0 * 6.0 - 8.0) / 8.0_f64.sqrt();
    assert!((first.z - expected_first).abs() < EPS);
    let second = acc.push(2).expect("second");
    let expected_second = (2.0 * 8.0 - 16.0) / 16.0_f64.sqrt();
    assert!((second.z - expected_second).abs() < EPS);
    assert!((second.proportion - 0.5).abs() < EPS);
}

#[test]
fn incremental_matches_batch() {
    let sample_bits = bits(16);
    let records = vec![record(1, 10), record(2, 6), record(3, 8)];
    let batch = analyze_records(sample_bits, records.clone()).expect("batch");
    let mut acc = Accumulator::new(sample_bits);
    for (record, expected) in records.iter().zip(batch.iter()) {
        let snap = acc.push_record(record).expect("inc");
        assert_eq!(snap.total_ones, expected.total_ones);
        assert_eq!(snap.total_bits, expected.total_bits);
        assert!((snap.z - expected.z).abs() < EPS);
        assert!((snap.proportion - expected.proportion).abs() < EPS);
        assert!((snap.deviation - expected.deviation).abs() < EPS);
    }
}

#[test]
fn rejects_ones_above_sample_bits() {
    let mut acc = Accumulator::new(bits(8));
    assert!(matches!(
        acc.push(9),
        Err(AnalysisError::OnesExceedSampleBits {
            ones: 9,
            sample_bits: 8
        })
    ));
}

#[test]
fn large_checked_accumulation() {
    let mut acc = Accumulator::new(bits(2048));
    let mut last = None;
    for _ in 0..10_000 {
        last = Some(acc.push(1024).expect("push"));
    }
    let snap = last.expect("last");
    assert_eq!(snap.sample_count, 10_000);
    assert_eq!(snap.total_bits, 20_480_000);
    assert_eq!(snap.total_ones, 10_240_000);
    assert!((snap.z).abs() < EPS);
}

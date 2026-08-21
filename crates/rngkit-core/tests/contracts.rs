//! Boundary tests for core domain contracts.

use rngkit_core::{
    ByteLength, EntropySource, Fold, IntervalSeconds, SampleBits, SampleIndex, SourceDescriptor,
    SourceError, SourceErrorKind, SourceId, count_ones,
};

struct MockSource {
    descriptor: SourceDescriptor,
    payload: Vec<u8>,
}

impl EntropySource for MockSource {
    fn descriptor(&self) -> &SourceDescriptor {
        &self.descriptor
    }

    fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, SourceError> {
        let expected = bits
            .bytes()
            .map_err(|err| SourceError::new(SourceErrorKind::InvalidRequest, err.to_string()))?;
        if self.payload.len() != expected {
            return Err(SourceError::new(
                SourceErrorKind::Protocol,
                "mock payload length mismatch",
            ));
        }
        Ok(self.payload.clone())
    }
}

#[test]
fn sample_bits_rejects_zero_and_unaligned() {
    assert!(matches!(
        SampleBits::new(0),
        Err(rngkit_core::CoreError::ZeroBitLength)
    ));
    assert!(matches!(
        SampleBits::new(7),
        Err(rngkit_core::CoreError::BitLengthNotByteAligned { requested_bits: 7 })
    ));
    let bits = SampleBits::new(8).expect("aligned");
    assert_eq!(bits.bytes().expect("fits"), 1);
}

#[test]
fn interval_rejects_zero() {
    assert!(IntervalSeconds::new(0).is_err());
    assert_eq!(IntervalSeconds::new(1).expect("min").get(), 1);
}

#[test]
fn sample_index_is_one_based() {
    assert!(SampleIndex::new(0).is_err());
    assert_eq!(SampleIndex::new(1).expect("first").get(), 1);
}

#[test]
fn source_id_grammar_and_known_tokens() {
    assert_eq!(SourceId::bitb().as_str(), "bitb");
    assert_eq!(SourceId::trng().as_str(), "trng");
    assert_eq!(SourceId::rdseed().as_str(), "rdseed");
    assert_eq!(SourceId::pseudo().as_str(), "pseudo");
    assert!(SourceId::new("futureid").is_ok());
    assert!(SourceId::new("").is_err());
    assert!(SourceId::new("TRNG").is_err());
    assert!(SourceId::new("bit_b").is_err());
    assert!(SourceId::new("1trng").is_err());
}

#[test]
fn fold_accepts_zero_through_four() {
    for value in 0..=4 {
        assert_eq!(Fold::new(value).expect("valid").get(), value);
    }
    assert!(Fold::new(5).is_err());
}

#[test]
fn descriptor_has_no_selector_fields() {
    let desc = SourceDescriptor::new(
        SourceId::pseudo(),
        "PseudoRNG",
        Some("ChaCha20".into()),
        None,
    )
    .expect("descriptor");
    assert!(desc.variant().is_some());
    assert!(desc.fold().is_none());
    let encoded = format!("{desc:?}");
    assert!(!encoded.contains("COM"));
    assert!(!encoded.contains("serial"));
    assert!(!encoded.contains("seed"));
}

#[test]
fn popcount_matches_hand_calculated_vectors() {
    assert_eq!(count_ones(&[]).expect("empty"), 0);
    assert_eq!(count_ones(&[0x00, 0x00]).expect("zeros"), 0);
    assert_eq!(count_ones(&[0xFF]).expect("ones"), 8);
    assert_eq!(count_ones(&[0xFF, 0xFF]).expect("all ones"), 16);
    assert_eq!(count_ones(&[0xAA]).expect("alternating"), 4);
    assert_eq!(count_ones(&[0x55, 0xAA]).expect("mixed"), 8);
    assert_eq!(count_ones(&[0b0000_0001, 0b1000_0000]).expect("ends"), 2);
}

#[test]
fn mock_source_returns_exact_length() {
    let bits = SampleBits::new(16).expect("bits");
    let mut src = MockSource {
        descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None)
            .expect("descriptor"),
        payload: vec![0x0F, 0xF0],
    };
    let bytes = src.read_bits(bits).expect("read");
    assert_eq!(bytes.len(), bits.bytes().expect("bytes"));
    assert_eq!(count_ones(&bytes).expect("ones"), 8);
}

#[test]
fn byte_length_matches_sample_bits() {
    let bits = SampleBits::new(2048).expect("bits");
    let length = ByteLength::from_sample_bits(bits).expect("length");
    assert_eq!(length.get(), 256);
}

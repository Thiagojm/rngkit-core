//! Adapter unit tests that do not enumerate or open hardware.

use rngkit_core::{EntropySource, Fold, SampleBits, SourceErrorKind};
use rngkit_sources::error_mapping::enforce_len;
use rngkit_sources::{SourceCandidate, SourceConfig};

struct Mock {
    descriptor: rngkit_core::SourceDescriptor,
    payload: Vec<u8>,
}

impl EntropySource for Mock {
    fn descriptor(&self) -> &rngkit_core::SourceDescriptor {
        &self.descriptor
    }

    fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, rngkit_core::SourceError> {
        let n = bits.bytes().unwrap();
        enforce_len(n, self.payload.clone())
    }
}

#[test]
fn common_trait_accepts_mock_per_source_id() {
    for id in [
        rngkit_core::SourceId::bitb(),
        rngkit_core::SourceId::trng(),
        rngkit_core::SourceId::rdseed(),
        rngkit_core::SourceId::pseudo(),
    ] {
        let fold = if id.is_bitb() {
            Some(Fold::new(0).unwrap())
        } else {
            None
        };
        let mut src = Mock {
            descriptor: rngkit_core::SourceDescriptor::new(id, "mock", None, fold).unwrap(),
            payload: vec![0xFF, 0x00],
        };
        let bytes = src.read_bits(SampleBits::new(16).unwrap()).unwrap();
        assert_eq!(bytes.len(), 2);
    }
}

#[test]
fn adapter_length_guard_rejects_partial() {
    let err = enforce_len(4, vec![0, 1]).unwrap_err();
    assert_eq!(err.kind(), SourceErrorKind::Protocol);
}

#[cfg(feature = "bitb")]
#[test]
fn bitb_config_carries_explicit_fold() {
    let cfg = SourceConfig::Bitb {
        fold: Fold::new(3).unwrap(),
        serial: None,
    };
    match cfg {
        SourceConfig::Bitb { fold, .. } => assert_eq!(fold.get(), 3),
        #[allow(unreachable_patterns)]
        _ => panic!("expected bitb"),
    }
}

#[cfg(feature = "bitb")]
#[test]
fn discovered_bitb_candidate_maps_to_explicit_config() {
    let candidate = SourceCandidate::Bitb {
        variant: "White".into(),
        serial: "fake-serial".into(),
    };
    let fold = Fold::new(3).unwrap();
    let config = match candidate {
        SourceCandidate::Bitb { serial, .. } => SourceConfig::Bitb {
            fold,
            serial: Some(serial),
        },
        #[allow(unreachable_patterns)]
        _ => panic!("expected bitb candidate"),
    };
    match config {
        SourceConfig::Bitb { fold, serial } => {
            assert_eq!(fold.get(), 3);
            assert_eq!(serial.as_deref(), Some("fake-serial"));
        }
        #[allow(unreachable_patterns)]
        _ => panic!("expected bitb config"),
    }
}

#[cfg(feature = "trng3")]
#[test]
fn discovered_trng_candidate_maps_to_explicit_config() {
    let candidate = SourceCandidate::Trng {
        port_name: "fake-port".into(),
    };
    let config = match candidate {
        SourceCandidate::Trng { port_name } => SourceConfig::Trng {
            path: Some(port_name),
        },
        #[allow(unreachable_patterns)]
        _ => panic!("expected trng candidate"),
    };
    match config {
        SourceConfig::Trng { path } => assert_eq!(path.as_deref(), Some("fake-port")),
        #[allow(unreachable_patterns)]
        _ => panic!("expected trng config"),
    }
}

#[cfg(feature = "pseudo")]
#[test]
fn open_pseudo_through_common_trait() {
    let mut src = rngkit_sources::open(SourceConfig::Pseudo {
        max_range_samples: None,
    })
    .expect("os seed");
    let bits = SampleBits::new(64).unwrap();
    let bytes = src.read_bits(bits).unwrap();
    assert_eq!(bytes.len(), 8);
    assert_eq!(src.descriptor().id().as_str(), "pseudo");
    let debug = format!("{:?}", src.descriptor());
    assert!(!debug.contains("seed"));
}

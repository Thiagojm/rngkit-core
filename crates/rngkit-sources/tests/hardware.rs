//! Ignored physical adapter tests. Run serially:
//!
//! ```text
//! cargo test -p rngkit-sources --test hardware -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! Only genuine source absence may skip. Device serials are never recorded.

use rngkit_core::{EntropySource, Fold, SampleBits, SourceErrorKind};

fn skip_if_absent(kind: SourceErrorKind, label: &str) -> bool {
    matches!(
        kind,
        SourceErrorKind::NotAvailable | SourceErrorKind::Unsupported
    ) && {
        eprintln!("skip {label}: {kind}");
        true
    }
}

#[cfg(feature = "bitb")]
#[test]
#[ignore]
fn physical_bitb() {
    match rngkit_sources::adapters::BitbAdapter::list() {
        Ok(list) if list.is_empty() => {
            eprintln!("skip bitb: no device");
            return;
        }
        Ok(list) if list.len() > 1 => {
            panic!("multiple BitBabbler devices present; refusing implicit first-device selection");
        }
        Ok(_) => {}
        Err(err) if skip_if_absent(err.kind(), "bitb") => return,
        Err(err) => panic!("{err}"),
    }
    let mut src = match rngkit_sources::adapters::BitbAdapter::open(Fold::new(0).unwrap(), None) {
        Ok(src) => src,
        Err(err) if skip_if_absent(err.kind(), "bitb") => return,
        Err(err) => panic!("{err}"),
    };
    let bytes = src
        .read_bits(SampleBits::new(64).unwrap())
        .expect("read_bits");
    assert_eq!(bytes.len(), 8);
    assert_eq!(src.descriptor().id().as_str(), "bitb");
}

#[cfg(feature = "trng3")]
#[test]
#[ignore]
fn physical_trng3() {
    match rngkit_sources::adapters::Trng3Adapter::list() {
        Ok(list) if list.is_empty() => {
            eprintln!("skip trng: no device");
            return;
        }
        Ok(list) if list.len() > 1 => {
            panic!("multiple TrueRNG devices present; refusing implicit first-device selection");
        }
        Ok(_) => {}
        Err(err) if skip_if_absent(err.kind(), "trng") => return,
        Err(err) => panic!("{err}"),
    }
    let mut src = match rngkit_sources::adapters::Trng3Adapter::open(None) {
        Ok(src) => src,
        Err(err) if skip_if_absent(err.kind(), "trng") => return,
        Err(err) => panic!("{err}"),
    };
    let bytes = src
        .read_bits(SampleBits::new(64).unwrap())
        .expect("read_bits");
    assert_eq!(bytes.len(), 8);
}

#[cfg(feature = "rdseed")]
#[test]
#[ignore]
fn physical_rdseed() {
    if !rngkit_sources::adapters::RdseedAdapter::is_supported() {
        eprintln!("skip rdseed: not supported");
        return;
    }
    let mut src = match rngkit_sources::adapters::RdseedAdapter::open(None, None) {
        Ok(src) => src,
        Err(err) if skip_if_absent(err.kind(), "rdseed") => return,
        Err(err) => panic!("{err}"),
    };
    let bytes = src
        .read_bits(SampleBits::new(64).unwrap())
        .expect("read_bits");
    assert_eq!(bytes.len(), 8);
}

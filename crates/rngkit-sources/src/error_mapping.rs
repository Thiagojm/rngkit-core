//! Maps source-crate errors into normalized [`rngkit_core::SourceError`] values.

use rngkit_core::{SourceError, SourceErrorKind};

/// Ensures a buffer matches the requested sample byte length.
pub fn enforce_len(expected: usize, bytes: Vec<u8>) -> Result<Vec<u8>, SourceError> {
    if bytes.len() != expected {
        return Err(SourceError::new(
            SourceErrorKind::Protocol,
            format!("source returned {} bytes, expected {expected}", bytes.len()),
        ));
    }
    Ok(bytes)
}

/// Maps a BitBabbler error into a normalized [`SourceError`].
#[cfg(feature = "bitb")]
pub fn map_bitb(err: bitb_rs::BitBabblerError) -> SourceError {
    use bitb_rs::BitBabblerError;
    let kind = match &err {
        BitBabblerError::NoDevice => SourceErrorKind::NotAvailable,
        BitBabblerError::MultipleDevices { .. } => SourceErrorKind::MultipleDevices,
        BitBabblerError::MissingSerial => SourceErrorKind::SelectionRequired,
        BitBabblerError::DeviceNotFound { .. } => SourceErrorKind::DeviceNotFound,
        BitBabblerError::PermissionDenied => SourceErrorKind::PermissionDenied,
        BitBabblerError::DeviceBusy => SourceErrorKind::DeviceBusy,
        BitBabblerError::DeviceDisconnected => SourceErrorKind::Disconnected,
        BitBabblerError::TransferTimeout { .. } => SourceErrorKind::Timeout,
        BitBabblerError::ProtocolViolation { .. }
        | BitBabblerError::InitializationFailed { .. } => SourceErrorKind::Protocol,
        BitBabblerError::ZeroBitLength
        | BitBabblerError::BitLengthNotByteAligned { .. }
        | BitBabblerError::InvalidFold { .. }
        | BitBabblerError::InvalidRange { .. } => SourceErrorKind::InvalidRequest,
        BitBabblerError::AllocationFailed { .. } => SourceErrorKind::AllocationFailed,
        BitBabblerError::ReadRetriesExhausted { .. } => SourceErrorKind::EntropyUnavailable,
        BitBabblerError::UnsupportedProduct { .. } => SourceErrorKind::Unsupported,
        BitBabblerError::Usb { .. } | BitBabblerError::RangeSamplingExhausted { .. } => {
            SourceErrorKind::Other
        }
        _ => SourceErrorKind::Other,
    };
    let message = err.to_string();
    SourceError::with_source(kind, message, err)
}

/// Maps a TrueRNG3 error into a normalized [`SourceError`].
#[cfg(feature = "trng3")]
pub fn map_trng3(err: trng3_rs::TrueRng3Error) -> SourceError {
    use trng3_rs::TrueRng3Error;
    let kind = match &err {
        TrueRng3Error::NoDevice => SourceErrorKind::NotAvailable,
        TrueRng3Error::MultipleDevices { .. } => SourceErrorKind::MultipleDevices,
        TrueRng3Error::MissingPath => SourceErrorKind::SelectionRequired,
        TrueRng3Error::DeviceNotFound { .. } => SourceErrorKind::DeviceNotFound,
        TrueRng3Error::PermissionDenied => SourceErrorKind::PermissionDenied,
        TrueRng3Error::DeviceUnavailable { .. } => SourceErrorKind::Disconnected,
        TrueRng3Error::ReadTimeout => SourceErrorKind::Timeout,
        TrueRng3Error::ZeroBitLength
        | TrueRng3Error::BitLengthNotByteAligned { .. }
        | TrueRng3Error::InvalidRange { .. } => SourceErrorKind::InvalidRequest,
        TrueRng3Error::AllocationFailed { .. } => SourceErrorKind::AllocationFailed,
        TrueRng3Error::Serial { .. } | TrueRng3Error::RangeSamplingExhausted { .. } => {
            SourceErrorKind::Other
        }
        _ => SourceErrorKind::Other,
    };
    let message = err.to_string();
    SourceError::with_source(kind, message, err)
}

/// Maps an RDSEED error into a normalized [`SourceError`].
#[cfg(feature = "rdseed")]
pub fn map_rdseed(err: intel_seed::RdSeedError) -> SourceError {
    use intel_seed::RdSeedError;
    let kind = match &err {
        RdSeedError::UnsupportedArchitecture | RdSeedError::UnsupportedInstruction => {
            SourceErrorKind::Unsupported
        }
        RdSeedError::ZeroBitLength
        | RdSeedError::BitLengthNotByteAligned { .. }
        | RdSeedError::InvalidRetryLimit
        | RdSeedError::InvalidRange { .. } => SourceErrorKind::InvalidRequest,
        RdSeedError::EntropyUnavailable { .. } | RdSeedError::RangeSamplingExhausted { .. } => {
            SourceErrorKind::EntropyUnavailable
        }
        RdSeedError::AllocationFailed { .. } => SourceErrorKind::AllocationFailed,
        _ => SourceErrorKind::Other,
    };
    let message = err.to_string();
    SourceError::with_source(kind, message, err)
}

/// Maps a PseudoRNG error into a normalized [`SourceError`].
#[cfg(feature = "pseudo")]
pub fn map_pseudo(err: pseudo_rng::PseudoRngError) -> SourceError {
    use pseudo_rng::PseudoRngError;
    let kind = match &err {
        PseudoRngError::SystemEntropyUnavailable { .. } => SourceErrorKind::EntropyUnavailable,
        PseudoRngError::ZeroBitLength
        | PseudoRngError::BitLengthNotByteAligned { .. }
        | PseudoRngError::InvalidSamplingLimit
        | PseudoRngError::InvalidRange { .. } => SourceErrorKind::InvalidRequest,
        PseudoRngError::RangeSamplingExhausted { .. } => SourceErrorKind::EntropyUnavailable,
        PseudoRngError::AllocationFailed { .. } => SourceErrorKind::AllocationFailed,
    };
    // PseudoRngError is exhaustive today; map any future variant as Other via
    // the Display path above.
    let message = err.to_string();
    SourceError::with_source(kind, message, err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rngkit_core::SourceErrorKind;

    #[test]
    fn enforce_len_rejects_partial() {
        let err = enforce_len(2, vec![1]).unwrap_err();
        assert_eq!(err.kind(), SourceErrorKind::Protocol);
    }

    #[cfg(feature = "bitb")]
    #[test]
    fn maps_bitb_multiple_devices() {
        let err = map_bitb(bitb_rs::BitBabblerError::MultipleDevices { count: 2 });
        assert_eq!(err.kind(), SourceErrorKind::MultipleDevices);
        assert!(std::error::Error::source(&err).is_some());
    }

    #[cfg(feature = "trng3")]
    #[test]
    fn maps_trng3_no_device() {
        let err = map_trng3(trng3_rs::TrueRng3Error::NoDevice);
        assert_eq!(err.kind(), SourceErrorKind::NotAvailable);
    }

    #[cfg(feature = "rdseed")]
    #[test]
    fn maps_rdseed_unsupported() {
        let err = map_rdseed(intel_seed::RdSeedError::UnsupportedInstruction);
        assert_eq!(err.kind(), SourceErrorKind::Unsupported);
    }

    #[cfg(feature = "pseudo")]
    #[test]
    fn maps_pseudo_zero_bits() {
        let err = map_pseudo(pseudo_rng::PseudoRngError::ZeroBitLength);
        assert_eq!(err.kind(), SourceErrorKind::InvalidRequest);
    }
}

//! Compile-time factory for configured sources.

use rngkit_core::{EntropySource, SourceError};

use crate::config::SourceConfig;

/// Owned handle to one opened source.
pub enum OpenedSource {
    /// BitBabbler.
    #[cfg(feature = "bitb")]
    Bitb(Box<crate::adapters::BitbAdapter>),
    /// TrueRNG v1/v2/v3.
    #[cfg(feature = "trng3")]
    Trng(Box<crate::adapters::Trng3Adapter>),
    /// Intel RDSEED.
    #[cfg(feature = "rdseed")]
    Rdseed(Box<crate::adapters::RdseedAdapter>),
    /// OS-seeded ChaCha20.
    #[cfg(feature = "pseudo")]
    Pseudo(Box<crate::adapters::PseudoAdapter>),
}

impl EntropySource for OpenedSource {
    fn descriptor(&self) -> &rngkit_core::SourceDescriptor {
        match self {
            #[cfg(feature = "bitb")]
            Self::Bitb(src) => src.descriptor(),
            #[cfg(feature = "trng3")]
            Self::Trng(src) => src.descriptor(),
            #[cfg(feature = "rdseed")]
            Self::Rdseed(src) => src.descriptor(),
            #[cfg(feature = "pseudo")]
            Self::Pseudo(src) => src.descriptor(),
        }
    }

    fn read_bits(&mut self, bits: rngkit_core::SampleBits) -> Result<Vec<u8>, SourceError> {
        match self {
            #[cfg(feature = "bitb")]
            Self::Bitb(src) => src.read_bits(bits),
            #[cfg(feature = "trng3")]
            Self::Trng(src) => src.read_bits(bits),
            #[cfg(feature = "rdseed")]
            Self::Rdseed(src) => src.read_bits(bits),
            #[cfg(feature = "pseudo")]
            Self::Pseudo(src) => src.read_bits(bits),
        }
    }
}

/// Opens the configured source. There is no fallback between source kinds.
///
/// # Errors
///
/// Returns mapped source errors. Multiple hardware devices never resolve to
/// an implicit first device.
pub fn open(config: SourceConfig) -> Result<OpenedSource, SourceError> {
    match config {
        #[cfg(feature = "bitb")]
        SourceConfig::Bitb { fold, serial } => {
            crate::adapters::BitbAdapter::open(fold, serial.as_deref())
                .map(|src| OpenedSource::Bitb(Box::new(src)))
        }
        #[cfg(feature = "trng3")]
        SourceConfig::Trng { path } => crate::adapters::Trng3Adapter::open(path.as_deref())
            .map(|src| OpenedSource::Trng(Box::new(src))),
        #[cfg(feature = "rdseed")]
        SourceConfig::Rdseed {
            max_rdseed_attempts_per_word,
            max_range_samples,
        } => crate::adapters::RdseedAdapter::open(max_rdseed_attempts_per_word, max_range_samples)
            .map(|src| OpenedSource::Rdseed(Box::new(src))),
        #[cfg(feature = "pseudo")]
        SourceConfig::Pseudo { max_range_samples } => {
            crate::adapters::PseudoAdapter::open(max_range_samples)
                .map(|src| OpenedSource::Pseudo(Box::new(src)))
        }
    }
}

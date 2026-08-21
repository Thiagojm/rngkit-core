//! Typed configuration for opening one entropy source.

#[cfg(feature = "bitb")]
use rngkit_core::Fold;

/// Configuration for a single session source.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceConfig {
    /// BitBabbler with an explicit fold and optional serial.
    #[cfg(feature = "bitb")]
    Bitb {
        /// Fold `0..=4`.
        fold: Fold,
        /// Exact serial when more than one device is present.
        serial: Option<String>,
    },
    /// TrueRNG v1/v2/v3 with optional recognized path.
    #[cfg(feature = "trng3")]
    Trng {
        /// OS path of a recognized `04D8:F5FE` device.
        path: Option<String>,
    },
    /// Intel RDSEED with optional retry policy limits.
    #[cfg(feature = "rdseed")]
    Rdseed {
        /// Per-word RDSEED attempts; `None` uses the crate default.
        max_rdseed_attempts_per_word: Option<u32>,
        /// Range-sampling budget; unused by collection but required to construct
        /// a policy when overriding the default.
        max_range_samples: Option<u32>,
    },
    /// OS-seeded ChaCha20 PseudoRNG.
    #[cfg(feature = "pseudo")]
    Pseudo {
        /// Range-sampling budget; unused by collection.
        max_range_samples: Option<u32>,
    },
}

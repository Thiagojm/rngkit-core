//! Intel RDSEED adapter.

use intel_seed::{RdSeed, RetryPolicy};

use rngkit_core::{
    EntropySource, SOURCE_ID_RDSEED, SampleBits, SourceDescriptor, SourceError, SourceErrorKind,
    SourceId,
};

use crate::error_mapping::{enforce_len, map_rdseed};

/// [`EntropySource`] wrapper around [`RdSeed`].
pub struct RdseedAdapter {
    inner: RdSeed,
    descriptor: SourceDescriptor,
}

impl RdseedAdapter {
    /// Reports whether RDSEED is usable on this process.
    #[must_use]
    pub fn is_supported() -> bool {
        RdSeed::is_supported()
    }

    /// Opens RDSEED after the runtime capability check in [`RdSeed::new`].
    ///
    /// # Errors
    ///
    /// Returns mapped architecture/instruction errors. There is no fallback.
    pub fn open(
        max_rdseed_attempts_per_word: Option<u32>,
        max_range_samples: Option<u32>,
    ) -> Result<Self, SourceError> {
        let inner = match (max_rdseed_attempts_per_word, max_range_samples) {
            (None, None) => RdSeed::new().map_err(map_rdseed)?,
            (a, r) => {
                let policy = RetryPolicy::new(
                    a.unwrap_or(RetryPolicy::DEFAULT_LIMIT),
                    r.unwrap_or(RetryPolicy::DEFAULT_LIMIT),
                )
                .map_err(map_rdseed)?;
                RdSeed::with_retry_policy(policy).map_err(map_rdseed)?
            }
        };
        let descriptor = SourceDescriptor::new(
            SourceId::new(SOURCE_ID_RDSEED).expect("rdseed id"),
            "Intel RDSEED",
            Some("RDSEED".into()),
            None,
        )
        .map_err(|err| SourceError::new(SourceErrorKind::InvalidRequest, err.to_string()))?;
        Ok(Self { inner, descriptor })
    }
}

impl EntropySource for RdseedAdapter {
    fn descriptor(&self) -> &SourceDescriptor {
        &self.descriptor
    }

    fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, SourceError> {
        let n = bits
            .bytes()
            .map_err(|err| SourceError::new(SourceErrorKind::InvalidRequest, err.to_string()))?;
        let bytes = self
            .inner
            .get_bits(bits.get() as usize)
            .map_err(map_rdseed)?;
        enforce_len(n, bytes)
    }
}

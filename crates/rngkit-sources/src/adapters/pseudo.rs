//! OS-seeded ChaCha20 PseudoRNG adapter.

use pseudo_rng::{PseudoRng, SamplingPolicy};

use rngkit_core::{
    EntropySource, SOURCE_ID_PSEUDO, SampleBits, SourceDescriptor, SourceError, SourceErrorKind,
    SourceId,
};

use crate::error_mapping::{enforce_len, map_pseudo};

/// [`EntropySource`] wrapper around [`PseudoRng`].
pub struct PseudoAdapter {
    inner: PseudoRng,
    descriptor: SourceDescriptor,
}

impl PseudoAdapter {
    /// Opens a generator after a successful OS seed. Never exposes seed or state.
    ///
    /// # Errors
    ///
    /// Returns mapped OS-entropy errors. There is no fallback.
    pub fn open(max_range_samples: Option<u32>) -> Result<Self, SourceError> {
        let inner = match max_range_samples {
            None => PseudoRng::new().map_err(map_pseudo)?,
            Some(limit) => {
                let policy = SamplingPolicy::new(limit).map_err(map_pseudo)?;
                PseudoRng::with_sampling_policy(policy).map_err(map_pseudo)?
            }
        };
        let descriptor = SourceDescriptor::new(
            SourceId::new(SOURCE_ID_PSEUDO).expect("pseudo id"),
            "PseudoRNG",
            Some("ChaCha20".into()),
            None,
        )
        .map_err(|err| SourceError::new(SourceErrorKind::InvalidRequest, err.to_string()))?;
        debug_assert!(!format!("{:?}", inner).contains("seed"));
        Ok(Self { inner, descriptor })
    }
}

impl EntropySource for PseudoAdapter {
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
            .map_err(map_pseudo)?;
        enforce_len(n, bytes)
    }
}

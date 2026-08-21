//! Analysis errors.

use rngkit_core::SampleBits;

/// Errors from incremental or batch descriptive statistics.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AnalysisError {
    /// A sample one-count exceeded the session sample size.
    #[error("ones {ones} exceed sample bits {sample_bits}")]
    OnesExceedSampleBits {
        /// Observed one-count.
        ones: u64,
        /// Session sample size in bits.
        sample_bits: u32,
    },
    /// A record used a different sample size than the accumulator.
    #[error("sample bits {found} do not match session sample bits {expected}")]
    SampleBitsMismatch {
        /// Expected session sample size.
        expected: SampleBits,
        /// Sample size on the rejected record.
        found: SampleBits,
    },
    /// A checked integer accumulation overflowed.
    #[error("checked {which} overflow")]
    Overflow {
        /// Which total overflowed.
        which: &'static str,
    },
}

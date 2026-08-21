//! Incremental descriptive cumulative statistics.

use rngkit_core::{SampleBits, SampleIndex, SampleRecord};

use crate::error::AnalysisError;

/// Descriptive snapshot after one committed sample.
///
/// Values are produced from integer totals `C` (ones) and `N` (evaluated bits):
///
/// ```text
/// p_hat = C / N
/// delta = p_hat - 0.5
/// Z     = (2*C - N) / sqrt(N)
/// ```
///
/// Positive `z` means excess ones; negative `z` means excess zeroes.
///
/// # Interpretation
///
/// `Z` is the signed frequency/monobit statistic. Its standard-normal reading
/// assumes i.i.d. Bernoulli bits with `P(1) = 0.5`. This crate does not
/// establish that assumption and does not treat the cumulative trajectory as a
/// sequential hypothesis test. Crossing a familiar fixed-horizon value such as
/// `±1.96` is not a significance, pass, or fail decision.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// One-based index of the sample that produced this snapshot.
    pub index: SampleIndex,
    /// Number of committed samples.
    pub sample_count: u64,
    /// Total evaluated bits `N`.
    pub total_bits: u128,
    /// Cumulative one-count `C`.
    pub total_ones: u128,
    /// Observed one proportion `C / N`.
    pub proportion: f64,
    /// Signed deviation from `0.5`.
    pub deviation: f64,
    /// Signed cumulative Z-score.
    pub z: f64,
}

/// Incremental accumulator using checked integer totals.
///
/// Batch analysis uses the same type. There is no second formula.
#[derive(Debug, Clone)]
pub struct Accumulator {
    sample_bits: SampleBits,
    sample_count: u64,
    total_ones: u128,
}

impl Accumulator {
    /// Starts an empty accumulator for a fixed sample size.
    #[must_use]
    pub fn new(sample_bits: SampleBits) -> Self {
        Self {
            sample_bits,
            sample_count: 0,
            total_ones: 0,
        }
    }

    /// Session sample size.
    #[must_use]
    pub fn sample_bits(&self) -> SampleBits {
        self.sample_bits
    }

    /// Number of committed samples.
    #[must_use]
    pub fn sample_count(&self) -> u64 {
        self.sample_count
    }

    /// Cumulative one-count.
    #[must_use]
    pub fn total_ones(&self) -> u128 {
        self.total_ones
    }

    /// Total evaluated bits `N = sample_count * sample_bits`.
    ///
    /// # Errors
    ///
    /// Returns [`AnalysisError::Overflow`] when the product overflows `u128`.
    pub fn total_bits(&self) -> Result<u128, AnalysisError> {
        u128::from(self.sample_count)
            .checked_mul(u128::from(self.sample_bits.get()))
            .ok_or(AnalysisError::Overflow {
                which: "total bits",
            })
    }

    /// Incorporates one sample's one-count.
    ///
    /// # Errors
    ///
    /// Returns [`AnalysisError::OnesExceedSampleBits`] when `ones` is greater
    /// than the session sample size.
    /// Returns [`AnalysisError::Overflow`] when a checked total overflows.
    pub fn push(&mut self, ones: u64) -> Result<Snapshot, AnalysisError> {
        let bits = u64::from(self.sample_bits.get());
        if ones > bits {
            return Err(AnalysisError::OnesExceedSampleBits {
                ones,
                sample_bits: self.sample_bits.get(),
            });
        }
        self.sample_count = self
            .sample_count
            .checked_add(1)
            .ok_or(AnalysisError::Overflow {
                which: "sample count",
            })?;
        self.total_ones = self
            .total_ones
            .checked_add(u128::from(ones))
            .ok_or(AnalysisError::Overflow { which: "ones" })?;
        self.snapshot()
    }

    /// Incorporates a normalized record after checking its one-count.
    ///
    /// # Errors
    ///
    /// Same as [`Self::push`].
    pub fn push_record(&mut self, record: &SampleRecord) -> Result<Snapshot, AnalysisError> {
        self.push(record.ones)
    }

    /// Current snapshot, if at least one sample has been pushed.
    ///
    /// # Errors
    ///
    /// Returns [`AnalysisError::Overflow`] when `N` overflows.
    pub fn snapshot(&self) -> Result<Snapshot, AnalysisError> {
        let n = self.total_bits()?;
        let index = SampleIndex::new(self.sample_count).map_err(|_| AnalysisError::Overflow {
            which: "sample index",
        })?;
        Ok(compute_snapshot(
            index,
            self.sample_count,
            n,
            self.total_ones,
        ))
    }
}

/// Batch analysis over normalized records using [`Accumulator`].
///
/// # Errors
///
/// Returns the first accumulator error. Empty input yields an empty `Vec`.
pub fn analyze_records(
    sample_bits: SampleBits,
    records: impl IntoIterator<Item = SampleRecord>,
) -> Result<Vec<Snapshot>, AnalysisError> {
    let mut acc = Accumulator::new(sample_bits);
    let mut out = Vec::new();
    for record in records {
        out.push(acc.push_record(&record)?);
    }
    Ok(out)
}

fn compute_snapshot(index: SampleIndex, sample_count: u64, n: u128, c: u128) -> Snapshot {
    // Convert to f64 only for proportion, deviation, and Z.
    let n_f = n as f64;
    let c_f = c as f64;
    let proportion = if n == 0 { 0.0 } else { c_f / n_f };
    let deviation = proportion - 0.5;
    let z = if n == 0 {
        0.0
    } else {
        (2.0 * c_f - n_f) / n_f.sqrt()
    };
    Snapshot {
        index,
        sample_count,
        total_bits: n,
        total_ones: c,
        proportion,
        deviation,
        z,
    }
}

//! Normalized session view shared by analysis and XLSX.

use rngkit_core::{
    Fold, IntervalSeconds, SampleBits, SampleRecord, SessionStatus, SourceId, TimestampProvenance,
    UtcTimestamp,
};
use serde::{Deserialize, Serialize};

use crate::native::reader::NativeSession;

/// Format classification for a selected standalone input.
///
/// `CurrentCsv` and `LegacyV3Csv` are also used on concatenation preview
/// entries. `Bin` is valid only for standalone inspection because Combine is
/// intentionally CSV-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandaloneInputFormat {
    /// Current seven-column native CSV.
    CurrentCsv,
    /// Headerless RngKitPSG version 3 CSV.
    LegacyV3Csv,
    /// Fixed-size binary samples from a current or legacy stem.
    Bin,
}

/// Format classification used by CSV concatenation entries.
pub type CsvInputFormat = StandaloneInputFormat;

/// Metadata needed to render analysis without re-parsing BIN/CSV.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedMeta {
    /// Session or file stem.
    pub stem: String,
    /// Source identifier.
    pub source_id: SourceId,
    /// Safe label.
    pub source_label: String,
    /// Safe variant.
    pub source_variant: Option<String>,
    /// Fold when BitBabbler.
    pub fold: Option<Fold>,
    /// Sample size.
    pub sample_bits: SampleBits,
    /// Interval.
    pub interval: IntervalSeconds,
    /// Start time when known.
    pub started_at: Option<UtcTimestamp>,
    /// Completion time when known.
    pub completed_at: Option<UtcTimestamp>,
    /// Reader-facing status.
    pub status: SessionStatus,
    /// Overrun count when known.
    pub overrun_count: Option<u64>,
    /// Timestamp provenance for the sample series.
    pub provenance: TimestampProvenance,
    /// Local UTC offset used for the filename, when known.
    pub local_utc_offset: Option<String>,
}

/// Streaming-friendly owned normalized session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedSession {
    meta: NormalizedMeta,
    records: Vec<SampleRecord>,
}

impl NormalizedSession {
    /// Builds from a native bundle.
    #[must_use]
    pub fn from_native(session: &NativeSession) -> Self {
        let manifest = session.manifest();
        Self {
            meta: NormalizedMeta {
                stem: manifest.stem().to_owned(),
                source_id: manifest.source_id().clone(),
                source_label: manifest.source_label().to_owned(),
                source_variant: manifest.source_variant().map(str::to_owned),
                fold: manifest.fold(),
                sample_bits: manifest.sample_bits(),
                interval: manifest.interval(),
                started_at: Some(manifest.started_at_utc()),
                completed_at: manifest.completed_at_utc(),
                status: manifest.reader_status(),
                overrun_count: Some(manifest.overrun_count()),
                provenance: TimestampProvenance::Recorded,
                local_utc_offset: Some(manifest.local_utc_offset().to_owned()),
            },
            records: session.records().to_vec(),
        }
    }

    /// Builds from legacy import.
    #[must_use]
    pub fn from_parts(meta: NormalizedMeta, records: Vec<SampleRecord>) -> Self {
        Self { meta, records }
    }

    /// Session metadata.
    #[must_use]
    pub fn meta(&self) -> &NormalizedMeta {
        &self.meta
    }

    /// Normalized samples.
    #[must_use]
    pub fn records(&self) -> &[SampleRecord] {
        &self.records
    }

    /// Number of samples.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether there are no samples.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Trait for types that can yield normalized records.
pub trait SessionOrigin {
    /// Session sample size.
    fn sample_bits(&self) -> SampleBits;
    /// Committed records.
    fn records(&self) -> &[SampleRecord];
}

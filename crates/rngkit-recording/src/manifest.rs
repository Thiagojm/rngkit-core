//! Native session manifest schema version 1.

use std::path::Path;

use rngkit_core::{
    Fold, IntervalSeconds, SampleBits, SessionStatus, SourceDescriptor, SourceId, UtcTimestamp,
};
use serde::{Deserialize, Serialize};
use time::UtcOffset;

use crate::error::RecordingError;
use crate::fsutil::replace_bytes;
use crate::naming::SessionStem;

/// Manifest schema version implemented by this crate.
pub const SCHEMA_VERSION: u32 = 1;

/// Native `manifest.json` contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    schema_version: u32,
    stem: String,
    status: ManifestStatus,
    source_id: SourceId,
    source_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_variant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fold: Option<Fold>,
    sample_bits: SampleBits,
    interval_seconds: IntervalSeconds,
    #[serde(with = "time::serde::rfc3339")]
    started_at_utc: time::OffsetDateTime,
    local_utc_offset: String,
    #[serde(default, with = "time::serde::rfc3339::option")]
    completed_at_utc: Option<time::OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_diagnostic: Option<String>,
    committed_samples: u64,
    overrun_count: u64,
    bin_file: String,
    csv_file: String,
}

/// Persistable recording status. Interrupted is a reader-side view of
/// `recording` after a crash, not a stored value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestStatus {
    /// The session is recording.
    Recording,
    /// The session completed cleanly.
    Completed,
    /// The session failed.
    Failed,
}

impl Manifest {
    /// Builds a recording manifest from a stem and safe descriptor.
    pub fn recording(
        stem: &SessionStem,
        descriptor: &SourceDescriptor,
        started_at: UtcTimestamp,
        local_offset: UtcOffset,
    ) -> Self {
        let name = stem.as_str().to_owned();
        Self {
            schema_version: SCHEMA_VERSION,
            stem: name.clone(),
            status: ManifestStatus::Recording,
            source_id: descriptor.id().clone(),
            source_label: descriptor.label().to_owned(),
            source_variant: descriptor.variant().map(str::to_owned),
            fold: stem.fold(),
            sample_bits: stem.sample_bits(),
            interval_seconds: stem.interval(),
            started_at_utc: started_at.inner(),
            local_utc_offset: format_offset(local_offset),
            completed_at_utc: None,
            failure_kind: None,
            failure_diagnostic: None,
            committed_samples: 0,
            overrun_count: 0,
            bin_file: format!("{name}.bin"),
            csv_file: format!("{name}.csv"),
        }
    }

    /// Schema version.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Session stem.
    #[must_use]
    pub fn stem(&self) -> &str {
        &self.stem
    }

    /// Stored status.
    #[must_use]
    pub fn status(&self) -> ManifestStatus {
        self.status
    }

    /// Reader-facing status, mapping leftover `recording` to interrupted.
    #[must_use]
    pub fn reader_status(&self) -> SessionStatus {
        match self.status {
            ManifestStatus::Recording => SessionStatus::Interrupted,
            ManifestStatus::Completed => SessionStatus::Completed,
            ManifestStatus::Failed => SessionStatus::Failed,
        }
    }

    /// Source identifier.
    #[must_use]
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Safe display label.
    #[must_use]
    pub fn source_label(&self) -> &str {
        &self.source_label
    }

    /// Safe variant, if present.
    #[must_use]
    pub fn source_variant(&self) -> Option<&str> {
        self.source_variant.as_deref()
    }

    /// Fold, if BitBabbler.
    #[must_use]
    pub fn fold(&self) -> Option<Fold> {
        self.fold
    }

    /// Sample size.
    #[must_use]
    pub fn sample_bits(&self) -> SampleBits {
        self.sample_bits
    }

    /// Interval.
    #[must_use]
    pub fn interval(&self) -> IntervalSeconds {
        self.interval_seconds
    }

    /// UTC start.
    #[must_use]
    pub fn started_at_utc(&self) -> UtcTimestamp {
        UtcTimestamp::new(self.started_at_utc)
    }

    /// Local offset used to build the filename, as `±HH:MM`.
    #[must_use]
    pub fn local_utc_offset(&self) -> &str {
        &self.local_utc_offset
    }

    /// UTC completion, if finalized.
    #[must_use]
    pub fn completed_at_utc(&self) -> Option<UtcTimestamp> {
        self.completed_at_utc.map(UtcTimestamp::new)
    }

    /// Terminal failure kind, if failed.
    #[must_use]
    pub fn failure_kind(&self) -> Option<&str> {
        self.failure_kind.as_deref()
    }

    /// Terminal diagnostic, if failed.
    #[must_use]
    pub fn failure_diagnostic(&self) -> Option<&str> {
        self.failure_diagnostic.as_deref()
    }

    /// Finalized committed count (CSV is authoritative after a crash).
    #[must_use]
    pub fn committed_samples(&self) -> u64 {
        self.committed_samples
    }

    /// Finalized overrun count.
    #[must_use]
    pub fn overrun_count(&self) -> u64 {
        self.overrun_count
    }

    /// Relative BIN filename.
    #[must_use]
    pub fn bin_file(&self) -> &str {
        &self.bin_file
    }

    /// Relative CSV filename.
    #[must_use]
    pub fn csv_file(&self) -> &str {
        &self.csv_file
    }

    /// Marks the session completed.
    pub fn complete(&mut self, committed: u64, overruns: u64, completed_at: UtcTimestamp) {
        self.status = ManifestStatus::Completed;
        self.committed_samples = committed;
        self.overrun_count = overruns;
        self.completed_at_utc = Some(completed_at.inner());
        self.failure_kind = None;
        self.failure_diagnostic = None;
    }

    /// Marks the session failed. Best-effort; does not hide the original error.
    pub fn fail(
        &mut self,
        committed: u64,
        overruns: u64,
        completed_at: UtcTimestamp,
        kind: impl Into<String>,
        diagnostic: impl Into<String>,
    ) {
        self.status = ManifestStatus::Failed;
        self.committed_samples = committed;
        self.overrun_count = overruns;
        self.completed_at_utc = Some(completed_at.inner());
        self.failure_kind = Some(kind.into());
        self.failure_diagnostic = Some(diagnostic.into());
    }

    /// Writes this manifest into `dir/manifest.json` via a sibling temp file.
    ///
    /// # Errors
    ///
    /// Returns I/O or JSON errors. A failure leaves either the previous
    /// complete manifest or the new complete manifest.
    pub fn write_to(&self, dir: &Path) -> Result<(), RecordingError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(RecordingError::UnsupportedSchema {
                version: self.schema_version,
            });
        }
        let dest = dir.join("manifest.json");
        let bytes = serde_json::to_vec_pretty(self)?;
        replace_bytes(&dest, &bytes)
    }

    /// Reads and rejects unknown schema versions.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::UnsupportedSchema`] when `schema_version` is
    /// not 1.
    pub fn read_from(dir: &Path) -> Result<Self, RecordingError> {
        let path = dir.join("manifest.json");
        let bytes = std::fs::read(path)?;
        let manifest: Self = serde_json::from_slice(&bytes)?;
        if manifest.schema_version != SCHEMA_VERSION {
            return Err(RecordingError::UnsupportedSchema {
                version: manifest.schema_version,
            });
        }
        Ok(manifest)
    }
}

fn format_offset(offset: UtcOffset) -> String {
    let whole = offset.whole_seconds();
    let sign = if whole < 0 { '-' } else { '+' };
    let abs = whole.unsigned_abs();
    let hours = abs / 3600;
    let minutes = (abs % 3600) / 60;
    format!("{sign}{hours:02}:{minutes:02}")
}

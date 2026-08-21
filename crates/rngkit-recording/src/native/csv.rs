//! Native CSV header and row types.

use rngkit_core::{
    ByteLength, ByteOffset, SampleIndex, SampleRecord, TimestampProvenance, UtcTimestamp,
    duration_as_millis,
};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;

use crate::error::RecordingError;

/// Native CSV column names in order.
pub const NATIVE_CSV_COLUMNS: [&str; 7] = [
    "sample_index",
    "captured_at_utc",
    "elapsed_ms",
    "acquisition_ms",
    "ones",
    "byte_offset",
    "byte_length",
];

/// One native CSV row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCsvRow {
    /// One-based index.
    pub sample_index: u64,
    /// RFC 3339 UTC timestamp.
    pub captured_at_utc: String,
    /// Elapsed milliseconds since session start.
    pub elapsed_ms: u64,
    /// Acquisition milliseconds.
    pub acquisition_ms: u64,
    /// One-bit count.
    pub ones: u64,
    /// Zero-based BIN offset.
    pub byte_offset: u64,
    /// Byte length of the sample.
    pub byte_length: u32,
}

impl NativeCsvRow {
    /// Builds a row from a committed sample.
    pub fn from_record(
        record: &SampleRecord,
        offset: ByteOffset,
        length: ByteLength,
    ) -> Result<Self, RecordingError> {
        let captured = record.timestamp.inner().format(&Rfc3339).map_err(|err| {
            RecordingError::InvalidName {
                reason: err.to_string(),
            }
        })?;
        Ok(Self {
            sample_index: record.index.get(),
            captured_at_utc: captured,
            elapsed_ms: record.elapsed.map(duration_as_millis).unwrap_or(0),
            acquisition_ms: record.acquisition.map(duration_as_millis).unwrap_or(0),
            ones: record.ones,
            byte_offset: offset.get(),
            byte_length: length.get(),
        })
    }

    /// Converts a CSV row to a normalized sample record.
    pub fn to_record(&self) -> Result<SampleRecord, RecordingError> {
        let timestamp =
            time::OffsetDateTime::parse(&self.captured_at_utc, &Rfc3339).map_err(|err| {
                RecordingError::InvalidName {
                    reason: err.to_string(),
                }
            })?;
        Ok(SampleRecord {
            index: SampleIndex::new(self.sample_index)?,
            timestamp: UtcTimestamp::new(timestamp),
            provenance: TimestampProvenance::Recorded,
            elapsed: Some(std::time::Duration::from_millis(self.elapsed_ms)),
            acquisition: Some(std::time::Duration::from_millis(self.acquisition_ms)),
            ones: self.ones,
            byte_offset: Some(ByteOffset::new(self.byte_offset)),
            byte_length: Some(ByteLength::from_sample_bits(rngkit_core::SampleBits::new(
                self.byte_length
                    .checked_mul(8)
                    .ok_or(rngkit_core::CoreError::SizeOverflow {
                        value: u64::from(self.byte_length) * 8,
                    })?,
            )?)?),
        })
    }
}

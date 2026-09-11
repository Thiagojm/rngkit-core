//! Derived concatenation bundle reader.

use std::fs;
use std::path::Path;

use rngkit_core::{
    SOURCE_ID_BITB, SOURCE_ID_PSEUDO, SOURCE_ID_RDSEED, SOURCE_ID_TRNG, SampleIndex, SampleRecord,
    SessionStatus, TimestampProvenance, UtcTimestamp,
};

use time::format_description::well_known::Rfc3339;

use crate::concatenation::manifest::{ConcatenationManifest, MIXED_SOURCE_ID, MIXED_SOURCE_LABEL};
use crate::concatenation::naming::ConcatenationStem;
use crate::concatenation::writer::{ConcatenationCsvRow, DERIVED_CSV_COLUMNS};
use crate::error::RecordingError;
use crate::fsutil::{open_contained, read_contained};
use crate::normalized::{NormalizedMeta, NormalizedSession};

/// Opens a derived concatenation directory and returns a normalized session.
///
/// Validates schema, kind, the contained same-stem CSV, row contiguity,
/// input/output ranges, and one-count bounds.
///
/// # Errors
///
/// Returns a typed [`RecordingError`] when the bundle is missing, corrupt,
/// unsupported, or inconsistent with its manifest.
pub fn open_concatenation(dir: impl AsRef<Path>) -> Result<NormalizedSession, RecordingError> {
    let dir = fs::canonicalize(dir.as_ref())?;
    let bytes = read_contained(&dir, "manifest.json")?;
    let manifest = ConcatenationManifest::from_slice(&bytes)?;
    let stem = ConcatenationStem::parse(manifest.stem())?;
    if manifest.csv_file() != stem.csv_basename() {
        return Err(RecordingError::InvalidName {
            reason: "concatenation csv file must be the same-stem basename".into(),
        });
    }
    let (_csv_path, csv_file) = open_contained(&dir, manifest.csv_file())?;
    let mut csv = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(csv_file);
    let headers = csv.headers()?;
    let expected: Vec<&str> = DERIVED_CSV_COLUMNS.to_vec();
    let observed: Vec<&str> = headers.iter().collect();
    if observed != expected {
        return Err(RecordingError::Corrupt {
            reason: "concatenation csv header does not match the derived schema".into(),
        });
    }

    let sample_bits = manifest.sample_bits();
    let mut records = Vec::new();
    let mut expected_index = 1u64;
    let mut first_ts = None;
    let mut last_ts = None;
    for row in csv.deserialize::<ConcatenationCsvRow>() {
        let row = row?;
        if row.sample_index != expected_index {
            return Err(RecordingError::Corrupt {
                reason: format!(
                    "sample_index {} is not contiguous (expected {expected_index})",
                    row.sample_index
                ),
            });
        }
        if row.ones > u64::from(sample_bits.get()) {
            return Err(RecordingError::OnesExceedSampleBits {
                ones: row.ones,
                sample_bits: sample_bits.get(),
            });
        }
        let timestamp =
            time::OffsetDateTime::parse(&row.captured_at_utc, &Rfc3339).map_err(|err| {
                RecordingError::InvalidName {
                    reason: err.to_string(),
                }
            })?;
        let timestamp = UtcTimestamp::new(timestamp);
        let (input_pos, input) = manifest
            .inputs()
            .iter()
            .enumerate()
            .find(|(_, input)| {
                expected_index >= input.output_start().get()
                    && expected_index <= input.output_end().get()
            })
            .ok_or(RecordingError::InconsistentConcatenationRange)?;
        let expected_input_index = u64::try_from(input_pos)
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or(RecordingError::ConcatenationCountOverflow)?;
        let expected_input_sample = expected_index
            .checked_sub(input.output_start().get())
            .and_then(|delta| delta.checked_add(1))
            .ok_or(RecordingError::InconsistentConcatenationRange)?;
        if row.input_index != expected_input_index
            || row.input_sample_index != expected_input_sample
        {
            return Err(RecordingError::InconsistentConcatenationRange);
        }
        if expected_index == input.output_start().get() && timestamp != input.first_timestamp() {
            return Err(RecordingError::Corrupt {
                reason: format!(
                    "concatenation csv first timestamp for {} does not match the manifest",
                    input.basename()
                ),
            });
        }
        if expected_index == input.output_end().get() && timestamp != input.last_timestamp() {
            return Err(RecordingError::Corrupt {
                reason: format!(
                    "concatenation csv last timestamp for {} does not match the manifest",
                    input.basename()
                ),
            });
        }
        records.push(SampleRecord {
            index: SampleIndex::new(row.sample_index)?,
            timestamp,
            provenance: TimestampProvenance::Recorded,
            elapsed: None,
            acquisition: None,
            ones: row.ones,
            byte_offset: None,
            byte_length: None,
        });
        if first_ts.is_none() {
            first_ts = Some(timestamp);
        }
        last_ts = Some(timestamp);
        expected_index = expected_index
            .checked_add(1)
            .ok_or(RecordingError::ConcatenationCountOverflow)?;
    }
    let actual_rows = expected_index.saturating_sub(1);
    if actual_rows != manifest.total_rows() {
        return Err(RecordingError::InconsistentConcatenationRange);
    }

    Ok(NormalizedSession::from_concatenation(
        &manifest, first_ts, last_ts, records,
    ))
}

pub(crate) fn concatenation_source_label(id: &str) -> String {
    match id {
        SOURCE_ID_BITB => "BitBabbler".into(),
        SOURCE_ID_TRNG => "TrueRNG v1/v2/v3".into(),
        SOURCE_ID_RDSEED => "RDSEED".into(),
        SOURCE_ID_PSEUDO => "PseudoRNG".into(),
        MIXED_SOURCE_ID => MIXED_SOURCE_LABEL.into(),
        other => other.into(),
    }
}

impl NormalizedSession {
    fn from_concatenation(
        manifest: &ConcatenationManifest,
        started_at: Option<UtcTimestamp>,
        completed_at: Option<UtcTimestamp>,
        records: Vec<SampleRecord>,
    ) -> Self {
        Self::from_parts(
            NormalizedMeta {
                stem: manifest.stem().to_owned(),
                source_id: manifest.source_id().clone(),
                source_label: concatenation_source_label(manifest.source_id().as_str()),
                source_variant: None,
                fold: manifest.fold(),
                sample_bits: manifest.sample_bits(),
                interval: manifest.interval(),
                started_at,
                completed_at,
                status: SessionStatus::Completed,
                overrun_count: None,
                provenance: TimestampProvenance::Recorded,
                local_utc_offset: Some(manifest.local_utc_offset().to_owned()),
            },
            records,
        )
    }
}

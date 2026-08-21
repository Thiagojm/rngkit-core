//! Read-only RngKitPSG version 3 import.

use std::path::{Path, PathBuf};

use rngkit_core::{SessionStatus, TimestampProvenance};

use crate::error::RecordingError;
use crate::naming::SessionStem;
use crate::normalized::{NormalizedMeta, NormalizedSession};

pub mod bin;
pub mod csv;

/// Opens a version 3 CSV and/or BIN path without modifying it.
///
/// Supported source IDs are `bitb`, `trng`, and `pseudo`. `rdseed` is native-only.
///
/// # Errors
///
/// Rejects version 2 names, mismatched sibling counts/popcounts, partial
/// trailing BIN samples, and one-counts greater than the declared sample size.
pub fn open_legacy(path: impl AsRef<Path>) -> Result<NormalizedSession, RecordingError> {
    let path = path.as_ref();
    let stem_str = file_stem(path)?;
    let stem = SessionStem::parse(&stem_str)?;
    if !matches!(
        stem.source().as_str(),
        rngkit_core::SOURCE_ID_BITB | rngkit_core::SOURCE_ID_TRNG | rngkit_core::SOURCE_ID_PSEUDO
    ) {
        return Err(RecordingError::UnsupportedVersion {
            reason: format!("legacy v3 does not include source {}", stem.source()),
        });
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let csv_path = dir.join(format!("{stem_str}.csv"));
    let bin_path = dir.join(format!("{stem_str}.bin"));
    let csv_exists = csv_path.is_file();
    let bin_exists = bin_path.is_file();

    let (records, provenance) = match (csv_exists, bin_exists) {
        (true, false) => {
            let rows = csv::read_legacy_csv(&csv_path, stem.sample_bits())?;
            (csv::records_from_csv(&rows)?, TimestampProvenance::Recorded)
        }
        (false, true) => {
            let mut reader = bin::LegacyBinReader::open(&bin_path, stem.sample_bits())?;
            (
                bin::records_from_bin(&stem, &mut reader)?,
                TimestampProvenance::Estimated,
            )
        }
        (true, true) => (
            paired_records(&csv_path, &bin_path, &stem)?,
            TimestampProvenance::Recorded,
        ),
        (false, false) => {
            return Err(RecordingError::InvalidName {
                reason: "legacy path has neither csv nor bin sibling".into(),
            });
        }
    };

    let meta = NormalizedMeta {
        stem: stem_str,
        source_id: stem.source().clone(),
        source_label: legacy_label(stem.source().as_str()),
        source_variant: None,
        fold: stem.fold(),
        sample_bits: stem.sample_bits(),
        interval: stem.interval(),
        started_at: Some(rngkit_core::UtcTimestamp::new(stem.local_start())),
        completed_at: records.last().map(|r| r.timestamp),
        status: SessionStatus::Completed,
        overrun_count: None,
        provenance,
        local_utc_offset: None,
    };
    Ok(NormalizedSession::from_parts(meta, records))
}

fn paired_records(
    csv_path: &Path,
    bin_path: &Path,
    stem: &SessionStem,
) -> Result<Vec<rngkit_core::SampleRecord>, RecordingError> {
    let rows = csv::read_legacy_csv(csv_path, stem.sample_bits())?;
    let mut reader = bin::LegacyBinReader::open(bin_path, stem.sample_bits())?;
    let row_count = u64::try_from(rows.len()).map_err(|_| RecordingError::Corrupt {
        reason: "legacy csv row count overflow".into(),
    })?;
    if row_count != reader.sample_count() {
        return Err(RecordingError::Corrupt {
            reason: format!(
                "legacy csv has {} rows but bin has {} samples",
                rows.len(),
                reader.sample_count()
            ),
        });
    }
    let mut records = Vec::new();
    for row in &rows {
        let Some(meta) = reader.read_next()? else {
            return Err(RecordingError::Corrupt {
                reason: "legacy bin ended before csv rows".into(),
            });
        };
        if row.ones != meta.ones {
            return Err(RecordingError::Corrupt {
                reason: format!(
                    "legacy csv ones {} do not match bin popcount {} at {}",
                    row.ones,
                    meta.ones,
                    meta.index.get()
                ),
            });
        }
        records.push(rngkit_core::SampleRecord {
            index: meta.index,
            timestamp: row.timestamp,
            provenance: TimestampProvenance::Recorded,
            elapsed: None,
            acquisition: None,
            ones: row.ones,
            byte_offset: Some(meta.byte_offset),
            byte_length: Some(meta.byte_length),
        });
    }
    Ok(records)
}

fn legacy_label(id: &str) -> String {
    match id {
        rngkit_core::SOURCE_ID_BITB => "BitBabbler".into(),
        rngkit_core::SOURCE_ID_TRNG => "TrueRNG v1/v2/v3".into(),
        rngkit_core::SOURCE_ID_PSEUDO => "PseudoRNG".into(),
        other => other.into(),
    }
}

fn file_stem(path: &Path) -> Result<String, RecordingError> {
    let name =
        path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| RecordingError::InvalidName {
                reason: "path is not valid utf-8".into(),
            })?;
    let stem = name
        .strip_suffix(".csv")
        .or_else(|| name.strip_suffix(".bin"))
        .unwrap_or(name);
    Ok(stem.to_owned())
}

/// Selected legacy file path used to place an XLSX sibling.
#[must_use]
pub fn selected_file(path: &Path) -> PathBuf {
    path.to_path_buf()
}

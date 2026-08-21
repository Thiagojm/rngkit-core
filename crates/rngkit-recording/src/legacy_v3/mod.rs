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
/// Rejects version 2 names, mismatched sibling counts/popcounts, and partial
/// trailing BIN samples.
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
            let rows = csv::read_legacy_csv(&csv_path)?;
            (csv::records_from_csv(&rows)?, TimestampProvenance::Recorded)
        }
        (false, true) => {
            let samples = bin::read_legacy_bin(&bin_path, stem.sample_bits())?;
            (
                bin::records_from_bin(&stem, &samples)?,
                TimestampProvenance::Estimated,
            )
        }
        (true, true) => {
            let rows = csv::read_legacy_csv(&csv_path)?;
            let samples = bin::read_legacy_bin(&bin_path, stem.sample_bits())?;
            if rows.len() != samples.len() {
                return Err(RecordingError::Corrupt {
                    reason: format!(
                        "legacy csv has {} rows but bin has {} samples",
                        rows.len(),
                        samples.len()
                    ),
                });
            }
            for (row, sample) in rows.iter().zip(samples.iter()) {
                if row.ones != sample.ones {
                    return Err(RecordingError::Corrupt {
                        reason: format!(
                            "legacy csv ones {} do not match bin popcount {} at {}",
                            row.ones,
                            sample.ones,
                            sample.index.get()
                        ),
                    });
                }
            }
            let mut records = csv::records_from_csv(&rows)?;
            let length = rngkit_core::ByteLength::from_sample_bits(stem.sample_bits())?;
            for (i, record) in records.iter_mut().enumerate() {
                record.byte_offset = Some(rngkit_core::ByteOffset::new(
                    (i as u64)
                        .checked_mul(u64::from(length.get()))
                        .ok_or_else(|| RecordingError::Corrupt {
                            reason: "byte offset overflow".into(),
                        })?,
                ));
                record.byte_length = Some(length);
            }
            (records, TimestampProvenance::Recorded)
        }
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

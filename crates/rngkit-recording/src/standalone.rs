//! Format-neutral standalone input detection and normalization.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use rngkit_core::{SampleBits, SampleRecord, SessionStatus, TimestampProvenance};

use crate::NATIVE_CSV_COLUMNS;
use crate::error::RecordingError;
use crate::legacy_v3::{bin::LegacyBinReader, open_legacy};
use crate::naming::SessionStem;
use crate::native::reader::read_current_csv;
use crate::normalized::{NormalizedMeta, NormalizedSession, StandaloneInputFormat};

/// Opens one current or legacy CSV/BIN without requiring a manifest.
///
/// The filename stem supplies source, sample size, interval, and fold. A
/// same-stem sibling is validated when present; no input is modified.
pub fn open_standalone(path: impl AsRef<Path>) -> Result<NormalizedSession, RecordingError> {
    let path = path.as_ref();
    let stem_string = file_stem(path)?;
    let stem = SessionStem::parse(&stem_string)?;
    let extension = path.extension().and_then(|value| value.to_str());
    match extension {
        Some("csv") => match detect_csv_format(path)? {
            StandaloneInputFormat::CurrentCsv => {
                open_current_csv_standalone(path, &stem_string, &stem)
            }
            StandaloneInputFormat::LegacyV3Csv => open_legacy(path),
            StandaloneInputFormat::Bin => unreachable!("BIN cannot be detected from a CSV path"),
        },
        Some("bin") => open_bin_standalone(path, &stem_string, &stem),
        _ => Err(RecordingError::InvalidName {
            reason: "standalone input must have a .csv or .bin extension".into(),
        }),
    }
}

/// Detects the exact current native header or the headerless legacy format.
pub(crate) fn detect_csv_format(path: &Path) -> Result<StandaloneInputFormat, RecordingError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let expected = NATIVE_CSV_COLUMNS.join(",");
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == expected {
            return Ok(StandaloneInputFormat::CurrentCsv);
        }
        if trimmed
            .split(',')
            .next()
            .is_some_and(|first| first.trim() == "sample_index")
        {
            return Err(RecordingError::InvalidNativeCsvHeader {
                basename: input_basename(path)?,
            });
        }
        return Ok(StandaloneInputFormat::LegacyV3Csv);
    }
    Ok(StandaloneInputFormat::LegacyV3Csv)
}

fn open_current_csv_standalone(
    path: &Path,
    stem_string: &str,
    stem: &SessionStem,
) -> Result<NormalizedSession, RecordingError> {
    validate_current_source(stem)?;
    let records = read_current_csv(path, stem.sample_bits())?;
    if let Some(bin_path) = sibling_path(path, "bin") {
        validate_bin_pair(&bin_path, stem.sample_bits(), &records)?;
    }
    Ok(normalized_from_records(
        stem_string,
        stem,
        records,
        TimestampProvenance::Recorded,
    ))
}

fn open_bin_standalone(
    path: &Path,
    stem_string: &str,
    stem: &SessionStem,
) -> Result<NormalizedSession, RecordingError> {
    if let Some(csv_path) = sibling_path(path, "csv") {
        match detect_csv_format(&csv_path)? {
            StandaloneInputFormat::CurrentCsv => {
                return open_current_csv_standalone(&csv_path, stem_string, stem);
            }
            StandaloneInputFormat::LegacyV3Csv => return open_legacy(path),
            StandaloneInputFormat::Bin => unreachable!("CSV cannot be detected as BIN"),
        }
    }

    validate_current_source(stem)?;
    let mut reader = LegacyBinReader::open(path, stem.sample_bits())?;
    let records = crate::legacy_v3::bin::records_from_bin(stem, &mut reader)?;
    Ok(normalized_from_records(
        stem_string,
        stem,
        records,
        TimestampProvenance::Estimated,
    ))
}

fn validate_bin_pair(
    path: &Path,
    sample_bits: SampleBits,
    records: &[SampleRecord],
) -> Result<(), RecordingError> {
    let mut reader = LegacyBinReader::open(path, sample_bits)?;
    let expected_count = u64::try_from(records.len()).map_err(|_| RecordingError::Corrupt {
        reason: "current csv row count overflow".into(),
    })?;
    if reader.sample_count() != expected_count {
        return Err(RecordingError::Corrupt {
            reason: format!(
                "current csv has {} rows but bin has {} samples",
                records.len(),
                reader.sample_count()
            ),
        });
    }
    for record in records {
        let Some(meta) = reader.read_next()? else {
            return Err(RecordingError::Corrupt {
                reason: "current bin ended before csv rows".into(),
            });
        };
        if meta.index != record.index
            || meta.ones != record.ones
            || Some(meta.byte_offset) != record.byte_offset
            || Some(meta.byte_length) != record.byte_length
        {
            return Err(RecordingError::Corrupt {
                reason: format!(
                    "current csv and bin differ at sample {}",
                    record.index.get()
                ),
            });
        }
    }
    Ok(())
}

fn normalized_from_records(
    stem_string: &str,
    stem: &SessionStem,
    records: Vec<SampleRecord>,
    provenance: TimestampProvenance,
) -> NormalizedSession {
    NormalizedSession::from_parts(
        NormalizedMeta {
            stem: stem_string.to_owned(),
            source_id: stem.source().clone(),
            source_label: source_label(stem.source().as_str()),
            source_variant: None,
            fold: stem.fold(),
            sample_bits: stem.sample_bits(),
            interval: stem.interval(),
            started_at: Some(rngkit_core::UtcTimestamp::new(stem.local_start())),
            completed_at: records.last().map(|record| record.timestamp),
            status: SessionStatus::Completed,
            overrun_count: None,
            provenance,
            local_utc_offset: None,
        },
        records,
    )
}

fn validate_current_source(stem: &SessionStem) -> Result<(), RecordingError> {
    if matches!(
        stem.source().as_str(),
        rngkit_core::SOURCE_ID_BITB
            | rngkit_core::SOURCE_ID_TRNG
            | rngkit_core::SOURCE_ID_RDSEED
            | rngkit_core::SOURCE_ID_PSEUDO
    ) {
        Ok(())
    } else {
        Err(RecordingError::UnsupportedVersion {
            reason: format!("standalone input does not support source {}", stem.source()),
        })
    }
}

fn source_label(id: &str) -> String {
    match id {
        rngkit_core::SOURCE_ID_BITB => "BitBabbler".into(),
        rngkit_core::SOURCE_ID_TRNG => "TrueRNG v1/v2/v3".into(),
        rngkit_core::SOURCE_ID_RDSEED => "RDSEED".into(),
        rngkit_core::SOURCE_ID_PSEUDO => "PseudoRNG".into(),
        other => other.into(),
    }
}

fn sibling_path(path: &Path, extension: &str) -> Option<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem()?.to_str()?;
    let sibling = parent.join(format!("{stem}.{extension}"));
    sibling.is_file().then_some(sibling)
}

fn file_stem(path: &Path) -> Result<String, RecordingError> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .ok_or_else(|| RecordingError::InvalidName {
            reason: "path is not valid utf-8".into(),
        })
}

fn input_basename(path: &Path) -> Result<String, RecordingError> {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .ok_or_else(|| RecordingError::InvalidName {
            reason: "path is not valid utf-8".into(),
        })
}

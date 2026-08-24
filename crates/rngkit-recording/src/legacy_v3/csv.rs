//! Headerless RngKitPSG version 3 CSV.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use rngkit_core::{SampleBits, SampleIndex, SampleRecord, TimestampProvenance, UtcTimestamp};
use time::{Date, Month, PrimitiveDateTime, Time};

use crate::error::RecordingError;

/// One version 3 CSV pair: timestamp and one-count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyCsvRow {
    /// Parsed timestamp from the CSV.
    pub timestamp: UtcTimestamp,
    /// One-bit count.
    pub ones: u64,
}

/// Reads headerless comma-delimited version 3 CSV.
///
/// Expected row: `YYYYMMDDTHHMMSS,<ones>`. The older
/// `YYYYMMDDTHH:MM:SS,<ones>` form remains readable for compatibility.
/// Space-delimited rows are rejected.
///
/// # Errors
///
/// Returns [`RecordingError::UnsupportedVersion`] for space-delimited input.
/// Returns [`RecordingError::OnesExceedSampleBits`] when a one-count exceeds
/// `sample_bits`.
pub fn read_legacy_csv(
    path: &Path,
    sample_bits: SampleBits,
) -> Result<Vec<LegacyCsvRow>, RecordingError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.contains(',') {
            return Err(RecordingError::UnsupportedVersion {
                reason: format!("line {} is not comma-delimited", i + 1),
            });
        }
        let mut parts = line.split(',');
        let ts = parts.next().ok_or_else(|| RecordingError::InvalidName {
            reason: format!("line {} missing timestamp", i + 1),
        })?;
        let ones_raw = parts.next().ok_or_else(|| RecordingError::InvalidName {
            reason: format!("line {} missing ones count", i + 1),
        })?;
        if parts.next().is_some() {
            return Err(RecordingError::InvalidName {
                reason: format!("line {} has extra fields", i + 1),
            });
        }
        if ts.contains(' ') || ones_raw.contains(' ') {
            return Err(RecordingError::UnsupportedVersion {
                reason: format!("line {} is space-delimited", i + 1),
            });
        }
        let timestamp = parse_legacy_timestamp(ts)?;
        let ones: u64 = ones_raw
            .trim()
            .parse()
            .map_err(|_| RecordingError::InvalidName {
                reason: format!("line {} has invalid ones count {ones_raw}", i + 1),
            })?;
        if ones > u64::from(sample_bits.get()) {
            return Err(RecordingError::OnesExceedSampleBits {
                ones,
                sample_bits: sample_bits.get(),
            });
        }
        rows.push(LegacyCsvRow { timestamp, ones });
    }
    Ok(rows)
}

/// Converts legacy CSV rows to normalized records.
pub fn records_from_csv(rows: &[LegacyCsvRow]) -> Result<Vec<SampleRecord>, RecordingError> {
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            Ok(SampleRecord {
                index: SampleIndex::new(
                    u64::try_from(i + 1)
                        .map_err(|_| rngkit_core::CoreError::SizeOverflow { value: i as u64 })?,
                )?,
                timestamp: row.timestamp,
                provenance: TimestampProvenance::Recorded,
                elapsed: None,
                acquisition: None,
                ones: row.ones,
                byte_offset: None,
                byte_length: None,
            })
        })
        .collect()
}

pub(crate) fn parse_legacy_timestamp(raw: &str) -> Result<UtcTimestamp, RecordingError> {
    let bytes = raw.as_bytes();
    let compact = bytes.len() == 15
        && bytes.iter().enumerate().all(|(i, &b)| {
            if i == 8 {
                b == b'T'
            } else {
                b.is_ascii_digit()
            }
        });
    let colon = bytes.len() == 17
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            8 => b == b'T',
            11 | 14 => b == b':',
            _ => b.is_ascii_digit(),
        });
    if !compact && !colon {
        return Err(RecordingError::InvalidName {
            reason: format!("expected YYYYMMDDTHHMMSS, got {raw}"),
        });
    }
    let year: i32 = raw[0..4].parse().map_err(|_| RecordingError::InvalidName {
        reason: format!("invalid timestamp {raw}"),
    })?;
    let month: u8 = raw[4..6].parse().map_err(|_| RecordingError::InvalidName {
        reason: format!("invalid timestamp {raw}"),
    })?;
    let day: u8 = raw[6..8].parse().map_err(|_| RecordingError::InvalidName {
        reason: format!("invalid timestamp {raw}"),
    })?;
    let hour: u8 = raw[9..11]
        .parse()
        .map_err(|_| RecordingError::InvalidName {
            reason: format!("invalid timestamp {raw}"),
        })?;
    let minute_start = if compact { 11 } else { 12 };
    let minute_end = if compact { 13 } else { 14 };
    let second_start = if compact { 13 } else { 15 };
    let second_end = if compact { 15 } else { 17 };
    let minute: u8 =
        raw[minute_start..minute_end]
            .parse()
            .map_err(|_| RecordingError::InvalidName {
                reason: format!("invalid timestamp {raw}"),
            })?;
    let second: u8 =
        raw[second_start..second_end]
            .parse()
            .map_err(|_| RecordingError::InvalidName {
                reason: format!("invalid timestamp {raw}"),
            })?;
    let month = Month::try_from(month).map_err(|_| RecordingError::InvalidName {
        reason: format!("invalid timestamp {raw}"),
    })?;
    let date =
        Date::from_calendar_date(year, month, day).map_err(|_| RecordingError::InvalidName {
            reason: format!("invalid timestamp {raw}"),
        })?;
    let time = Time::from_hms(hour, minute, second).map_err(|_| RecordingError::InvalidName {
        reason: format!("invalid timestamp {raw}"),
    })?;
    Ok(UtcTimestamp::new(
        PrimitiveDateTime::new(date, time).assume_utc(),
    ))
}

#[cfg(test)]
mod tests {
    use super::parse_legacy_timestamp;

    #[test]
    fn valid_timestamp_parses() {
        parse_legacy_timestamp("20260821T18:30:00").expect("valid v3 timestamp");
        parse_legacy_timestamp("20260821T183000").expect("valid compact v3 timestamp");
    }

    #[test]
    fn malformed_multibyte_timestamp_is_error_not_panic() {
        let result = std::panic::catch_unwind(|| parse_legacy_timestamp("202é123T12:34:56"));
        assert!(result.is_ok(), "timestamp parser unwound");
        assert!(result.unwrap().is_err());
    }
}

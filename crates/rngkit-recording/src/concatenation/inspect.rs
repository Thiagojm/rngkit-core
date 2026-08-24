//! Streaming inspection of legacy v3 CSV concatenation inputs.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use rngkit_core::{
    Fold, IntervalSeconds, SOURCE_ID_BITB, SOURCE_ID_PSEUDO, SOURCE_ID_RDSEED, SOURCE_ID_TRNG,
    SampleBits, SampleIndex, SourceId, UtcTimestamp,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::NATIVE_CSV_COLUMNS;
use crate::concatenation::manifest::{ConcatenationInputEntry, ContentSha256, validate_basename};
use crate::error::{ConcatenationCompatibilityField, RecordingError};
use crate::legacy_v3::csv::parse_legacy_timestamp;
use crate::naming::SessionStem;
use crate::native::NativeCsvRow;
use crate::normalized::StandaloneInputFormat;
use crate::standalone::detect_csv_format;

/// Safe inspect preview: compatibility fields and ordered input metadata.
///
/// Absolute input paths are not stored, debugged, or serialized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConcatenationPreview {
    source_id: SourceId,
    sample_bits: SampleBits,
    interval: IntervalSeconds,
    #[serde(skip_serializing_if = "Option::is_none")]
    fold: Option<Fold>,
    total_rows: u64,
    inputs: Vec<ConcatenationInputEntry>,
}

impl ConcatenationPreview {
    /// Source identifier shared by every input.
    #[must_use]
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Sample size shared by every input.
    #[must_use]
    pub fn sample_bits(&self) -> SampleBits {
        self.sample_bits
    }

    /// Interval shared by every input.
    #[must_use]
    pub fn interval(&self) -> IntervalSeconds {
        self.interval
    }

    /// Fold shared by every input, when BitBabbler.
    #[must_use]
    pub fn fold(&self) -> Option<Fold> {
        self.fold
    }

    /// Total rows across ordered inputs.
    #[must_use]
    pub fn total_rows(&self) -> u64 {
        self.total_rows
    }

    /// Inputs in derived chronological order.
    #[must_use]
    pub fn inputs(&self) -> &[ConcatenationInputEntry] {
        &self.inputs
    }
}

/// Inspected preview plus the corresponding ordered input paths.
///
/// Paths are crate-internal so creation can reopen the same files. They are
/// never stored on [`ConcatenationPreview`].
pub(crate) struct OrderedInspection {
    pub preview: ConcatenationPreview,
    pub paths: Vec<PathBuf>,
}

/// Inspects legacy v3 CSV paths without modifying them or retaining rows.
///
/// The returned preview is ordered by first-row timestamp. It contains
/// basenames and SHA-256 digests and never serializes absolute input paths.
///
/// # Errors
///
/// Returns a typed [`RecordingError`] when the input list is empty, an input
/// is empty, malformed, native, duplicated, incompatible, internally
/// decreasing, or overlapping, including equal timestamp boundaries between
/// files.
pub fn inspect_legacy_csvs(paths: &[PathBuf]) -> Result<ConcatenationPreview, RecordingError> {
    Ok(inspect_csv_inputs_ordered(paths, false)?.preview)
}

/// Inspects current, legacy, or mixed CSV inputs and returns a normalized
/// preview without retaining rows or absolute paths.
pub fn inspect_csv_inputs(paths: &[PathBuf]) -> Result<ConcatenationPreview, RecordingError> {
    Ok(inspect_csv_inputs_ordered(paths, true)?.preview)
}

/// Inspects inputs and returns preview metadata with matching ordered paths.
pub(crate) fn inspect_csv_inputs_ordered(
    paths: &[PathBuf],
    allow_current: bool,
) -> Result<OrderedInspection, RecordingError> {
    if paths.is_empty() {
        return Err(RecordingError::EmptyConcatenationInputs);
    }

    let mut seen = HashSet::with_capacity(paths.len());
    let mut files = Vec::with_capacity(paths.len());
    let mut expected: Option<Compatibility> = None;

    for path in paths {
        let basename = input_basename(path)?;
        if !basename.ends_with(".csv") {
            return Err(RecordingError::ConcatenationInputNotCsv { basename });
        }
        validate_basename(&basename)?;

        let canonical = fs::canonicalize(path)?;
        if !seen.insert(canonical) {
            return Err(RecordingError::DuplicateConcatenationInput { basename });
        }

        let stem_str = basename.strip_suffix(".csv").ok_or_else(|| {
            RecordingError::ConcatenationInputNotCsv {
                basename: basename.clone(),
            }
        })?;
        let stem = SessionStem::parse(stem_str)?;
        let format = detect_csv_format(path)?;
        if format == StandaloneInputFormat::CurrentCsv && !allow_current {
            return Err(RecordingError::NativeConcatenationInput { basename });
        }
        validate_source(stem.source(), format)?;

        let inspected = inspect_one_csv(path, &basename, stem.sample_bits(), format)?;
        match &expected {
            None => {
                expected = Some(Compatibility {
                    source: stem.source().clone(),
                    sample_bits: stem.sample_bits(),
                    interval: stem.interval(),
                    fold: stem.fold(),
                    basename: basename.clone(),
                });
            }
            Some(compat) => check_compat(compat, &stem, &basename)?,
        }
        files.push(InspectedFile {
            path: path.clone(),
            basename,
            format,
            sha256: inspected.sha256,
            row_count: inspected.row_count,
            first: inspected.first,
            last: inspected.last,
        });
    }

    files.sort_by(|left, right| {
        left.first
            .cmp(&right.first)
            .then_with(|| left.basename.cmp(&right.basename))
    });

    for pair in files.windows(2) {
        if pair[0].last >= pair[1].first {
            return Err(RecordingError::OverlappingConcatenationRanges {
                left_basename: pair[0].basename.clone(),
                right_basename: pair[1].basename.clone(),
            });
        }
    }

    let mut next_index = 1u64;
    let mut total_rows = 0u64;
    let mut entries = Vec::with_capacity(files.len());
    let mut ordered_paths = Vec::with_capacity(files.len());
    for file in files {
        total_rows = total_rows
            .checked_add(file.row_count)
            .ok_or(RecordingError::ConcatenationCountOverflow)?;
        let output_start = SampleIndex::new(next_index)?;
        let output_end_raw = next_index
            .checked_add(file.row_count - 1)
            .ok_or(RecordingError::ConcatenationCountOverflow)?;
        let output_end = SampleIndex::new(output_end_raw)?;
        next_index = output_end_raw
            .checked_add(1)
            .ok_or(RecordingError::ConcatenationCountOverflow)?;
        ordered_paths.push(file.path);
        let entry = if allow_current {
            ConcatenationInputEntry::new_with_format(
                file.basename,
                file.sha256,
                file.row_count,
                file.first,
                file.last,
                output_start,
                output_end,
                file.format,
            )?
        } else {
            ConcatenationInputEntry::new(
                file.basename,
                file.sha256,
                file.row_count,
                file.first,
                file.last,
                output_start,
                output_end,
            )?
        };
        entries.push(entry);
    }

    let Some(compat) = expected else {
        return Err(RecordingError::EmptyConcatenationInputs);
    };
    Ok(OrderedInspection {
        preview: ConcatenationPreview {
            source_id: compat.source,
            sample_bits: compat.sample_bits,
            interval: compat.interval,
            fold: compat.fold,
            total_rows,
            inputs: entries,
        },
        paths: ordered_paths,
    })
}

struct Compatibility {
    source: SourceId,
    sample_bits: SampleBits,
    interval: IntervalSeconds,
    fold: Option<Fold>,
    basename: String,
}

struct InspectedFile {
    path: PathBuf,
    basename: String,
    format: StandaloneInputFormat,
    sha256: ContentSha256,
    row_count: u64,
    first: UtcTimestamp,
    last: UtcTimestamp,
}

pub(crate) struct RowScan {
    pub sha256: ContentSha256,
    pub row_count: u64,
    pub first: UtcTimestamp,
    pub last: UtcTimestamp,
}

struct HashingReader<R> {
    inner: R,
    hasher: Sha256,
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 {
            self.hasher.update(&buf[..n]);
        }
        Ok(n)
    }
}

fn inspect_one_csv(
    path: &Path,
    basename: &str,
    sample_bits: SampleBits,
    format: StandaloneInputFormat,
) -> Result<RowScan, RecordingError> {
    for_each_csv_row(path, basename, sample_bits, format, |_timestamp, _ones| {
        Ok(())
    })
}

/// Streams either supported CSV representation, hashing raw bytes and
/// visiting normalized timestamp/one-count pairs.
pub(crate) fn for_each_csv_row(
    path: &Path,
    basename: &str,
    sample_bits: SampleBits,
    format: StandaloneInputFormat,
    visit: impl FnMut(UtcTimestamp, u64) -> Result<(), RecordingError>,
) -> Result<RowScan, RecordingError> {
    match format {
        StandaloneInputFormat::LegacyV3Csv => {
            for_each_legacy_csv_row(path, basename, sample_bits, visit)
        }
        StandaloneInputFormat::CurrentCsv => {
            for_each_current_csv_row(path, basename, sample_bits, visit)
        }
        StandaloneInputFormat::Bin => Err(RecordingError::ConcatenationInputNotCsv {
            basename: basename.to_owned(),
        }),
    }
}

fn for_each_current_csv_row(
    path: &Path,
    basename: &str,
    sample_bits: SampleBits,
    mut visit: impl FnMut(UtcTimestamp, u64) -> Result<(), RecordingError>,
) -> Result<RowScan, RecordingError> {
    let file = File::open(path)?;
    let mut hashing = HashingReader {
        inner: file,
        hasher: Sha256::new(),
    };
    let mut csv = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(&mut hashing);
    let headers = csv.headers()?;
    let expected: Vec<&str> = NATIVE_CSV_COLUMNS.to_vec();
    let observed: Vec<&str> = headers.iter().collect();
    if observed != expected {
        return Err(RecordingError::InvalidNativeCsvHeader {
            basename: basename.to_owned(),
        });
    }
    let sample_bytes =
        u64::try_from(sample_bits.bytes()?).map_err(|_| RecordingError::Corrupt {
            reason: "sample byte length does not fit in u64".into(),
        })?;
    let mut first = None;
    let mut last = None;
    let mut prev = None;
    let mut row_count = 0u64;
    let mut expected_index = 1u64;
    let mut expected_offset = 0u64;
    for row in csv.deserialize::<NativeCsvRow>() {
        let row = row?;
        if row.sample_index != expected_index || row.byte_offset != expected_offset {
            return Err(RecordingError::Corrupt {
                reason: format!(
                    "current csv indexes or offsets are not contiguous at row {}",
                    row_count + 1
                ),
            });
        }
        if u64::from(row.byte_length) != sample_bytes {
            return Err(RecordingError::Corrupt {
                reason: format!(
                    "byte_length {} does not match sample size {sample_bytes}",
                    row.byte_length
                ),
            });
        }
        if row.ones > u64::from(sample_bits.get()) {
            return Err(RecordingError::OnesExceedSampleBits {
                ones: row.ones,
                sample_bits: sample_bits.get(),
            });
        }
        let timestamp = row.to_record()?.timestamp;
        if let Some(previous) = prev {
            if timestamp < previous {
                return Err(RecordingError::DecreasingConcatenationTimestamp {
                    basename: basename.to_owned(),
                });
            }
        }
        visit(timestamp, row.ones)?;
        first.get_or_insert(timestamp);
        last = Some(timestamp);
        prev = Some(timestamp);
        row_count = row_count
            .checked_add(1)
            .ok_or(RecordingError::ConcatenationCountOverflow)?;
        expected_index = expected_index
            .checked_add(1)
            .ok_or(RecordingError::ConcatenationCountOverflow)?;
        expected_offset = row
            .byte_offset
            .checked_add(u64::from(row.byte_length))
            .ok_or_else(|| RecordingError::Corrupt {
                reason: "byte range overflow".into(),
            })?;
    }
    let first = first.ok_or_else(|| RecordingError::EmptyConcatenationInput {
        basename: basename.to_owned(),
    })?;
    let last = last.ok_or_else(|| RecordingError::EmptyConcatenationInput {
        basename: basename.to_owned(),
    })?;
    let digest = hashing.hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(digest.as_ref());
    Ok(RowScan {
        sha256: ContentSha256::from_bytes(&bytes),
        row_count,
        first,
        last,
    })
}

/// Streams one legacy CSV, hashing raw bytes and visiting each data row.
///
/// Does not retain the combined rows. The visitor may write derived output.
pub(crate) fn for_each_legacy_csv_row(
    path: &Path,
    basename: &str,
    sample_bits: SampleBits,
    mut visit: impl FnMut(UtcTimestamp, u64) -> Result<(), RecordingError>,
) -> Result<RowScan, RecordingError> {
    let file = File::open(path)?;
    let mut hashing = HashingReader {
        inner: file,
        hasher: Sha256::new(),
    };
    let mut first = None;
    let mut last = None;
    let mut prev = None;
    let mut row_count = 0u64;

    {
        let reader = BufReader::new(&mut hashing);
        for (index, line) in reader.lines().enumerate() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if row_count == 0 && trimmed == NATIVE_CSV_COLUMNS.join(",") {
                return Err(RecordingError::NativeConcatenationInput {
                    basename: basename.to_owned(),
                });
            }
            let (timestamp, ones) = parse_legacy_csv_line(trimmed, index + 1, sample_bits)?;
            if let Some(previous) = prev {
                if timestamp < previous {
                    return Err(RecordingError::DecreasingConcatenationTimestamp {
                        basename: basename.to_owned(),
                    });
                }
            }
            visit(timestamp, ones)?;
            if first.is_none() {
                first = Some(timestamp);
            }
            last = Some(timestamp);
            prev = Some(timestamp);
            row_count = row_count
                .checked_add(1)
                .ok_or(RecordingError::ConcatenationCountOverflow)?;
        }
    }

    let first = first.ok_or_else(|| RecordingError::EmptyConcatenationInput {
        basename: basename.to_owned(),
    })?;
    let last = last.ok_or_else(|| RecordingError::EmptyConcatenationInput {
        basename: basename.to_owned(),
    })?;
    let digest = hashing.hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(digest.as_ref());
    Ok(RowScan {
        sha256: ContentSha256::from_bytes(&bytes),
        row_count,
        first,
        last,
    })
}

fn parse_legacy_csv_line(
    line: &str,
    line_number: usize,
    sample_bits: SampleBits,
) -> Result<(UtcTimestamp, u64), RecordingError> {
    if !line.contains(',') {
        return Err(RecordingError::UnsupportedVersion {
            reason: format!("line {line_number} is not comma-delimited"),
        });
    }
    let mut parts = line.split(',');
    let ts = parts.next().ok_or_else(|| RecordingError::InvalidName {
        reason: format!("line {line_number} missing timestamp"),
    })?;
    let ones_raw = parts.next().ok_or_else(|| RecordingError::InvalidName {
        reason: format!("line {line_number} missing ones count"),
    })?;
    if parts.next().is_some() {
        return Err(RecordingError::InvalidName {
            reason: format!("line {line_number} has extra fields"),
        });
    }
    if ts.contains(' ') || ones_raw.contains(' ') {
        return Err(RecordingError::UnsupportedVersion {
            reason: format!("line {line_number} is space-delimited"),
        });
    }
    let timestamp = parse_legacy_timestamp(ts)?;
    let ones: u64 = ones_raw
        .trim()
        .parse()
        .map_err(|_| RecordingError::InvalidName {
            reason: format!("line {line_number} has invalid ones count {ones_raw}"),
        })?;
    if ones > u64::from(sample_bits.get()) {
        return Err(RecordingError::OnesExceedSampleBits {
            ones,
            sample_bits: sample_bits.get(),
        });
    }
    Ok((timestamp, ones))
}

fn validate_source(source: &SourceId, format: StandaloneInputFormat) -> Result<(), RecordingError> {
    let supported = match format {
        StandaloneInputFormat::LegacyV3Csv => {
            matches!(
                source.as_str(),
                SOURCE_ID_BITB | SOURCE_ID_TRNG | SOURCE_ID_PSEUDO
            )
        }
        StandaloneInputFormat::CurrentCsv => matches!(
            source.as_str(),
            SOURCE_ID_BITB | SOURCE_ID_TRNG | SOURCE_ID_RDSEED | SOURCE_ID_PSEUDO
        ),
        StandaloneInputFormat::Bin => false,
    };
    if !supported {
        return Err(RecordingError::UnsupportedVersion {
            reason: format!(
                "standalone {} does not include source {source}",
                format_name(format)
            ),
        });
    }
    Ok(())
}

fn format_name(format: StandaloneInputFormat) -> &'static str {
    match format {
        StandaloneInputFormat::CurrentCsv => "current csv",
        StandaloneInputFormat::LegacyV3Csv => "legacy v3 csv",
        StandaloneInputFormat::Bin => "bin",
    }
}

fn check_compat(
    expected: &Compatibility,
    stem: &SessionStem,
    basename: &str,
) -> Result<(), RecordingError> {
    let mismatch = if expected.source != *stem.source() {
        Some(ConcatenationCompatibilityField::Source)
    } else if expected.sample_bits != stem.sample_bits() {
        Some(ConcatenationCompatibilityField::SampleBits)
    } else if expected.interval != stem.interval() {
        Some(ConcatenationCompatibilityField::Interval)
    } else if expected.fold != stem.fold() {
        Some(ConcatenationCompatibilityField::Fold)
    } else {
        None
    };
    if let Some(field) = mismatch {
        return Err(RecordingError::IncompatibleConcatenationInputs {
            field,
            left_basename: expected.basename.clone(),
            right_basename: basename.to_owned(),
        });
    }
    Ok(())
}

fn input_basename(path: &Path) -> Result<String, RecordingError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| RecordingError::InvalidName {
            reason: "path is not valid utf-8".into(),
        })?;
    Ok(name.to_owned())
}

//! Streaming native session reader.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use rngkit_core::{SampleRecord, count_ones};

use crate::consistency::{ConsistencyReport, ConsistencyWarning};
use crate::error::RecordingError;
use crate::fsutil::open_contained;
use crate::manifest::Manifest;
use crate::naming::SessionStem;
use crate::native::csv::NativeCsvRow;
use crate::normalized::{NormalizedSession, SessionOrigin};

/// Opened native session bundle.
#[derive(Debug)]
pub struct NativeSession {
    dir: PathBuf,
    stem: SessionStem,
    manifest: Manifest,
    csv_path: PathBuf,
    bin_path: PathBuf,
    bin: File,
    report: ConsistencyReport,
    records: Vec<SampleRecord>,
}

impl NativeSession {
    /// Opens a native session directory read-only.
    ///
    /// Committed count is derived from CSV. An uncommitted BIN tail is a
    /// warning. CSV references beyond BIN EOF are hard corruption.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::Corrupt`] when CSV points past BIN EOF or
    /// indexes/offsets/lengths are not contiguous.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, RecordingError> {
        let dir = fs::canonicalize(dir.as_ref())?;
        let manifest = Manifest::read_from(&dir)?;
        let stem = manifest.session_stem()?;
        let (csv_path, csv_file) = open_contained(&dir, manifest.csv_file())?;
        let (bin_path, bin) = open_contained(&dir, manifest.bin_file())?;
        let mut csv = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(csv_file);
        let bin_len = bin.metadata()?.len();
        let sample_bytes = u64::from(manifest.sample_bits().bytes()? as u32);
        let mut records = Vec::new();
        let mut expected_index = 1u64;
        let mut expected_offset = 0u64;
        for row in csv.deserialize::<NativeCsvRow>() {
            let row = row?;
            if row.sample_index != expected_index {
                return Err(RecordingError::Corrupt {
                    reason: format!(
                        "sample_index {} is not contiguous (expected {expected_index})",
                        row.sample_index
                    ),
                });
            }
            if row.byte_offset != expected_offset {
                return Err(RecordingError::Corrupt {
                    reason: format!(
                        "byte_offset {} is not contiguous (expected {expected_offset})",
                        row.byte_offset
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
            let end = row
                .byte_offset
                .checked_add(u64::from(row.byte_length))
                .ok_or_else(|| RecordingError::Corrupt {
                    reason: "byte range overflow".into(),
                })?;
            if end > bin_len {
                return Err(RecordingError::Corrupt {
                    reason: format!("csv references bytes {end} beyond bin eof {bin_len}"),
                });
            }
            records.push(row.to_record()?);
            expected_index =
                expected_index
                    .checked_add(1)
                    .ok_or_else(|| RecordingError::Corrupt {
                        reason: "sample index overflow".into(),
                    })?;
            expected_offset = end;
        }
        let mut report = ConsistencyReport::new(records.len() as u64);
        if bin_len > expected_offset {
            report
                .warnings
                .push(ConsistencyWarning::UncommittedBinaryTail {
                    path: bin_path.clone(),
                    committed_bytes: expected_offset,
                    file_bytes: bin_len,
                });
        }
        Ok(Self {
            dir,
            stem,
            manifest,
            csv_path,
            bin_path,
            bin,
            report,
            records,
        })
    }

    /// Session directory.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.dir
    }

    /// Validated session stem.
    #[must_use]
    pub fn session_stem(&self) -> &SessionStem {
        &self.stem
    }

    /// Manifest.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Consistency report. CSV is the commit marker.
    #[must_use]
    pub fn consistency(&self) -> &ConsistencyReport {
        &self.report
    }

    /// Committed records derived from CSV.
    #[must_use]
    pub fn records(&self) -> &[SampleRecord] {
        &self.records
    }

    /// Streams the raw bytes for committed samples without loading the whole BIN.
    pub fn raw_samples(&self) -> RawSampleIter<'_> {
        RawSampleIter {
            bin: &self.bin,
            records: self.records.iter(),
            file: None,
        }
    }

    /// Normalized view used by analysis and XLSX.
    #[must_use]
    pub fn normalized(&self) -> NormalizedSession {
        NormalizedSession::from_native(self)
    }

    /// CSV path, for tests.
    #[must_use]
    pub fn csv_path(&self) -> &Path {
        &self.csv_path
    }

    /// BIN path, for tests.
    #[must_use]
    pub fn bin_path(&self) -> &Path {
        &self.bin_path
    }
}

/// Iterator that seeks and reads one committed sample at a time.
pub struct RawSampleIter<'a> {
    bin: &'a File,
    records: std::slice::Iter<'a, SampleRecord>,
    file: Option<File>,
}

impl Iterator for RawSampleIter<'_> {
    type Item = Result<(SampleRecord, Vec<u8>), RecordingError>;

    fn next(&mut self) -> Option<Self::Item> {
        let record = self.records.next()?.clone();
        Some(read_one(self, record))
    }
}

fn read_one(
    iter: &mut RawSampleIter<'_>,
    record: SampleRecord,
) -> Result<(SampleRecord, Vec<u8>), RecordingError> {
    if iter.file.is_none() {
        iter.file = Some(iter.bin.try_clone()?);
    }
    let file = iter.file.as_mut().expect("opened");
    let offset = record
        .byte_offset
        .ok_or_else(|| RecordingError::Corrupt {
            reason: "native row missing byte_offset".into(),
        })?
        .get();
    let length = record
        .byte_length
        .ok_or_else(|| RecordingError::Corrupt {
            reason: "native row missing byte_length".into(),
        })?
        .get();
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; length as usize];
    file.read_exact(&mut buf)?;
    let ones = count_ones(&buf)?;
    if ones != record.ones {
        return Err(RecordingError::Corrupt {
            reason: format!(
                "bin popcount {ones} does not match csv ones {}",
                record.ones
            ),
        });
    }
    Ok((record, buf))
}

impl SessionOrigin for NativeSession {
    fn sample_bits(&self) -> rngkit_core::SampleBits {
        self.manifest.sample_bits()
    }

    fn records(&self) -> &[SampleRecord] {
        &self.records
    }
}

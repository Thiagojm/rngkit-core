//! Append-only native BIN/CSV writer.

use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rngkit_core::{
    ByteLength, ByteOffset, SampleBits, SampleIndex, SampleRecord, SourceDescriptor,
    TimestampProvenance, UtcTimestamp, count_ones,
};
use time::UtcOffset;

use crate::error::RecordingError;
use crate::fsutil::join_contained;
use crate::manifest::Manifest;
use crate::naming::SessionStem;
use crate::native::csv::{NATIVE_CSV_COLUMNS, NativeCsvRow};

/// Optional commit-boundary failure injection for tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailPoint {
    /// Fail after validation, before BIN append.
    AfterValidate,
    /// Fail after BIN append, before BIN sync.
    AfterBinAppend,
    /// Fail after BIN sync, before CSV append.
    AfterBinSync,
    /// Fail after CSV append, before CSV sync.
    AfterCsvAppend,
    /// Fail while writing the completed manifest.
    CompleteManifest,
    /// Fail while writing the failed manifest.
    FailManifest,
    /// Fail both completed-manifest and failed-manifest writes.
    CompleteAndFailManifest,
}

/// Owned, non-cloneable writer for one native session.
#[derive(Debug)]
pub struct SessionWriter {
    dir: PathBuf,
    stem: SessionStem,
    sample_bits: SampleBits,
    byte_length: ByteLength,
    bin: File,
    csv: csv::Writer<File>,
    manifest: Manifest,
    next_index: u64,
    next_offset: u64,
    committed: u64,
    overruns: u64,
    fail: Option<FailPoint>,
}

impl SessionWriter {
    /// Creates a new session directory, BIN, CSV header, and recording manifest.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::AlreadyExists`] when the stem path exists.
    /// Invalid bits/intervals fail in [`SessionStem::new`] before this is called.
    pub fn create(
        root: &Path,
        stem: SessionStem,
        descriptor: &SourceDescriptor,
        started_at: UtcTimestamp,
        local_offset: UtcOffset,
    ) -> Result<Self, RecordingError> {
        if !root.exists() {
            fs::create_dir_all(root)?;
        }
        let dir = join_contained(root, stem.as_str())?;
        if dir.exists() {
            return Err(RecordingError::AlreadyExists { path: dir });
        }
        fs::create_dir(&dir)?;
        let bin_path = dir.join(format!("{}.bin", stem.as_str()));
        let csv_path = dir.join(format!("{}.csv", stem.as_str()));
        let bin = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&bin_path)?;
        let csv_file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&csv_path)?;
        let mut csv = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(csv_file);
        csv.write_record(NATIVE_CSV_COLUMNS)?;
        csv.flush()?;
        csv.get_ref().sync_all()?;
        let manifest = Manifest::recording(&stem, descriptor, started_at, local_offset);
        manifest.write_to(&dir)?;
        let byte_length = ByteLength::from_sample_bits(stem.sample_bits())?;
        Ok(Self {
            dir,
            sample_bits: stem.sample_bits(),
            stem,
            byte_length,
            bin,
            csv,
            manifest,
            next_index: 1,
            next_offset: 0,
            committed: 0,
            overruns: 0,
            fail: None,
        })
    }

    /// Session directory.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.dir
    }

    /// Session stem.
    #[must_use]
    pub fn stem(&self) -> &SessionStem {
        &self.stem
    }

    /// Committed sample count.
    #[must_use]
    pub fn committed(&self) -> u64 {
        self.committed
    }

    /// Recorded overrun count.
    #[must_use]
    pub fn overruns(&self) -> u64 {
        self.overruns
    }

    /// Increments the timing-overrun counter.
    pub fn add_overrun(&mut self) {
        self.overruns = self.overruns.saturating_add(1);
    }

    /// Test-only commit fault injection.
    #[doc(hidden)]
    pub fn set_fail_point(&mut self, point: Option<FailPoint>) {
        self.fail = point;
    }

    /// Validates, appends BIN, syncs BIN, appends CSV, syncs CSV, then reports
    /// the sample committed.
    ///
    /// # Errors
    ///
    /// Returns a typed error if validation or I/O fails. A sample is never
    /// reported committed before both the raw range and CSV row are durable.
    pub fn commit_sample(
        &mut self,
        bytes: &[u8],
        captured_at: UtcTimestamp,
        elapsed: Duration,
        acquisition: Duration,
    ) -> Result<SampleRecord, RecordingError> {
        let expected = self.sample_bits.bytes()?;
        if bytes.len() != expected {
            return Err(RecordingError::Commit {
                stage: "validate",
                reason: format!("expected {expected} bytes, got {}", bytes.len()),
            });
        }
        let ones = count_ones(bytes)?;
        if ones > u64::from(self.sample_bits.get()) {
            return Err(RecordingError::Commit {
                stage: "validate",
                reason: format!("ones {ones} exceed sample bits"),
            });
        }
        self.check_fail(FailPoint::AfterValidate)?;

        let offset = self.bin.seek(SeekFrom::End(0))?;
        if offset != self.next_offset {
            return Err(RecordingError::Commit {
                stage: "validate",
                reason: format!(
                    "bin offset {offset} does not match expected {}",
                    self.next_offset
                ),
            });
        }
        self.bin.write_all(bytes)?;
        self.check_fail(FailPoint::AfterBinAppend)?;
        self.bin.flush()?;
        self.bin.sync_all()?;
        self.check_fail(FailPoint::AfterBinSync)?;

        let index = SampleIndex::new(self.next_index)?;
        let record = SampleRecord {
            index,
            timestamp: captured_at,
            provenance: TimestampProvenance::Recorded,
            elapsed: Some(elapsed),
            acquisition: Some(acquisition),
            ones,
            byte_offset: Some(ByteOffset::new(self.next_offset)),
            byte_length: Some(self.byte_length),
        };
        let row = NativeCsvRow::from_record(
            &record,
            ByteOffset::new(self.next_offset),
            self.byte_length,
        )?;
        self.csv.serialize(&row)?;
        self.check_fail(FailPoint::AfterCsvAppend)?;
        self.csv.flush()?;
        self.csv.get_ref().sync_all()?;

        self.next_index = self
            .next_index
            .checked_add(1)
            .ok_or_else(|| RecordingError::Commit {
                stage: "csv",
                reason: "sample index overflow".into(),
            })?;
        self.next_offset = self
            .next_offset
            .checked_add(u64::from(self.byte_length.get()))
            .ok_or_else(|| RecordingError::Commit {
                stage: "bin",
                reason: "byte offset overflow".into(),
            })?;
        self.committed = self
            .committed
            .checked_add(1)
            .ok_or_else(|| RecordingError::Commit {
                stage: "csv",
                reason: "committed count overflow".into(),
            })?;
        Ok(record)
    }

    /// Finalizes a clean stop and returns the session directory.
    ///
    /// # Errors
    ///
    /// Returns a recording error when the completed manifest cannot be written.
    /// Prefer [`Self::finalize_completed`] when the caller still needs this
    /// writer for failed-manifest fallback.
    pub fn complete(mut self) -> Result<PathBuf, RecordingError> {
        self.finalize_completed()?;
        Ok(self.dir)
    }

    /// Writes the completed manifest without consuming the writer.
    ///
    /// # Errors
    ///
    /// Returns a recording error when the completed manifest cannot be written.
    pub fn finalize_completed(&mut self) -> Result<(), RecordingError> {
        self.check_fail_any(&[
            FailPoint::CompleteManifest,
            FailPoint::CompleteAndFailManifest,
        ])?;
        self.manifest
            .complete(self.committed, self.overruns, UtcTimestamp::now());
        self.manifest.write_to(&self.dir)
    }

    /// Best-effort failed-manifest write. The original terminal error remains
    /// the caller's responsibility.
    ///
    /// # Errors
    ///
    /// Returns a recording error when the failed manifest cannot be written.
    pub fn finalize_failed(&mut self, kind: &str, diagnostic: &str) -> Result<(), RecordingError> {
        self.check_fail_any(&[FailPoint::FailManifest, FailPoint::CompleteAndFailManifest])?;
        self.manifest.fail(
            self.committed,
            self.overruns,
            UtcTimestamp::now(),
            kind,
            diagnostic,
        );
        self.manifest.write_to(&self.dir)
    }

    /// Session directory after the caller has finished terminal finalization.
    #[must_use]
    pub fn into_directory(self) -> PathBuf {
        self.dir
    }

    fn check_fail(&self, point: FailPoint) -> Result<(), RecordingError> {
        self.check_fail_any(&[point])
    }

    fn check_fail_any(&self, points: &[FailPoint]) -> Result<(), RecordingError> {
        match self.fail {
            Some(point) if points.contains(&point) => Err(RecordingError::Commit {
                stage: fail_stage(point),
                reason: "injected failure".into(),
            }),
            _ => Ok(()),
        }
    }
}

fn fail_stage(point: FailPoint) -> &'static str {
    match point {
        FailPoint::AfterValidate => "validate",
        FailPoint::AfterBinAppend => "bin-append",
        FailPoint::AfterBinSync => "bin-sync",
        FailPoint::AfterCsvAppend => "csv-append",
        FailPoint::CompleteManifest | FailPoint::CompleteAndFailManifest => "complete-manifest",
        FailPoint::FailManifest => "fail-manifest",
    }
}

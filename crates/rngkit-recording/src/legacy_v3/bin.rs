//! Streaming fixed-size RngKitPSG version 3 BIN reader.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use rngkit_core::{
    ByteLength, ByteOffset, SampleBits, SampleIndex, SampleRecord, TimestampProvenance,
    UtcTimestamp, count_ones,
};

use crate::error::RecordingError;
use crate::naming::SessionStem;

/// Metadata for one exact BIN sample. The raw payload is not retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBinSampleMeta {
    /// One-based index.
    pub index: SampleIndex,
    /// Popcount of the current sample.
    pub ones: u64,
    /// Byte offset of this sample in the BIN file.
    pub byte_offset: ByteOffset,
    /// Byte length of this sample.
    pub byte_length: ByteLength,
}

/// Streams exact fixed-size samples from a version 3 BIN file.
///
/// The reader reuses a single sample-sized buffer and does not retain the
/// complete raw payload. A trailing partial sample is rejected at open.
#[derive(Debug)]
pub struct LegacyBinReader {
    file: File,
    buf: Vec<u8>,
    remaining: u64,
    total: u64,
    next_index: u64,
    next_offset: u64,
    byte_length: ByteLength,
}

impl LegacyBinReader {
    /// Opens `path` read-only and validates that its length is an exact
    /// multiple of one sample.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::Corrupt`] when the file length is not a
    /// multiple of the sample byte size, or when a size conversion overflows.
    pub fn open(path: &Path, sample_bits: SampleBits) -> Result<Self, RecordingError> {
        let mut file = File::open(path)?;
        let len = file.seek(SeekFrom::End(0))?;
        file.seek(SeekFrom::Start(0))?;
        let sample_len = sample_bits.bytes()?;
        if sample_len == 0 {
            return Err(RecordingError::Corrupt {
                reason: "sample byte length is zero".into(),
            });
        }
        let sample_len_u64 = u64::try_from(sample_len).map_err(|_| RecordingError::Corrupt {
            reason: "sample byte length does not fit in u64".into(),
        })?;
        if len % sample_len_u64 != 0 {
            return Err(RecordingError::Corrupt {
                reason: format!(
                    "partial trailing sample: file length {len} is not a multiple of {sample_len}"
                ),
            });
        }
        let total = len / sample_len_u64;
        let byte_length = ByteLength::from_sample_bits(sample_bits)?;
        Ok(Self {
            file,
            buf: vec![0u8; sample_len],
            remaining: total,
            total,
            next_index: 1,
            next_offset: 0,
            byte_length,
        })
    }

    /// Number of complete samples in the file.
    #[must_use]
    pub fn sample_count(&self) -> u64 {
        self.total
    }

    /// Length of the reused per-sample raw buffer.
    #[must_use]
    pub fn raw_buffer_len(&self) -> usize {
        self.buf.len()
    }

    /// Capacity of the reused per-sample raw buffer.
    #[must_use]
    pub fn raw_buffer_capacity(&self) -> usize {
        self.buf.capacity()
    }

    /// Reads the next sample, returning metadata only.
    ///
    /// The raw bytes live in the reused buffer until the next call and are not
    /// stored in the returned value.
    ///
    /// # Errors
    ///
    /// Returns I/O, popcount, or index/offset overflow errors.
    pub fn read_next(&mut self) -> Result<Option<LegacyBinSampleMeta>, RecordingError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.file.read_exact(&mut self.buf)?;
        let ones = count_ones(&self.buf)?;
        let index = SampleIndex::new(self.next_index)?;
        let meta = LegacyBinSampleMeta {
            index,
            ones,
            byte_offset: ByteOffset::new(self.next_offset),
            byte_length: self.byte_length,
        };
        self.remaining = self.remaining.saturating_sub(1);
        self.next_index =
            self.next_index
                .checked_add(1)
                .ok_or_else(|| RecordingError::Corrupt {
                    reason: "sample index overflow".into(),
                })?;
        self.next_offset = self
            .next_offset
            .checked_add(u64::from(self.byte_length.get()))
            .ok_or_else(|| RecordingError::Corrupt {
                reason: "byte offset overflow".into(),
            })?;
        Ok(Some(meta))
    }
}

impl Iterator for LegacyBinReader {
    type Item = Result<LegacyBinSampleMeta, RecordingError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_next() {
            Ok(Some(meta)) => Some(Ok(meta)),
            Ok(None) => None,
            Err(err) => {
                self.remaining = 0;
                Some(Err(err))
            }
        }
    }
}

/// BIN-only records with estimated timestamps from filename start plus interval.
pub fn records_from_bin(
    stem: &SessionStem,
    reader: &mut LegacyBinReader,
) -> Result<Vec<SampleRecord>, RecordingError> {
    let start = stem.local_start();
    let interval_secs = i64::from(stem.interval().get());
    let mut records = Vec::new();
    while let Some(meta) = reader.read_next()? {
        let steps = i64::try_from(meta.index.get().saturating_sub(1)).map_err(|_| {
            RecordingError::Corrupt {
                reason: "sample index exceeds timestamp range".into(),
            }
        })?;
        let ts = start + time::Duration::seconds(interval_secs.saturating_mul(steps));
        records.push(SampleRecord {
            index: meta.index,
            timestamp: UtcTimestamp::new(ts),
            provenance: TimestampProvenance::Estimated,
            elapsed: None,
            acquisition: None,
            ones: meta.ones,
            byte_offset: Some(meta.byte_offset),
            byte_length: Some(meta.byte_length),
        });
    }
    Ok(records)
}

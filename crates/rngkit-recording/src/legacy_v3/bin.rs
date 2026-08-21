//! Fixed-size RngKitPSG version 3 BIN reader.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use rngkit_core::{
    ByteLength, ByteOffset, SampleBits, SampleIndex, SampleRecord, TimestampProvenance,
    UtcTimestamp, count_ones,
};

use crate::error::RecordingError;
use crate::naming::SessionStem;

/// One exact BIN sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBinSample {
    /// One-based index.
    pub index: SampleIndex,
    /// Raw bytes.
    pub bytes: Vec<u8>,
    /// Popcount of `bytes`.
    pub ones: u64,
}

/// Streams exact fixed-size samples. A trailing partial sample is an error.
///
/// # Errors
///
/// Returns [`RecordingError::Corrupt`] when the file length is not a multiple
/// of the sample byte size.
pub fn read_legacy_bin(
    path: &Path,
    sample_bits: SampleBits,
) -> Result<Vec<LegacyBinSample>, RecordingError> {
    let mut file = File::open(path)?;
    let len = file.seek(SeekFrom::End(0))?;
    file.seek(SeekFrom::Start(0))?;
    let sample_len = sample_bits.bytes()?;
    if sample_len == 0 {
        return Err(RecordingError::Corrupt {
            reason: "sample byte length is zero".into(),
        });
    }
    if len % sample_len as u64 != 0 {
        return Err(RecordingError::Corrupt {
            reason: format!(
                "partial trailing sample: file length {len} is not a multiple of {sample_len}"
            ),
        });
    }
    let count = (len / sample_len as u64) as usize;
    let mut samples = Vec::with_capacity(count);
    for i in 0..count {
        let mut buf = vec![0u8; sample_len];
        file.read_exact(&mut buf)?;
        let ones = count_ones(&buf)?;
        samples.push(LegacyBinSample {
            index: SampleIndex::new((i as u64) + 1)?,
            bytes: buf,
            ones,
        });
    }
    Ok(samples)
}

/// BIN-only records with estimated timestamps from filename start plus interval.
pub fn records_from_bin(
    stem: &SessionStem,
    samples: &[LegacyBinSample],
) -> Result<Vec<SampleRecord>, RecordingError> {
    let start = stem.local_start();
    let length = ByteLength::from_sample_bits(stem.sample_bits())?;
    let mut offset = 0u64;
    let mut records = Vec::with_capacity(samples.len());
    let interval_secs = i64::from(stem.interval().get());
    for sample in samples {
        let steps = i64::try_from(sample.index.get().saturating_sub(1)).unwrap_or(i64::MAX);
        let ts = start + time::Duration::seconds(interval_secs.saturating_mul(steps));
        records.push(SampleRecord {
            index: sample.index,
            timestamp: UtcTimestamp::new(ts),
            provenance: TimestampProvenance::Estimated,
            elapsed: None,
            acquisition: None,
            ones: sample.ones,
            byte_offset: Some(ByteOffset::new(offset)),
            byte_length: Some(length),
        });
        offset =
            offset
                .checked_add(u64::from(length.get()))
                .ok_or_else(|| RecordingError::Corrupt {
                    reason: "byte offset overflow".into(),
                })?;
    }
    Ok(records)
}

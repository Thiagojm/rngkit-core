//! Shared session and sample value types.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset};

use crate::error::CoreError;
use crate::sample::SampleBits;
use crate::source::{Fold, SourceDescriptor, SourceId};

/// Integer collection interval in seconds, minimum one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct IntervalSeconds(u32);

impl IntervalSeconds {
    /// Validates a strictly positive interval.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::ZeroInterval`] when `seconds` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use rngkit_core::IntervalSeconds;
    ///
    /// assert_eq!(IntervalSeconds::new(1)?.get(), 1);
    /// assert!(IntervalSeconds::new(0).is_err());
    /// # Ok::<(), rngkit_core::CoreError>(())
    /// ```
    pub fn new(seconds: u32) -> Result<Self, CoreError> {
        if seconds == 0 {
            return Err(CoreError::ZeroInterval);
        }
        Ok(Self(seconds))
    }

    /// Interval in whole seconds.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Interval as a [`Duration`].
    #[must_use]
    pub fn duration(self) -> Duration {
        Duration::from_secs(u64::from(self.0))
    }
}

impl TryFrom<u32> for IntervalSeconds {
    type Error = CoreError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<IntervalSeconds> for u32 {
    fn from(value: IntervalSeconds) -> Self {
        value.0
    }
}

/// One-based contiguous sample index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct SampleIndex(u64);

impl SampleIndex {
    /// Validates a one-based index.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::ZeroSampleIndex`] when `index` is zero.
    pub fn new(index: u64) -> Result<Self, CoreError> {
        if index == 0 {
            return Err(CoreError::ZeroSampleIndex);
        }
        Ok(Self(index))
    }

    /// Numeric index.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Next index, if it fits in `u64`.
    #[must_use]
    pub fn checked_next(self) -> Option<Self> {
        self.0.checked_add(1).and_then(|n| Self::new(n).ok())
    }
}

impl TryFrom<u64> for SampleIndex {
    type Error = CoreError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SampleIndex> for u64 {
    fn from(value: SampleIndex) -> Self {
        value.0
    }
}

/// Zero-based byte offset in a `.bin` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ByteOffset(u64);

impl ByteOffset {
    /// Constructs an offset.
    #[must_use]
    pub const fn new(offset: u64) -> Self {
        Self(offset)
    }

    /// Numeric offset.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Byte length of one recorded sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ByteLength(u32);

impl ByteLength {
    /// Constructs a length from validated sample bits.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::SizeOverflow`] if `bits.bytes()` overflows `u32`.
    pub fn from_sample_bits(bits: SampleBits) -> Result<Self, CoreError> {
        let bytes = bits.bytes()?;
        let value = u32::try_from(bytes).map_err(|_| CoreError::SizeOverflow {
            value: bytes as u64,
        })?;
        Ok(Self(value))
    }

    /// Numeric length.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// UTC timestamp captured after a complete source read, or estimated for
/// legacy BIN-only import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UtcTimestamp(#[serde(with = "time::serde::rfc3339")] OffsetDateTime);

impl UtcTimestamp {
    /// Wraps an offset datetime, converting it to UTC.
    #[must_use]
    pub fn new(datetime: OffsetDateTime) -> Self {
        Self(datetime.to_offset(UtcOffset::UTC))
    }

    /// Current UTC time.
    #[must_use]
    pub fn now() -> Self {
        Self::new(OffsetDateTime::now_utc())
    }

    /// Inner UTC datetime.
    #[must_use]
    pub fn inner(self) -> OffsetDateTime {
        self.0
    }
}

impl From<OffsetDateTime> for UtcTimestamp {
    fn from(value: OffsetDateTime) -> Self {
        Self::new(value)
    }
}

/// How a sample timestamp was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampProvenance {
    /// Timestamp was recorded at capture time.
    Recorded,
    /// Timestamp was estimated from a filename start plus interval.
    Estimated,
}

/// Native or imported session status as exposed to readers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// The session is still recording.
    Recording,
    /// The session completed after cancellation or a clean stop.
    Completed,
    /// The session ended in a terminal failure.
    Failed,
    /// A native bundle was left in `recording` after a crash.
    Interrupted,
}

/// Common sample fields shared by native CSV rows and normalized readers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleRecord {
    /// One-based sample index.
    pub index: SampleIndex,
    /// Capture or estimated timestamp.
    pub timestamp: UtcTimestamp,
    /// Whether [`Self::timestamp`] was recorded or estimated.
    pub provenance: TimestampProvenance,
    /// Monotonic elapsed time since session start, when known.
    pub elapsed: Option<Duration>,
    /// Source-read duration, when known.
    pub acquisition: Option<Duration>,
    /// One-bit count of the complete sample.
    pub ones: u64,
    /// Zero-based byte offset, when a BIN file is present.
    pub byte_offset: Option<ByteOffset>,
    /// Declared byte length, when a BIN file is present.
    pub byte_length: Option<ByteLength>,
}

/// Session identity used by recording, engine, and reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionIdentity {
    /// Stable source identifier.
    pub source_id: SourceId,
    /// Safe source metadata.
    pub descriptor: SourceDescriptor,
    /// Sample size.
    pub sample_bits: SampleBits,
    /// Collection interval.
    pub interval: IntervalSeconds,
    /// BitBabbler fold, when applicable.
    pub fold: Option<Fold>,
}

/// Converts a duration to whole milliseconds, saturating at `u64::MAX`.
#[must_use]
pub fn duration_as_millis(duration: Duration) -> u64 {
    let millis = duration.as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

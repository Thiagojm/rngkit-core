#![doc = include_str!("../README.md")]

pub mod error;
pub mod sample;
pub mod session;
pub mod source;

pub use error::{CoreError, SourceError, SourceErrorKind};
pub use sample::{SampleBits, count_ones};
pub use session::{
    ByteLength, ByteOffset, IntervalSeconds, SampleIndex, SampleRecord, SessionIdentity,
    SessionStatus, TimestampProvenance, UtcTimestamp, duration_as_millis,
};
pub use source::{
    EntropySource, Fold, SOURCE_ID_BITB, SOURCE_ID_PSEUDO, SOURCE_ID_RDSEED, SOURCE_ID_TRNG,
    SourceDescriptor, SourceId,
};

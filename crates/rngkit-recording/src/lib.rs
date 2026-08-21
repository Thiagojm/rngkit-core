#![doc = include_str!("../README.md")]

pub mod consistency;
pub mod error;
pub mod legacy_v3;
pub mod manifest;
pub mod naming;
pub mod native;
pub mod normalized;

mod fsutil;

pub use consistency::{ConsistencyReport, ConsistencyWarning};
pub use error::RecordingError;
pub use legacy_v3::open_legacy;
pub use manifest::{Manifest, ManifestStatus, SCHEMA_VERSION};
pub use naming::{SessionStem, now_local};
pub use native::{FailPoint, NATIVE_CSV_COLUMNS, NativeCsvRow, NativeSession, SessionWriter};
pub use normalized::{NormalizedMeta, NormalizedSession, SessionOrigin};

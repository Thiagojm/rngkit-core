#![doc = include_str!("../README.md")]

pub mod concatenation;
pub mod consistency;
pub mod error;
pub mod legacy_v3;
pub mod manifest;
pub mod naming;
pub mod native;
pub mod normalized;

mod fsutil;

pub use concatenation::{
    CONCATENATION_KIND, CONCATENATION_SCHEMA_VERSION, ConcatenationFailPoint,
    ConcatenationInputEntry, ConcatenationManifest, ConcatenationPreview, ConcatenationStem,
    ContentSha256, DERIVED_CSV_COLUMNS, create_legacy_csv_concatenation,
    create_legacy_csv_concatenation_at, inspect_legacy_csvs, open_concatenation,
    with_concatenation_fail_point, with_concatenation_inspect_hook,
    with_concatenation_promote_hook,
};
pub use consistency::{ConsistencyReport, ConsistencyWarning};
pub use error::{ConcatenationCompatibilityField, RecordingError};
pub use fsutil::join_contained;
pub use legacy_v3::open_legacy;
pub use manifest::{Manifest, ManifestStatus, SCHEMA_VERSION};
pub use naming::{SessionStem, now_local};
pub use native::{FailPoint, NATIVE_CSV_COLUMNS, NativeCsvRow, NativeSession, SessionWriter};
pub use normalized::{NormalizedMeta, NormalizedSession, SessionOrigin};

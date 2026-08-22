//! Derived legacy-v3 CSV concatenation contracts and inspection.

pub mod inspect;
pub mod manifest;
pub mod naming;

pub use inspect::{ConcatenationPreview, inspect_legacy_csvs};
pub use manifest::{
    CONCATENATION_KIND, CONCATENATION_SCHEMA_VERSION, ConcatenationInputEntry,
    ConcatenationManifest, ContentSha256,
};
pub use naming::ConcatenationStem;

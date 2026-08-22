//! Derived legacy-v3 CSV concatenation contracts, inspection, and bundles.

pub mod inspect;
pub mod manifest;
pub mod naming;
pub mod reader;
pub mod writer;

pub use inspect::{ConcatenationPreview, inspect_legacy_csvs};
pub use manifest::{
    CONCATENATION_KIND, CONCATENATION_SCHEMA_VERSION, ConcatenationInputEntry,
    ConcatenationManifest, ContentSha256,
};
pub use naming::ConcatenationStem;
pub use reader::open_concatenation;
pub use writer::{
    ConcatenationFailPoint, DERIVED_CSV_COLUMNS, create_legacy_csv_concatenation,
    create_legacy_csv_concatenation_at, with_concatenation_fail_point,
    with_concatenation_inspect_hook, with_concatenation_promote_hook,
};

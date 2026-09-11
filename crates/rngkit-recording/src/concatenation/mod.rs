//! Derived legacy-v3 CSV concatenation contracts, inspection, and bundles.

pub mod inspect;
pub mod manifest;
pub mod naming;
pub mod reader;
pub mod writer;

pub use inspect::{ConcatenationPreview, inspect_csv_inputs, inspect_legacy_csvs};
pub use manifest::{
    CONCATENATION_KIND, CONCATENATION_SCHEMA_VERSION, CSV_CONCATENATION_KIND,
    CSV_CONCATENATION_SCHEMA_VERSION, ConcatenationInputEntry, ConcatenationManifest,
    ContentSha256, MIXED_CSV_CONCATENATION_SCHEMA_VERSION, MIXED_SOURCE_ID, MIXED_SOURCE_LABEL,
};
pub use naming::ConcatenationStem;
pub use reader::open_concatenation;
pub use writer::{
    ConcatenationFailPoint, DERIVED_CSV_COLUMNS, create_csv_concatenation,
    create_csv_concatenation_at, create_legacy_csv_concatenation,
    create_legacy_csv_concatenation_at, with_concatenation_fail_point,
    with_concatenation_inspect_hook, with_concatenation_promote_hook,
};

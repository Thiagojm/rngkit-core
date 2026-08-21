//! Sheet names, labels, and Excel limits.

/// User-visible summary sheet.
pub const SUMMARY_SHEET: &str = "Summary";
/// User-visible samples sheet.
pub const SAMPLES_SHEET: &str = "Samples";
/// Descriptive final Z label.
pub const DESCRIPTIVE_Z_LABEL: &str = "Descriptive final Z";
/// Positive visual reference series name.
pub const REF_PLUS: &str = "Reference +1.96";
/// Negative visual reference series name.
pub const REF_MINUS: &str = "Reference -1.96";
/// Zero reference series name.
pub const REF_ZERO: &str = "Zero";
/// Excel worksheet maximum row count, including the header.
pub const EXCEL_MAX_ROWS: u64 = 1_048_576;
/// Maximum sample rows (header occupies row 1).
pub const EXCEL_MAX_SAMPLE_ROWS: u64 = EXCEL_MAX_ROWS - 1;

/// Summary field labels in display order.
pub const SUMMARY_FIELDS: [&str; 17] = [
    "Source",
    "Variant",
    "Fold",
    "Sample bits",
    "Interval seconds",
    "Started at UTC",
    "Completed at UTC",
    "Duration",
    "Status",
    "Committed samples",
    "Total bits",
    "Total ones",
    "Observed proportion",
    "Deviation from 0.5",
    DESCRIPTIVE_Z_LABEL,
    "Timing overruns",
    "Timestamp provenance",
];

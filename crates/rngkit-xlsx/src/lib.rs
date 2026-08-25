#![doc = include_str!("../README.md")]

pub mod error;
pub mod layout;
pub mod report;

pub use error::XlsxError;
pub use layout::{
    EXCEL_MAX_SAMPLE_ROWS, REF_MINUS, REF_PLUS, REF_ZERO, SAMPLES_SHEET, SUMMARY_SHEET,
};
pub use report::{
    ChartXAxisMode, Overwrite, ReportOptions, ReportPresentation, derived_report_path,
    legacy_report_path, native_report_path, with_report_promote_hook, with_workbook_write_failure,
    write_report, write_report_with_options,
};

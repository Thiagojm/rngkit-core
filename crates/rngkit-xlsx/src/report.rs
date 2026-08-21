//! Workbook generation.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rngkit_analysis::{Snapshot, analyze_records};
use rngkit_core::TimestampProvenance;
use rngkit_recording::NormalizedSession;
use rust_xlsxwriter::{
    Chart, ChartLine, ChartLineDashType, ChartType, Format, Workbook, Worksheet,
    XlsxError as BookError,
};
use time::format_description::well_known::Rfc3339;

use crate::error::XlsxError;
use crate::layout::{
    EXCEL_MAX_SAMPLE_ROWS, REF_MINUS, REF_PLUS, REF_ZERO, SAMPLES_SHEET, SUMMARY_FIELDS,
    SUMMARY_SHEET,
};

/// Whether to replace an existing report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overwrite {
    /// Fail if the destination exists.
    ErrorIfExists,
    /// Replace the destination after a successful workbook close.
    Replace,
}

/// Output path for a native session: `<dir>/<stem>.xlsx`.
#[must_use]
pub fn native_report_path(session_dir: &Path, stem: &str) -> PathBuf {
    session_dir.join(format!("{stem}.xlsx"))
}

/// Output path for a legacy input: sibling of the selected file.
#[must_use]
pub fn legacy_report_path(selected: &Path) -> PathBuf {
    let stem = selected
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("report");
    match selected.parent() {
        Some(parent) => parent.join(format!("{stem}.xlsx")),
        None => PathBuf::from(format!("{stem}.xlsx")),
    }
}

/// Writes a two-sheet analysis workbook from a normalized session.
///
/// # Errors
///
/// Fails on row-limit, existing output without overwrite, analysis errors, and
/// write failures. A failure leaves no partial final workbook.
pub fn write_report(
    session: &NormalizedSession,
    dest: &Path,
    overwrite: Overwrite,
) -> Result<PathBuf, XlsxError> {
    let count = u64::try_from(session.len()).unwrap_or(u64::MAX);
    if count > EXCEL_MAX_SAMPLE_ROWS {
        return Err(XlsxError::RowLimit {
            count,
            limit: EXCEL_MAX_SAMPLE_ROWS,
        });
    }
    if dest.exists() && overwrite == Overwrite::ErrorIfExists {
        return Err(XlsxError::AlreadyExists {
            path: dest.to_path_buf(),
        });
    }
    let snapshots = analyze_records(
        session.meta().sample_bits,
        session.records().iter().cloned(),
    )?;
    let tmp = sibling_temp(dest);
    let result = write_workbook(session, &snapshots, &tmp);
    match result {
        Ok(()) => {
            replace_file(&tmp, dest)?;
            Ok(dest.to_path_buf())
        }
        Err(err) => {
            let _ = fs::remove_file(&tmp);
            Err(err)
        }
    }
}

fn write_workbook(
    session: &NormalizedSession,
    snapshots: &[Snapshot],
    path: &Path,
) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    write_summary(workbook.add_worksheet(), session, snapshots)?;
    {
        let samples = workbook.add_worksheet();
        write_samples(samples, session, snapshots)?;
        if !snapshots.is_empty() {
            let last = u32::try_from(snapshots.len()).map_err(|_| XlsxError::RowLimit {
                count: snapshots.len() as u64,
                limit: EXCEL_MAX_SAMPLE_ROWS,
            })?;
            let chart = build_chart(last)?;
            samples.insert_chart(0, 12, &chart).map_err(map_book)?;
        }
    }
    workbook.save(path).map_err(map_book)?;
    Ok(())
}

fn write_summary(
    worksheet: &mut Worksheet,
    session: &NormalizedSession,
    snapshots: &[Snapshot],
) -> Result<(), XlsxError> {
    worksheet.set_name(SUMMARY_SHEET).map_err(map_book)?;
    let bold = Format::new().set_bold();
    worksheet
        .write_with_format(0, 0, "Field", &bold)
        .map_err(map_book)?;
    worksheet
        .write_with_format(0, 1, "Value", &bold)
        .map_err(map_book)?;
    let meta = session.meta();
    let last = snapshots.last();
    let values: Vec<String> = vec![
        meta.source_id.as_str().to_owned(),
        meta.source_variant.clone().unwrap_or_default(),
        meta.fold.map(|f| f.get().to_string()).unwrap_or_default(),
        meta.sample_bits.get().to_string(),
        meta.interval.get().to_string(),
        fmt_ts(meta.started_at),
        fmt_ts(meta.completed_at),
        duration_label(meta.started_at, meta.completed_at),
        format!("{:?}", meta.status).to_ascii_lowercase(),
        snapshots.len().to_string(),
        last.map(|s| s.total_bits.to_string())
            .unwrap_or_else(|| "0".into()),
        last.map(|s| s.total_ones.to_string())
            .unwrap_or_else(|| "0".into()),
        last.map(|s| format!("{:.10}", s.proportion))
            .unwrap_or_else(|| "0".into()),
        last.map(|s| format!("{:.10}", s.deviation))
            .unwrap_or_else(|| "0".into()),
        last.map(|s| format!("{:.10}", s.z))
            .unwrap_or_else(|| "0".into()),
        meta.overrun_count
            .map(|n| n.to_string())
            .unwrap_or_default(),
        provenance_label(meta.provenance).to_owned(),
    ];
    for (i, (field, value)) in SUMMARY_FIELDS.iter().zip(values.iter()).enumerate() {
        let row = u32::try_from(i + 1).map_err(|_| XlsxError::RowLimit {
            count: i as u64,
            limit: EXCEL_MAX_SAMPLE_ROWS,
        })?;
        worksheet.write(row, 0, *field).map_err(map_book)?;
        worksheet.write(row, 1, value.as_str()).map_err(map_book)?;
    }
    Ok(())
}

fn write_samples(
    worksheet: &mut Worksheet,
    session: &NormalizedSession,
    snapshots: &[Snapshot],
) -> Result<(), XlsxError> {
    worksheet.set_name(SAMPLES_SHEET).map_err(map_book)?;
    let headers = [
        "sample_index",
        "timestamp",
        "elapsed_ms",
        "acquisition_ms",
        "ones",
        "cumulative_ones",
        "cumulative_proportion",
        "cumulative_z",
        REF_ZERO,
        REF_PLUS,
        REF_MINUS,
    ];
    for (col, header) in headers.iter().enumerate() {
        worksheet
            .write(0, u16::try_from(col).expect("col"), *header)
            .map_err(map_book)?;
    }
    for (i, (record, snap)) in session.records().iter().zip(snapshots.iter()).enumerate() {
        let row = u32::try_from(i + 1).map_err(|_| XlsxError::RowLimit {
            count: i as u64,
            limit: EXCEL_MAX_SAMPLE_ROWS,
        })?;
        worksheet
            .write_number(row, 0, record.index.get() as f64)
            .map_err(map_book)?;
        worksheet
            .write_string(row, 1, fmt_ts(Some(record.timestamp)))
            .map_err(map_book)?;
        write_opt_ms(worksheet, row, 2, record.elapsed)?;
        write_opt_ms(worksheet, row, 3, record.acquisition)?;
        worksheet
            .write_number(row, 4, record.ones as f64)
            .map_err(map_book)?;
        worksheet
            .write_number(row, 5, snap.total_ones as f64)
            .map_err(map_book)?;
        worksheet
            .write_number(row, 6, snap.proportion)
            .map_err(map_book)?;
        worksheet.write_number(row, 7, snap.z).map_err(map_book)?;
        worksheet.write_number(row, 8, 0.0).map_err(map_book)?;
        worksheet.write_number(row, 9, 1.96).map_err(map_book)?;
        worksheet.write_number(row, 10, -1.96).map_err(map_book)?;
    }
    for col in 8..=10 {
        worksheet.set_column_hidden(col).map_err(map_book)?;
    }
    Ok(())
}

fn build_chart(n: u32) -> Result<Chart, XlsxError> {
    let last = n;
    let mut chart = Chart::new(ChartType::Line);
    chart.set_name("Cumulative signed Z");
    chart.title().set_name("Cumulative signed Z");
    chart
        .add_series()
        .set_name("Cumulative Z")
        .set_categories((SAMPLES_SHEET, 1, 0, last, 0))
        .set_values((SAMPLES_SHEET, 1, 7, last, 7));
    chart
        .add_series()
        .set_name(REF_ZERO)
        .set_categories((SAMPLES_SHEET, 1, 0, last, 0))
        .set_values((SAMPLES_SHEET, 1, 8, last, 8));
    chart
        .add_series()
        .set_name(REF_PLUS)
        .set_categories((SAMPLES_SHEET, 1, 0, last, 0))
        .set_values((SAMPLES_SHEET, 1, 9, last, 9))
        .set_format(ChartLine::new().set_dash_type(ChartLineDashType::Dash));
    chart
        .add_series()
        .set_name(REF_MINUS)
        .set_categories((SAMPLES_SHEET, 1, 0, last, 0))
        .set_values((SAMPLES_SHEET, 1, 10, last, 10))
        .set_format(ChartLine::new().set_dash_type(ChartLineDashType::Dash));
    chart.show_hidden_data();
    let _ = last;
    Ok(chart)
}

fn write_opt_ms(
    worksheet: &mut Worksheet,
    row: u32,
    col: u16,
    duration: Option<std::time::Duration>,
) -> Result<(), XlsxError> {
    match duration {
        Some(duration) => worksheet
            .write_number(row, col, rngkit_core::duration_as_millis(duration) as f64)
            .map_err(map_book)?,
        None => worksheet.write_string(row, col, "").map_err(map_book)?,
    };
    Ok(())
}

fn fmt_ts(ts: Option<rngkit_core::UtcTimestamp>) -> String {
    ts.and_then(|t| t.inner().format(&Rfc3339).ok())
        .unwrap_or_default()
}

fn duration_label(
    start: Option<rngkit_core::UtcTimestamp>,
    end: Option<rngkit_core::UtcTimestamp>,
) -> String {
    match (start, end) {
        (Some(start), Some(end)) => {
            let delta = end.inner() - start.inner();
            format!("{}s", delta.whole_seconds())
        }
        _ => String::new(),
    }
}

fn provenance_label(p: TimestampProvenance) -> &'static str {
    match p {
        TimestampProvenance::Recorded => "recorded",
        TimestampProvenance::Estimated => "estimated",
    }
}

fn map_book(err: BookError) -> XlsxError {
    XlsxError::Workbook(err.to_string())
}

fn sibling_temp(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map(|n| format!(".{}.tmp", n.to_string_lossy()))
        .unwrap_or_else(|| ".report.tmp".into());
    match dest.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

fn replace_file(tmp: &Path, dest: &Path) -> io::Result<()> {
    match fs::rename(tmp, dest) {
        Ok(()) => Ok(()),
        Err(_) if dest.exists() => replace_existing(tmp, dest),
        Err(err) => Err(err),
    }
}

#[cfg(windows)]
fn replace_existing(tmp: &Path, dest: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    unsafe extern "system" {
        fn ReplaceFileW(
            replaced: *const u16,
            replacement: *const u16,
            backup: *const u16,
            flags: u32,
            exclude: *mut core::ffi::c_void,
            reserved: *mut core::ffi::c_void,
        ) -> i32;
    }

    let dest_w = wide(dest);
    let tmp_w = wide(tmp);
    // SAFETY: NUL-terminated UTF-16 paths; ReplaceFileW does not retain pointers.
    let ok = unsafe {
        ReplaceFileW(
            dest_w.as_ptr(),
            tmp_w.as_ptr(),
            ptr::null(),
            0,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_existing(tmp: &Path, dest: &Path) -> io::Result<()> {
    fs::rename(tmp, dest)
}

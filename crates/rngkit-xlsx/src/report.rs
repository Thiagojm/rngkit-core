//! Workbook generation.

use std::cell::{Cell, RefCell};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rngkit_analysis::{Snapshot, analyze_records};
use rngkit_core::TimestampProvenance;
use rngkit_recording::{ConcatenationStem, NormalizedSession, SessionStem, join_contained};
use rust_xlsxwriter::{
    Chart, ChartFormat, ChartLegendPosition, ChartLine, ChartLineDashType, ChartSolidFill,
    ChartType, Format, Workbook, Worksheet, XlsxError as BookError,
};
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;

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

/// The chart category presentation used for the sample series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartXAxisMode {
    /// Use recorded sample timestamps formatted as clock labels.
    RecordedTimestamp,
    /// Use one-based sample indexes.
    SampleIndex,
}

/// Explicit source and chart presentation context for an XLSX report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportOptions {
    source_basename: String,
    x_axis_mode: ChartXAxisMode,
}

impl ReportOptions {
    /// Creates validated report presentation options.
    pub fn new(
        source_basename: impl Into<String>,
        x_axis_mode: ChartXAxisMode,
    ) -> Result<Self, XlsxError> {
        let source_basename = source_basename.into();
        let valid = !source_basename.is_empty()
            && source_basename != "."
            && source_basename != ".."
            && !source_basename.contains('/')
            && !source_basename.contains('\\')
            && !source_basename.chars().any(char::is_control)
            && matches!(
                Path::new(&source_basename)
                    .extension()
                    .and_then(|extension| extension.to_str()),
                Some("csv" | "bin")
            );
        if !valid {
            return Err(XlsxError::InvalidSourceBasename {
                basename: source_basename,
            });
        }
        Ok(Self {
            source_basename,
            x_axis_mode,
        })
    }

    /// Creates compatible defaults from normalized metadata.
    pub fn for_session(session: &NormalizedSession) -> Result<Self, XlsxError> {
        let extension = match session.meta().provenance {
            TimestampProvenance::Recorded => "csv",
            TimestampProvenance::Estimated => "bin",
        };
        let mode = match session.meta().provenance {
            TimestampProvenance::Recorded => ChartXAxisMode::RecordedTimestamp,
            TimestampProvenance::Estimated => ChartXAxisMode::SampleIndex,
        };
        Self::new(format!("{}.{}", session.meta().stem, extension), mode)
    }

    /// Safe source artifact basename.
    #[must_use]
    pub fn source_basename(&self) -> &str {
        &self.source_basename
    }

    /// Chart category mode.
    #[must_use]
    pub fn x_axis_mode(&self) -> ChartXAxisMode {
        self.x_axis_mode
    }
}

/// Alias emphasizing that these options control workbook presentation.
pub type ReportPresentation = ReportOptions;

/// Output path for a native session: `<dir>/<stem>.xlsx`.
///
/// `stem` must already be a validated [`SessionStem`]. The resolved path is
/// required to stay inside `session_dir`.
///
/// # Errors
///
/// Returns [`XlsxError::Recording`] when the filename would escape
/// `session_dir` or `session_dir` cannot be canonicalized.
pub fn native_report_path(session_dir: &Path, stem: &SessionStem) -> Result<PathBuf, XlsxError> {
    let name = format!("{}.xlsx", stem.as_str());
    Ok(join_contained(session_dir, &name)?)
}

/// Output path for a derived concatenation bundle: `<dir>/<stem>.xlsx`.
///
/// `stem` must already be a validated [`ConcatenationStem`]. The resolved path
/// is required to stay inside `bundle_dir`.
///
/// # Errors
///
/// Returns [`XlsxError::Recording`] when the filename would escape
/// `bundle_dir` or `bundle_dir` cannot be canonicalized.
pub fn derived_report_path(
    bundle_dir: &Path,
    stem: &ConcatenationStem,
) -> Result<PathBuf, XlsxError> {
    let name = format!("{}.xlsx", stem.as_str());
    Ok(join_contained(bundle_dir, &name)?)
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
/// The workbook is written to a uniquely owned temporary file created with
/// create-new semantics in the destination directory, then promoted. A
/// pre-existing temporary name is never followed, truncated, or reused.
/// [`Overwrite::ErrorIfExists`] refuses to replace a destination that exists
/// at promotion time, including one created concurrently.
///
/// # Errors
///
/// Fails on row-limit, existing output without overwrite, analysis errors, and
/// write failures. A failure leaves no partial final workbook and does not
/// modify source session files.
pub fn write_report(
    session: &NormalizedSession,
    dest: &Path,
    overwrite: Overwrite,
) -> Result<PathBuf, XlsxError> {
    let options = ReportOptions::for_session(session)?;
    write_report_with_options(session, dest, overwrite, &options)
}

/// Writes a report with explicit source filename and chart-axis context.
pub fn write_report_with_options(
    session: &NormalizedSession,
    dest: &Path,
    overwrite: Overwrite,
    options: &ReportOptions,
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
    let parent = dest
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let (tmp_path, mut file) = create_owned_temp(parent)?;
    let write_result = if FAIL_WORKBOOK.with(|flag| flag.get()) {
        Err(XlsxError::Workbook("injected workbook failure".into()))
    } else {
        write_workbook(session, &snapshots, options, &mut file).and_then(|()| {
            file.flush()?;
            file.sync_all()?;
            Ok(())
        })
    };
    drop(file);
    if let Err(err) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }
    BEFORE_PROMOTE.with(|hook| {
        if let Some(hook) = hook.borrow().as_ref() {
            hook(&tmp_path);
        }
    });
    let promoted = match overwrite {
        Overwrite::ErrorIfExists => promote_noclobber(&tmp_path, dest),
        Overwrite::Replace => replace_file(&tmp_path, dest),
    };
    match promoted {
        Ok(()) => Ok(dest.to_path_buf()),
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            if overwrite == Overwrite::ErrorIfExists
                && (err.kind() == io::ErrorKind::AlreadyExists || dest.exists())
            {
                Err(XlsxError::AlreadyExists {
                    path: dest.to_path_buf(),
                })
            } else {
                Err(err.into())
            }
        }
    }
}

fn write_workbook(
    session: &NormalizedSession,
    snapshots: &[Snapshot],
    options: &ReportOptions,
    writer: &mut File,
) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    write_summary(workbook.add_worksheet(), session, snapshots)?;
    {
        let samples = workbook.add_worksheet();
        write_samples(samples, session, snapshots, options)?;
        if !snapshots.is_empty() {
            let last = u32::try_from(snapshots.len()).map_err(|_| XlsxError::RowLimit {
                count: snapshots.len() as u64,
                limit: EXCEL_MAX_SAMPLE_ROWS,
            })?;
            let chart = build_chart(last, session, options)?;
            samples.insert_chart(0, 13, &chart).map_err(map_book)?;
        }
    }
    workbook.save_to_writer(writer).map_err(map_book)?;
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
    options: &ReportOptions,
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
        "chart_category",
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
        match options.x_axis_mode() {
            ChartXAxisMode::RecordedTimestamp => worksheet
                .write_string(row, 11, fmt_clock(record.timestamp))
                .map_err(map_book)?,
            ChartXAxisMode::SampleIndex => worksheet
                .write_number(row, 11, record.index.get() as f64)
                .map_err(map_book)?,
        };
    }
    for col in 8..=11 {
        worksheet.set_column_hidden(col).map_err(map_book)?;
    }
    Ok(())
}

fn build_chart(
    n: u32,
    session: &NormalizedSession,
    options: &ReportOptions,
) -> Result<Chart, XlsxError> {
    let last = n;
    let category_col = match options.x_axis_mode() {
        ChartXAxisMode::RecordedTimestamp => 11,
        ChartXAxisMode::SampleIndex => 0,
    };
    let mut chart = Chart::new(ChartType::Line);
    chart
        .set_name("Cumulative signed Z")
        .set_width(720)
        .set_height(420);
    let title = format!("Z-Score Analysis — {}", options.source_basename());
    chart.title().set_name(&title);
    let x_axis_title = match options.x_axis_mode() {
        ChartXAxisMode::RecordedTimestamp => format!(
            "Sample time — configured interval: {} s",
            session.meta().interval.get()
        ),
        ChartXAxisMode::SampleIndex => "Sample number".to_owned(),
    };
    chart.x_axis().set_name(&x_axis_title);
    let y_axis_title = format!(
        "Cumulative signed Z — sample size: {} bits",
        session.meta().sample_bits.get()
    );
    chart.y_axis().set_name(&y_axis_title);
    chart.x_axis().set_major_gridlines(false);
    chart
        .y_axis()
        .set_major_gridlines_line(ChartLine::new().set_color("#D1D5DB").set_width(0.75));
    chart
        .chart_area()
        .set_format(ChartFormat::new().set_solid_fill(ChartSolidFill::new().set_color("#FFFFFF")));
    chart
        .plot_area()
        .set_format(ChartFormat::new().set_solid_fill(ChartSolidFill::new().set_color("#F8FAFC")));
    chart.legend().set_position(ChartLegendPosition::Bottom);
    chart
        .add_series()
        .set_name("Cumulative Z")
        .set_categories((SAMPLES_SHEET, 1, category_col, last, category_col))
        .set_values((SAMPLES_SHEET, 1, 7, last, 7))
        .set_format(
            ChartFormat::new().set_line(ChartLine::new().set_color("#2563EB").set_width(2.25)),
        );
    chart
        .add_series()
        .set_name(REF_ZERO)
        .set_categories((SAMPLES_SHEET, 1, category_col, last, category_col))
        .set_values((SAMPLES_SHEET, 1, 8, last, 8))
        .set_format(ChartFormat::new().set_line(ChartLine::new().set_color("#94A3B8")));
    chart
        .add_series()
        .set_name(REF_PLUS)
        .set_categories((SAMPLES_SHEET, 1, category_col, last, category_col))
        .set_values((SAMPLES_SHEET, 1, 9, last, 9))
        .set_format(
            ChartFormat::new().set_line(
                ChartLine::new()
                    .set_color("#94A3B8")
                    .set_width(1.0)
                    .set_dash_type(ChartLineDashType::Dash),
            ),
        );
    chart
        .add_series()
        .set_name(REF_MINUS)
        .set_categories((SAMPLES_SHEET, 1, category_col, last, category_col))
        .set_values((SAMPLES_SHEET, 1, 10, last, 10))
        .set_format(
            ChartFormat::new().set_line(
                ChartLine::new()
                    .set_color("#94A3B8")
                    .set_width(1.0)
                    .set_dash_type(ChartLineDashType::Dash),
            ),
        );
    chart.show_hidden_data();
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

fn fmt_clock(ts: rngkit_core::UtcTimestamp) -> String {
    ts.inner()
        .format(&format_description!("[hour]:[minute]:[second]"))
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

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

type PromoteHook = Box<dyn Fn(&Path)>;

thread_local! {
    static FAIL_WORKBOOK: Cell<bool> = const { Cell::new(false) };
    static BEFORE_PROMOTE: RefCell<Option<PromoteHook>> = RefCell::new(None);
}

/// Runs `body` so workbook generation fails after the owned temporary file is
/// created. The temporary artifact is removed and `dest` is left untouched.
#[doc(hidden)]
pub fn with_workbook_write_failure<R>(body: impl FnOnce() -> R) -> R {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            FAIL_WORKBOOK.with(|flag| flag.set(false));
        }
    }
    FAIL_WORKBOOK.with(|flag| flag.set(true));
    let _reset = Reset;
    body()
}

/// Runs `body` and invokes `hook` with the owned temporary path after the
/// workbook is durable and before promotion.
#[doc(hidden)]
pub fn with_report_promote_hook<R>(hook: impl Fn(&Path) + 'static, body: impl FnOnce() -> R) -> R {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            BEFORE_PROMOTE.with(|cell| *cell.borrow_mut() = None);
        }
    }
    BEFORE_PROMOTE.with(|cell| *cell.borrow_mut() = Some(Box::new(hook)));
    let _reset = Reset;
    body()
}

fn create_owned_temp(dir: &Path) -> io::Result<(PathBuf, File)> {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for _ in 0..1024u32 {
        let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!(".rngkit-xlsx-{pid}-{nanos}-{n}.tmp");
        let path = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create a unique xlsx temporary file",
    ))
}

fn promote_noclobber(tmp: &Path, dest: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::hard_link(tmp, dest)?;
        fs::remove_file(tmp)?;
        Ok(())
    }
    #[cfg(windows)]
    {
        promote_noclobber_windows(tmp, dest)
    }
    #[cfg(not(any(unix, windows)))]
    {
        if dest.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "xlsx destination exists",
            ));
        }
        fs::rename(tmp, dest)
    }
}

#[cfg(windows)]
fn promote_noclobber_windows(tmp: &Path, dest: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    let tmp_w = wide(tmp);
    let dest_w = wide(dest);
    // SAFETY: both paths are NUL-terminated UTF-16; MoveFileExW does not
    // retain the pointers. Flags omit MOVEFILE_REPLACE_EXISTING so a
    // destination created concurrently is left in place.
    let ok = unsafe { MoveFileExW(tmp_w.as_ptr(), dest_w.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
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

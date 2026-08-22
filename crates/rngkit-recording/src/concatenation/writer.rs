//! Derived concatenation bundle creation.

use std::cell::{Cell, RefCell};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rngkit_core::UtcTimestamp;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use super::ConcatenationPreview;
use crate::concatenation::inspect::{for_each_legacy_csv_row, inspect_legacy_csvs_ordered};
use crate::concatenation::manifest::ConcatenationManifest;
use crate::concatenation::naming::ConcatenationStem;
use crate::error::RecordingError;
use crate::fsutil::{join_contained, validate_contained_name};
use crate::naming::now_local;

/// Derived concatenation CSV columns in order.
pub const DERIVED_CSV_COLUMNS: [&str; 5] = [
    "sample_index",
    "captured_at_utc",
    "ones",
    "input_index",
    "input_sample_index",
];

/// One derived concatenation CSV row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ConcatenationCsvRow {
    pub sample_index: u64,
    pub captured_at_utc: String,
    pub ones: u64,
    pub input_index: u64,
    pub input_sample_index: u64,
}

/// Optional write-boundary failure injection for tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcatenationFailPoint {
    /// Fail after revalidation, before staging is created.
    AfterInspect,
    /// Fail after CSV rows are written, before CSV sync.
    AfterCsvWrite,
    /// Fail after CSV sync, before the manifest is written.
    AfterCsvSync,
    /// Fail after the manifest is written, before manifest sync.
    AfterManifestWrite,
    /// Fail after manifest sync, before promotion.
    AfterManifestSync,
}

type InspectHook = Box<dyn Fn()>;
type PromoteHook = Box<dyn Fn(&Path)>;

thread_local! {
    static FAIL_POINT: Cell<Option<ConcatenationFailPoint>> = const { Cell::new(None) };
    static INSPECT_HOOK: RefCell<Option<InspectHook>> = const { RefCell::new(None) };
    static PROMOTE_HOOK: RefCell<Option<PromoteHook>> = const { RefCell::new(None) };
}

static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Creates a derived concatenation bundle from legacy v3 CSV inputs.
///
/// Inputs are reopened and completely revalidated. Rows are streamed into a
/// unique staging directory, the CSV and manifest are synced, and the staging
/// directory is promoted without replacing an existing final path. Inputs are
/// never modified.
///
/// # Errors
///
/// Returns a typed [`RecordingError`] when inspection fails, an input changes
/// after inspection, the destination exists, or writing/promotion fails. A
/// failure leaves no partial final bundle.
pub fn create_legacy_csv_concatenation(
    paths: &[PathBuf],
    output_root: &Path,
) -> Result<PathBuf, RecordingError> {
    let (local, offset) = now_local()?;
    create_legacy_csv_concatenation_at(paths, output_root, local, offset)
}

/// Clock-injectable creation used by tests.
///
/// # Errors
///
/// Same as [`create_legacy_csv_concatenation`].
#[doc(hidden)]
pub fn create_legacy_csv_concatenation_at(
    paths: &[PathBuf],
    output_root: &Path,
    local_created: OffsetDateTime,
    local_offset: UtcOffset,
) -> Result<PathBuf, RecordingError> {
    let inspected = inspect_legacy_csvs_ordered(paths)?;
    INSPECT_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow().as_ref() {
            hook();
        }
    });
    check_fail(ConcatenationFailPoint::AfterInspect)?;

    let preview = inspected.preview;
    let ordered_paths = inspected.paths;
    let stem = ConcatenationStem::new(
        local_created,
        preview.source_id().clone(),
        preview.sample_bits(),
        preview.interval(),
        preview.fold(),
    )?;
    if !output_root.exists() {
        fs::create_dir_all(output_root)?;
    }
    let dest = join_contained(output_root, stem.as_str())?;
    if dest.exists() {
        return Err(RecordingError::AlreadyExists { path: dest });
    }

    let staging = create_staging_dir(output_root)?;
    let mut guard = StagingGuard {
        path: Some(staging.clone()),
    };
    let write_result = write_staging(
        &staging,
        &stem,
        &preview,
        &ordered_paths,
        UtcTimestamp::new(local_created),
        local_offset,
        &dest,
    );
    match write_result {
        Ok(()) => {
            guard.disarm();
            Ok(dest)
        }
        Err(err) => Err(err),
    }
}

fn write_staging(
    staging: &Path,
    stem: &ConcatenationStem,
    preview: &ConcatenationPreview,
    ordered_paths: &[PathBuf],
    created_at: UtcTimestamp,
    local_offset: UtcOffset,
    dest: &Path,
) -> Result<(), RecordingError> {
    write_derived_csv(staging, stem, preview, ordered_paths)?;
    write_manifest(staging, stem, preview, created_at, local_offset)?;
    promote_dir(staging, dest)
}

fn write_derived_csv(
    staging: &Path,
    stem: &ConcatenationStem,
    preview: &ConcatenationPreview,
    ordered_paths: &[PathBuf],
) -> Result<(), RecordingError> {
    let csv_path = staging.join(stem.csv_basename());
    let csv_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&csv_path)?;
    let mut csv = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(csv_file);
    csv.write_record(DERIVED_CSV_COLUMNS)?;

    let mut output_index = 1u64;
    let sample_bits = preview.sample_bits();
    for (input_i, (path, entry)) in ordered_paths.iter().zip(preview.inputs()).enumerate() {
        let input_index = u64::try_from(input_i)
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or(RecordingError::ConcatenationCountOverflow)?;
        let mut input_sample_index = 1u64;
        let basename = entry.basename();
        let scan =
            for_each_legacy_csv_row(path, basename, sample_bits, |timestamp, ones| {
                let captured = timestamp.inner().format(&Rfc3339).map_err(|err| {
                    RecordingError::InvalidName {
                        reason: err.to_string(),
                    }
                })?;
                csv.serialize(&ConcatenationCsvRow {
                    sample_index: output_index,
                    captured_at_utc: captured,
                    ones,
                    input_index,
                    input_sample_index,
                })?;
                output_index = output_index
                    .checked_add(1)
                    .ok_or(RecordingError::ConcatenationCountOverflow)?;
                input_sample_index = input_sample_index
                    .checked_add(1)
                    .ok_or(RecordingError::ConcatenationCountOverflow)?;
                Ok(())
            })?;
        if scan.sha256 != *entry.sha256()
            || scan.row_count != entry.row_count()
            || scan.first != entry.first_timestamp()
            || scan.last != entry.last_timestamp()
        {
            return Err(RecordingError::ConcatenationInputChanged {
                basename: basename.to_owned(),
            });
        }
    }

    check_fail(ConcatenationFailPoint::AfterCsvWrite)?;
    csv.flush()?;
    csv.get_ref().sync_all()?;
    check_fail(ConcatenationFailPoint::AfterCsvSync)?;
    Ok(())
}

fn write_manifest(
    staging: &Path,
    stem: &ConcatenationStem,
    preview: &ConcatenationPreview,
    created_at: UtcTimestamp,
    local_offset: UtcOffset,
) -> Result<(), RecordingError> {
    let manifest =
        ConcatenationManifest::new(stem, created_at, local_offset, preview.inputs().to_vec())?;
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    let path = staging.join("manifest.json");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)?;
    file.write_all(&bytes)?;
    check_fail(ConcatenationFailPoint::AfterManifestWrite)?;
    file.flush()?;
    file.sync_all()?;
    check_fail(ConcatenationFailPoint::AfterManifestSync)?;
    Ok(())
}

fn create_staging_dir(root: &Path) -> Result<PathBuf, RecordingError> {
    if !root.exists() {
        fs::create_dir_all(root)?;
    }
    let root_canon = fs::canonicalize(root)?;
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for _ in 0..1024u32 {
        let n = STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!(".rngkit-concat-{pid}-{nanos}-{n}.tmp");
        validate_contained_name(&name, &root_canon)?;
        let path = root_canon.join(&name);
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.into()),
        }
    }
    Err(RecordingError::ConcatenationWrite {
        stage: "staging",
        reason: "could not create a unique concatenation staging directory".into(),
    })
}

fn promote_dir(staging: &Path, dest: &Path) -> Result<(), RecordingError> {
    if dest.exists() {
        return Err(RecordingError::AlreadyExists {
            path: dest.to_path_buf(),
        });
    }
    PROMOTE_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow().as_ref() {
            hook(dest);
        }
    });
    match rename_noclobber(staging, dest) {
        Ok(()) => Ok(()),
        Err(err) => {
            if dest.exists() || err.kind() == io::ErrorKind::AlreadyExists {
                Err(RecordingError::AlreadyExists {
                    path: dest.to_path_buf(),
                })
            } else {
                Err(err.into())
            }
        }
    }
}

fn rename_noclobber(staging: &Path, dest: &Path) -> io::Result<()> {
    #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
        target_os = "redox",
    ))]
    {
        promote_noclobber_unix(staging, dest)
    }
    #[cfg(all(
        unix,
        not(any(
            target_os = "android",
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "watchos",
            target_os = "redox",
        ))
    ))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic no-replace directory promotion is unavailable on this Unix target",
        ))
    }
    #[cfg(windows)]
    {
        promote_noclobber_windows(staging, dest)
    }
    #[cfg(not(any(unix, windows)))]
    {
        if dest.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "concatenation destination exists",
            ));
        }
        fs::rename(staging, dest)
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "visionos",
    target_os = "watchos",
    target_os = "redox",
))]
fn promote_noclobber_unix(staging: &Path, dest: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, staging, CWD, dest, RenameFlags::NOREPLACE).map_err(Into::into)
}

#[cfg(windows)]
fn promote_noclobber_windows(staging: &Path, dest: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    let staging_w = wide(staging);
    let dest_w = wide(dest);
    // SAFETY: both paths are NUL-terminated UTF-16; MoveFileExW does not
    // retain the pointers. Flags omit MOVEFILE_REPLACE_EXISTING so a
    // destination created concurrently is left in place.
    let ok = unsafe { MoveFileExW(staging_w.as_ptr(), dest_w.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

struct StagingGuard {
    path: Option<PathBuf>,
}

impl StagingGuard {
    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_dir_all(&path);
        }
    }
}

fn check_fail(point: ConcatenationFailPoint) -> Result<(), RecordingError> {
    FAIL_POINT.with(|cell| {
        if cell.get() == Some(point) {
            Err(RecordingError::ConcatenationWrite {
                stage: fail_stage(point),
                reason: "injected failure".into(),
            })
        } else {
            Ok(())
        }
    })
}

fn fail_stage(point: ConcatenationFailPoint) -> &'static str {
    match point {
        ConcatenationFailPoint::AfterInspect => "inspect",
        ConcatenationFailPoint::AfterCsvWrite => "csv-write",
        ConcatenationFailPoint::AfterCsvSync => "csv-sync",
        ConcatenationFailPoint::AfterManifestWrite => "manifest-write",
        ConcatenationFailPoint::AfterManifestSync => "manifest-sync",
    }
}

/// Runs `body` so concatenation fails at `point`. Owned staging data is
/// removed and the final destination is left untouched.
#[doc(hidden)]
pub fn with_concatenation_fail_point<R>(
    point: ConcatenationFailPoint,
    body: impl FnOnce() -> R,
) -> R {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            FAIL_POINT.with(|cell| cell.set(None));
        }
    }
    FAIL_POINT.with(|cell| cell.set(Some(point)));
    let _reset = Reset;
    body()
}

/// Runs `hook` after inspection/revalidation and before staging.
#[doc(hidden)]
pub fn with_concatenation_inspect_hook<R>(
    hook: impl Fn() + 'static,
    body: impl FnOnce() -> R,
) -> R {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            INSPECT_HOOK.with(|cell| *cell.borrow_mut() = None);
        }
    }
    INSPECT_HOOK.with(|cell| *cell.borrow_mut() = Some(Box::new(hook)));
    let _reset = Reset;
    body()
}

/// Runs `hook` with the intended final directory after durable CSV/manifest
/// writes and before promotion.
#[doc(hidden)]
pub fn with_concatenation_promote_hook<R>(
    hook: impl Fn(&Path) + 'static,
    body: impl FnOnce() -> R,
) -> R {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            PROMOTE_HOOK.with(|cell| *cell.borrow_mut() = None);
        }
    }
    PROMOTE_HOOK.with(|cell| *cell.borrow_mut() = Some(Box::new(hook)));
    let _reset = Reset;
    body()
}

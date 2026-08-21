//! Path containment and platform-correct file replacement.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::error::RecordingError;

/// Rejects empty names, separators, parent components, prefixes, and `..`.
pub(crate) fn validate_contained_name(name: &str, root: &Path) -> Result<(), RecordingError> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || Path::new(name).components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(RecordingError::PathEscapesRoot {
            root: root.to_path_buf(),
            path: PathBuf::from(name),
        });
    }
    Ok(())
}

/// Joins a validated basename under a canonicalized directory.
///
/// `name` must be a single path segment with no separators, prefixes, or
/// parent components. The resolved path stays within `root`. This is lexical
/// containment for newly created names; open existing artifacts with
/// [`open_contained`] so symbolic links and reparse points cannot escape.
///
/// # Errors
///
/// Returns [`RecordingError::PathEscapesRoot`] when `name` is not a safe
/// basename. Returns I/O errors when `root` cannot be canonicalized.
pub fn join_contained(root: &Path, name: &str) -> Result<PathBuf, RecordingError> {
    validate_contained_name(name, root)?;
    let root_canon = fs::canonicalize(root)?;
    Ok(root_canon.join(name))
}

/// Opens `name` under `root` without following a symbolic link or reparse point.
///
/// The basename is validated, `root` is canonicalized, and the directory entry
/// is opened with no-follow semantics. A link, reparse point, or non-file is
/// rejected so the caller never reads an object outside the session directory.
///
/// # Errors
///
/// Returns [`RecordingError::PathEscapesRoot`] when `name` is unsafe or the
/// entry is a link/reparse point. Returns [`RecordingError::Corrupt`] when the
/// entry exists but is not a regular file.
pub(crate) fn open_contained(root: &Path, name: &str) -> Result<(PathBuf, File), RecordingError> {
    validate_contained_name(name, root)?;
    let root_canon = fs::canonicalize(root)?;
    let path = root_canon.join(name);
    let file = match open_nofollow_read(&path) {
        Ok(file) => file,
        Err(err) if is_symlink_open_error(&err) => {
            return Err(RecordingError::PathEscapesRoot {
                root: root_canon,
                path,
            });
        }
        Err(err) => return Err(err.into()),
    };
    let meta = file.metadata()?;
    if is_reparse_or_symlink(&meta) {
        return Err(RecordingError::PathEscapesRoot {
            root: root_canon,
            path,
        });
    }
    if !meta.is_file() {
        return Err(RecordingError::Corrupt {
            reason: format!("{} is not a regular file", path.display()),
        });
    }
    Ok((path, file))
}

/// Reads `name` under `root` using [`open_contained`].
pub(crate) fn read_contained(root: &Path, name: &str) -> Result<Vec<u8>, RecordingError> {
    let (_path, mut file) = open_contained(root, name)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn open_nofollow_read(path: &Path) -> io::Result<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .read(true)
            .custom_flags(unix_o_nofollow())
            .open(path)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        OpenOptions::new().read(true).open(path)
    }
}

#[cfg(unix)]
const fn unix_o_nofollow() -> i32 {
    // O_NOFOLLOW: Linux/Android 0400000, Darwin/BSD 0x0100.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        0o200000
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        0o0000400
    }
}

#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

fn is_reparse_or_symlink(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

fn is_symlink_open_error(err: &io::Error) -> bool {
    match err.raw_os_error() {
        #[cfg(unix)]
        Some(code) if code == unix_eloop() => true,
        #[cfg(windows)]
        Some(1920 | 1463 | 4390 | 4392 | 4393) => true,
        _ => false,
    }
}

#[cfg(unix)]
const fn unix_eloop() -> i32 {
    // ELOOP: Linux/Android 40, Darwin/BSD 62.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        40
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        62
    }
}

/// Writes `bytes` to `dest` via a sibling temporary file, then replaces.
pub(crate) fn replace_bytes(dest: &Path, bytes: &[u8]) -> Result<(), RecordingError> {
    let tmp = sibling_temp(dest);
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)?;
        use std::io::Write;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
    }
    replace_file(&tmp, dest)?;
    Ok(())
}

fn sibling_temp(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map(|n| format!(".{}.tmp", n.to_string_lossy()))
        .unwrap_or_else(|| ".tmp".into());
    match dest.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

/// Replaces `dest` with `tmp`, leaving either the previous complete file or
/// the new complete file.
pub(crate) fn replace_file(tmp: &Path, dest: &Path) -> Result<(), RecordingError> {
    match fs::rename(tmp, dest) {
        Ok(()) => Ok(()),
        Err(err) if dest.exists() => replace_existing(tmp, dest).map_err(|replace_err| {
            let _ = err;
            RecordingError::Io(replace_err)
        }),
        Err(err) => Err(RecordingError::Io(err)),
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
    // SAFETY: both paths are NUL-terminated UTF-16 from OsStr; ReplaceFileW
    // reads them as C strings and does not retain the pointers.
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

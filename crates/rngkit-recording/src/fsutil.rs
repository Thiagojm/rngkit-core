//! Path containment and platform-correct file replacement.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::RecordingError;

/// Joins a validated stem under a canonicalized output root.
pub(crate) fn child_under_root(root: &Path, stem: &str) -> Result<PathBuf, RecordingError> {
    if stem.is_empty()
        || stem.contains('/')
        || stem.contains('\\')
        || stem.contains("..")
        || Path::new(stem).components().any(|c| {
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
            path: PathBuf::from(stem),
        });
    }
    let root_canon = fs::canonicalize(root)?;
    Ok(root_canon.join(stem))
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

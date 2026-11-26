//! M2: free disk space, for the Folder screen's check.

use std::path::Path;

/// The bytes free (to this user) where `path` would go: asked of the nearest
/// folder that exists, since the install folder usually doesn't yet.
pub fn free_bytes(path: &Path) -> Option<u64> {
    let mut here = Some(path);
    while let Some(dir) = here {
        if dir.is_dir() {
            return statvfs_free(dir);
        }
        here = dir.parent();
    }
    None
}

/// Whether `needed` bytes fit at `path` (unknown space counts as enough:
/// the install itself still fails cleanly, rolled back, if it doesn't).
pub fn enough(needed: u64, path: &Path) -> bool {
    free_bytes(path).map(|free| free >= needed).unwrap_or(true)
}

#[cfg(unix)]
fn statvfs_free(dir: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c_path = CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: c_path is a valid C string and stats a valid out-pointer.
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stats) } != 0 {
        return None;
    }
    Some((stats.f_bavail as u64).saturating_mul(stats.f_frsize as u64))
}

#[cfg(not(unix))]
fn statvfs_free(_dir: &Path) -> Option<u64> {
    None
}

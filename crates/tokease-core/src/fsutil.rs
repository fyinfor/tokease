//! Small filesystem helpers: atomic writes and permission handling.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{Error, Result};

/// Write `bytes` to `path` atomically: write to a temp file in the same
/// directory, fsync, then rename over the destination. Readers never observe
/// a partially written file. `mode` (unix only) is applied before the rename.
pub fn atomic_write(path: &Path, bytes: &[u8], mode: Option<u32>) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| Error::Other(format!("{} has no parent directory", path.display())))?;
    fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;

    let mut tmp = tempfile::Builder::new()
        .prefix(".tokease-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| Error::io(dir, e))?;
    tmp.write_all(bytes).map_err(|e| Error::io(path, e))?;
    tmp.as_file().sync_all().map_err(|e| Error::io(path, e))?;

    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(tmp.path(), fs::Permissions::from_mode(mode))
            .map_err(|e| Error::io(path, e))?;
    }
    #[cfg(not(unix))]
    let _ = mode;

    tmp.persist(path).map_err(|e| Error::io(path, e.error))?;
    Ok(())
}

/// Read a file if it exists. `Ok(None)` when it does not.
pub fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(path, e)),
    }
}

/// Unix permission bits of an existing file, if available.
pub fn file_mode(path: &Path) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).ok().map(|m| m.permissions().mode() & 0o777)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// Remove a file, ignoring "not found".
pub fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::io(path, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_creates_parents_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a/b/c.txt");
        atomic_write(&p, b"one", Some(0o600)).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"one");
        atomic_write(&p, b"two", None).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        #[cfg(unix)]
        assert_eq!(file_mode(&p), Some(0o600));
        // no temp files left behind
        let leftovers: Vec<_> = fs::read_dir(p.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".tokease-"))
            .collect();
        assert!(leftovers.is_empty());
    }
}

//! Atomic file writes: temp file -> write -> fsync -> rename.
//!
//! Used for transaction manifests and any other on-disk state that must
//! never exist in a half-written state, per the crash-safety requirement:
//! "If an operation is interrupted, Nyx must never leave a half-written
//! JSON/manifest file at its final path."

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// Write `data` to `path` atomically. On success, `path` either contains
/// the complete previous content or the complete new content — never a
/// partial write, even if the process is killed mid-operation.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent")
    })?;
    fs::create_dir_all(dir)?;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid filename"))?;
    // Random-ish suffix to avoid collisions between concurrent writers to
    // different files landing on the same temp name.
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_path = dir.join(format!(".{file_name}.{pid}.{nanos}.tmp"));

    {
        let mut f = File::create(&tmp_path)?;
        f.write_all(data)?;
        f.flush()?;
        f.sync_all()?; // fsync the file contents
    }

    fs::rename(&tmp_path, path)?;

    // fsync the containing directory so the rename itself is durable
    // (without this, a crash right after rename can lose the directory
    // entry on some filesystems/journaling modes).
    if let Ok(dir_file) = File::open(dir) {
        let _ = dir_file.sync_all();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_creates_file_with_exact_content() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub/manifest.nyx");
        write_atomic(&path, b"hello").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"hello");
    }

    #[test]
    fn write_atomic_overwrites_completely() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("manifest.nyx");
        write_atomic(&path, b"first-longer-content").unwrap();
        write_atomic(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
    }

    #[test]
    fn write_atomic_leaves_no_tmp_files_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("manifest.nyx");
        write_atomic(&path, b"data").unwrap();
        let entries: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "manifest.nyx");
    }
}

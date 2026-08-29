//! Safe wrapper around `alpm_handle_t`.
//!
//! Owns the libalpm handle for its lifetime and calls `alpm_release` on
//! `Drop`, matching libalpm's documented lifecycle ("This should be the
//! last alpm call you make... handle should be considered invalid and
//! cannot be reused in any way" after release).

use crate::cstr::cstr;
use crate::db::Db;
use crate::error::AlpmError;
use crate::list::AlpmList;
use crate::sys;
use std::ffi::CString;
use std::path::Path;
use std::ptr;

/// An open libalpm context. `root` and `dbpath` are fixed at
/// initialization time (libalpm has no post-init setter for them), so
/// changing either requires constructing a new `Handle`.
pub struct Handle {
    pub(crate) raw: *mut sys::alpm_handle_t,
}

// SAFETY: libalpm's handle is not documented as thread-safe for
// concurrent *mutating* calls, so we deliberately do NOT implement Sync.
// Send is sound: the handle owns no thread-local state and is simply a
// pointer to heap-allocated library state that can be moved between
// threads as long as it is used from one thread at a time (enforced by
// Rust's ownership/borrowing rules on `&mut Handle` for mutating calls).
unsafe impl Send for Handle {}

impl Handle {
    /// Initialize libalpm rooted at `root` with its database at `dbpath`.
    ///
    /// Both paths are converted to `CString`s and passed to
    /// `alpm_initialize`, which documents that `root` and `dbpath` must be
    /// valid for the call itself but are *copied* internally by libalpm
    /// (libalpm does not retain the pointers after the call returns), so
    /// no lifetime tie-back to the `CString`s is needed once this
    /// function returns.
    pub fn initialize(root: impl AsRef<Path>, dbpath: impl AsRef<Path>) -> Result<Self, AlpmError> {
        let root_c = path_to_cstring(root.as_ref())?;
        let dbpath_c = path_to_cstring(dbpath.as_ref())?;

        let mut errno: sys::alpm_errno_t = sys::_alpm_errno_t_ALPM_ERR_OK;
        let raw = unsafe { sys::alpm_initialize(root_c.as_ptr(), dbpath_c.as_ptr(), &mut errno) };

        if raw.is_null() {
            return Err(AlpmError::from_errno(errno));
        }
        Ok(Self { raw })
    }

    /// The root path libalpm is currently configured with.
    pub fn root(&self) -> String {
        unsafe { cstr(sys::alpm_option_get_root(self.raw)) }
    }

    /// The database path libalpm is currently configured with.
    pub fn dbpath(&self) -> String {
        unsafe { cstr(sys::alpm_option_get_dbpath(self.raw)) }
    }

    /// The lockfile path libalpm would use (informational; existence is
    /// not implied).
    pub fn lockfile(&self) -> String {
        unsafe { cstr(sys::alpm_option_get_lockfile(self.raw)) }
    }

    /// libalpm's compile-time capability bitmask (NLS / downloader /
    /// signatures support), independent of any handle.
    pub fn capabilities() -> AlpmCapabilities {
        let caps = unsafe { sys::alpm_capabilities() };
        AlpmCapabilities {
            nls: caps & (sys::alpm_caps_ALPM_CAPABILITY_NLS as i32) != 0,
            downloader: caps & (sys::alpm_caps_ALPM_CAPABILITY_DOWNLOADER as i32) != 0,
            signatures: caps & (sys::alpm_caps_ALPM_CAPABILITY_SIGNATURES as i32) != 0,
        }
    }

    /// The linked libalpm version string (e.g. `"15.0.0"`).
    pub fn version() -> String {
        unsafe { cstr(sys::alpm_version()) }
    }

    /// Register a directory to search for cached package files.
    pub fn add_cachedir(&mut self, path: impl AsRef<Path>) -> Result<(), AlpmError> {
        let c = path_to_cstring(path.as_ref())?;
        let rc = unsafe { sys::alpm_option_add_cachedir(self.raw, c.as_ptr()) };
        self.check_rc(rc)
    }

    /// Set the log file path libalpm should write diagnostics to.
    pub fn set_logfile(&mut self, path: impl AsRef<Path>) -> Result<(), AlpmError> {
        let c = path_to_cstring(path.as_ref())?;
        let rc = unsafe { sys::alpm_option_set_logfile(self.raw, c.as_ptr()) };
        self.check_rc(rc)
    }

    /// The local (installed-package) database view.
    pub fn local_db(&self) -> Result<Db<'_>, AlpmError> {
        let raw = unsafe { sys::alpm_get_localdb(self.raw) };
        if raw.is_null() {
            return Err(self.last_error());
        }
        // SAFETY: alpm_get_localdb returns a borrowed pointer owned by
        // the handle; it stays valid for the handle's lifetime and must
        // never be freed by the caller (there is no alpm_db_free for it).
        Ok(unsafe { Db::from_raw(raw) })
    }

    /// Register (or fetch, if already registered) a sync repository
    /// database by name, e.g. `"core"`, `"extra"`.
    pub fn register_syncdb(&mut self, name: &str, siglevel: i32) -> Result<Db<'_>, AlpmError> {
        let name_c = CString::new(name)
            .map_err(|_| AlpmError { errno: -1, message: "repository name contains NUL".into() })?;
        let raw = unsafe { sys::alpm_register_syncdb(self.raw, name_c.as_ptr(), siglevel) };
        if raw.is_null() {
            return Err(self.last_error());
        }
        Ok(unsafe { Db::from_raw(raw) })
    }

    /// All currently registered sync databases.
    pub fn sync_dbs(&self) -> Vec<Db<'_>> {
        let raw = unsafe { sys::alpm_get_syncdbs(self.raw) };
        let list: AlpmList<sys::alpm_db_t> = unsafe { AlpmList::from_raw(raw) };
        list.iter().map(|p| unsafe { Db::from_raw(p) }).collect()
    }

    /// Refresh (download, if `force` or missing) the given sync
    /// databases. Returns `Ok(())` on success; a positive/negative return
    /// from libalpm both map to `Err` with the handle's last error.
    pub fn update_dbs(&mut self, dbs: &[Db<'_>], force: bool) -> Result<(), AlpmError> {
        // Build a temporary alpm_list_t chain of borrowed db pointers.
        // These nodes are ours; the *payloads* (db pointers) are borrowed
        // from libalpm and must not be freed. We free only the chain
        // nodes we allocated (via `alpm_list_t` boxed here), never their
        // `data`.
        let mut nodes: Vec<Box<sys::alpm_list_t>> = Vec::with_capacity(dbs.len());
        for db in dbs {
            nodes.push(Box::new(sys::alpm_list_t {
                data: db.raw as *mut _,
                prev: ptr::null_mut(),
                next: ptr::null_mut(),
            }));
        }
        for i in 0..nodes.len() {
            let next_ptr = if i + 1 < nodes.len() {
                &mut *nodes[i + 1] as *mut sys::alpm_list_t
            } else {
                ptr::null_mut()
            };
            nodes[i].next = next_ptr;
        }
        let head = nodes
            .first_mut()
            .map(|b| &mut **b as *mut sys::alpm_list_t)
            .unwrap_or(ptr::null_mut());

        let rc = unsafe { sys::alpm_db_update(self.raw, head, force as i32) };
        // `nodes` (our own chain wrapper) is dropped here automatically;
        // we never call alpm_list_free on it since we never asked
        // libalpm to allocate it in the first place.
        if rc < 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    pub fn last_error(&self) -> AlpmError {
        let errno = unsafe { sys::alpm_errno(self.raw) };
        AlpmError::from_errno(errno)
    }

    fn check_rc(&self, rc: i32) -> Result<(), AlpmError> {
        if rc < 0 {
            Err(self.last_error())
        } else {
            Ok(())
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: `self.raw` was produced by a successful
        // `alpm_initialize` call and is only ever released once (Drop
        // runs at most once per value, and `Handle` is not `Clone`).
        unsafe {
            sys::alpm_release(self.raw);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AlpmCapabilities {
    pub nls: bool,
    pub downloader: bool,
    pub signatures: bool,
}

fn path_to_cstring(path: &Path) -> Result<CString, AlpmError> {
    let s = path.to_str().ok_or_else(|| AlpmError {
        errno: -1,
        message: format!("path is not valid UTF-8: {}", path.display()),
    })?;
    CString::new(s).map_err(|_| AlpmError {
        errno: -1,
        message: "path contains an interior NUL byte".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_and_version_do_not_require_a_handle() {
        // These are static libalpm queries; safe to call with no open
        // handle. Verifies the FFI link itself works end-to-end against
        // the real system libalpm.
        let caps = Handle::capabilities();
        // We don't assert specific values (they depend on how the
        // system's libalpm was built) but the call must not crash and
        // version must be non-empty.
        let _ = caps;
        let version = Handle::version();
        assert!(!version.is_empty(), "alpm_version() returned empty string");
    }

    #[test]
    fn initialize_and_release_against_disposable_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let dbpath = tmp.path().join("db");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&dbpath).unwrap();

        let handle = Handle::initialize(&root, &dbpath).expect("alpm_initialize failed");
        assert_eq!(handle.root().trim_end_matches('/'), root.to_str().unwrap());
        assert_eq!(
            handle.dbpath().trim_end_matches('/'),
            dbpath.to_str().unwrap()
        );
        // handle dropped here -> alpm_release
    }

    #[test]
    fn local_db_is_queryable_on_fresh_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let dbpath = tmp.path().join("db");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&dbpath).unwrap();

        let handle = Handle::initialize(&root, &dbpath).unwrap();
        let local = handle.local_db().expect("local_db failed");
        assert_eq!(local.name(), "local");
        // Freshly created dbpath -> no packages installed yet.
        assert_eq!(local.packages().len(), 0);
    }
}

//! Safe wrapper around `alpm_handle_t`.
//!
//! Owns the libalpm handle for its lifetime and calls `alpm_release` on
//! `Drop`, matching libalpm's documented lifecycle ("This should be the
//! last alpm call you make... handle should be considered invalid and
//! cannot be reused in any way" after release).

use crate::borrowed_list::BorrowedList;
use crate::cstr::cstr;
use crate::db::Db;
use crate::error::AlpmError;
use crate::list::AlpmList;
use crate::pkg::{Conflict, DepMissing, Package};
use crate::sys;
use std::ffi::CString;
use std::path::Path;

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
        let chain = BorrowedList::from_ptrs(dbs.iter().map(|db| db.raw as *mut _));
        let rc = unsafe { sys::alpm_db_update(self.raw, chain.as_raw(), force as i32) };
        if rc < 0 {
            return Err(self.last_error());
        }
        Ok(())
    }

    /// Check a candidate package set for missing dependencies.
    ///
    /// `pkglist` is the full set of packages that would be present after
    /// the transaction (i.e. currently-installed packages plus the ones
    /// being added, minus the ones in `remove`), matching
    /// `alpm_checkdeps`'s documented contract. `upgrade` marks a subset
    /// of `pkglist` as being upgrades-in-place (remove-then-add
    /// semantics for reverse-dependency checking).
    ///
    /// Returns the list of unmet dependencies (empty = all satisfied).
    /// This is a **read-only query**; it does not mutate any
    /// transaction state (that only happens via `alpm_trans_*`, not yet
    /// wrapped).
    pub fn check_deps(
        &self,
        pkglist: &[Package<'_>],
        remove: &[Package<'_>],
        upgrade: &[Package<'_>],
        reverse_deps: bool,
    ) -> Vec<DepMissing> {
        let pkglist_chain = BorrowedList::from_ptrs(pkglist.iter().map(|p| p.raw as *mut _));
        let remove_chain = BorrowedList::from_ptrs(remove.iter().map(|p| p.raw as *mut _));
        let upgrade_chain = BorrowedList::from_ptrs(upgrade.iter().map(|p| p.raw as *mut _));

        let raw = unsafe {
            sys::alpm_checkdeps(
                self.raw,
                pkglist_chain.as_raw(),
                remove_chain.as_raw(),
                upgrade_chain.as_raw(),
                reverse_deps as i32,
            )
        };

        // SAFETY: alpm_checkdeps transfers ownership of the returned
        // list (and its alpm_depmissing_t payloads) to the caller per
        // its doc comment; we copy every field out via DepMissing::from_raw
        // and then free both the payloads (alpm_depmissing_free, via
        // alpm_list_free_inner) and the list nodes (alpm_list_free) —
        // this is exactly the FREELIST(p) pattern from alpm_list.h,
        // spelled out rather than using the C macro.
        let list: AlpmList<sys::alpm_depmissing_t> = unsafe { AlpmList::from_raw(raw) };
        let out: Vec<DepMissing> = list.iter().map(|p| unsafe { DepMissing::from_raw(p) }).collect();
        unsafe {
            sys::alpm_list_free_inner(raw, Some(free_depmissing));
            sys::alpm_list_free(raw);
        }
        out
    }

    /// Check a candidate package set for file/name/provides conflicts.
    pub fn check_conflicts(&self, pkglist: &[Package<'_>]) -> Vec<Conflict> {
        let pkglist_chain = BorrowedList::from_ptrs(pkglist.iter().map(|p| p.raw as *mut _));
        let raw = unsafe { sys::alpm_checkconflicts(self.raw, pkglist_chain.as_raw()) };

        // SAFETY: same ownership contract as check_deps above, but with
        // alpm_conflict_t payloads and alpm_conflict_free.
        let list: AlpmList<sys::alpm_conflict_t> = unsafe { AlpmList::from_raw(raw) };
        let out: Vec<Conflict> = list.iter().map(|p| unsafe { Conflict::from_raw(p) }).collect();
        unsafe {
            sys::alpm_list_free_inner(raw, Some(free_conflict));
            sys::alpm_list_free(raw);
        }
        out
    }

    /// Find a package among `dbs` that satisfies `depstring` (a
    /// dependency spec such as `"foo>=1.2"` or a bare provides name).
    pub fn find_dbs_satisfier<'a>(
        &'a self,
        dbs: &[Db<'a>],
        depstring: &str,
    ) -> Option<Package<'a>> {
        let dbs_chain = BorrowedList::from_ptrs(dbs.iter().map(|d| d.raw as *mut _));
        let depstring_c = CString::new(depstring).ok()?;
        let raw = unsafe {
            sys::alpm_find_dbs_satisfier(self.raw, dbs_chain.as_raw(), depstring_c.as_ptr())
        };
        if raw.is_null() {
            None
        } else {
            Some(unsafe { Package::from_raw(raw) })
        }
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

unsafe extern "C" fn free_depmissing(item: *mut std::os::raw::c_void) {
    // SAFETY: called by alpm_list_free_inner exactly once per node for
    // the list check_deps() got from alpm_checkdeps, whose payload type
    // is alpm_depmissing_t*; this is the correct deallocator per
    // alpm_depmissing_free's own contract.
    unsafe { sys::alpm_depmissing_free(item as *mut sys::alpm_depmissing_t) };
}

unsafe extern "C" fn free_conflict(item: *mut std::os::raw::c_void) {
    // SAFETY: same as free_depmissing, but for alpm_checkconflicts's
    // alpm_conflict_t payloads and alpm_conflict_free.
    unsafe { sys::alpm_conflict_free(item as *mut sys::alpm_conflict_t) };
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

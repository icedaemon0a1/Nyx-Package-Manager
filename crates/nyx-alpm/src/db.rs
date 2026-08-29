//! Safe wrapper around `alpm_db_t` (a local or sync package database).
//!
//! Every `Db<'a>` is **borrowed**: libalpm owns the database object for
//! the lifetime of the [`crate::handle::Handle`] that produced it (via
//! `alpm_get_localdb` / `alpm_register_syncdb` / `alpm_get_syncdbs`).
//! There is no `alpm_db_free` in libalpm's public API — databases are
//! released only when the owning handle is released — so `Db` has no
//! `Drop` impl of its own.

use crate::cstr::cstr;
use crate::list::AlpmList;
use crate::pkg::Package;
use crate::sys;
use std::ffi::CString;
use std::marker::PhantomData;

pub struct Db<'a> {
    pub(crate) raw: *mut sys::alpm_db_t,
    _marker: PhantomData<&'a ()>,
}

impl<'a> Db<'a> {
    /// # Safety
    /// `raw` must be a non-null pointer to an `alpm_db_t` obtained from
    /// the handle this `Db` is borrowed from, valid for at least `'a`.
    pub(crate) unsafe fn from_raw(raw: *mut sys::alpm_db_t) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    /// The database's repository name (`"local"` for the local db,
    /// otherwise the sync repo name e.g. `"core"`).
    pub fn name(&self) -> String {
        unsafe { cstr(sys::alpm_db_get_name(self.raw)) }
    }

    /// Look up a single package by exact name. Returns `None` if not
    /// present (this is libalpm's own null-return convention, not
    /// necessarily an error).
    pub fn get_pkg(&self, name: &str) -> Option<Package<'a>> {
        let name_c = CString::new(name).ok()?;
        let raw = unsafe { sys::alpm_db_get_pkg(self.raw, name_c.as_ptr()) };
        if raw.is_null() {
            None
        } else {
            Some(unsafe { Package::from_raw(raw) })
        }
    }

    /// All packages currently in this database's cache.
    pub fn packages(&self) -> Vec<Package<'a>> {
        let raw = unsafe { sys::alpm_db_get_pkgcache(self.raw) };
        let list: AlpmList<sys::alpm_pkg_t> = unsafe { AlpmList::from_raw(raw) };
        list.iter().map(|p| unsafe { Package::from_raw(p) }).collect()
    }

    /// Search this database for packages whose name/description match
    /// every given regular expression (libalpm ANDs the needles
    /// together; each needle is treated as a POSIX extended regex, per
    /// `alpm_db_search`'s documented semantics).
    ///
    /// The list `alpm_db_search` allocates is a *result* list owned by
    /// the caller for its node structure, but its payload pointers point
    /// into the database's own package cache and must not be freed
    /// individually; we free only the list nodes via `alpm_list_free`
    /// once we've copied out the `Package` handles.
    pub fn search(&self, needles: &[&str]) -> Result<Vec<Package<'a>>, crate::error::AlpmError> {
        let needle_cstrings: Vec<CString> = needles
            .iter()
            .map(|s| CString::new(*s))
            .collect::<Result<_, _>>()
            .map_err(|_| crate::error::AlpmError {
                errno: -1,
                message: "search needle contains an interior NUL byte".into(),
            })?;

        // Build a temporary alpm_list_t chain of needle C-string
        // pointers. Same ownership pattern as Handle::update_dbs: the
        // chain *nodes* are ours (boxed, freed on drop of `nodes`); the
        // `data` payloads point at our own `CString`s, which outlive the
        // call, so nothing here is ever passed to `alpm_list_free`.
        let mut nodes: Vec<Box<sys::alpm_list_t>> = Vec::with_capacity(needle_cstrings.len());
        for c in &needle_cstrings {
            nodes.push(Box::new(sys::alpm_list_t {
                data: c.as_ptr() as *mut _,
                prev: std::ptr::null_mut(),
                next: std::ptr::null_mut(),
            }));
        }
        for i in 0..nodes.len() {
            let next_ptr = if i + 1 < nodes.len() {
                &mut *nodes[i + 1] as *mut sys::alpm_list_t
            } else {
                std::ptr::null_mut()
            };
            nodes[i].next = next_ptr;
        }
        let head = nodes
            .first_mut()
            .map(|b| &mut **b as *mut sys::alpm_list_t)
            .unwrap_or(std::ptr::null_mut());

        let mut result: *mut sys::alpm_list_t = std::ptr::null_mut();
        let rc = unsafe { sys::alpm_db_search(self.raw, head, &mut result) };
        if rc != 0 {
            return Err(crate::error::AlpmError {
                errno: -1,
                message: "alpm_db_search failed".into(),
            });
        }

        let result_list: AlpmList<sys::alpm_pkg_t> = unsafe { AlpmList::from_raw(result) };
        let packages: Vec<Package<'a>> = result_list
            .iter()
            .map(|p| unsafe { Package::from_raw(p) })
            .collect();

        // SAFETY: `result` is a list *we* were handed ownership of by
        // `alpm_db_search` (its own docs: caller must free the returned
        // list). Its node payloads are borrowed package pointers from
        // the db's cache (not owned by the list), so a plain
        // `alpm_list_free` (which does not touch `data`) is correct here
        // — never `alpm_list_free_inner`/a payload-freeing variant.
        unsafe {
            sys::alpm_list_free(result);
        }

        Ok(packages)
    }
}

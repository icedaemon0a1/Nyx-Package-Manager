//! Safe wrapper around `alpm_pkg_t` and its associated dependency/file
//! list element types.
//!
//! Every `Package<'a>` is a **borrowed** view: libalpm owns the
//! `alpm_pkg_t` for as long as the database (or, for `alpm_pkg_load`,
//! the loaded-package handle) that produced it is alive. Nyx never calls
//! `alpm_pkg_free` on packages obtained from a database's package cache
//! (`alpm_db_get_pkg`/`alpm_db_get_pkgcache`) — only packages obtained
//! from `alpm_pkg_load` are ever owned and freed by us, and that is not
//! yet wired up (Phase 1: installing from a local file).

use crate::cstr::{cstr, opt_cstr};
use crate::list::AlpmList;
use crate::sys;
use std::marker::PhantomData;

/// A package record borrowed from a database's package cache.
///
/// The `'a` lifetime ties this value to the [`crate::db::Db`] (and
/// transitively the [`crate::handle::Handle`]) it was obtained from.
#[derive(Clone, Copy)]
pub struct Package<'a> {
    pub(crate) raw: *mut sys::alpm_pkg_t,
    _marker: PhantomData<&'a ()>,
}

impl<'a> Package<'a> {
    /// # Safety
    /// `raw` must be a non-null pointer to an `alpm_pkg_t` that stays
    /// valid for at least `'a` (true for every pointer libalpm's
    /// database accessors hand back, for the lifetime of the owning
    /// database/handle).
    pub(crate) unsafe fn from_raw(raw: *mut sys::alpm_pkg_t) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    pub fn name(&self) -> String {
        unsafe { cstr(sys::alpm_pkg_get_name(self.raw)) }
    }

    pub fn version(&self) -> String {
        unsafe { cstr(sys::alpm_pkg_get_version(self.raw)) }
    }

    pub fn desc(&self) -> Option<String> {
        unsafe { opt_cstr(sys::alpm_pkg_get_desc(self.raw)) }
    }

    pub fn url(&self) -> Option<String> {
        unsafe { opt_cstr(sys::alpm_pkg_get_url(self.raw)) }
    }

    pub fn packager(&self) -> Option<String> {
        unsafe { opt_cstr(sys::alpm_pkg_get_packager(self.raw)) }
    }

    pub fn arch(&self) -> Option<String> {
        unsafe { opt_cstr(sys::alpm_pkg_get_arch(self.raw)) }
    }

    pub fn base(&self) -> Option<String> {
        unsafe { opt_cstr(sys::alpm_pkg_get_base(self.raw)) }
    }

    pub fn filename(&self) -> Option<String> {
        unsafe { opt_cstr(sys::alpm_pkg_get_filename(self.raw)) }
    }

    /// Unix timestamp the package was built, or `0` if unknown.
    pub fn build_date(&self) -> i64 {
        unsafe { sys::alpm_pkg_get_builddate(self.raw) }
    }

    /// Unix timestamp the package was installed, or `0` if not
    /// applicable (e.g. a sync-db package that isn't installed).
    pub fn install_date(&self) -> i64 {
        unsafe { sys::alpm_pkg_get_installdate(self.raw) }
    }

    /// Compressed package size in bytes.
    pub fn size(&self) -> i64 {
        unsafe { sys::alpm_pkg_get_size(self.raw) }
    }

    /// Installed size in bytes.
    pub fn installed_size(&self) -> i64 {
        unsafe { sys::alpm_pkg_get_isize(self.raw) }
    }

    pub fn reason(&self) -> PkgReason {
        PkgReason::from_raw(unsafe { sys::alpm_pkg_get_reason(self.raw) })
    }

    pub fn licenses(&self) -> Vec<String> {
        let raw = unsafe { sys::alpm_pkg_get_licenses(self.raw) };
        let list: AlpmList<std::os::raw::c_char> = unsafe { AlpmList::from_raw(raw) };
        list.iter()
            .map(|p| unsafe { cstr(p as *const std::os::raw::c_char) })
            .collect()
    }

    pub fn groups(&self) -> Vec<String> {
        let raw = unsafe { sys::alpm_pkg_get_groups(self.raw) };
        let list: AlpmList<std::os::raw::c_char> = unsafe { AlpmList::from_raw(raw) };
        list.iter()
            .map(|p| unsafe { cstr(p as *const std::os::raw::c_char) })
            .collect()
    }

    pub fn depends(&self) -> Vec<Dependency> {
        depend_list(unsafe { sys::alpm_pkg_get_depends(self.raw) })
    }

    pub fn optdepends(&self) -> Vec<Dependency> {
        depend_list(unsafe { sys::alpm_pkg_get_optdepends(self.raw) })
    }

    pub fn conflicts(&self) -> Vec<Dependency> {
        depend_list(unsafe { sys::alpm_pkg_get_conflicts(self.raw) })
    }

    pub fn provides(&self) -> Vec<Dependency> {
        depend_list(unsafe { sys::alpm_pkg_get_provides(self.raw) })
    }

    pub fn replaces(&self) -> Vec<Dependency> {
        depend_list(unsafe { sys::alpm_pkg_get_replaces(self.raw) })
    }

    /// Backup file entries (`etc/foo.conf` + its packaged-file hash).
    pub fn backup(&self) -> Vec<BackupEntry> {
        let raw = unsafe { sys::alpm_pkg_get_backup(self.raw) };
        let list: AlpmList<sys::alpm_backup_t> = unsafe { AlpmList::from_raw(raw) };
        list.iter()
            .map(|p| unsafe {
                let b = &*p;
                BackupEntry {
                    name: cstr(b.name),
                    hash: cstr(b.hash),
                }
            })
            .collect()
    }

    /// The full list of files this package owns, or an empty vec if the
    /// package has no file list loaded (e.g. a sync-db package that has
    /// not been downloaded).
    pub fn files(&self) -> Vec<PackageFile> {
        let raw = unsafe { sys::alpm_pkg_get_files(self.raw) };
        if raw.is_null() {
            return Vec::new();
        }
        // SAFETY: alpm_pkg_get_files returns a borrowed, non-owning
        // pointer to a struct (not a list node) containing `count` and a
        // `files` array of that length, valid for the package's lifetime.
        unsafe {
            let filelist = &*raw;
            let mut out = Vec::with_capacity(filelist.count);
            for i in 0..filelist.count {
                let f = &*filelist.files.add(i);
                out.push(PackageFile {
                    name: cstr(f.name),
                    size: f.size,
                    mode: f.mode,
                });
            }
            out
        }
    }
}

fn depend_list(raw: *mut sys::alpm_list_t) -> Vec<Dependency> {
    let list: AlpmList<sys::alpm_depend_t> = unsafe { AlpmList::from_raw(raw) };
    list.iter()
        .map(|p| unsafe { Dependency::from_raw(p) })
        .collect()
}

/// A single dependency/conflict/provides/replaces entry.
#[derive(Debug, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub desc: Option<String>,
    pub modifier: DepMod,
}

impl Dependency {
    /// # Safety
    /// `raw` must point to a valid `alpm_depend_t` for the duration of
    /// this call (true for every element of the lists nyx-alpm reads
    /// them from; we copy the fields out rather than retain the
    /// pointer).
    unsafe fn from_raw(raw: *mut sys::alpm_depend_t) -> Self {
        let d = &*raw;
        Self {
            name: cstr(d.name),
            version: opt_cstr(d.version),
            desc: opt_cstr(d.desc),
            modifier: DepMod::from_raw(d.mod_),
        }
    }

    /// Render as libalpm itself would (e.g. `"foo>=1.2"`), via
    /// `alpm_dep_compute_string` rather than reimplementing the
    /// formatting rules.
    pub fn to_display_string(&self) -> String {
        match (&self.version, self.modifier) {
            (Some(v), m) if m != DepMod::Any => format!("{}{}{}", self.name, m.as_str(), v),
            _ => self.name.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepMod {
    Any,
    Eq,
    Ge,
    Le,
    Gt,
    Lt,
}

impl DepMod {
    fn from_raw(raw: sys::alpm_depmod_t) -> Self {
        match raw {
            sys::_alpm_depmod_t_ALPM_DEP_MOD_EQ => DepMod::Eq,
            sys::_alpm_depmod_t_ALPM_DEP_MOD_GE => DepMod::Ge,
            sys::_alpm_depmod_t_ALPM_DEP_MOD_LE => DepMod::Le,
            sys::_alpm_depmod_t_ALPM_DEP_MOD_GT => DepMod::Gt,
            sys::_alpm_depmod_t_ALPM_DEP_MOD_LT => DepMod::Lt,
            _ => DepMod::Any,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            DepMod::Any => "",
            DepMod::Eq => "=",
            DepMod::Ge => ">=",
            DepMod::Le => "<=",
            DepMod::Gt => ">",
            DepMod::Lt => "<",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgReason {
    Explicit,
    Depend,
    Unknown,
}

impl PkgReason {
    fn from_raw(raw: sys::alpm_pkgreason_t) -> Self {
        match raw {
            sys::_alpm_pkgreason_t_ALPM_PKG_REASON_EXPLICIT => PkgReason::Explicit,
            sys::_alpm_pkgreason_t_ALPM_PKG_REASON_DEPEND => PkgReason::Depend,
            _ => PkgReason::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackupEntry {
    pub name: String,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct PackageFile {
    pub name: String,
    pub size: i64,
    pub mode: u32,
}

/// A single unmet dependency, as reported by `alpm_checkdeps`.
///
/// Owned: this struct copies every field out of the `alpm_depmissing_t`
/// libalpm handed us (which `Handle::check_deps` frees via
/// `alpm_depmissing_free` immediately after building this), so unlike
/// [`Package`] it has no borrowed-pointer lifetime.
#[derive(Debug, Clone)]
pub struct DepMissing {
    /// Name of the package the missing dependency applies to.
    pub target: String,
    /// The unmet dependency spec itself.
    pub depend: Dependency,
    /// If this dependency became unmet because of a package being
    /// removed/replaced in the same transaction, that package's name.
    pub causing_pkg: Option<String>,
}

impl DepMissing {
    /// # Safety
    /// `raw` must point to a valid, non-null `alpm_depmissing_t` (true
    /// for every element of the list `alpm_checkdeps` returns).
    pub(crate) unsafe fn from_raw(raw: *mut sys::alpm_depmissing_t) -> Self {
        let m = &*raw;
        Self {
            target: cstr(m.target),
            depend: Dependency::from_raw(m.depend),
            causing_pkg: opt_cstr(m.causingpkg),
        }
    }
}

/// A conflict between two packages in a candidate set, as reported by
/// `alpm_checkconflicts`.
///
/// `package1`/`package2` are copied out as plain names (not borrowed
/// `Package` handles) since the underlying `alpm_conflict_t` is freed
/// (via `alpm_conflict_free`) immediately after `Handle::check_conflicts`
/// builds this struct, and the conflicting `alpm_pkg_t` pointers it
/// references are not guaranteed to remain otherwise reachable.
#[derive(Debug, Clone)]
pub struct Conflict {
    pub package1: String,
    pub package2: String,
    pub reason: Dependency,
}

impl Conflict {
    /// # Safety
    /// `raw` must point to a valid, non-null `alpm_conflict_t` (true for
    /// every element of the list `alpm_checkconflicts` returns), and its
    /// `package1`/`package2` fields must be valid, non-null `alpm_pkg_t`
    /// pointers for the duration of this call (both hold: libalpm builds
    /// them from the same pkglist passed into the call, which is alive
    /// for the entire `alpm_checkconflicts` invocation).
    pub(crate) unsafe fn from_raw(raw: *mut sys::alpm_conflict_t) -> Self {
        let c = &*raw;
        let p1 = Package::from_raw(c.package1);
        let p2 = Package::from_raw(c.package2);
        Self {
            package1: p1.name(),
            package2: p2.name(),
            reason: Dependency::from_raw(c.reason),
        }
    }
}

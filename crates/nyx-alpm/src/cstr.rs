//! Helpers for converting libalpm's `*const c_char` accessor results into
//! safe Rust string types.
//!
//! Every libalpm string accessor (`alpm_pkg_get_name`, `_get_desc`, ...)
//! returns a pointer that is either null (field not set / not
//! applicable) or a valid NUL-terminated C string owned by libalpm for
//! the lifetime of the package/handle it came from. Nyx never frees these
//! pointers.

use std::ffi::CStr;
use std::os::raw::c_char;

/// Convert a possibly-null libalpm C string pointer to `Option<String>`.
///
/// # Safety
/// `ptr` must be either null or point to a valid NUL-terminated C string
/// that remains valid for the duration of this call (true for all
/// pointers returned by the libalpm accessors nyx-alpm wraps, which stay
/// valid as long as the owning object is alive).
pub(crate) unsafe fn opt_cstr(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
    }
}

/// Same as [`opt_cstr`] but for accessors that libalpm guarantees are
/// non-null in practice (e.g. `alpm_pkg_get_name`), returning an empty
/// string as a defensive fallback rather than panicking if libalpm ever
/// violates that guarantee.
///
/// # Safety
/// Same invariant as [`opt_cstr`].
pub(crate) unsafe fn cstr(ptr: *const c_char) -> String {
    opt_cstr(ptr).unwrap_or_default()
}

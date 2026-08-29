//! Typed errors surfaced from libalpm, wrapping `alpm_errno_t` /
//! `alpm_strerror` rather than exposing raw C error codes to callers.

use crate::sys;
use std::ffi::CStr;

/// A typed ALPM error: the raw errno from libalpm plus the human message
/// libalpm itself provides via `alpm_strerror`.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct AlpmError {
    pub errno: i32,
    pub message: String,
}

impl AlpmError {
    /// Build from a raw `alpm_errno_t` by asking libalpm for the message.
    ///
    /// # Safety invariant
    /// `alpm_strerror` is a pure function over an enum value with no
    /// pointer/lifetime dependency: it returns a `'static` C string owned
    /// by libalpm, so it is always safe to call for any `errno` value the
    /// library itself returned to us.
    pub(crate) fn from_errno(errno: sys::alpm_errno_t) -> Self {
        let message = unsafe {
            let ptr = sys::alpm_strerror(errno);
            if ptr.is_null() {
                format!("unknown libalpm error (errno {errno})")
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        };
        Self {
            errno: errno as i32,
            message,
        }
    }
}

impl From<AlpmError> for nyx_core::NyxError {
    fn from(err: AlpmError) -> Self {
        nyx_core::NyxError::with_source(
            nyx_core::ErrorCategory::Alpm,
            err.message.clone(),
            err,
        )
    }
}

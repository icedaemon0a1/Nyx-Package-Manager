//! Safe Rust wrapper around the system libalpm — Nyx's controlled ALPM
//! compatibility layer.
//!
//! This crate deliberately does **not** reimplement libalpm. It links
//! against the system's real `libalpm` (located via `pkg-config` at
//! build time; see `build.rs`) and generates raw FFI bindings from the
//! actually-installed `alpm.h` with `bindgen`, then wraps a
//! Phase-0/Phase-1 subset of that API in safe, ownership-correct Rust
//! types.
//!
//! Module layout:
//! - [`sys`]: the raw, `unsafe` FFI boundary (bindgen output). Nothing
//!   outside this crate should ever touch it.
//! - [`error`]: [`AlpmError`], wrapping `alpm_errno_t`/`alpm_strerror`.
//! - [`list`]: safe iteration over libalpm's public `alpm_list_t`.
//! - [`cstr`]: C-string conversion helpers shared by the wrapper modules.
//! - [`handle`]: [`Handle`], the RAII wrapper around `alpm_handle_t`.
//! - [`db`]: [`Db`], a borrowed view of a local/sync database.
//! - [`pkg`]: [`Package`] and the dependency/file types it exposes.

pub mod sys;

mod borrowed_list;
mod cstr;
mod db;
mod error;
mod handle;
mod list;
mod pkg;

pub use db::Db;
pub use error::AlpmError;
pub use handle::{AlpmCapabilities, Handle};
pub use list::AlpmList;
pub use pkg::{BackupEntry, Conflict, DepMissing, DepMod, Dependency, PackageFile, PkgReason};
pub use pkg::Package;

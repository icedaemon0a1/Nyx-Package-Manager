//! Raw FFI bindings to libalpm, generated at build time from the real
//! installed `alpm.h` by `build.rs` (see that file's allowlist for exactly
//! which functions/types are exposed).
//!
//! **This module is the entire `unsafe` FFI boundary of nyx-alpm.** Nothing
//! outside this crate should ever import from here directly; every other
//! module in nyx-alpm exists to wrap these raw signatures in a safe API.
//! `#[allow(...)]` is applied narrowly to bindgen-generated naming/style
//! that we do not control.

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]
#![allow(clippy::all)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

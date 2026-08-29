//! nyx-core: shared foundation for all Nyx crates.
//!
//! Contains the stable error model, terminal output abstraction,
//! filesystem path layout, content hashing, and atomic file writes.
//! Nothing here talks to libalpm, TOML, or the network — those are the
//! job of `nyx-alpm`, `nyx-config`, and later crates.

pub mod atomic;
pub mod error;
pub mod hash;
pub mod output;
pub mod paths;

pub use error::{ErrorCategory, NyxError, Result};
pub use output::{ColorMode, Output, Verbosity};
pub use paths::NyxPaths;

//! Stable error model for Nyx.
//!
//! Every Nyx error carries a stable machine-readable [`ErrorCategory`] in
//! addition to a human message, so scripts consuming `--json` output (or
//! matching on process exit behaviour) never need to parse free-text error
//! strings to decide what happened. This follows the CLI requirement that
//! "errors should use stable error categories/codes suitable for scripts".

use std::fmt;
use std::path::PathBuf;

/// Stable, script-facing error category. Adding new variants is fine;
/// renaming/removing existing ones is a breaking change for consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Configuration file could not be parsed or contained an invalid value.
    Config,
    /// The requested package/group/repository does not exist.
    NotFound,
    /// A dependency, conflict, or provides constraint could not be satisfied.
    Resolution,
    /// The underlying libalpm call failed (see attached alpm errno if any).
    Alpm,
    /// I/O failure (permissions, disk full, missing directory, ...).
    Io,
    /// A transaction manifest was invalid, missing, or failed atomic write.
    Transaction,
    /// The operation requires root / CAP_* privileges that are not present.
    Privilege,
    /// Caller-provided input failed validation before reaching lower layers.
    InvalidInput,
    /// Internal invariant violated; always a Nyx bug, never user error.
    Internal,
}

impl ErrorCategory {
    /// Short machine-readable token, stable across releases, safe to match
    /// on in scripts (e.g. `nyx install foo --json` error payloads).
    pub const fn code(self) -> &'static str {
        match self {
            ErrorCategory::Config => "config",
            ErrorCategory::NotFound => "not_found",
            ErrorCategory::Resolution => "resolution",
            ErrorCategory::Alpm => "alpm",
            ErrorCategory::Io => "io",
            ErrorCategory::Transaction => "transaction",
            ErrorCategory::Privilege => "privilege",
            ErrorCategory::InvalidInput => "invalid_input",
            ErrorCategory::Internal => "internal",
        }
    }

    /// Process exit code convention used by `nyx-cli`.
    pub const fn exit_code(self) -> i32 {
        match self {
            ErrorCategory::Config => 10,
            ErrorCategory::NotFound => 11,
            ErrorCategory::Resolution => 12,
            ErrorCategory::Alpm => 13,
            ErrorCategory::Io => 14,
            ErrorCategory::Transaction => 15,
            ErrorCategory::Privilege => 16,
            ErrorCategory::InvalidInput => 17,
            ErrorCategory::Internal => 70,
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// A Nyx error: a category plus context. Implements `std::error::Error` so
/// it composes with `anyhow`/`thiserror` in downstream crates.
#[derive(Debug, thiserror::Error)]
pub struct NyxError {
    pub category: ErrorCategory,
    pub message: String,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl fmt::Display for NyxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl NyxError {
    pub fn new(category: ErrorCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(
        category: ErrorCategory,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            category,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    pub fn not_found(what: impl fmt::Display) -> Self {
        Self::new(ErrorCategory::NotFound, format!("not found: {what}"))
    }

    pub fn config(msg: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Config, msg)
    }

    pub fn config_at(path: &PathBuf, line: usize, col: usize, msg: impl fmt::Display) -> Self {
        Self::new(
            ErrorCategory::Config,
            format!("{}:{}:{}: {}", path.display(), line, col, msg),
        )
    }

    pub fn io(msg: impl Into<String>, source: std::io::Error) -> Self {
        Self::with_source(ErrorCategory::Io, msg, source)
    }

    pub fn privilege(msg: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Privilege, msg)
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Internal, msg)
    }
}

pub type Result<T> = std::result::Result<T, NyxError>;

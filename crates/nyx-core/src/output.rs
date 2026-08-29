//! Terminal output abstraction.
//!
//! Design rule (see project brief, "CLI design"): Nyx is a normal Linux
//! tool. No boxes, no emoji, no fake dashboards, no "SECURITY CLEAN"
//! banners. When nothing is wrong, Nyx says nothing extra. Colour is used
//! only to aid scanning (package names, warnings), never decoration, and
//! is disabled automatically when not writing to a TTY or when
//! `--color=never`/`NO_COLOR` is set.

use std::io::IsTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl std::str::FromStr for ColorMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(ColorMode::Auto),
            "always" => Ok(ColorMode::Always),
            "never" => Ok(ColorMode::Never),
            other => Err(format!("invalid color mode: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    Quiet,
    #[default]
    Normal,
    Verbose,
}

/// Shared output context threaded through CLI command implementations.
pub struct Output {
    pub color: bool,
    pub verbosity: Verbosity,
    pub is_tty: bool,
    pub json: bool,
}

impl Output {
    pub fn new(color: ColorMode, verbosity: Verbosity, json: bool) -> Self {
        let is_tty = std::io::stdout().is_terminal();
        let no_color_env = std::env::var_os("NO_COLOR").is_some();
        let color = match color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => is_tty && !no_color_env,
        };
        Self {
            color,
            verbosity,
            is_tty,
            json,
        }
    }

    pub fn is_quiet(&self) -> bool {
        matches!(self.verbosity, Verbosity::Quiet)
    }
    pub fn is_verbose(&self) -> bool {
        matches!(self.verbosity, Verbosity::Verbose)
    }

    /// Normal informational line. Suppressed under `--quiet`.
    pub fn line(&self, s: impl AsRef<str>) {
        if !self.is_quiet() {
            println!("{}", s.as_ref());
        }
    }

    /// Emphasised line (e.g. package name/version header). Bold when
    /// colour is enabled, otherwise plain — never relies on colour to
    /// convey information.
    pub fn header(&self, s: impl AsRef<str>) {
        if self.is_quiet() {
            return;
        }
        if self.color {
            let style = anstyle::Style::new().bold();
            println!("{style}{}{style:#}", s.as_ref());
        } else {
            println!("{}", s.as_ref());
        }
    }

    /// A warning that requires attention (e.g. privileged-change review).
    /// Printed even under `--quiet` since it may gate a decision; suppress
    /// entirely only for genuinely silent scripts via output redirection.
    pub fn warn(&self, s: impl AsRef<str>) {
        if self.color {
            let style = anstyle::Style::new().fg_color(Some(anstyle::AnsiColor::Yellow.into()));
            eprintln!("{style}{}{style:#}", s.as_ref());
        } else {
            eprintln!("{}", s.as_ref());
        }
    }

    pub fn error(&self, s: impl AsRef<str>) {
        if self.color {
            let style = anstyle::Style::new().fg_color(Some(anstyle::AnsiColor::Red.into()));
            eprintln!("{style}error:{style:#} {}", s.as_ref());
        } else {
            eprintln!("error: {}", s.as_ref());
        }
    }

    /// Verbose/debug diagnostic. Opt-in only, per logging requirements.
    pub fn debug(&self, s: impl AsRef<str>) {
        if self.is_verbose() {
            eprintln!("debug: {}", s.as_ref());
        }
    }
}

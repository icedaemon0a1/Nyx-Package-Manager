//! Filesystem layout constants.
//!
//! Nyx keeps no general-purpose database. All persistent state lives under
//! these three roots as immutable files, content-addressed caches, or
//! regeneratable indexes. See docs/adr/0001-phase0-phase1-foundation.md.

use std::path::{Path, PathBuf};

/// Root-relative Nyx paths. Everything is relative to `root` so tests can
/// point Nyx at a disposable chroot/tmpdir instead of the real `/`.
#[derive(Debug, Clone)]
pub struct NyxPaths {
    root: PathBuf,
}

impl NyxPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn system() -> Self {
        Self::new("/")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn join(&self, p: &str) -> PathBuf {
        self.root.join(p.trim_start_matches('/'))
    }

    // /etc/nyx/*
    pub fn etc_dir(&self) -> PathBuf {
        self.join("etc/nyx")
    }
    pub fn main_config(&self) -> PathBuf {
        self.etc_dir().join("nyx.toml")
    }
    pub fn conf_d(&self) -> PathBuf {
        self.etc_dir().join("conf.d")
    }
    pub fn repos_d(&self) -> PathBuf {
        self.etc_dir().join("repos.d")
    }
    pub fn policies_d(&self) -> PathBuf {
        self.etc_dir().join("policies.d")
    }
    pub fn profiles_dir(&self) -> PathBuf {
        self.etc_dir().join("profiles")
    }
    pub fn packages_d(&self) -> PathBuf {
        self.etc_dir().join("packages.d")
    }

    // /var/lib/nyx/*
    pub fn lib_dir(&self) -> PathBuf {
        self.join("var/lib/nyx")
    }
    pub fn alpm_db_dir(&self) -> PathBuf {
        self.lib_dir().join("alpm")
    }
    pub fn transactions_dir(&self) -> PathBuf {
        self.lib_dir().join("transactions")
    }
    pub fn baselines_dir(&self) -> PathBuf {
        self.lib_dir().join("baselines")
    }
    pub fn snapshots_dir(&self) -> PathBuf {
        self.lib_dir().join("snapshots")
    }
    pub fn locks_dir(&self) -> PathBuf {
        self.lib_dir().join("locks")
    }

    // /var/cache/nyx/*
    pub fn cache_dir(&self) -> PathBuf {
        self.join("var/cache/nyx")
    }
    pub fn pkg_cache_dir(&self) -> PathBuf {
        self.cache_dir().join("pkg")
    }
    pub fn scan_cache_dir(&self) -> PathBuf {
        self.cache_dir().join("scan")
    }
    pub fn mirrors_cache_dir(&self) -> PathBuf {
        self.cache_dir().join("mirrors")
    }
    pub fn indexes_cache_dir(&self) -> PathBuf {
        self.cache_dir().join("indexes")
    }

    /// Legacy pacman path, only consulted for `nyx migrate pacman` /
    /// interop — never part of the normal Nyx UX.
    pub fn legacy_pacman_conf(&self) -> PathBuf {
        self.join("etc/pacman.conf")
    }
    pub fn legacy_pacman_lib(&self) -> PathBuf {
        self.join("var/lib/pacman")
    }
    pub fn legacy_pacman_cache(&self) -> PathBuf {
        self.join("var/cache/pacman/pkg")
    }

    /// User config directory (`$XDG_CONFIG_HOME/nyx` or `~/.config/nyx`).
    pub fn user_config_dir() -> Option<PathBuf> {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("nyx"));
            }
        }
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".config/nyx"))
    }

    pub fn user_config_file() -> Option<PathBuf> {
        Self::user_config_dir().map(|d| d.join("config.toml"))
    }
}

/// Create all Nyx-owned directories under `paths` if missing (mode 0755).
/// Idempotent; used by `nyx doctor`/first-run and by tests using a tmpdir
/// root.
pub fn ensure_layout(paths: &NyxPaths) -> std::io::Result<()> {
    for dir in [
        paths.etc_dir(),
        paths.conf_d(),
        paths.repos_d(),
        paths.policies_d(),
        paths.profiles_dir(),
        paths.packages_d(),
        paths.lib_dir(),
        paths.alpm_db_dir(),
        paths.transactions_dir(),
        paths.baselines_dir(),
        paths.snapshots_dir(),
        paths.locks_dir(),
        paths.cache_dir(),
        paths.pkg_cache_dir(),
        paths.scan_cache_dir(),
        paths.mirrors_cache_dir(),
        paths.indexes_cache_dir(),
    ] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_relative_to_root() {
        let p = NyxPaths::new("/tmp/nyx-test-root");
        assert_eq!(
            p.main_config(),
            PathBuf::from("/tmp/nyx-test-root/etc/nyx/nyx.toml")
        );
        assert_eq!(
            p.alpm_db_dir(),
            PathBuf::from("/tmp/nyx-test-root/var/lib/nyx/alpm")
        );
    }

    #[test]
    fn ensure_layout_creates_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let p = NyxPaths::new(tmp.path());
        ensure_layout(&p).unwrap();
        assert!(p.repos_d().is_dir());
        assert!(p.transactions_dir().is_dir());
        assert!(p.scan_cache_dir().is_dir());
    }
}

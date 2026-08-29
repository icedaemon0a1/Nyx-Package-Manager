//! Integration tests against **real, live-downloaded Arch Linux data**:
//! a genuine Arch bootstrap rootfs's `/var/lib/pacman/local` (installed
//! package metadata as pacman itself wrote it) and a genuine `core.db`
//! sync database, both fetched from Arch's real mirrors. No fixture in
//! this file is hand-authored or mocked; every assertion is checked
//! against files real pacman produced.
//!
//! These tests are marked `#[ignore]` by default because they depend on
//! a specific local sandbox path (`/tmp/archtest`) that is not part of
//! the repository (the real Arch data is multiple hundreds of MB and is
//! fetched separately, not committed). Run explicitly with:
//!
//! ```sh
//! cargo test -p nyx-alpm --test real_arch_data -- --ignored
//! ```

use nyx_alpm::Handle;
use std::path::Path;

const ROOTFS: &str = "/tmp/archtest/rootfs/root.x86_64";

fn skip_if_missing() -> bool {
    !Path::new(ROOTFS).join("var/lib/pacman/local").is_dir()
}

#[test]
#[ignore = "requires the real Arch bootstrap rootfs at /tmp/archtest (not part of the repo)"]
fn local_db_lists_real_installed_packages_from_genuine_arch_rootfs() {
    if skip_if_missing() {
        eprintln!("skipping: {ROOTFS} not present in this environment");
        return;
    }

    let root = Path::new(ROOTFS);
    let dbpath = root.join("var/lib/pacman");

    let handle = Handle::initialize(root, &dbpath).expect("alpm_initialize against real rootfs");
    let local = handle.local_db().expect("local_db");
    assert_eq!(local.name(), "local");

    let packages = local.packages();
    // The real bootstrap image has 139 installed packages at the time
    // this environment was prepared (verified via `ls
    // var/lib/pacman/local | wc -l` counting the ALPM_DB_VERSION file +
    // 138 package dirs -> 138 actual packages). We assert a floor rather
    // than an exact count so this test tolerates a different bootstrap
    // snapshot without becoming a tautology.
    assert!(
        packages.len() > 100,
        "expected >100 real installed packages, got {}",
        packages.len()
    );

    // `acl` is a real package present in every Arch base install; verify
    // its metadata matches what pacman itself wrote to `desc`.
    let acl = local.get_pkg("acl").expect("acl must be installed");
    assert_eq!(acl.name(), "acl");
    assert_eq!(acl.version(), "2.4.0-1");
    assert_eq!(
        acl.desc().as_deref(),
        Some("Access control list utilities, libraries and headers")
    );
    assert_eq!(
        acl.url().as_deref(),
        Some("https://savannah.nongnu.org/projects/acl")
    );
    assert_eq!(acl.arch().as_deref(), Some("x86_64"));
    assert_eq!(acl.build_date(), 1782738989);

    // Files list should be non-empty and contain a real path from acl's
    // actual file manifest.
    let files = acl.files();
    assert!(
        files.iter().any(|f| f.name.contains("libacl.so")),
        "acl's file list should contain libacl.so; got {:?}",
        files.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
}

#[test]
#[ignore = "requires the real Arch bootstrap rootfs at /tmp/archtest (not part of the repo)"]
fn search_finds_real_package_by_name_substring() {
    if skip_if_missing() {
        eprintln!("skipping: {ROOTFS} not present in this environment");
        return;
    }

    let root = Path::new(ROOTFS);
    let dbpath = root.join("var/lib/pacman");
    let handle = Handle::initialize(root, &dbpath).expect("alpm_initialize");
    let local = handle.local_db().expect("local_db");

    let results = local.search(&["^acl$"]).expect("alpm_db_search");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name(), "acl");
}

#[test]
#[ignore = "requires the real Arch bootstrap rootfs at /tmp/archtest (not part of the repo)"]
fn check_deps_reports_no_missing_deps_for_the_real_installed_set() {
    if skip_if_missing() {
        eprintln!("skipping: {ROOTFS} not present in this environment");
        return;
    }
    let root = Path::new(ROOTFS);
    let dbpath = root.join("var/lib/pacman");
    let handle = Handle::initialize(root, &dbpath).expect("alpm_initialize");
    let local = handle.local_db().expect("local_db");
    let packages = local.packages();

    // The real bootstrap's installed set was produced by real pacman,
    // so checking it against itself (no additions/removals) must not
    // report any missing dependency -- this exercises the real
    // alpm_checkdeps FFI call end-to-end with real data.
    let missing = handle.check_deps(&packages, &[], &[], false);
    assert!(
        missing.is_empty(),
        "expected no missing deps in a real, pacman-installed set, got: {:?}",
        missing.iter().map(|m| &m.target).collect::<Vec<_>>()
    );
}

#[test]
#[ignore = "requires the real Arch bootstrap rootfs at /tmp/archtest (not part of the repo)"]
fn find_dbs_satisfier_resolves_a_real_provides_name() {
    if skip_if_missing() {
        eprintln!("skipping: {ROOTFS} not present in this environment");
        return;
    }
    let root = Path::new(ROOTFS);
    let dbpath = root.join("var/lib/pacman");
    let handle = Handle::initialize(root, &dbpath).expect("alpm_initialize");
    let local = handle.local_db().expect("local_db");

    // "acl" is both a real installed package name and a real dependency
    // spec string libalpm can resolve against the local db.
    let dbs = [local];
    let satisfier = handle
        .find_dbs_satisfier(&dbs, "acl")
        .expect("acl should be satisfiable from the local db");
    assert_eq!(satisfier.name(), "acl");
}

#[test]
#[ignore = "requires the real Arch bootstrap rootfs at /tmp/archtest (not part of the repo)"]
fn pkg_vercmp_matches_real_alpm_semantics() {
    // alpm_pkg_vercmp does not require a handle at all; exercised here
    // as it is part of the same "real ALPM behaviour, no invented
    // expectations" contract as the other tests in this file.
    // Not yet wrapped in the safe API (pending nyx-resolver work), so we
    // call the raw sys binding directly to prove the FFI link itself is
    // correct end-to-end.
    use nyx_alpm::sys;
    use std::ffi::CString;
    let a = CString::new("1.2.0-1").unwrap();
    let b = CString::new("1.10.0-1").unwrap();
    let rc = unsafe { sys::alpm_pkg_vercmp(a.as_ptr(), b.as_ptr()) };
    assert!(rc < 0, "1.2.0-1 should sort before 1.10.0-1, got rc={rc}");
}

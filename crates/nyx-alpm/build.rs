//! Locates the system libalpm via pkg-config and generates raw FFI
//! bindings from the *actual installed* `alpm.h`/`alpm_list.h` with
//! `bindgen`, rather than hand-transcribing constants/structs. This keeps
//! nyx-alpm honest about which libalpm ABI it was built against (15.x in
//! this Debian dev sandbox; 16.x/17.x on a real current Arch host) and
//! means a header change is caught at build time, not silently ignored.
//!
//! Only the subset of the ALPM API that Phase 0/1 of Nyx actually uses is
//! allow-listed. This keeps the generated surface small and reviewable
//! instead of pulling in libalpm's entire (large) public API, most of
//! which (sandboxing hooks, XferCommand callbacks, hook events, ...) is
//! not exercised until later phases.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=wrapper.h");

    let lib = match pkg_config::Config::new().probe("libalpm") {
        Ok(lib) => lib,
        Err(err) => {
            panic!(
                "nyx-alpm requires libalpm to be discoverable via pkg-config \
                 (`pkg-config --cflags --libs libalpm`) but probing failed: {err}. \
                 On Arch install the `pacman` package (or `libalpm-dev` on this \
                 Debian dev sandbox); nyx-alpm's ALPM compatibility layer links \
                 against the system libalpm rather than vendoring/reimplementing it."
            );
        }
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .allowlist_type("alpm_handle_t")
        .allowlist_type("alpm_db_t")
        .allowlist_type("alpm_pkg_t")
        .allowlist_type("alpm_list_t")
        .allowlist_type("alpm_depend_t")
        .allowlist_type("alpm_conflict_t")
        .allowlist_type("alpm_depmissing_t")
        .allowlist_type("alpm_errno_t")
        .allowlist_type("alpm_siglevel_t")
        .allowlist_type("alpm_pkgreason_t")
        .allowlist_type("alpm_pkgvalidation_t")
        .allowlist_type("alpm_pkgfrom_t")
        .allowlist_type("alpm_depmod_t")
        .allowlist_type("alpm_trans_flag_t")
        .allowlist_type("alpm_filelist_t")
        .allowlist_type("alpm_file_t")
        .allowlist_type("alpm_backup_t")
        .allowlist_type("alpm_db_usage_t")
        .allowlist_type("alpm_caps")
        // Lifecycle
        .allowlist_function("alpm_initialize")
        .allowlist_function("alpm_release")
        .allowlist_function("alpm_errno")
        .allowlist_function("alpm_strerror")
        .allowlist_function("alpm_capabilities")
        .allowlist_function("alpm_version")
        // Options (root/dbpath are set only via alpm_initialize; there is
        // no setter for them post-init in this libalpm API version)
        .allowlist_function("alpm_option_get_dbpath")
        .allowlist_function("alpm_option_get_root")
        .allowlist_function("alpm_option_get_lockfile")
        .allowlist_function("alpm_option_add_cachedir")
        .allowlist_function("alpm_option_set_default_siglevel")
        .allowlist_function("alpm_option_get_default_siglevel")
        .allowlist_function("alpm_option_set_logfile")
        // Databases
        .allowlist_function("alpm_get_localdb")
        .allowlist_function("alpm_register_syncdb")
        .allowlist_function("alpm_get_syncdbs")
        .allowlist_function("alpm_db_update")
        .allowlist_function("alpm_db_get_name")
        .allowlist_function("alpm_db_get_pkg")
        .allowlist_function("alpm_db_get_pkgcache")
        .allowlist_function("alpm_db_set_usage")
        .allowlist_function("alpm_db_search")
        // Packages
        .allowlist_function("alpm_pkg_get_filename")
        .allowlist_function("alpm_pkg_get_name")
        .allowlist_function("alpm_pkg_get_version")
        .allowlist_function("alpm_pkg_get_desc")
        .allowlist_function("alpm_pkg_get_url")
        .allowlist_function("alpm_pkg_get_builddate")
        .allowlist_function("alpm_pkg_get_installdate")
        .allowlist_function("alpm_pkg_get_packager")
        .allowlist_function("alpm_pkg_get_arch")
        .allowlist_function("alpm_pkg_get_size")
        .allowlist_function("alpm_pkg_get_isize")
        .allowlist_function("alpm_pkg_get_reason")
        .allowlist_function("alpm_pkg_get_licenses")
        .allowlist_function("alpm_pkg_get_groups")
        .allowlist_function("alpm_pkg_get_depends")
        .allowlist_function("alpm_pkg_get_optdepends")
        .allowlist_function("alpm_pkg_get_conflicts")
        .allowlist_function("alpm_pkg_get_provides")
        .allowlist_function("alpm_pkg_get_replaces")
        .allowlist_function("alpm_pkg_get_files")
        .allowlist_function("alpm_pkg_get_backup")
        .allowlist_function("alpm_pkg_get_base")
        .allowlist_function("alpm_pkg_should_ignore")
        .allowlist_function("alpm_pkg_load")
        .allowlist_function("alpm_pkg_free")
        .allowlist_function("alpm_pkg_vercmp")
        .allowlist_function("alpm_dep_compute_string")
        .allowlist_function("alpm_filelist_contains")
        // Dependency / conflict resolution
        .allowlist_function("alpm_find_satisfier")
        .allowlist_function("alpm_find_dbs_satisfier")
        .allowlist_function("alpm_checkdeps")
        .allowlist_function("alpm_checkconflicts")
        .allowlist_function("alpm_depmissing_free")
        .allowlist_function("alpm_conflict_free")
        // Transactions
        .allowlist_function("alpm_trans_init")
        .allowlist_function("alpm_trans_prepare")
        .allowlist_function("alpm_trans_commit")
        .allowlist_function("alpm_trans_release")
        .allowlist_function("alpm_trans_get_add")
        .allowlist_function("alpm_trans_get_remove")
        .allowlist_function("alpm_add_pkg")
        .allowlist_function("alpm_remove_pkg")
        .allowlist_function("alpm_sync_sysupgrade")
        .allowlist_function("alpm_sync_get_new_version")
        // alpm_list_t helpers (declared in alpm_list.h, bundled by alpm.h)
        .allowlist_function("alpm_list_free")
        .allowlist_function("alpm_list_free_inner")
        .allowlist_function("alpm_list_count")
        .allowlist_function("alpm_list_nth")
        .allowlist_function("alpm_list_next")
        .allowlist_function("alpm_list_previous")
        .allowlist_function("alpm_list_last")
        // Layout: use libc's types for size_t/off_t/time_t rather than
        // duplicating glibc definitions.
        .size_t_is_usize(true)
        .derive_default(true)
        .derive_debug(true)
        .layout_tests(false)
        .generate_comments(true);

    for path in &lib.include_paths {
        builder = builder.clang_arg(format!("-I{}", path.display()));
    }

    let bindings = builder
        .generate()
        .expect("failed to generate libalpm bindings from the installed alpm.h");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("failed to write generated libalpm bindings");

    // Re-emit link flags pkg-config already determined so cargo doesn't
    // need us to guess `-lalpm`.
    for path in &lib.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
    for lib_name in &lib.libs {
        println!("cargo:rustc-link-lib=dylib={lib_name}");
    }
}

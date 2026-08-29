//! Locates the system libalpm via pkg-config so nyx-alpm links against
//! whatever ALPM the host provides (libalpm 15 in this Debian dev sandbox;
//! libalpm 16/17 on a real current Arch host). No FFI bindings are
//! generated yet in this PR — see docs/adr/0001 "Remaining work" — this
//! build script only establishes the link step so the crate compiles as
//! an empty placeholder without a hand-maintained `-lalpm` guess.

fn main() {
    match pkg_config::Config::new().probe("libalpm") {
        Ok(_) => {
            // pkg-config already emitted the correct cargo:rustc-link-lib
            // and search paths.
        }
        Err(err) => {
            // Do not hard-fail the whole workspace build in this
            // placeholder stage if libalpm.pc is absent (e.g. minimal CI
            // containers); nyx-alpm has no real FFI calls yet. Real FFI
            // bindings (Phase 0 remaining work) must make this fatal.
            println!(
                "cargo:warning=libalpm not found via pkg-config ({err}); nyx-alpm is a placeholder in this PR"
            );
        }
    }
}

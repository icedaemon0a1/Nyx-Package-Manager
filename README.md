# Nyx — Arch Linux Package Manager (Phase 0 in progress)

Nyx is a from-scratch, production-oriented replacement for the pacman
package-management stack on Arch Linux. See
[`docs/adr/0001-phase0-phase1-foundation.md`](docs/adr/0001-phase0-phase1-foundation.md)
for the architecture decision record covering scope, the libalpm
compatibility strategy, the no-database storage design, and the
development/verification environment (real libalpm, real pacman, a
real live-downloaded Arch bootstrap rootfs — nothing mocked).

## Status

This PR delivers the **first slice of Phase 0 (Foundation)**:

* Rust workspace laid out per the target crate structure.
* `nyx-core` — complete and tested:
  * stable, script-facing error model (`ErrorCategory` + exit codes)
  * terminal output abstraction (quiet-by-default UX, TTY/`NO_COLOR`
    detection, `--color auto|always|never` semantics)
  * Nyx filesystem path layout (`/etc/nyx`, `/var/lib/nyx`,
    `/var/cache/nyx`, plus legacy pacman interop paths), parameterised
    over a root so tests never touch the real filesystem
  * BLAKE3 content hashing with bounded-memory streaming file hashing
    and CAS shard-path derivation
  * atomic file writes (temp file → write → fsync → rename), with
    tests asserting no partial file is ever left at the final path
* `nyx-alpm` — real, working ALPM compatibility layer over the
  system's actual `libalpm.so` (no reimplementation, no mocking):
  * `build.rs` locates `libalpm` via `pkg-config` and generates raw
    FFI bindings from the **actually installed** `alpm.h` with
    `bindgen`, allow-listing only the Phase 0/1 subset of the API.
  * `sys` — the raw FFI boundary (bindgen output), the only `unsafe`
    entry point in the crate.
  * `error::AlpmError` — typed wrapper over `alpm_errno_t`/
    `alpm_strerror`, convertible into `nyx_core::NyxError`.
  * `list::AlpmList` — safe, read-only iteration over libalpm's public
    `alpm_list_t`, with ownership-safety documentation distinguishing
    borrowed vs. caller-owned lists.
  * `handle::Handle` — RAII wrapper over `alpm_handle_t`
    (`initialize`/`Drop`→`alpm_release`, `root`/`dbpath`/`lockfile`/
    `capabilities`/`version`, `add_cachedir`/`set_logfile`,
    `local_db`/`register_syncdb`/`sync_dbs`/`update_dbs`).
  * `db::Db` — borrowed view of a local/sync database (`name`,
    `get_pkg`, `packages`, `search`).
  * `pkg::Package` — borrowed package record with typed accessors for
    every field Phase 0/1 needs (name/version/desc/url/arch/dates/
    sizes/reason/licenses/groups/depends/optdepends/conflicts/
    provides/replaces/backup/files).
  * Unit tests (`cargo test -p nyx-alpm`) run against the real system
    libalpm 15.0.0 using disposable `tempfile` roots — no mocked ALPM
    data anywhere.
  * Integration tests (`cargo test -p nyx-alpm --test real_arch_data
    -- --ignored`) run against the genuine, live-downloaded Arch
    bootstrap rootfs's real `/var/lib/pacman/local` package metadata
    (`acl`, real installed-package count, real file lists) — ignored
    by default since the ~120MB rootfs is a local sandbox fixture, not
    checked into the repo.
* `nyx-config`, `nyx-resolver`, `nyx-transaction`, `nyx-cli` crates
  are scaffolded (manifest + workspace wiring) but **not yet
  implemented** — see "Remaining work" below. They are intentionally
  left as placeholders rather than filled with mocked logic, per this
  project's explicit "no fake success paths" rule.

Run the tests that exist so far:

```bash
cargo test -p nyx-core
cargo test -p nyx-alpm
cargo test -p nyx-alpm --test real_arch_data -- --ignored   # needs /tmp/archtest rootfs
```

## Development/verification environment

This sandbox runs Debian 13, not Arch. To avoid mocking ALPM
semantics, development is verified against real artifacts:

* Debian's `libalpm-dev`/`libalpm15` (15.0.0) — an actual build of
  libalpm from the real `pacman-package-manager` source package,
  with real `/usr/include/alpm.h`.
* The real `pacman` 7.0.0 binary, installed as the golden-comparison
  reference.
* A genuine, live-downloaded Arch Linux bootstrap rootfs
  (`geo.mirror.pkgbuild.com`), chrootable, containing real
  `pacman 7.1.0` / `libalpm 16.0.1` and a real `/etc/pacman.conf`.
* A real Arch `core.db` sync database downloaded from the live
  mirror network, used as real package metadata for search/info
  testing (not hand-invented fixtures).

See the ADR for full detail and the rationale for each decision.

## Remaining work (not yet done in this PR)

* `nyx-config`: implement the layered TOML engine (defaults →
  `/etc/nyx/nyx.toml` → `conf.d`/`repos.d`/`policies.d` → user config
  → env → CLI) with per-value provenance tracking via `toml_edit`
  spans, plus the `nyx config get/set/unset/list/list --effective/
  validate/reset/explain` subcommands.
* `nyx-resolver`: dependency/conflict/provides/replaces resolution
  built on `nyx-alpm`'s `alpm_checkdeps`/`alpm_checkconflicts`/
  `alpm_find_dbs_satisfier`.
* `nyx-transaction`: immutable manifest struct + atomic
  read/write/list using `nyx-core::atomic::write_atomic`.
* `nyx-cli`: `install`, `remove`, `update`, `search`, `info`, `list`,
  `files`, `owns` wired to the above, with `--json`, `--color`,
  `--quiet`, `--verbose` global flags.
* Integration tests against the real Arch bootstrap chroot (already
  verified reachable/usable in this sandbox — see ADR) for install/
  remove/update against a real ALPM local database.
* Golden comparison tests vs the installed `pacman`/`vercmp` for
  version comparison and dependency resolution.
* Benchmark harness (`nyx --help`, warm search/info) vs `pacman`.
* A concrete remaining-incompatibilities report once Phase 1 command
  surface is functional end-to-end.

## Non-goals for this PR

Per the project brief, Phase 2 onward (transactions/rollback
persistence beyond the manifest format, security scanning, sandbox,
AUR, build tooling, privileged commit daemon) are out of scope until
Phase 1 is a reliable, real package manager. No crates or CLI
subcommands for those phases are stubbed out here.

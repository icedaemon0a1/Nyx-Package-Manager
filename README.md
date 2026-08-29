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
* `nyx-config`, `nyx-alpm`, `nyx-resolver`, `nyx-transaction`,
  `nyx-cli` crates are scaffolded (manifest + workspace wiring) but
  **not yet implemented** — see "Remaining work" below. They are
  intentionally left as placeholders rather than filled with mocked
  logic, per this project's explicit "no fake success paths" rule.

Run the tests that exist so far:

```bash
cargo test -p nyx-core
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
* `nyx-alpm`: `bindgen`-generated FFI over the system `libalpm.so`
  (verified reachable via `pkg-config --cflags --libs libalpm` in
  this sandbox — libalpm 15.0.0), wrapped in a safe, typed API
  (`Handle`, `Db`, `Package`, typed `alpm_errno_t` errors, safe
  `alpm_list_t` iteration), with all `unsafe` confined to a small
  `sys`/`ffi` boundary module with documented invariants.
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

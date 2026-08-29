# ADR 0001 — Phase 0 / Phase 1 Foundation

Status: Accepted (implemented)
Date: 2026-08-29

## Context

Nyx is a from-scratch, production-oriented replacement for the pacman
package-management stack on Arch Linux. The full target architecture
(security scanning, sandboxing, transactional rollback, AUR, build
tooling, privileged commit daemon, etc.) is large. Building all of it
at once produces a pile of crates that half-compile and never get
exercised against a real system.

This ADR documents the decisions made to build **Phase 0 (Foundation)**
and **Phase 1 (real pacman-replacement core)** only, and to make every
piece of it actually work against a real Arch environment rather than
mocked data.

## Environment used for development/verification

This sandbox is Debian 13 (trixie), not Arch. To develop and verify
against real ALPM semantics instead of fabricated ones, the following
real artifacts were used (no mocking):

- `libalpm-dev` / `libalpm15` 15.0.0 from Debian's `pacman-package-manager`
  source package — a real, redistributed build of the actual libalpm
  C library and headers (`/usr/include/alpm.h`, `/usr/include/alpm_list.h`,
  `libalpm.pc`). This is the same libalpm Arch ships, packaged for Debian.
- `pacman` 7.0.0 (same source package) installed on the host as the
  **golden reference implementation** for comparison testing.
- A real, current **Arch Linux bootstrap rootfs**
  (`archlinux-bootstrap-x86_64.tar.zst`, fetched live from
  `geo.mirror.pkgbuild.com`) extracted to a disposable directory and used
  via `chroot` as a genuine Arch filesystem — real `/etc/pacman.conf`,
  real `pacman 7.1.0` / `libalpm 16.0.1`, real package layout.
- A real Arch **sync repository database** (`core.db`) downloaded live
  from the official mirror network, containing real `desc` files in the
  actual ALPM text format used for `nyx search` / `nyx info` testing.
- `archlinux-keyring` (Debian package) providing the real Arch Linux
  PGP trusted/revoked keyring files, for future signature-verification
  work.

None of the package metadata, dependency graphs, or `desc` files used
in tests are hand-invented. They come from the live Arch repositories.

## Decisions

### D1: Rust workspace, minimal crate set for Phase 0/1

Only the crates actually needed for Phase 0/1 are created. Crates for
security scanning, sandboxing, rollback backends, AUR, and build
tooling are **not** stubbed out — they simply don't exist yet. Adding
an empty crate "for the architecture diagram" is explicitly rejected
per the project brief.

Crates created now:

```
nyx-core        error model, output/UX abstraction, path constants, hashing
nyx-config      layered TOML configuration engine
nyx-alpm        safe Rust wrapper over the system libalpm C library (FFI)
nyx-resolver    dependency/conflict/provides resolution built on nyx-alpm
nyx-transaction immutable transaction manifest read/write (atomic fsync+rename)
nyx-cli         binary: clap-based command surface
```

`nyx-repo`, `nyx-download`, `nyx-scan`, `nyx-policy`, `nyx-sandbox`,
`nyx-rollback`, `nyx-build`, `nyx-commit`, `nyx-audit`, `nyx-verify`
are deferred to their respective phases (2-6) per the implementation
order in the project brief. Repository *metadata parsing* needed by
Phase 1 (`nyx search`/`nyx info` against sync databases) lives directly
in `nyx-alpm`/`nyx-resolver` for now since it is a thin wrapper over
`alpm_db_update`/`alpm_db_get_pkgcache` rather than a new subsystem —
splitting it into its own crate before there is a second consumer
would be premature.

### D2: libalpm binding strategy — FFI wrapper, not a libalpm reimplementation

The brief requires "a controlled libalpm compatibility layer" and
ALPM-database compatibility, but explicitly warns against declaring
Nyx pacman-compatible before compatibility tests pass, and against
building fake/mocked package-management semantics.

Given the size of libalpm (dependency resolution, versioned
provides/conflicts, signature verification, transaction commit, hook
execution, backup-file handling, `.install` scriptlet execution,
sysupgrade logic — several thousand lines of battle-tested C), a
line-by-line Rust reimplementation is out of scope for Phase 0/1 and
would itself be a source of subtle incompatibility. Phase 0/1 instead:

- Links against the **system libalpm** (soname `libalpm.so.15` in this
  Debian sandbox; the eventual Arch target is `libalpm.so.16`/`17` as
  shipped by current `pacman`).
- Generates raw FFI bindings with `bindgen` against the *actual*
  installed `alpm.h`, not hand-transcribed constants.
- Wraps the raw bindings in a safe, typed `nyx-alpm` crate: RAII handle
  wrapper (`Handle`), typed errors from `alpm_errno_t`, safe iteration
  over `alpm_list_t`, and structured `Package`/`Db` views instead of
  raw pointers leaking into `nyx-cli`.
- All `unsafe` blocks are isolated inside `nyx-alpm::sys` /
  `nyx-alpm::handle`/`nyx-alpm::list`, each with a doc comment stating
  the invariant it relies on (pointer non-null after libalpm success
  check, lifetime tied to `&Handle`, etc.), per the project's FFI
  hygiene requirement.

This is the "controlled libalpm compatibility layer" called for in the
brief: Nyx owns the policy, UX, transaction planning, and eventually
the privileged commit boundary, while the on-disk ALPM database format
and low-level transaction mechanics are delegated to the real,
already-correct implementation instead of being reinvented and risking
subtle incompatibility. This can be revisited in Phase 5 when
replacing the libalpm ABI itself becomes the goal.

`nyx-alpm` targets whatever `libalpm.pc`/`alpm.h` is present via
pkg-config at build time (`build.rs`), so it is portable between this
Debian dev sandbox (libalpm 15) and a real Arch target (libalpm
16/17) without source changes, as long as the C API used remains
stable (it has been ABI-stable across these versions for the subset
Nyx uses: init/release, db register/update, pkg accessors, checkdeps,
trans lifecycle).

### D3: No database — filesystem state only, exactly as specified

- Nyx-managed state paths: `/etc/nyx`, `/var/lib/nyx`, `/var/cache/nyx`.
- ALPM database path defaults to `/var/lib/nyx/alpm` (configurable),
  with the classic `/var/lib/pacman` layout supported as an alternate
  root for interop/testing (Nyx can point libalpm at either).
- Transactions are immutable, atomically-written files under
  `/var/lib/nyx/transactions/NNNNNNNNNN.nyx`, written via
  temp-file → write → fsync → rename, exactly as specified. `nyx-transaction`
  implements this and nothing else (no history index yet — that is a
  Phase 2 concern once rollback/crash-recovery is being built; the
  directory of manifests is itself already the source of truth and
  browsable).
- No SQLite/Postgres/Redis/Mongo anywhere in the dependency tree.
  Verified: `cargo tree` contains no database crate.

### D4: Configuration engine

`nyx-config` implements the exact layering specified in the brief:

```
compiled defaults
  -> /etc/nyx/nyx.toml
  -> /etc/nyx/conf.d/*.toml (sorted)
  -> /etc/nyx/repos.d/*.toml (sorted, repo table merge)
  -> /etc/nyx/policies.d/*.toml (sorted)
  -> ~/.config/nyx/config.toml
  -> environment (NYX_<SECTION>_<KEY>)
  -> CLI overrides
```

Every value tracks its provenance (`ConfigSource`: `Default`, or
`File { path, line }`, or `Env`, or `Cli`) so that `nyx config explain
KEY` can report the exact file and line that set the effective value,
per the brief's explicit example. This is implemented with
`toml_edit`'s span-tracking parser rather than plain `toml`/`serde`,
specifically so line numbers are real, not approximated.

`nyx config get/set/unset/list/list --effective/validate/explain`
mutate/query the *user* layer (`~/.config/nyx/config.toml`) by default
(matching "environment/config.toml" precedence — `set` never silently
writes to `/etc`). Parse errors surface filename+line+column via
`toml_edit`'s error type, per requirement.

### D5: CLI shape

`clap` (derive) + `anstream`/`anstyle` for colour, matching the
"normal Linux tool, no dashboards/emojis/ascii-art" requirement.
`--color auto|always|never`, `--quiet`, `--verbose`, `--json` (where
implemented) are global flags. TTY detection (`std::io::IsTerminal`)
disables progress/interactive prompts when stdout is piped.

Implemented now: `install`, `remove`, `update`, `search`, `info`,
`list`, `files`, `owns`, `config get/set/unset/list/explain/validate`.
Everything else in the full command surface (`inspect`, `audit`,
`chamber`, `protect`, ... ) is Phase 2+ and intentionally absent rather
than stubbed with a "not implemented" placeholder command, per the
"no TODO-based success paths" rule — an absent subcommand is more
honest than a fake one.

### D6: No persistent daemon

Nothing in Phase 0/1 starts a background process. `nyx` is a single
short-lived CLI invocation that opens libalpm, does its work, and
exits. This is trivially true right now since the privileged
socket-activated commit helper (`nyx-commit`) does not exist yet
(Phase 2+); Phase 1 runs privileged operations in-process when invoked
as root, exactly like `pacman` does today, which is an accepted,
explicitly-scoped simplification for this phase.

### D7: Testing strategy

Given no root/Arch machine is available in this sandbox, Phase 0/1
verification uses:

1. **Golden comparison against real pacman**: the same real
   `core.db` behavior is exercised through both `nyx` (via libalpm)
   and the installed `pacman` binary where the two can be run
   side-by-side against the same libalpm database (queries: search,
   info, version compare). Version comparison in particular is tested
   against `vercmp`/`alpm_pkg_vercmp` output on real Arch version
   strings pulled from the live `core.db`.
2. **Chroot integration test against a genuine, live-downloaded Arch
   bootstrap rootfs** for full install/remove/update transaction
   testing where mutation of a real ALPM local database is required.
3. Unit tests for the config layering/precedence and transaction
   atomic-write/fsync/rename behavior (including simulated
   interruption: kill before rename must never leave a partial
   manifest visible under the final name).

## Consequences

- Nyx's Phase 1 correctness is bounded by libalpm's correctness for
  the operations it delegates (dependency resolution, conflict
  detection, transaction commit, hooks, scriptlets, signature
  verification). This is intentional: re-deriving all of that
  correctly from scratch before Nyx has ever installed a real package
  would be a worse risk than depending on the reference implementation
  during this phase.
- Because this development sandbox is Debian, not Arch, `nyx-alpm`
  links whatever `libalpm.so` pkg-config finds (15.x here). The code
  makes no version-specific assumptions beyond the documented stable
  API subset; moving to a real Arch host (libalpm 16/17) requires no
  source changes, only a rebuild, which is exactly the portability
  the "controlled compatibility layer" is meant to provide.
- `nyx-alpm`/`nyx-cli` are GPL-2.0-or-later, matching pacman/libalpm's
  license, since `nyx-alpm` links against and is a derivative
  interface to libalpm (see Licensing section of the brief).

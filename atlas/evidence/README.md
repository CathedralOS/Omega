# Reproduce the evidence

These files describe the tracked Rust workspace at the revision in
[data/manifest.json](data/manifest.json). They support the Atlas plan; they do
not prove that a declared dependency is removable or that a compiler stage
runs on an ordinary build route.

From the repository root, with Python 3.10 or newer and the repository's Cargo
toolchain installed:

```bash
python3 -m unittest discover -s atlas/evidence/tests -v
python3 atlas/evidence/scripts/regenerate.py --source-revision 3f9ce43f0c9d179048911d18faa91ba93f60176d
```

The generator extracts a full Git archive of the chosen revision, verifies
each tracked blob against its Git object identity, and runs
`cargo metadata --format-version 1 --no-deps --locked` in that temporary tree.
It does not build the compiler or switch the checkout. Cargo configuration,
the lockfile and toolchain file come from the archive. The pinned tree supplies
the tracked-file inventory, so newer checkout files and generated or untracked
files cannot enter an older snapshot.

The chosen commit must exist locally and its Cargo toolchain must be available.
A shallow or fresh clone may need `git fetch origin <source-revision>` first;
the failure message names the missing revision.
Missing commits, omitted or substituted archive blobs, and source symlinks
stop the run. Gitlink contents are outside the inventory; the snapshot's
`samples/apps/squalr` gitlink is not a Cargo workspace package or dependency.
Omitting `--source-revision` records `HEAD`. `--output-dir /tmp/atlas-evidence`
writes a comparison copy. The output contains no checkout paths or generation
times. A second run at the same source with the same scanners must produce
identical files.

The archive adapter preserves the seven evidence payloads from the earlier
scan at `3f9ce43f0c9`; its manifest records the updated scanner hashes and source
policy. Scanner repairs change that provenance even when payloads stay the
same. Reproduce the current eight files together; do not carry forward old
scanner hashes after changing a scanner.

| File | Meaning |
| --- | --- |
| `manifest.json` | Source revision, exact metadata command, scanner SHA-256 hashes, output hashes, counting policy and limits. |
| `graph.json` | Workspace package paths and Cargo path dependency declarations. Every edge records its kind, rename, optional flag and target condition. |
| `counts.json` | `packages[name]` has `path`, `rust_files`, `all_rs_lines` and `src_rs_lines`; `groups` aggregates foundation, backend, realization and the workspace. |
| `users.txt` | Foundation dependency users and their directory-based positions. |
| `backend_users.txt` | Backend counts under both line-count scopes, plus declared user positions. |
| `stage_dependencies.txt` | Non-stage crates declaring normal or build dependencies on numbered stages. These edges are not a call graph. |
| `usage.json`, `usage_be.json` | Textual foundation/backend references: first path segment (`usage`), first two segments (`usage2`), caller positions (`pos`), files examined (`scanned_file_counts`), and `unobserved_dependencies`. |

## What the numbers count

- **Line counts:** physical lines in Git-tracked `.rs` files, including blank
  lines, comments, tests, benches, examples and build scripts. A final line
  without a newline still counts. Nested crates own their files, counted once.
  `src_rs_lines` is the `src/` subtree, which can also contain tests; it does
  not mean production-only code. Cargo's actual output directories are excluded
  by full path. The source crate `omega-rust/omega/compilation/backend/target/`
  is included.
- **Dependencies:** declarations from real `cargo metadata --no-deps`, limited
  to workspace path dependencies. “Production” means normal plus build
  dependencies. Development dependencies are separate. Optional and
  target-conditional declarations are included; this is not a resolved graph
  for one feature/target selection. The same crate pair can have more than one
  declaration.
- **References:** a small textual Rust scanner over tracked files owned by
  workspace crates. It excludes `tests/`, `benches/`, `examples/` relative to
  each crate, conventional test filenames (`tests.rs`, `*_test.rs`,
  `*_tests.rs`, `test_*.rs`, `source_guards.rs`), and exact `#[cfg(test)]` items.
  It masks comments and literals, observes Cargo-renamed imports, simple crate
  aliases, nested import groups and glob presence. `*` means a glob was seen;
  `<crate>` means a bare crate import was seen. Neither identifies individual
  imported items.

The scanner does not expand macros, evaluate feature/target cfg expressions,
resolve shadowing or glob contents, or follow reexports to their original
crate. It can miss uses and count inactive code. A reference is evidence that
source names a crate, not proof of execution. An `unobserved_dependencies`
entry is **unverified**, not “unused”: review the source and run appropriate
Cargo checks before removing a declaration. This replaces the misleading old
`declared_unused` key. The old backend scanner also defaulted to foundation
unless `SCAN_DIR` was set; the backend wrapper now always selects backend.

## Focused queries

For a fresh metadata file and individual reports **from the current checkout**:

```bash
cargo metadata --format-version 1 --no-deps --locked > /tmp/omega-meta.json
python3 atlas/evidence/scripts/modscan.py /tmp/omega-meta.json /tmp/usage.json
python3 atlas/evidence/scripts/modscan_be.py /tmp/omega-meta.json /tmp/usage_be.json
python3 atlas/evidence/scripts/users.py /tmp/omega-meta.json
python3 atlas/evidence/scripts/backend_users.py < /tmp/omega-meta.json
python3 atlas/evidence/scripts/stage_dependencies.py < /tmp/omega-meta.json
python3 atlas/evidence/scripts/sub.py atlas/evidence/data/usage.json language-core float_operations
python3 atlas/evidence/scripts/report.py atlas/evidence/data/usage.json /tmp/omega-meta.json language-core --by-file
```

`report.py` prints candidate definition locations. It keeps multiple matches
visible and excludes the same test source as the usage scan. Candidates from
macros, reexports and repeated names still need manual inspection. Focused
scripts alone do not refresh the manifest; use `regenerate.py` when replacing
the saved snapshot.

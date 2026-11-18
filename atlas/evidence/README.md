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

## Verify the four matching native hosts

The [native execution checkpoint](checkpoints/native-host-execution-f658.json) records the actual OS and CPU, job URL, format, image size, SHA-256 and exit for each compiler’s program. The official compiler used commit `f658f30ec966`. The migration compiler used the integration containing `3d14fe87dff4`, with the upstream merge at `683122d180a9`. Its binary digest pins the build. The jobs at proof commit `edc830ed21a3` ran those published files on their matching operating systems. [Workflow and logs](https://github.com/CathedralOS/Omega/actions/runs/44227027634) · [Committed fixture images](https://github.com/CathedralOS/Omega/blob/edc830ed21a353efd63ac94e19503c7224e13a01/native-host-proof/images.json) · [Execution code](https://github.com/CathedralOS/Omega/blob/edc830ed21a353efd63ac94e19503c7224e13a01/native-host-proof/verify.py#L44-L69)

From the repository root, run the evidence checker with GitHub CLI access to the job logs and the cited Git objects available locally.

```bash
python3 atlas/evidence/scripts/check_native_hosts.py
```

The checker reads each process result from its job log, verifies the complete set of eight executions and checks the image bytes against the proof commit. To match the original local build products as well, pass `--products /path/to/acceptance-f658-products`. This also checks both compiler binaries, the fixture source hashes, every official library file and all eight locally published images. The [host proof readme](https://github.com/CathedralOS/Omega/blob/edc830ed21a353efd63ac94e19503c7224e13a01/native-host-proof/README.md) gives the build command.

## Compare the two published compiler builds

The [target execution record](checkpoints/target-execution-4650.json) contains the exact compiler SHA-256 values, Git revisions, fixture hashes, toolchain library tree and results for the selected-provider fixture. It compares official build `46502a522a16` with migration build `fcb284a45c40`. The arm64 Mach-O result records offline signature verification. Execution on Darwin was *unverified* for those two recorded builds.

To replay, set `BASELINE_OMEGA` and `CANDIDATE_OMEGA` to the absolute paths of those executables. Check their hashes with `sha256sum "$BASELINE_OMEGA" "$CANDIDATE_OMEGA"`. Run the following from a checkout containing the official commit. A new temporary directory gives both compilers the same source root and separate output directories.

```bash
repo=$(git rev-parse --show-toplevel)
scratch=$(mktemp -d)
baseline=46502a522a162975887c11f07a2c6abe02ebdcab
fixture=tests/fixtures/run/providers/target_selected_implementation
git -C "$repo" archive "$baseline" source/library "$fixture" | tar -x -C "$scratch"
export OMEGA_LIBRARY_ROOT="$scratch/source/library" RAYON_NUM_THREADS=1
cd "$scratch/$fixture"

replay() {
    "$1" --build-dir "$2" --target linux_x86_64 --target linux_arm64 \
        --target macos_arm64 --target windows_x86_64 \
        --build-input build.omg --build-input main.omg main.omg || return
    for target in linux_x86_64 linux_arm64 macos_arm64 windows_x86_64
    do
        "$1" run --engine psi --target "$target" \
            --build-input build.omg --build-input main.omg main.omg || return
    done
}
replay "$BASELINE_OMEGA" "$scratch/baseline"
replay "$CANDIDATE_OMEGA" "$scratch/candidate"
python3 "$repo/atlas/evidence/scripts/check_target_execution.py" \
    "$scratch/baseline" "$scratch/candidate" --run
```

For the ordinary dual-engine observation, run `"$BASELINE_OMEGA" run --both --build-input build.omg --build-input main.omg main.omg` from the fixture and repeat with `"$CANDIDATE_OMEGA"`. Each printed native and interpreter exit 0. Adding `--target linux_x86_64` selected compilation only on these builds.

The checker compares both sets of executable bytes with the record and verifies the Mach-O entry point, signed code pages and special slots. It flips the entry byte in memory and checks that the signature refuses it. With `--run`, Linux x86-64 runs directly, Linux arm64 under `qemu-aarch64`, and Windows x86-64 under `wine64`. Pass `--qemu` or `--wine` for a different runner path. Omitting `--run` checks the executable hashes and Mach-O signature only.

The implementation checkpoint also cites the compiler's declaration ledger. To count the records and files at its source revision:

```bash
git show 0e5d56643021b7633553af23cfede3c93021e1d5:omega-rust/pipeline-contracts.json |
    python3 -c 'import json, sys; rows = json.load(sys.stdin); print(len(rows["items"]), len(rows["roots"]))'
```

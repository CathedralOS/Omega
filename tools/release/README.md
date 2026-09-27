# Release tooling

## GitHub draft prereleases

`.github/workflows/release.yml` builds and validates Windows x64, macOS ARM64,
and Linux x64/ARM64 on separate native runners. Dispatch **Release bundles**
on `main` with a new tag such as `v0.1.0-alpha.1`, or push that prerelease tag
for a commit already on `main`. It accepts only explicit alpha/beta/rc versions.

Every build checks out the same exact commit and runs the packager below. Only
after all four pass does the publication job verify the complete asset set,
create the tag if needed, and create a **draft prerelease**. Four fresh runners
then download its actual GitHub assets, verify hashes and exact identities, and
repeat the installed-product smoke with no toolchain on PATH or preexisting
package cache. A final job records successful verification in the draft notes.
The release stays draft for human review; the workflow never publishes it publicly.

```sh
gh workflow run release.yml --ref main -f tag=v0.1.0-alpha.1
gh run list --workflow release.yml
```

Existing releases and conflicting tags reject, never overwrite. Failure before
publication creates no release; failure afterwards leaves a draft explicitly
marked pending validation. Fixes use a new prerelease version rather than moving
a tag or replacing distributed bytes. Build jobs have read-only repository
permissions; only draft creation and verification-note update have write access.
Action dependencies are pinned by commit. These jobs do not implement installers
or change `omega update`, which remains the package-update command.

The hosted matrix is an installation test, not the compiler-completion matrix.
Release notes identify its tested hosts and known Windows Git-cache limitation.
Broader OS compatibility, code signing, dependency-license review and installer
channels remain release-distribution work, not claims made by the smoke tests.

## Build a relocatable bundle

From a clean checkout, on the host you are distributing:

```sh
python3 tools/release/package.py --version 0.1.0-alpha.1 --output build/releases
```

On Windows use `python` with the same arguments. Requirements are the pinned
Rust toolchain, its native linker, Python 3.9+, and Git. No new release tag or
GitHub publication is performed. The script uses `mbx` when available, Cargo
otherwise, and builds the host executable with `--locked --release`.
Windows MSVC bundles statically link the C runtime. Other hosts retain their
ordinary OS library dependencies; host/minimum-OS release support still requires
testing on those systems.

The archive contains `bin/omega` (`omega.exe` on Windows), the matching `.omg`
sources under `lib/omega/library/`, licenses, and `manifest.json`. Compiler
sources, package caches, tests, and the Rust toolchain are not shipped. The
manifest identifies version, source commit, Rust host/compiler, library digest,
and each bundled file's SHA-256. The archive has a sibling `.sha256` file.
Hashes identify content; neither they nor this packaging smoke test certify
compiler completeness or authenticate a publisher.

Before publishing its local output, the command unpacks the archive into a new
directory containing spaces, checks the manifest and `--version`, checks source
using the bundled library, and compiles/runs a native program that exits 42.
The smoke process has an empty `PATH` and fresh temporary user/cache directories,
so local compilation cannot rely on the producer's Rust toolchain, linker, or
existing package snapshots. The fixed native fixture exercises `omega update`
and accepts its generated local-library review decisions before compilation;
this is not a dependency audit or automatic acceptance policy for user projects.
Negative controls remove the installed library and corrupt one source: both must
reject rather than use the build checkout. POSIX runs also exercise an executable
symlink. The output is a ZIP on Windows and a tar.gz elsewhere. Existing archive
names are not overwritten. A failed smoke test produces no final archive.

`--allow-dirty` is for developing the packager, not publishing a release: it
marks both the version and manifest dirty. Normal packaging requires a clean
checkout. The smoke suite is installed-product coverage, not the eight-gate
compiler-completion matrix below. Run the same command on each supported host;
a Windows result does not establish a macOS/Linux pass.

### Runtime library discovery

An installed compiler resolves its real executable location, including symlinks,
then reads `../lib/omega/library` relative to `bin`. Release builds contain no
checkout fallback and compare that library's exact `.omg` tree against an embedded
digest. Keep compiler and library together when moving or upgrading them.

For explicit deployment/embedding, set `OMEGA_LIBRARY_ROOT` to an absolute
library directory before starting Omega, or call
`build_evaluation::sources::toolchain_library::configure` before the first
compiler lookup. This selects one immutable process-level root, not an import
search path; release digest checks still apply. Invalid explicit roots reject
without fallback. Ordinary Cargo development builds retain their checkout's
library when no installed bundle or explicit root is present.

These checks catch broken or mixed installations, not malicious mutation by the
host while a process is running. Installing release assets and authenticating
their publisher remain the installer/receiver's responsibility. Git-based package
acquisition still requires host Git; compiling with bundled/local sources does
not require the producer's Rust checkout.

## Release-record substrate

`release_record.py` is the substrate for the release record required by
[the completion contract](../../wiki/drafts/reference/rust_compiler_completion.md#closure-rule):
the record must contain the commit, pinned Rust toolchain, host OS and
architecture, commands, results, elapsed time, and the exact list of expected
skips. The tool owns the eight-gate manifest verbatim, runs a gate's command
block on one host, and writes one `omega-release-record/1` JSON document per
runner row under `records/`.

The substrate records evidence; it does not certify the matrix. A failed
command is recorded as a `fail` gate, an unrun gate or runner row stays
`open`, and the closure reads `closed` only when all eight gates pass and all
four platform runs are recorded on the same commit. `check` re-validates a
stored record so a hand-edited record cannot read as closed.

Requirements: Python 3.9+ and Git. Standard library only; identical under
PowerShell on Windows and sh on macOS/Linux. Gate commands run through `mbx`
when installed and through Cargo otherwise, per AGENTS.md.

Host prerequisites:

- macOS: the pinned nightly's linker (Apple clang 17) cannot parse
  `tbd-version: 4` stubs that list `arm64e.x1-*` architectures — a
  CommandLineTools `MacOSX27.0.sdk` promoted to the default
  `MacOSX.sdk` symlink makes every build-script and binary link fail with
  `tapi error: malformed file ... unknown architecture`.

  Check what the default actually is before changing anything:

  ```sh
  readlink "$(xcode-select -p)/SDKs/MacOSX.sdk"
  ```

  Only the 27.0 stub needs avoiding. Measured 2026-09-27 on a
  CommandLineTools-only host where it resolves to `MacOSX15.5.sdk`: the full
  native corpus leg emits and EXECUTES Mach-O products with `SDKROOT` unset,
  under the pinned `nightly-2026-09-04` / `rustc 1.100.0-nightly (a69a63265)`.
  No override is needed there.

  When the default IS the 27.0 stub, point `SDKROOT` at an SDK that is not:

  ```sh
  export SDKROOT="$(xcode-select -p)/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk"   # Xcode
  export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX15.5.sdk                     # no Xcode
  ```

  The first path exists only with Xcode installed; a CommandLineTools-only
  host has no `Platforms/` directory at all, so the second form is the one
  that applies there.

  Deep parser/checker threads also need `RUST_MIN_STACK=67108864` — the
  measured suites on this host (corpus legs, native differential) ran under
  it.

## Recording a runner row

Each runner row is a *lane*: the contract requires the emitted programs to
be executed directly on the matching host, so `run` must be invoked on a
host that can execute the lane's target, and must carry the direct-execution
observation for that lane:

```text
python3 tools/release/release_record.py run --target linux_x86_64 \
    --native-execution "python3 tools/corpus_gate.py --native"
```

`--native-execution` is the lane's native-observation evidence: one command
that runs the emitted programs natively and exits 0 on success. Its command,
exit, elapsed time, and bounded output tail are stored on the lane's
`platform_runs` row; the row reads `recorded` only when it exits 0. A lane
that fails its observation still writes the record — the row stays `open`.
On a host that cannot execute the lane at all, `run` refuses rather than
writing a record that cannot carry the lane.

PowerShell uses the same invocation with `python`. Restrict to one gate with
`--gate RC-REPOSITORY` (repeatable). A red gate still writes a record — the
record is the evidence, open rows and all — so `run` exits 0 after writing
and reports `closure: open` with the open rows.

The `linux_arm64` lane additionally accepts any host when the record names
the emulator (the contract's "emulation is acceptable only when the release
record names the emulator and version" allowance); the observation command
runs through that emulator:

```text
python3 tools/release/release_record.py run --target linux_arm64 \
    --emulator "qemu-aarch64 9.0.0" --gate RC-NATIVE-MATRIX \
    --native-execution "qemu-aarch64 <path-to-emitted-elf>"
```

## Expected skips

A platform test may skip only when irrelevant to its runner. Declare each
expected skip while recording:

```text
--expect-skip '<gate>|<skipped test path>|<why this runner cannot run it>'
```

Skipped tests observed in nextest/cargo output that are not declared this way
land in `unlisted_skips` and keep the closure `open` — the contract requires
the *exact* list.

## Validating a record

```text
python3 tools/release/release_record.py check records/linux_x86_64__*.json
```

`check` exits nonzero when the schema drifts, a required field is missing, a
gate's recorded command no longer matches the contract, a `recorded`
platform row lacks a passing direct-execution observation or its lane's
matching host, an emulator appears on a non-arm64 row, or the recorded
closure contradicts the rows.

`tools/tests/test_release_record.py` guards the substrate itself: the gate
manifest must stay verbatim-equal to the completion document's command
blocks, and committed records under `records/` must re-validate.

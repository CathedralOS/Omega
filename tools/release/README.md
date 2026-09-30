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
package cache. A separate fresh-cache leg uses host Git to acquire the public
repository's `omega_language_std` package at the release commit, approve the
fixed fixture's audit, then compile and run offline without changing its lock.
A final job records successful verification in the draft notes.
The release stays draft for human review; the workflow never publishes it publicly.

```sh
gh workflow run release.yml --ref main -f tag=v0.1.0-alpha.1
gh run list --workflow release.yml
```

Existing releases and conflicting tags reject, never overwrite. Failure before
publication creates no release; failure afterwards leaves a draft explicitly
marked pending validation. Fixes use a new prerelease version rather than moving
a tag or replacing distributed bytes. Build jobs have read-only repository
permissions. Draft creation, verification-note update and draft downloads need
write access: GitHub hides drafts from read-only credentials. Download jobs
expose the token only in the download step; the subsequent installed-compiler
smoke has no release token and checkout does not persist Git credentials.
Action dependencies are pinned by commit. These jobs do not implement installers
or change `omega update`, which remains the package-update command.

The hosted matrix is an installation test, not the compiler-completion matrix.
Release notes identify its tested hosts and verified source-acquisition paths.
Broader OS compatibility, code signing, final license review and installer
channels remain release-distribution work, not claims made by the smoke tests.

## Direct Windows installer

`windows-installer.yml` wraps existing verified Windows release archives in a
per-user MSI. It does not rebuild the compiler. MSI owns the whole bundle under
`%LOCALAPPDATA%\Programs\Omega`, user PATH registration, transactional major
upgrades, downgrade rejection, and uninstall. No custom actions or recursive
directory deletion are authored; user projects and caches remain outside it.

Dispatch from any development host with GitHub CLI:

```sh
gh workflow run windows-installer.yml --ref main \
    -f previous_tag=v0.1.0-alpha.2 -f tag=v0.1.0-alpha.3 \
    -f previous_installer_version=0.1.2 -f installer_version=0.1.3
```

MSI versions are an **independent monotonically increasing installer sequence**:
three integers bounded by `255.255.65535`. They are not the compiler's SemVer
version. Never reuse a published installer version for different bytes, including
a packaging-only change. The product's display name and `omega --version` retain
the full compiler version. Keep the UpgradeCode stable across releases.

The disposable Windows job installs the old MSI, builds a Git-backed native
program returning 42, upgrades the whole bundle, rebuilds the same project
offline without rewriting its lock, rejects a downgrade, and uninstalls. It
checks exact installed manifests and restoration of unrelated PATH entries.
Only a successful job uploads the new MSI/checksum as `windows-msi`; attach
those exact files to the release after reviewing `windows-msi-evidence`. The
workflow does not automatically publish or overwrite release assets.

Authoring is Python-portable; MSI construction needs Windows, .NET, and pinned
WiX 4.0.6 installed locally (not globally):

```powershell
dotnet tool install wix --version 4.0.6 --tool-path build/wix
python tools/release/windows_installer.py --bundle EXTRACTED_VERIFIED_BUNDLE --installer-version 0.1.3 --output build/msi --wix build/wix/wix.exe
```

The builder verifies the bundle manifest, rejects dirty/non-Windows bundles,
and refuses an existing output directory. The hosted check also verifies the
input archive checksums and their exact release-tag commits. WiX 4 is pinned
to its existing open-source licensing; upgrading the packaging dependency is
an explicit review, not a floating tool update. Installers remain unsigned.

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

`licenses/third-party/` contains an inventory and license/notice files from the
host-filtered Cargo dependency closure of `omega` (including build tools, excluding
dev-only edges), plus the pinned Rust standard library's copyright inventory and
license texts. Nested vendored notices are retained. This is conservative input
coverage, not a claim that every listed crate contributes bytes to the executable.
Missing text or an unreviewed license expression stops packaging; the only
supplemental upstream text is version-pinned under `tools/release/licenses/`.
The inventory supports distribution review; it does not replace legal review.

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

Add `--git-smoke` to run the network-dependent Git acquisition leg locally.
The GitHub fresh-download gate always runs it. The ordinary packager remains
usable without contacting a source remote after its Cargo dependencies are present.

`--allow-dirty` is for developing the packager, not publishing a release: it
marks both the version and manifest dirty. Normal packaging requires a clean
checkout. The smoke suite is installed-product coverage, not the eight-gate
compiler-completion matrix below. Run the same command on each supported host;
a Windows result does not establish a macOS/Linux pass.

## Prepare installation channels

After downloading all four bundles and their checksums for one release:

```sh
python3 tools/release/channels.py --assets build/release-assets \
  --output build/release-channels --tag v0.1.0-alpha.3 --commit FULL_SOURCE_COMMIT
```

PowerShell uses the same arguments on one line with `python`. Generation verifies
all bundle identities and checksums before producing `Formula/omega.rb` for a
Cathedral-owned Homebrew tap and a three-file manifest set under `winget/`
for WinGet's community catalog. Validate the whole directory with
`winget validate --manifest <output>/winget`; singleton manifests are not
accepted by that catalog.
The output directory must not exist. These are preparation artifacts: their
download URLs become usable only after the matching GitHub release is public.

Homebrew installs the complete bundle under `libexec` and symlinks only its
command; WinGet's ZIP/portable manifest retains the archive layout and registers
the nested executable. Both declare Git as a dependency. Neither runs Rust or
builds Omega on the user's machine. Upgrades belong to those package managers;
`omega update` remains project dependency review/update.

Before advertising a channel, validate its generated manifest with the actual
manager, install it, run the source/native smoke through its registered command,
upgrade from a prior version, and repeat. WinGet community publication additionally
requires its normal manifest review; generating YAML is not catalog acceptance.
The Homebrew tap is for experimental releases, not a submission to Homebrew/core.
Code-signing/notarization require publisher credentials and are not implied by
manifest validation or archive checksums.

`.github/workflows/install-upgrade.yml` tests two existing prereleases through
Homebrew on macOS ARM64/Linux x64 and WinGet on Windows Server 2025:

```sh
gh workflow run install-upgrade.yml --ref main \
  -f previous_tag=v0.1.0-alpha.2 -f tag=v0.1.0-alpha.3
```

It downloads draft assets in a credential-bearing step, then drops those
credentials. The installers receive the same generated formula/manifest with
only its download URL redirected to a loopback server holding the verified
bytes. It installs the older release, acquires and reviews the fixed Git-backed
fixture, builds it, upgrades Omega, and rebuilds the **same project and cache**
offline. Both executables must return 42 and the accepted lock must not change.
The installed command must match the exact version and complete bundle manifest.
This proves the local package-manager path, not public catalog discoverability.

The mutation entrypoint is restricted to disposable GitHub Actions hosts and
refuses an existing `omega` command. WinGet local manifests are enabled only on
that disposable host; hashes and archive malware scanning remain enabled.
No local workstation security settings are changed. Successful runs upload a
small result record; failure does not produce a passing record. The separate
workflow lets installer checks be rerun without rebuilding the four compilers.

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
block on one host, and writes one `omega-release-record/2` JSON document per
runner row under `records/`.

The substrate records evidence; it does not certify the matrix. A failed
command is recorded as a `fail` gate, an unrun gate or runner row stays
`open`, and the closure reads `closed` only when all eight gates pass and all
four platform runs are recorded on the same commit. `check` re-validates a
stored record so a hand-edited record cannot read as closed.

An exit status alone does not pass a gate. A gate whose commands all exit 0
reads `pass` only when the record also proves a complete run:

- every skip the runners counted is named (`run` sets
  `NEXTEST_STATUS_LEVEL=skip` so nextest prints the names, and reads
  ignored doc tests from cargo's per-test lines);
- no doc test was filtered out;
- each nextest, doc-test, corpus-gate or sample-refresh command reports at
  least one executed test, crate harness, fixture or sample in its summary,
  so an interrupted run without a summary never passes;
- the checkout was clean (`worktree_clean`) and no inherited
  `OMEGA_CORPUS_FIXTURE_FILTER`, `OMEGA_CORPUS_SHARD`, `NEXTEST_PROFILE` or
  `NEXTEST_RUN_IGNORED` narrowed the selection (`environment_filters`).

Otherwise the gate reads `open`. `check` derives each gate status, the
recorded lane rows and `closure.open_rows` from that stored evidence and
rejects a record whose fields disagree with it.

Requirements: Python 3.9+ and Git. Standard library only; identical under
PowerShell on Windows and sh on macOS/Linux. Gate commands run through `mbx`
when installed and through Cargo otherwise, per AGENTS.md.

Host prerequisites:

- macOS: the Apple linker must be at least as new as the SDK it links
  against. Rust's link step runs `cc`, which uses the linker of the active
  developer directory (`xcode-select -p`) and the SDK that
  `xcrun --show-sdk-path` names. The macOS 27.0 SDK's `libSystem.B.tbd`
  and sibling stubs list an `arm64e.x1-macos` architecture; a linker from
  before that SDK (Xcode 26.3 ships `ld-1230.1`) rejects the whole file, so
  every build-script and binary link fails with
  `ld: tapi error: malformed file ... unknown architecture`. The 27.0 SDK
  itself is fine: `ld-27037.1`, shipped by both Xcode 27.0 and
  CommandLineTools 27.0, links against it.

  The mismatch arises on hosts with Xcode and CommandLineTools of different
  generations. Measured 2026-09-28 with Xcode 26.3 selected and
  CommandLineTools 27.0 installed: `xcrun --sdk macosx --show-sdk-path`
  named Xcode's 26.2 SDK, but plain `xcrun --show-sdk-path` named the
  CommandLineTools 27.0 SDK, so the default link paired `ld-1230.1` with the
  27.0 stubs. Compare the two before changing anything:

  ```sh
  ld -v 2>&1 | head -1           # linker of the active developer directory
  xcrun --show-sdk-path          # SDK the default link uses
  ```

  Fix the mismatch at its source: install the Xcode (or select the
  CommandLineTools, `sudo xcode-select -s /Library/Developer/CommandLineTools`)
  whose release matches the newest installed SDK. After installing Xcode 27.0
  on the host above, `mbx build -p omega` and
  `python3 tools/corpus_gate.py --native --filter
  runtime_named_state_remainder_endpoint_exit` built and executed Mach-O
  with `SDKROOT` unset. A CommandLineTools-only host is always matched; one
  measured 2026-09-27 at `MacOSX15.5.sdk` ran the full native corpus leg with
  `SDKROOT` unset under the pinned `nightly-2026-09-04` /
  `rustc 1.100.0-nightly (a69a63265)`.

  When the toolchain cannot be updated, point `SDKROOT` at an SDK no newer
  than the linker. Resolve the path through `xcrun` rather than hard-coding a
  version: a hard-coded SDK directory disappears when Xcode updates, and a
  stale `SDKROOT` then breaks every link.

  ```sh
  export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
  ```

  On the measured mismatched host this resolved to the selected Xcode's 26.2
  SDK, the one matching `ld-1230.1`. Confirm with the two commands above
  that the pair agrees before relying on it.

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

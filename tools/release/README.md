# Release tooling

Standalone Python release scripts (`package.py`, `windows_installer.py`,
`channels.py`, `release_record.py`) and draft-release GitHub Actions
workflows have been retired. The live six-gate release matrix and closure rules
are maintained in
[Rust compiler completion](../../wiki/drafts/reference/rust_compiler_completion.md).

This directory retains the compiler's relocatable-bundle contract, supplemental
upstream license texts under `tools/release/licenses/`, and archived host
release records under `tools/release/records/`.

## Build a relocatable bundle

A relocatable Omega distribution places the host executable and bundled
standard library in one self-contained directory tree produced with the pinned
Rust toolchain (`rust-toolchain.toml`) via `mbx`:

- `bin/omega` (`bin/omega.exe` on Windows): host compiler CLI executable
  (Windows MSVC bundles statically link the C runtime; POSIX hosts retain their
  standard OS library dependencies).
- `lib/omega/library/`: the matching bundled `.omg` standard library tree
  (`source/library/`).
- `licenses/`: third-party license and copyright notice texts matching the
  host dependency closure and pinned Rust standard library. Any supplemental
  upstream notice text kept in-repo lives under `tools/release/licenses/`
  (see `tools/release/licenses/README.md`).
- `manifest.json`: identifies version, source commit, Rust host/compiler,
  bundled library digest, and file SHA-256 digests.

Installed compiler verification requires running with an empty `PATH` and isolated
user/cache directories so compilation does not depend on the producer's Rust
checkout, and running the target executable natively on each host in
[the completion matrix](../../wiki/drafts/reference/rust_compiler_completion.md#release-matrix).

### Runtime library discovery

An installed compiler resolves its real executable location, including symlinks,
then reads `../lib/omega/library` relative to `bin`. Release builds contain no
checkout fallback and compare that library's exact `.omg` tree against an embedded
digest. Keep compiler and library together when moving or upgrading them.

For explicit deployment/embedding, set `OMEGA_LIBRARY_ROOT` to an absolute
library directory before starting Omega, or call
`build_settings::sources::toolchain_library::configure` before the first
compiler lookup. This selects one immutable process-level root, not an import
search path; release digest checks still apply. Invalid explicit roots reject
without fallback. Ordinary development builds via `mbx` retain their checkout's
library when no installed bundle or explicit root is present.

These checks catch broken or mixed installations, not malicious mutation by the
host while a process is running. Installing release assets and authenticating
their publisher remain the installer/receiver's responsibility. Git-based package
acquisition still requires host Git; compiling with bundled/local sources does
not require the producer's Rust checkout.

## Host linker and environment notes

- **macOS:** the Apple linker must be at least as new as the SDK it links
  against (`ld -v` vs `xcrun --show-sdk-path`). If Xcode and CommandLineTools
  differ in generation (for example `ld: tapi error: malformed file ... unknown architecture`),
  align the selected developer directory with `sudo xcode-select -s ...` or set
  `export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"` when the toolchain
  cannot be updated.
- **Parser/checker stack depth:** set `RUST_MIN_STACK=67108864` on hosts whose
  default thread stack is smaller than deep AST/type recursion needs.

## Licenses and archived records

- `tools/release/licenses/`: version-pinned supplemental third-party license
  texts (`tools/release/licenses/README.md`).
- `tools/release/records/`: retained historical `omega-release-record/3` JSON
  artifacts (such as `windows_x86_64__5dbe86d816__20261105T043454Z.json`).
  Historical records retain their original commit, toolchain, and open-gate
  evidence; they are not promoted as fresh validation.

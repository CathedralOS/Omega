# Release tooling

The standalone Python release scripts (`package.py`, `windows_installer.py`,
`channels.py`, `release_record.py`) and draft-release GitHub Actions
workflows were retired. The six release gates and their closing rules live in
[Rust compiler completion](../../wiki/drafts/reference/rust_compiler_completion.md).

This directory keeps the relocatable bundle layout, extra upstream license
texts and old host release records (see the last section).

## Build a relocatable bundle

A relocatable Omega distribution is one self-contained directory, built with
`mbx` and the pinned Rust toolchain (`rust-toolchain.toml`):

- `bin/omega` (`bin/omega.exe` on Windows): the compiler executable. Windows
  MSVC bundles link the C runtime statically; POSIX hosts keep their standard
  OS library dependencies.
- `lib/omega/library/`: the matching bundled `.omg` standard library tree
  (`source/library/`).
- `licenses/`: third-party license and copyright notices for the host
  dependencies and the pinned Rust standard library. Extra upstream notices
  kept in the repository live under `tools/release/licenses/`
  (see `tools/release/licenses/README.md`).
- `manifest.json`: version, source commit, Rust host and compiler, bundled
  library digest, and each file's SHA-256 digest.

To verify an installed compiler, run it with an empty `PATH` and separate
user and cache directories, so it cannot use the producer's Rust checkout,
and run the compiled executable natively on each host in
[the completion matrix](../../wiki/drafts/reference/rust_compiler_completion.md#release-matrix).

### Runtime library discovery

An installed compiler finds its real executable path, following symlinks,
and reads `../lib/omega/library` relative to `bin`. Release builds never fall
back to a checkout and compare that library's exact `.omg` tree with an
embedded digest. Move or upgrade both together.

To deploy or embed the compiler, set `OMEGA_LIBRARY_ROOT` to an absolute
library directory before starting Omega, or call
`package_compilation::toolchain_library::configure` before the first
compiler lookup. This fixes one library root for the process; it is not an
import search path, and release digest checks still apply. An invalid
explicit root is rejected with no fallback. Development builds made with
`mbx` use their checkout's library when no installed bundle or explicit root
exists.

These checks catch broken or mixed installations, not tampering while a
process runs. The installer or receiver must install release
assets and check who published them. Fetching Git packages still needs host Git; compiling bundled or local sources does not need the
producer's Rust checkout.

## Host linker and environment notes

- **macOS:** the Apple linker must be at least as new as the SDK it links
  against (`ld -v` vs `xcrun --show-sdk-path`). If Xcode and CommandLineTools
  differ in generation (for example `ld: tapi error: malformed file ... unknown architecture`),
  select the matching developer directory with `sudo xcode-select -s ...`, or
  set `export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"` when the
  toolchain cannot be updated.
- **Parser/checker stack depth:** set `RUST_MIN_STACK=67108864` on hosts whose
  default thread stack is too small for deep syntax-tree and type recursion.

## Licenses and archived records

- `tools/release/licenses/`: version-pinned extra third-party license texts
  (`tools/release/licenses/README.md`).
- `tools/release/records/`: old `omega-release-record/3` JSON records
  (such as `windows_x86_64__5dbe86d816__20261105T043454Z.json`). Each keeps
  its original commit, toolchain and open-gate evidence; it is history, not
  fresh validation.

# Local testing with nextest

Use `mbx nextest run` for Rust unit and integration tests. This is entirely
local; GitHub stores code and landing coordination only. `cargo-nextest` 0.9.140
or newer and Python 3.9+ are prerequisites. Check `mbx nextest --version`;
install nextest using its [official instructions](https://nexte.st/docs/installation/pre-built-binaries/)
if absent. Cargo still builds the binaries through mbx. Nextest does not run
doctests; use `mbx test --doc` when doctest coverage is needed.

## Focused development

```sh
mbx nextest run -p compiler --test canary_suite entry_and_abi::pass_canaries_compile
mbx nextest run --workspace --lib --no-fail-fast -E 'rdeps(=x86-encoding)'
```

The second command keeps workspace feature unification and selects a crate plus
its reverse dependencies. It is a manual development filter; the automated
selector below additionally accounts for known source readers.

## Rechecking a verified base

Supply the exact commit whose baseline you already verified. Do not substitute
the latest `origin/main` merely because someone pushed it. These commands work
in PowerShell with `python`; use `python3` on macOS:

```sh
python tools/test_affected.py --base VERIFIED_COMMIT --plan
python tools/test_affected.py --base VERIFIED_COMMIT
```

The JSON plan reports the resolved base, changed paths, selected packages,
fallback reasons, and exact argument arrays. Git compares the base tree with
current working files, including staged, unstaged, deleted, moved, and untracked
files. It compares the whole delta, including incoming main changes; it does not
use a merge-base that could omit incoming changes. Invalid references fail.

Only `.rs` changes under a known workspace crate's `src/` select narrowly.
The runner expands declared reverse dependencies (including dev, build,
optional, and platform dependencies) and known source-reader edges. For example,
`terminal-codec` embeds verifier and proof sources outside its own directory.
Architecture tests always run because they inspect repository layout and source.
Manifests, build scripts, lockfiles, toolchain/config changes, shared fixtures,
Omega library sources, docs, tooling, and unknown paths select all libraries.
No change skips only the library phase, with `none()` explicitly in the plan.
A selection containing only binary crates may have zero library tests; nextest
reports that explicitly and succeeds. Full runs still reject an unexpectedly
empty suite. The separate integration requirements continue to apply.

This is conservative dependency-based selection, not a formal independence
proof. Any new cross-crate source/fixture reader must be represented in
`SOURCE_READERS` or force a full run. Review that map when changing the codec's
source closure. Arbitrary runtime filesystem reads cannot be inferred reliably
from Cargo metadata. Narrow selection assumes the same toolchain, host, feature
flags, environment and external inputs as the verified baseline. If these have
changed, or baseline evidence is missing, use `--full`.

The selector replaces only architecture/library test commands during a recheck.
Formatting, Clippy, workspace checking, and relevant integration/bootstrap gates
still apply. It does not certify a base, cache results, hide failures, or grant
a landing reservation. Review the plan and retain its output with test results.
Do not edit the worktree during a run. Both phases run even if architecture
fails, and any failure produces a nonzero exit status.

## Full portable test baseline

```sh
python tools/test_affected.py --full
```

Equivalent explicit commands:

```sh
mbx nextest run --locked -p omega-architecture-test --all-targets --no-fail-fast
mbx nextest run --locked --workspace --lib --no-fail-fast
```

This retains the existing library-only baseline. Platform runtime integration
tests remain separate and must report skips explicitly. Existing ignored tests
stay ignored; nextest retries are disabled and failures remain visible.
Resource-limit tests reserve nextest's whole pool because their former Rust
mutex cannot serialize separate processes. Long package fixture tests receive
earlier scheduling priority; no universal thread cap is imposed.

For timing evidence and the limits of the Windows experiments, see
[testing_performance.md](testing_performance.md). Nextest adoption alone is not
a measured full-suite speedup. The expected saving is avoiding unrelated test
execution; this selector deliberately does not split workspace compilation.

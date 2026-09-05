# Rust Compiler Completion Contract

Status: governing release contract. This document defines when the maintained
Rust implementation may be called complete enough for Omega self-hosting to
resume. It does not assert that the contract is currently closed.

“100% done” is not a useful measure. Crate counts, test counts, encoded carrier
counts, and lines of code do not establish a working language product. Rust
compiler completion means that every capability row below closes on one exact
revision.

## Scope

This contract covers the Rust implementation from authored `.omg` source
through checked semantics, canonical Terminal Psi, and the four hosted native
products currently claimed by the repository:

- Linux x86-64;
- Linux AArch64;
- macOS AArch64; and
- Windows x86-64.

Freestanding EFI work remains a separately stated target milestone until it is
promoted into this hosted matrix. Optimization quality, the Alpha-through-Delta
bootstrap implementations, and the Omega-written compiler are separate
programs of work. They cannot compensate for a failed row here, and unfinished
bootstrap work does not by itself keep a passing Rust product open.

The accepted language surface is the checked source behavior documented by the
language guide and exercised by the repository's positive, negative, and
runtime fixtures at the release revision. A feature described as accepted must
have an exercising fixture. A deliberately fenced or experimental feature must
reject explicitly and is not silently counted as accepted.

## Release matrix

| Gate | Capability that must be true | Automated invocation |
| --- | --- | --- |
| `RC-REPOSITORY` | The pinned toolchain formats, lints, type-checks, and preserves architectural dependency boundaries. | The baseline command block below. |
| `RC-SOURCE-SEMANTICS` | Every accepted positive fixture reaches its promised checked or product stage; every negative fixture rejects; individual semantic integration tests pass. | `mbx test -p compiler --all-targets` |
| `RC-PCC-REPLAY` | Canonical semantics and proof bytes round-trip, hostile or substituted evidence rejects, and independent verification precedes interpretation or Omega lowering. | `mbx test -p checked-trees-to-terminal-psi -p terminal-codec -p terminal-verifier -p terminal-interpreter -p terminal-psi-to-abstract-operations` |
| `RC-PORTABLE-PSI` | One process publishes a complete source-free Terminal Psi envelope and exits; a second process reconstructs, verifies, and interprets it using newly supplied authority. | `mbx test -p compiler --test canary_suite portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary -- --exact` |
| `RC-BUILD-AND-PACKAGES` | Build declarations, immutable inputs, package identities, reviewed evidence, resolution, and compilation handoff agree without path/name inference or hidden ambient mutation. | The package/build command block below. |
| `RC-NATIVE-MATRIX` | Each hosted target produces independently validated machine code, object/image bytes, ABI behavior, provider settlement, and observable execution on its matching host. | `mbx test -p omega-native-differential-test --all-targets`, plus `RC-SOURCE-SEMANTICS`, on every required host in the platform table below. |
| `RC-DIAGNOSTICS` | Rejected source and failed product admission report stable, actionable diagnostics rather than panics, silent fallback, or accidental acceptance. | `mbx test -p compiler --test canary_suite proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment -- --exact` and the negative cases included by `RC-SOURCE-SEMANTICS`. |
| `RC-REPRESENTATIVE-PROGRAMS` | Every maintained sample reaches checked semantics; every sample with an authored host entry reaches its native product; every documented deterministic exit/output oracle passes. | `mbx test -p compiler --test samples_compile` on every required host. |

`RC-REPOSITORY` is:

```bash
cargo fmt --all -- --check
mbx clippy --workspace --all-targets -- -D warnings
mbx test -p omega-architecture-test --all-targets
mbx check --workspace --all-targets
mbx test --workspace --lib --no-fail-fast
```

`RC-BUILD-AND-PACKAGES` is:

```bash
mbx test \
  -p build-declarations \
  -p build-evaluation \
  -p package-compilation \
  -p package-source \
  -p resolver-execution \
  -p package-evidence \
  -p package-advisory \
  -p package-manager
mbx test -p compiler \
  --test build_config_granted \
  --test build_log_facet \
  --test build_target_activation \
  --test checked_build_machine_identity \
  --test evaluated_via_binding \
  --test package_compilation_inputs
```

## Required platform runs

| Runner | Product identity | Required native observation |
| --- | --- | --- |
| Linux x86-64 | `linux_x86_64` | Directly execute the emitted ELF x86-64 programs. |
| Linux AArch64 | `linux_arm64` | Directly execute the emitted ELF AArch64 programs. Emulation is acceptable only when the release record names the emulator and version. |
| macOS AArch64 | `macos_arm64` | Directly execute the emitted Mach-O AArch64 programs. |
| Windows x86-64 | `windows_x86_64` | Directly execute the emitted PE x86-64 programs. |

Cross-target byte generation on a different host is useful coverage but cannot
replace matching-host execution. A platform test may skip only when it is
irrelevant to that runner. A missing runner, unavailable runtime dependency,
unexpected ignored test, timeout, or resource exhaustion leaves the row open;
it is not a pass.

## Closure rule

The contract closes only when all eight named gates pass from a clean checkout
of the same commit and all four required platform runs are recorded. The
release record must contain the commit, pinned Rust toolchain, host OS and
architecture, commands, results, elapsed time, and the exact list of expected
skips. There is no partial percentage and no averaging between rows.

Any accepted language or product change must update an existing gate's corpus,
or deliberately revise this finite matrix in the same change. Adding a crate,
an evidence carrier, or an isolated unit test without exercising a matrix
capability is not completion progress.

A regression reopens its row. A flaky, prohibitively slow, or routinely
resource-exhausting gate is also open until repaired or replaced by equal or
stronger deterministic coverage. Deleting or weakening a gate requires an
explicit contract revision; it cannot be treated as a successful run.

Once every row closes, remaining Rust work is maintenance and differential-
compiler support. Self-hosting may resume without deleting the Rust compiler:
the maintained Rust implementation remains an independent comparison compiler.

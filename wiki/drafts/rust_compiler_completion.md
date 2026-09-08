# Rust compiler completion

Temporary acceptance plan for finishing the Rust reference compiler before
resuming Omega self-hosting. All eight gates and four hosted runs below remain
required; this document does not claim they pass. Delete the plan after closure,
retaining the release evidence and maintained regression gates with their owners.
It defines readiness for this migration, not language semantics or routine
development landing requirements.

## Scope

This contract covers the Rust implementation from authored `.omg` source
through checked semantics, canonical Terminal Psi, and the four hosted native
products required for completion:

- Linux x86-64;
- Linux AArch64;
- macOS AArch64; and
- Windows x86-64.

Freestanding EFI work remains a separately stated target milestone until it is
promoted into this hosted matrix. Optimization quality, the Alpha-through-Epsilon
bootstrap implementations, and the Omega-written compiler are separate
programs of work. They cannot compensate for a failed row here, and unfinished
bootstrap work does not by itself keep a passing Rust product open.

The accepted surface is defined by the [specification and guide](../README.md),
not inferred from what the compiler happens to accept. Every accepted feature
needs exercising positive, negative, and runtime fixtures as appropriate. An
implementation fence rejects explicitly and cannot silently reduce the promised
completion surface. Unaccepted experiments are not counted as accepted features.

## Release matrix

| Gate | Capability that must be true | Automated invocation |
| --- | --- | --- |
| `RC-REPOSITORY` | The pinned toolchain formats, lints, type-checks, and preserves architectural dependency boundaries. | The baseline command block below. |
| `RC-SOURCE-SEMANTICS` | Every accepted positive fixture reaches its promised checked or product stage; every negative fixture rejects; individual semantic integration tests pass. | `mbx nextest run -p compiler --all-targets --no-fail-fast` |
| `RC-PCC-REPLAY` | Canonical semantics and proof bytes round-trip, hostile or substituted evidence rejects, and independent verification precedes interpretation or Omega lowering. | `mbx nextest run -p checked-trees-to-lowered-psi -p terminal-codec -p terminal-verifier -p terminal-interpreter -p terminal-psi-to-abstract-operations --no-fail-fast`; also run the same package selection with `mbx test --doc`. |
| `RC-PORTABLE-PSI` | One process publishes a complete source-free Terminal Psi envelope and exits; a second process reconstructs, verifies, and interprets it using newly supplied authority. | `mbx nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` |
| `RC-BUILD-AND-PACKAGES` | Build declarations, immutable inputs, package identities, reviewed evidence, resolution, and compilation handoff agree without path/name inference or hidden ambient mutation. | The package/build command block below. |
| `RC-NATIVE-MATRIX` | Each hosted target produces independently validated machine code, object/image bytes, ABI behavior, provider settlement, and observable execution on its matching host. | `mbx nextest run -p omega-native-differential-test --all-targets --no-fail-fast`, plus `RC-SOURCE-SEMANTICS`, on every required host in the platform table below. |
| `RC-DIAGNOSTICS` | Rejected source and failed product admission report stable, actionable diagnostics rather than panics, silent fallback, or accidental acceptance. | `mbx nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment)'` and the negative cases included by `RC-SOURCE-SEMANTICS`. |
| `RC-REPRESENTATIVE-PROGRAMS` | Every maintained sample reaches checked semantics; every sample with an authored host entry reaches its native product; every documented deterministic exit/output oracle passes. | `mbx nextest run -p compiler --test samples_compile --no-fail-fast` on every required host. |

`RC-REPOSITORY` is:

```bash
cargo fmt --all -- --check
mbx clippy --workspace --all-targets -- -D warnings
mbx nextest run -p omega-architecture-test --all-targets --no-fail-fast
mbx check --workspace --all-targets
mbx nextest run --workspace --lib --no-fail-fast
```

`RC-BUILD-AND-PACKAGES` is:

```text
mbx nextest run -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-advisory -p package-manager --no-fail-fast
mbx test --doc -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-advisory -p package-manager
mbx nextest run -p compiler --test build_config_granted --test build_log_facet --test build_target_activation --test checked_build_machine_identity --test evaluated_via_binding --test package_compilation_inputs --no-fail-fast
```

Commands work in PowerShell and a POSIX shell as written. Use Cargo if `mbx` is
unavailable; follow [testing prerequisites](../../AGENTS.md#cargo-wrapper).
Unset canary/sample filters for release coverage and retain the exact selected
test set. Nextest does not run doctests; the separate invocations above preserve
that coverage. A filtered or empty run cannot satisfy a full gate.

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

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
| `RC-PCC-REPLAY` | Requested artifact/`.proof` pairs round-trip; hostile or substituted evidence rejects before PCC-required interpretation or lowering. Ordinary output still checks without publishing PCC. | `mbx nextest run -p checked-trees-to-lowered-psi -p terminal-codec -p terminal-verifier -p terminal-interpreter -p terminal-psi-to-abstract-operations --no-fail-fast`; also run the same package selection with `mbx test --doc`. |
| `RC-PORTABLE-PSI` | One process publishes a complete source-free Terminal Psi envelope and exits; a second process reconstructs, verifies, and interprets it using newly supplied authority. | `mbx nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary)'` |
| `RC-BUILD-AND-PACKAGES` | Build declarations, immutable inputs, package identities, reviewed evidence, resolution, and compilation handoff agree without path/name inference or hidden ambient mutation. | The package/build command block below. |
| `RC-NATIVE-MATRIX` | Each hosted target produces independently validated machine code, object/image bytes, ABI behavior, provider settlement, and observable execution on its matching host. | `mbx nextest run -p omega-native-differential-test --all-targets --no-fail-fast`, plus `RC-SOURCE-SEMANTICS`, on every required host in the platform table below. |
| `RC-DIAGNOSTICS` | Rejected source and failed product admission report stable, actionable diagnostics rather than panics, silent fallback, or accidental acceptance. | `mbx nextest run -p compiler --test canary_suite --no-fail-fast --no-tests fail -E 'test(=proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment)'` and the negative cases included by `RC-SOURCE-SEMANTICS`. |
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
mbx nextest run -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager --no-fail-fast
mbx test --doc -p build-declarations -p build-evaluation -p package-compilation -p package-source -p resolver-execution -p package-evidence -p package-manager
mbx nextest run -p compiler --test build_config_granted --test build_log_facet --test build_target_activation --test checked_build_machine_identity --test evaluated_via_binding --test package_compilation_inputs --no-fail-fast
```

Commands work in PowerShell and a POSIX shell as written. Use Cargo if `mbx` is
unavailable; follow [testing prerequisites](../../../AGENTS.md#cargo-wrapper).
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

### Recorded platform runs

#### linux_x86_64 — 2026-09-20 — row open

- Commit: `a0b906db936ae64b24277225423fc460bfd7bba2` (previous record at
  `9684ea54ff70d7318c5501a1e81bc1589f977997`).
- Toolchain: `nightly-2026-09-04` (`rustc 1.100.0-nightly (a69a63265
  2026-09-03)`, cargo `b2e9d5f9d`); cargo-nextest 0.9.144. `mbx` is not
  present in this environment; `cargo` was used directly.
- Host: Linux x86_64 (`x86_64-unknown-linux-gnu`).
- Gate command (`mbx nextest run -p omega-native-differential-test
  --all-targets --no-fail-fast`, cargo equivalent): **compiles and runs**
  at this revision — the custody-fixture drift (`decision_custody.rs` 6↔7
  `Optimization` roster, `ordinary_graph_controls.rs` `Crash` arm) recorded
  at `9684ea54ff` is repaired. Result: **1006 passed (36 slow) / 116
  failed / 1 skipped** of 1122 tests, wall-clock 25m34s (aggregate
  1534s). Failure split by target: `coverage` ~65 (dominated by the
  `Service<R>`-spelling fixture drift — bare `Console` trait in value
  position, same family RC-REPOSITORY-CLOSURE records),
  `recast_views` ~10, `real_fs` ~10, `pipeline_ownership` ~9,
  `terminal_byte_views` ~8, `terminal_psi_record_returns` ~6,
  `terminal_psi_runnable` ~4, `scalar_case_results` ~2,
  `terminal_psi_calls` ~1, `gui_headless` ~1.
- Direct-execution evidence on this host: `cargo nextest run -p compiler
  --test canary_suite -E 'test(/_runs$/)' --no-fail-fast` — 913 selected run
  tests each compile a canary for the host target and directly execute the
  emitted ELF binary through `Command::new`, checking exit code and stdout.
  Result: **143 passed / 770 failed / 0 skipped** (531 non-matching tests
  excluded by the filter expression; every selected test is host-eligible, so
  there are no expected skips). Wall-clock 110m19s (aggregate ~870m).
- Passing executions include `runtime_i8/i16/i64_signed_arith`,
  `runtime_contained_machine`, `runtime_gcd_euclid`,
  `executable_domain_membership_*`, `checked_boundary_*_dispatch`, and
  `runtime_const_array_length` — emitted ELF programs run directly and exit
  with the expected status on this host.
- Dominant failure families (all pre-execution or during emission except the
  named exit mismatches): ~549 canaries fail at `selected ProgramEntry
  establishment rejoins 0 Terminal attachment identities; expected one`;
  `Lowering(Unsupported)` families (whole byte-view parameters, structural
  call custody, wrapping/checked conversion policy realization, nested call
  roots, primitive-projection carriers); default-domain `[u8]::Utf8`
  field-requirement proofs; index-proof refusals; operational-envelope
  acknowledgement mismatches. Genuine execution mismatches (binary emitted
  and ran, wrong exit) — unchanged from the prior record:
  `runtime_shift_signedness`, `runtime_shift_right_atwidth`,
  `const_fold_unsigned_shift_right_arg`, `runtime_bitwise_high_ops`
  (exit 71 where 70 expected).
- Next acceptance: the dominant ProgramEntry-establishment family and the
  `Service<R>` fixture drift in `coverage`; re-run both commands on a
  matching host after those legs land.

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

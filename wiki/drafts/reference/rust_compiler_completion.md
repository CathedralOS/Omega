# Rust compiler completion

Temporary acceptance plan for finishing the Rust reference compiler before
resuming Omega self-hosting. All six gates and four hosted runs below remain
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
| `RC-SOURCE-SEMANTICS` | Every accepted positive fixture reaches its promised checked or product stage; every negative fixture rejects; individual semantic integration tests pass. | `python tools/corpus_gate.py` |
| `RC-BUILD-AND-PACKAGES` | Build declarations, immutable inputs, package identities, reviewed evidence, resolution, and compilation handoff agree without path/name inference or hidden ambient mutation. | The package/build command block below. |
| `RC-NATIVE-MATRIX` | Each hosted target produces independently validated machine code, object/image bytes, ABI behavior, provider settlement, and observable execution on its matching host. | `python tools/corpus_gate.py --native`, which builds every pass and run fixture and executes the run tier and `*_exit` fixtures, on every required host in the platform table below. |
| `RC-DIAGNOSTICS` | Rejected source and failed product admission report stable, actionable diagnostics rather than panics, silent fallback, or accidental acceptance. | `python tools/corpus_gate.py --filter fail/`, which also belongs to `RC-SOURCE-SEMANTICS`. |
| `RC-REPRESENTATIVE-PROGRAMS` | Every maintained sample reaches checked semantics; every sample with an authored host entry reaches its native product; every documented deterministic exit/output oracle passes. | `python tools/sample_native_gate.py` on every required host. |

`RC-REPOSITORY` is:

```bash
python tools/fmt.py --check
mbx clippy --workspace --all-targets -- -D warnings
mbx check --workspace --all-targets
mbx nextest run --workspace --lib --no-fail-fast
```

`RC-BUILD-AND-PACKAGES` is:

```text
mbx nextest run -p build-declarations -p package-source -p resolver-execution --no-fail-fast
mbx test --doc -p build-declarations -p package-source -p resolver-execution
```

Commands work in PowerShell and a POSIX shell as written. Use Cargo if `mbx` is
unavailable; follow [testing prerequisites](../../../AGENTS.md#commands).
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
- Gate command at this revision: the Rust native differential crate's
  `--all-targets` run (cargo equivalent), which was removed on 2026-09-25; the
  gate is now `python tools/corpus_gate.py --native`. It **compiles and runs**
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
  carrier-spelling fixture drift in `coverage`; run the current
  RC-NATIVE-MATRIX gate and the direct-execution command on a matching host
  after those legs land.

#### linux_arm64 — 2026-10-08 — structural references only, row open

This is bounded qualification for **STRUCTURAL-BORROW-IDENTITY**, not an
RC-NATIVE-MATRIX run. The compiler is the Rust reference producer at
`4d7bbd96d2bfc892d2a06a1a051a9eb699a98316`, built with the pinned
`nightly-2026-09-04` toolchain and `CARGO_PROFILE_DEV_OPT_LEVEL=1` on macOS
26.5.1 AArch64. The new fixture accompanies this record; no compiler repair
is inferred from these passing executions.

Execution host: the existing Colima 0.10.1 AArch64 VM, using Apple's
Virtualization.Framework (same-ISA virtualization, not CPU emulation), Ubuntu
24.04.4 LTS, kernel `6.8.0-100-generic`. Each emitted ELF AArch64 image runs
directly in that guest. The VM was stopped before qualification and restored
to that state afterward; no runner image was created.

From the checkout, with its freshly built `omega` on PATH, prepare and compile
each fixture using its authored Build declaration:

```sh
fixture=tests/omega/pass/borrows/borrowed_record_stack_parameter_writeback_exit
omega update --project "$fixture" --target linux_arm64 --dangerously-accept-all
omega --target linux_arm64 --build-dir "$fixture/build/linux-arm64" "$fixture/main.omg"
```

Execute the resulting absolute path with the guest shell, capturing the
program's exit before comparing it with the authored oracle:

```sh
colima ssh -- sh -c '"/absolute/checkout/tests/omega/pass/borrows/borrowed_record_stack_parameter_writeback_exit/build/linux-arm64/omega-program"; observed_exit=$?; printf "native exit %s\n" "$observed_exit"; test "$observed_exit" -eq 70'
```

The assertion wrapper returns 0 on an exact match. Colima itself otherwise
maps a guest's nonzero program exit to host status 1, which is not the
program's exit. Package review uses the normal developer acceptance command;
neither compilation nor execution bypasses proof or custody checks.

All 19 fixtures execute with their authored exits. The four-job sequence of
package preparation, compilation and guest assertions takes 86.44 seconds;
the new fixture's focused macOS native/Terminal gate takes 1.767 seconds.
Paths below are under `tests/omega/pass/borrows/` unless qualified:

| Fixtures | Native exit |
| --- | --- |
| `borrowed_record_stack_parameter_writeback_exit`, `branch_selected_record_receiver_exit`, `returned_record_projected_call_exit`, `nested_reference_crossings_exit` | 70 |
| `linear_owner_materialized_shared_reference_exit`, `linear_owner_materialized_mutable_projection_exit`, `linear_call_result_shared_observation_exit`, `projected_claim_owner_shared_observation_exit` | 70 |
| `local_shared_record_sequence_observations_exit`, `local_reference_scalar_completion_exit`, `returned_owned_byte_field_snapshot_exit`, `local_record_byte_field_snapshot_exit` | 70 |
| `local_record_nested_sequence_extent_exit`, `owner_byte_read_during_shared_record_loan_exit`, `returned_record_field_read_exit`, `nested_reference_byte_observations_exit`, `calls/unread_multistate_receiver_exit` | 70 |
| `static_record_reference_return_exit`, `borrow/runtime_indexed_write_only_local_receiver_exit` | 0 |

The new stack-parameter fixture passes eight readable record pointers and a
ninth mutable record pointer, returns that ninth reference, and checks both
written fields through the returned carrier and the original owner. An
inspection of the emitted AArch64 instructions confirms a real call: the
caller stores the ninth pointer at outgoing stack offset 0; the callee's
16-byte frame reloads it at incoming stack offset 0, writes offsets 0 and 8
through that pointer, and returns the pointer. Stack accesses use a computed
address register, so searching only for loads/stores whose base is `sp` misses
the actual transport. Seeing callee-local values would not prove writeback;
the caller's owner-field checks do.

The inspected artifact is bound to these SHA-256 identities:

- New fixture `main.omg`: `63db1cb5bdc60ad04706c9d9efbb8a8b97a49a49347aea1a04e59e4e1333d7bd`.
- Emitted Linux ELF: `3c4413968d4e15c6e64151db4c8bd25cefbd1e63928da8279f70bea6deb9a4b1`.

The corresponding macOS AArch64 native/Terminal differential gate uses
`python3 tools/corpus_gate.py --both --filter <comma-separated paths above>`
(each path prefixed with `pass/` and its group): 19 built, 19 exact native and
Terminal exit matches. A check gate filtered to
`fail/borrows/{local_record_duplicate_mut_argument,shared_record_byte_write_before_last_observation,materialized_linear_owner_reference_live_write,materialized_reference_does_not_discharge_linear_owner,shared_observation_does_not_discharge_linear_owner}`
rejects all five controls with their expected diagnostic fragments. The new
fixture also exits 70 through its README's public `omega run --both` command.
Linux x86-64 and Windows x86-64 still
need current matching-host qualification. The full Linux AArch64 release row
also remains open: this record is not an unfiltered corpus run and contains
no expected skips.

## Closure rule

The contract closes only when all six named gates pass from a clean checkout
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

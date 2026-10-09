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

#### Borrowed-storage Linux qualification

Recorded 2026-10-08 for **BORROWED-STORAGE-RESTORATION**, from clean producer
`81cfb80f52cca7198ea721e9488aa1315ea7a601`. The pinned
`nightly-2026-09-04` toolchain (`rustc 1.100.0-nightly`, `a69a63265`) built
`omega` with `CARGO_PROFILE_DEV_OPT_LEVEL=1` on macOS 26.5.1 AArch64.
No compiler or fixture change was needed for these executions.

Execution used the existing Colima 0.10.1 VM: Ubuntu 24.04.4 LTS, kernel
`6.8.0-100-generic`, AArch64, Apple's Virtualization.Framework. ARM64 ELF
images executed directly, without CPU emulation. The x86-64 images executed
through `qemu-x86_64` 7.0.0 user-mode emulation in that same guest; those results
do not substitute for matching-host Linux x86-64 qualification. The VM was
stopped before this run and returned to that state afterward.

All 15 programs below met their authored exit on both products: 15 direct
ARM64 observations and 15 emulated x86-64 observations, with no skips or
mismatches. Sequential package preparation, compilation and guest assertions
took 91.603 seconds for ARM64 and 136.835 seconds for x86-64, excluding the
compiler build and VM startup. Paths are under `tests/omega/pass/`.

| Fixture | Authored exit |
| --- | --- |
| `ownership/runtime_borrowed_record_transform_exit` | 42 |
| `ownership/runtime_borrowed_projected_claim_transform_exit` | 42 |
| `ownership/move_keyword_field_assignment` | 41 |
| `borrows/runtime_record_carried_loan_state_return_exit` | 42 |
| `ownership/runtime_carried_loan_field_replacement_exit` | 42 |
| `ownership/runtime_borrowed_carried_loan_roundtrip_exit` | 42 |
| `ownership/runtime_borrowed_shared_alias_carrier_transform_exit` | 42 |
| `ownership/runtime_whole_borrowed_transform_exit` | 42 |
| `ownership/runtime_retained_alias_referent_transform_exit` | 42 |
| `ownership/runtime_borrowed_carried_loan_transform_exit` | 42 |
| `ownership/runtime_borrowed_carrier_repeated_repair_exit` | 42 |
| `ownership/runtime_owned_carried_loan_extract_transform_exit` | 42 |
| `ownership/runtime_captured_replacement_selector_exit` | 0 |
| `ownership/runtime_captured_construction_selector_exit` | 0 |
| `ownership/runtime_captured_selector_call_exit` | 0 |

With the freshly built `omega` on PATH, prepare and compile each fixture using
its authored Build declaration. For example:

```sh
fixture=tests/omega/pass/ownership/runtime_captured_selector_call_exit
omega update --project "$fixture" --target linux_arm64 --dangerously-accept-all
omega --target linux_arm64 --build-dir "$fixture/build/linux-arm64" "$fixture/main.omg"
```

The guest assertion captures the program's exit before comparing it with the
table; it is not Colima's host-side exit mapping:

```sh
colima ssh -- sh -c '"/absolute/checkout/tests/omega/pass/ownership/runtime_captured_selector_call_exit/build/linux-arm64/omega-program"; observed_exit=$?; printf "native exit %s\n" "$observed_exit"; test "$observed_exit" -eq 0'
```

The emulated comparison uses a separately emitted x86-64 product:

```sh
omega update --project "$fixture" --target linux_x86_64 --dangerously-accept-all
omega --target linux_x86_64 --build-dir "$fixture/build/linux-x86-64" "$fixture/main.omg"
colima ssh -- sh -c 'qemu-x86_64 "/absolute/checkout/tests/omega/pass/ownership/runtime_captured_selector_call_exit/build/linux-x86-64/omega-program"; observed_exit=$?; printf "emulated exit %s\n" "$observed_exit"; test "$observed_exit" -eq 0'
```

The selector-call fixture observes exact-once target computation, the captured
destination despite RHS mutation to an out-of-range selector, and the unchanged
sibling. It does not independently count destructor invocations. Its SHA-256
identities are:

- `main.omg`: `0a53de191db449232f6d85cc85b3e3821b782d2c0ac7440e8da21599347b2087`.
- ARM64 ELF: `56eeea4219b5ba25eb180843b48f77deb4daf9e03470266fddc4931ccbebf09a`.
- x86-64 ELF: `adf25ac6371725bdcee65aee0509d5e21989ae83366a7ad9f7779a9259652aa9`.

The 15-program run qualifies the listed ordinary restoration and replacement
programs only.
It does not qualify open windows across recoverable failure, suspension,
resume or cancellation, or all referent shapes. Windows x86-64 and matching-host
Linux x86-64 observations remain missing. Both Linux release rows remain open:
neither run is the unfiltered RC-NATIVE-MATRIX gate.

The follow-on run on 2026-10-08 used clean, published producer
`ab493ee76345e9f4dd0e10eebfe64d81a7e2ff5c`, the same pinned toolchain and
host/guest versions, and `CARGO_PROFILE_DEV_OPT_LEVEL=1`. Building the CLI took
2 minutes 46 seconds. Each of these unchanged programs returned its authored
exit 0 directly on Linux ARM64 and under Linux x86-64 QEMU: eight observations,
no skips or mismatches.

The repository's subsequent history rewrite renamed that producer to
`6d55b46b0d694b8f2170ce4ca960fddc1e6de790`; their Git trees are identical.

- `ownership/runtime_borrowed_case_result_restore_exit`.
- `ownership/runtime_linear_case_payload_forward_exit`.
- `ownership/runtime_borrowed_projected_linear_return_exit`.
- `ownership/runtime_nested_projected_claim_return_exit`.

The same package-preparation, separate target-emission and guest assertion
commands above were used, substituting each fixture path. The Case restoration
program exercises both recoverable outcomes, restores the exact borrowed field
and retains its sibling. The forwarding program also transports an unexamined
sum and a sum projected from an aggregate. Its companion
`fail/ownership/linear_case_result_second_transfer` still rejects repeated
consumption. On this producer, three focused Case programs returned exit 0 in
both macOS native and Terminal execution, and 16 invalid controls rejected with
their expected diagnostic fragments. The VM returned to its initial stopped
state after qualification.

The Case restoration program's SHA-256 identities are:

- `main.omg`: `69eebd06b13ed7fcf3fb26a8789330abbfdc346699cf558e08ad6b35eb182ae7`.
- ARM64 ELF: `810f3fe5e36bfd37e6cabb38fb178e052dee820a7583fba512f3c148ead0070d`.
- x86-64 ELF: `b1fddf0bbaf7bf9acc3ecb832e861137f7cddb3699d235bd76ba57715a12afab`.

This follow-on run covers recoverable Case results, not suspension, resume,
cancellation or every referent shape. Windows x86-64 and matching-host Linux
x86-64 remain unqualified for these four programs; the emulated observations
do not close either release row.

The 2026-10-09 follow-on qualification used clean, published producer
`9b656be387602ca5eed691fff28bbcd217a2cb91`, the same pinned toolchain,
macOS/Colima/Ubuntu/kernel/QEMU versions above, and
`CARGO_PROFILE_DEV_OPT_LEVEL=1`. The fresh CLI build took 2 minutes 13 seconds.
The following five authored programs matched their exits directly on Linux
ARM64 and under Linux x86-64 QEMU: ten observations, no skips or mismatches.
No compiler or fixture change was required for this qualification.

| Fixture under `tests/omega/pass/` | Authored exit |
| --- | --- |
| `ownership/runtime_borrowed_sum_retag_exit` | 0 |
| `ownership/runtime_nested_borrowed_consumption_exit` | 0 |
| `ownership/runtime_borrowed_operand_order_exit` | 0 |
| `ownership/runtime_borrowed_record_transform_exit` | 42 |
| `calls/runtime_qualified_owned_result_argument_exit` | 0 |

The first two restore a consumed sum directly or through nested calls, keeping
the caller's disjoint sibling and exact linear custody. The nested program
uses both projected-field and whole-referent destinations. The ordering
program reads a scalar before extraction and cleanup; the record program
checks both fresh and identity replacements. The final program retains a
declared scalar range when passing an established affine result to another
machine.

Each project was prepared with `omega update --project <fixture> --target
linux_arm64 --target linux_x86_64`. The console-backed record fixture required
acceptance of 570 first-party review decisions across those targets; its local
proposal was resumed with `--dangerously-accept-all`. That is not evidence of
a manual audit or approval of an external dependency. The other four projects
had no pending decisions. Emission used the authored Build declarations:

```sh
omega --target linux_arm64 --target linux_x86_64 \
  --build-dir "$fixture/build/linux-qualification" "$fixture/main.omg"
```

The guest assertions above were applied to the separately emitted products at
`build/linux-qualification/linux_arm64/omega-program` and
`build/linux-qualification/linux_x86_64/omega-program`, using the table's
authored exit. The nested program's SHA-256 identities are:

- `main.omg`: `c4526756da8a58315e820d759ff5c43ffa4bce68789bdea4e5108220a5e3d644`.
- ARM64 ELF: `e15e47316f020ed1926124951a7bac08cb0f7c99acb8d277cf358569f6773aaa`.
- x86-64 ELF: `4615338ec590d0e2391feab662f5b2a4494f7546efe9a4df4f330230ef5a4696`.

The existing VM started stopped and returned to that state. This record covers
ordinary consuming replacements, operand ordering and constrained result
transfer only. It does not qualify suspension, resume, cancellation or every
referent shape. Windows x86-64 and matching-host Linux x86-64 remain open;
emulation does not close either release row, and this was not an unfiltered
Linux ARM64 release gate.

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

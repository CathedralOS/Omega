# RC native matrix — linux_arm64 row

Witnessed row of the release-candidate native matrix for `linux_arm64`,
recorded at revision `6ef64f6dd6` (2026-09-20) from an
`x86_64-unknown-linux-gnu` host, pinned toolchain `nightly-2026-09-04`,
cargo-nextest (mbx unavailable). Cross-emit legs ran natively here; the one
execution leg ran under userspace emulation — `qemu-aarch64-static`
`1:6.2+dfsg-2ubuntu6.31` (qemu-aarch64 6.2.0, Debian), named per the closure
rule that emulated runs must name the emulator and version. Canaries'
`#[cfg(target_arch = "aarch64")]` execution legs skip on this host, so
emulated evidence is reported separately below rather than folded into the
suite counts.

Verdict: **red** — 14 pass / 11 fail across the 25 counted legs. All
failures sit in `aarch64_entry_abi`'s cross-compile fixtures and reduce to
three diagnostic families, none in the entry bridge itself: the hosted
receiver compiles, emits, and (under emulation) executes.

## Baseline gates

| leg | command | result |
|-----|---------|--------|
| Bootstrap topology/path hygiene | `sh tools/bootstrap/check-chain-hygiene.sh` | pass (exit 0) |
| Retired-domain corpus audit | `cargo nextest run -p compiler --test canary_suite -E 'test(=surface_and_targets::retired_domain_when_surface_is_absent_from_authored_corpus)'` | pass (4.0s) |

## Native legs (canary_suite, `-p compiler --test canary_suite`)

| leg | filter | result |
|-----|--------|--------|
| Hosted receiver, aarch64 | `test(/hosted_receiver_linux_arm64/)` (3 tests) | 3/3 |
| AAPCS64 entry ABI | `test(/aarch64_entry_abi/)` (11 tests) | 0/11 |
| Entry calling policy | `cargo nextest run -p compiler --test calling_policy_plans -E 'test(/^linux_entry::/)'` (covers linux_x86_64 + linux_arm64 surfaces) | 6/6 |
| Source-evaluated arm64 hosted receiver | `cargo nextest run -p compiler --test source_evaluated_native_realization -E 'test(/arm64/)'` (2 tests) | 2/2 |

- `hosted_receiver_linux_arm64` is cross-emit-only on this host: both
  runtime legs compile the authored `Service<Console>` receiver to an ELF
  and print `SKIP: hosted receiver runtime requires Linux ARM64`, since
  `#[cfg(target_arch = "aarch64")]` gates process execution. The bare
  `Console` field leg fails closed with the `Service<R>`-carrier rejection
  as specified.
- `linux_entry` policy legs evaluate both surfaces' calling plans; the three
  arm64 legs are green.

## Emulated execution leg (not a suite count)

| leg | command | result |
|-----|---------|--------|
| `cli_mvp` default selection compile | `omega --timings --target linux_arm64 samples/cli/basics/cli_mvp/main.omg` | pass — `published native output` (ELF 64-bit LSB aarch64, statically linked), 1279539 ms |
| `cli_mvp` run under named emulator | `qemu-aarch64-static 6.2.0 <executable>` with stdin `/dev/null` | pass — stdout `Hello, Omega.` + `[press Enter to close]` prompt, exit 0 |

This is the first recorded execution of an emitted aarch64 artifact for
this row. A native `aarch64-unknown-linux-gnu` host re-run remains the
closure evidence: emulation does not cover kernel ABI paths the emulator
short-circuits, and the suite's `#[cfg]`-gated runtime legs still wait on
real hardware.

## Failure families (`aarch64_entry_abi`, 11 legs)

1. **Exact-arithmetic `u64` proof obligation** (5 legs:
   `large_aggregate_entry_loads_a_stack_passed_pointer`,
   `small_aggregate_entry_falls_wholly_to_the_stack`,
   `large_aggregate_entry_copies_from_the_indirect_pointer`,
   `small_aggregate_entry_spreads_consecutive_x_registers`,
   `wide_aggregate_entry_uses_general_indirect_classification`). Checking
   rejects `operands are not provably in range (decision 17)` — fixture
   spelling, same family the x86-64 row saw respelled onto bounded
   operands.
2. **Borrowed-storage result transfer** (3 legs:
   `small_result_entry_loads_x0_and_x1`,
   `large_result_entry_saves_x8_and_copies_through_it`,
   `hfa_result_entry_loads_d0_d1_and_d2`). `cannot transfer a non-copy
   value out of borrowed storage without replacing its owner` — the
   storage-field construction respell the x86-64 row landed has not been
   applied to these aarch64 fixtures.
3. **Entry selection refuses param/result fixtures** (3 legs:
   `aggregate_literal_entry_result_uses_native_fragments`,
   `indexed_scalar_entry_result_uses_native_registers`,
   `hfa_entry_argument_spreads_vector_registers`).
   `native-artifact production requires one exact selected program entry`
   — the hosted `ProgramEntry` slot admits no visible parameters or
   result; the boundary-ABI corpus's recorded residual owned by
   ENTRY-CONTENT-ROOTS, identical to the x86-64 row's dominant family.

## Row gaps

- Runtime legs inside `hosted_receiver_linux_arm64` and the
  source-evaluated arm64 module execute only on a real aarch64 host
  (`#[cfg]`-gated); emulated evidence above supplements but does not
  replace them.
- The eleven `aarch64_entry_abi` legs are compile-level refusals; none
  reached emission, so no aarch64 ABI correctness claim is made here
  beyond the hosted-receiver bridge.
- Not run: `canary_suite` full corpus (recorded red elsewhere),
  `workspace --lib` baseline, other host rows (macOS/Windows/UEFI).

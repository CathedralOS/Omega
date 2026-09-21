# RC native matrix — linux_arm64 row

Witnessed row of the release-candidate native matrix for the linux_arm64
target. Recorded at revision `96b4afed92` (2026-09-20, re-witnessing
`e76d715c8e` from earlier the same day), host `x86_64-unknown-linux-gnu`,
pinned toolchain `nightly-2026-09-04`, cargo-nextest (mbx unavailable).
Every command below was executed on this host at that revision. No leg
executes an AArch64 binary — this host is x86-64, so all evidence here is
cross-compile/plan evidence; native execution legs require a linux/arm64
host.

Verdict: **red** — 13 pass / 73 fail across 86 witnessed legs. Failures sit
in the same frontier and fixture-migration residuals recorded for the
linux_x86_64 row; no leg that reached a produced artifact misbehaved on the
AArch64 side of its assertions.

## Baseline gates

| leg | command | result |
|-----|---------|--------|
| Bootstrap topology/path hygiene | `sh tools/bootstrap/check-chain-hygiene.sh` | pass (exit 0) |
| Retired-domain corpus audit | `cargo nextest run -p compiler --test canary_suite -E 'test(/retired_domain/)'` | pass (5.2s) |

## Native legs

| leg | filter | result |
|-----|--------|--------|
| Entry calling policy (linux x86_64 + linux_arm64 shared module) | `cargo nextest run -p compiler --test calling_policy_plans -E 'test(/linux/) or test(/arm64/) or test(/aarch64/)'` (6 tests) | 6/6 |
| Hosted receiver bridge | `hosted_receiver_linux_arm64` substring (module `entry_and_abi::hosted_receiver_linux_arm64`, 3 tests) | 3/3 |
| `linux_arm64` substring cohort (adds the float-semantic-twin leg) | `cargo nextest run -p compiler --test canary_suite -E 'test(/linux_arm64/)'` (4 tests) | 3/4 |
| `aarch64` substring cohort (AAPCS64 entry ABI, cross-aarch64 import custody, artifact footprints, asm aarch64 refusals, aarch64 float rewrites) | `cargo nextest run -p compiler --test canary_suite -E 'test(/aarch64/)'` (72 tests) | 0/72 |
| Source-evaluated hosted receiver | `cargo nextest run -p compiler --test source_evaluated_native_realization -E 'test(/arm64/)'` (2 tests) | 2/2 |

- `hosted_receiver_linux_arm64` is green on every leg — the `Service<R>`
  fixture migration that the linux_x86_64 row recorded as 0/3 at
  `8ae40607a3` has since been repaired for the native-tree legs.
- The six `linux_entry::*` calling-policy legs (AAPCS64 physical/semantic
  replay + signature rejection for both linux profiles) are host-independent
  and green.
- The `hosted_receiver_linux_arm64` module's three tests also appear inside
  the `linux_arm64` substring cohort; distinct leg counts above dedupe them.
- The source-evaluated pair turned green between `e76d715c8e` and
  `96b4afed92` — the `Service<R>` carrier-spelling fixture residual
  (`the core Service carrier is closed; it admits no authored qualification`
  + `no domain named Bound is declared for Service<Console>`) is closed on
  both native-tree and source-evaluated legs now.

## Emulated execution leg (not a suite count)

Recorded separately at revision `6ef64f6dd6` (the z128 leg), same host and
toolchain; the emulator is named per the closure rule that emulated runs
must name the emulator and version: `qemu-aarch64-static`
`1:6.2+dfsg-2ubuntu6.31` (qemu-aarch64 6.2.0, Debian).

| leg | command | result |
|-----|---------|--------|
| `cli_mvp` default selection compile | `omega --timings --target linux_arm64 samples/cli/basics/cli_mvp/main.omg` (after `tools/benchmark/benchmark.py prepare` settled `omega.lock`) | pass — `published native output` (ELF 64-bit LSB aarch64, statically linked), 1279539 ms |
| `cli_mvp` run under named emulator | `qemu-aarch64-static 6.2.0 <executable>`, stdin `/dev/null` | pass — stdout `Hello, Omega.` + `[press Enter to close]` prompt, exit 0 |

First recorded execution of an emitted aarch64 artifact for this row. A
native `aarch64-unknown-linux-gnu` host re-run remains the closure
evidence: qemu-user does not cover kernel-ABI paths the emulator
short-circuits, and the suite's `#[cfg]`-gated runtime legs still wait on
real hardware.

## Failure families

All 73 failures reduce to these residuals (`/aarch64/` cohort counted per
first diagnostic; the `linux_arm64` cohort's one red leg is family 6):

1. **Selected ProgramEntry rejoin** (34 legs, dominant): `selected
   ProgramEntry establishment rejoins 0 Terminal attachment identities;
   expected one` — the live selected-dispatch service-custody frontier, the
   same stop recorded in the product-compiler epic and the linux_x86_64
   row's family 2. Covers the cross_aarch64 import-custody legs and most
   `artifact_footprints` compiler-body legs.

2. **Entry binding** (8 legs): `native-artifact production requires one
   exact selected program entry` — the 5 inline-asm byte canaries plus 3
   `aarch64_entry_abi` legs (aggregate-literal / indexed-scalar result,
   hfa argument spread).

3. **Unsupported terminal-production lowering** (7 legs):
   `record store destination projected beyond its authored root` (4),
   `indexed reads require a whole byte-view parameter`,
   `nested call has a runtime receiver`,
   `Unit body omits or duplicates an authored call`.

4. **Exact-arithmetic obligations** (8 legs): `exact arithmetic ... may
   overflow` proof obligations — 7 u64 legs across `aarch64_entry_abi`
   aggregates and footprints plus one u32 (`bounded_carrier_regressions`),
   the aarch64 mirror of the sysv family that dominates the linux_x86_64
   row; owner family ENTRY-CONTENT-ROOTS.

5. **Borrowed-storage ownership transfer** (6 legs, down from ~20 at
   `e76d715c8e`): `cannot transfer a non-copy value out of borrowed
   storage without replacing its owner` plus its boundary-call variant
   `cannot make a boundary or service call ... while `self.*` is absent:
   restore the value moved out of borrowed storage first` — only the
   aarch64_entry_abi result legs and the cross_region footprints remain;
   the borrow-debt tightening residual (`48579a50d3`/`b33de114e4` family)
   cleared on the rest.

6. **Operator-occurrence resolution** (1 leg, in the `linux_arm64`
   cohort): `linux_arm64_float_semantic_edge_twin_retains_artifact_evidence`
   still stops at `authored Operator declaration selection occurrence 178
   remained unresolved after successful checking (CheckedOperator)` —
   selected-dispatch residual.

7. **Ensures/domain proof residuals** (4 legs): `cannot prove initializer
   ... in domain `[u8]::Utf8`/`[u8; N]::Utf8`` (2), `cannot prove ensures
   contract for exit from FixedVecI32x4::clear` (1), `cannot prove
   default-domain field requirement` (1).

8. **Unit-closure machine plan** (1 leg): `attached Unit closure is
   missing a checked transitive machine plan` (place_guard footprints).

9. **Missing footprint evidence file** (1 leg): `storage_bit_field_write`
   panics `Os { code: 2, NotFound }` reading expected evidence — fixture
   gap, not a compile rejection.

10. **Named float rewrite selection** (2 legs):
    `*_selects_aarch64_fmadd_and_executes` — one asserts on an empty
    selected-rewrites set, the other reports the x86 FMA ProviderPlan
    selection diagnostic.

The `Service<R>` carrier-spelling family (2 legs, source_evaluated hosted
receiver) recorded at `e76d715c8e` is closed — both legs are green at
`96b4afed92`.

## Row gaps

- No leg executed an AArch64 image: this host is x86-64. Runtime-status
  legs (`host/process_exit_i32_status`, `host/runtime_console_exit_i32_status`,
  `host/process_exit_i32_status_mapped`, `providers/external_leaf_syscall_compile`)
  are compile-only legs inside the monolithic `pass_canaries_compile`
  aggregate, which mixes every target's legs and is not separately
  filterable; it was not run for this row. Counted legs are the discrete
  test names above.
- `program_entries_and_image_validation` adjacent legs were sampled red on
  the linux_x86_64 row for the entry-binding diagnostic; they are
  target-shared, not arm64-specific, and were not re-run here.
- Not run: `canary_suite` full corpus (recorded red elsewhere),
  `workspace --lib` baseline, macOS/Windows/UEFI host legs (other rows).

## Re-run condition

Re-run this row on a linux/arm64 host once the ProgramEntry-rejoin frontier
and the ENTRY-CONTENT-ROOTS fixture migrations close; the row then also
gains real execution legs (`run.sh` status pins, exit-code evidence) that
cannot be witnessed from an x86-64 host.

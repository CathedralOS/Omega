# RC native matrix — linux_arm64 row

Witnessed row of the release-candidate native matrix for the linux_arm64
target. Recorded at revision `e76d715c8e` (2026-09-20), host
`x86_64-unknown-linux-gnu`, pinned toolchain `nightly-2026-09-04`,
cargo-nextest (mbx unavailable). Every command below was executed on this
host at that revision. No leg executes an AArch64 binary — this host is
x86-64, so all evidence here is cross-compile/plan evidence; native
execution legs require a linux/arm64 host.

Verdict: **red** — 11 pass / 75 fail across 86 witnessed legs. Failures sit
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
| Source-evaluated hosted receiver | `cargo nextest run -p compiler --test source_evaluated_native_realization -E 'test(/arm64/)'` (2 tests) | 0/2 |

- `hosted_receiver_linux_arm64` is green on every leg — the `Service<R>`
  fixture migration that the linux_x86_64 row recorded as 0/3 at
  `8ae40607a3` has since been repaired for the native-tree legs.
- The six `linux_entry::*` calling-policy legs (AAPCS64 physical/semantic
  replay + signature rejection for both linux profiles) are host-independent
  and green.
- The `hosted_receiver_linux_arm64` module's three tests also appear inside
  the `linux_arm64` substring cohort; distinct leg counts above dedupe them.

## Failure families

All 75 failures reduce to these residuals:

1. **Selected ProgramEntry rejoin** (~40 legs, dominant): `selected
   ProgramEntry establishment rejoins 0 Terminal attachment identities;
   expected one` — the live selected-dispatch service-custody frontier, the
   same stop recorded in the product-compiler epic and the linux_x86_64
   row's family 2. Includes the rooted `host/` and `providers/` leg
   shapes under `pass_canaries_compile`'s roster that surfaced as
   `native-artifact production requires one exact selected program entry`
   (5 inline-asm legs, several footprint legs).

2. **Borrowed-storage ownership transfer** (~20 legs: all 12
   `aarch64_entry_abi` aggregate/HFA legs, several footprint legs):
   `cannot transfer a non-copy value out of borrowed storage without
   replacing its owner` — the borrow-debt tightening residual
   (`48579a50d3`/`b33de114e4` family), owner family ENTRY-CONTENT-ROOTS.

3. **`Service<R>` carrier spelling** (2 legs, source_evaluated hosted
   receiver): `the core Service carrier is closed; it admits no authored
   qualification` + `no domain named Bound is declared for
   Service<Console>` — fixture migration residual owned by
   ENTRY-CONTENT-ROOTS. The native-tree siblings were repaired; the
   source-evaluated spelling was not.

4. **Ensures-contract / domain-field proof** (~3 footprint legs):
   `cannot prove ensures contract for exit from FixedVecI32x4::{clear,push}`
   and `parameter self.source.label requires [u8]::Utf8` — proof-corpus
   residuals gated before the artifact legs in the same fixtures.

5. **Unit-closure machine plan** (2 footprint legs): `attached Unit closure
   is missing a checked transitive machine plan` and `Unit body omits or
   duplicates an authored call` — lowering residuals (LOWERED-* family).

6. **Missing footprint evidence file** (1 leg): `storage_bit_field_write`
   panics `Os { code: 2, NotFound }` reading expected evidence — fixture
   gap, not a compile rejection.

7. **Named float rewrite selection** (2 legs):
   `*_selects_aarch64_fmadd_and_executes` assert on an empty selected-rewrites
   set — the fmadd name never landed in the plan.

8. **Operator-occurrence resolution** (1 leg):
   `linux_arm64_float_semantic_edge_twin_retains_artifact_evidence` stops at
   `authored Operator declaration selection occurrence 178 remained
   unresolved after successful checking (CheckedOperator)` — selected-
   dispatch residual.

9. **Wire lowering** (1 leg): `indexed reads require a whole byte-view
   parameter` — unsupported lowering arm.

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

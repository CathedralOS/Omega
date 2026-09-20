# RC native matrix — linux_arm64 row

Witnessed row of the release-candidate native matrix for the linux_arm64
target. Originally recorded at `e76d715c8e`; re-witnessed at revision
`c267df86ac` (2026-09-20), host `x86_64-unknown-linux-gnu`, pinned
toolchain `nightly-2026-09-04`, cargo-nextest (mbx unavailable). Every
command below was executed on this host at `c267df86ac`. No leg executes an
AArch64 binary — this host is x86-64, so all evidence here is
cross-compile/plan evidence; native execution legs require a linux/arm64
host.

Verdict: **red** — 13 pass / 73 fail across 86 witnessed legs at
`c267df86ac` (was 11/75 at `e76d715c8e`). Failures sit in the same frontier
and fixture-migration residuals recorded for the linux_x86_64 row; the
two source-evaluated hosted-receiver legs repaired since the first sweep,
while a new exact-arithmetic proof-obligation family surfaced inside the
aarch64 cohort.

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
| Source-evaluated hosted receiver | `cargo nextest run -p compiler --test source_evaluated_native_realization -E 'test(/arm64/)'` (2 tests) | 2/2 (was 0/2 at `e76d715c8e`) |

- `hosted_receiver_linux_arm64` is green on every leg — the `Service<R>`
  fixture migration that the linux_x86_64 row recorded as 0/3 at
  `8ae40607a3` has since been repaired for the native-tree legs.
- The six `linux_entry::*` calling-policy legs (AAPCS64 physical/semantic
  replay + signature rejection for both linux profiles) are host-independent
  and green.
- The `hosted_receiver_linux_arm64` module's three tests also appear inside
  the `linux_arm64` substring cohort; distinct leg counts above dedupe them.
- The two `linux_arm64_hosted_receiver` source-evaluated legs went green
  since `e76d715c8e` — the `Service<R>` carrier-spelling fixture residual
  recorded as family 3 in the first sweep is repaired.

## Failure families

All 73 failures reduce to these residuals (counts from the `c267df86ac`
sweep):

1. **Selected ProgramEntry rejoin** (~43 legs, dominant): `selected
   ProgramEntry establishment rejoins 0 Terminal attachment identities;
   expected one` (34 diagnostics) and its sibling surface
   `native-artifact production requires one exact selected program entry`
   (8 diagnostics, incl. all 5 inline-asm legs) — the live selected-dispatch
   service-custody frontier, the same stop recorded in the
   product-compiler epic and the linux_x86_64 row's family 2.

2. **Exact-arithmetic proof obligation** (~27 diagnostics, new since
   `e76d715c8e`): `exact arithmetic in machine ... assignment may overflow
   u64/u32/i32 ... decision 17 -- exact arithmetic is a proof obligation` —
   footprint and wire fixtures now stop at the checked-arithmetic gate
   before reaching the artifact assertions.

3. **Borrowed-storage ownership transfer** (~6 legs, shrank from ~20):
   `cannot transfer a non-copy value out of borrowed storage without
   replacing its owner` — the borrow-debt tightening residual
   (`48579a50d3`/`b33de114e4` family), owner family ENTRY-CONTENT-ROOTS.
   Most former members now stop earlier at the family-2 arithmetic gate.

4. **Lowering(Unsupported) record stores** (~8 legs): `record store
   destination projected beyond its authored root` and adjacent
   unsupported arms inside `native-artifact Terminal production failed:
   Lowering(...)`.

5. **Ensures-contract / domain-field proof** (~3 footprint legs):
   `cannot prove ensures contract for exit from FixedVecI32x4::{clear,push}`
   and `parameter self.source.label requires [u8]::Utf8` — proof-corpus
   residuals gated before the artifact legs in the same fixtures.

6. **Unit-closure machine plan** (1 leg): `attached Unit closure is
   missing a checked transitive machine plan` — lowering residual
   (LOWERED-* family).

7. **Missing footprint evidence file** (1 leg): `storage_bit_field_write`
   panics `Os { code: 2, NotFound }` reading expected evidence — fixture
   gap, not a compile rejection.

8. **Named float rewrite selection** (2 legs):
   `*_selects_aarch64_fmadd_and_executes` still red, diagnostics shifted —
   the directed-variant leg still asserts on an empty selected-rewrites
   set; the other now stops earlier at `selected x86 scalar FMA
   ProviderPlan ... requires explicit AVX+FMA3 admission`.

9. **Operator-occurrence resolution** (1 leg):
   `linux_arm64_float_semantic_edge_twin_retains_artifact_evidence` stops at
   `authored Operator declaration selection occurrence 178 remained
   unresolved after successful checking (CheckedOperator)` — selected-
   dispatch residual. Wire-lowering leg (`indexed reads require a whole
   byte-view parameter`) now reports through family 4's
   `Lowering(Unsupported)` diagnostics.

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

# WORKSPACE-ROLLOUT — wave w9 gate state (linux x86-64)

Re-measurement of the `WORKSPACE-ROLLOUT` constraint
([TASKS_OPTIMIZER.md](../../TASKS_OPTIMIZER.md)) at origin/main `96b4afed92`
(2026-09-20, swarm VM, cargo — `mbx` unavailable). Follows the
[rc_repository_gate_closure_z142.md](rc_repository_gate_closure_z142.md)
convention: record the gate state, attribute every failure, leave claimed
surfaces untouched.

## Frozen-tree gate command

`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo test --workspace --no-fail-fast`
(the item's exact command — `cargo` in place of `mbx`, no `--all-targets`):
**fails at compile before any test runs.**

```text
error[E0432]: unresolved import `effects::ComponentEraJournalRoster`
 --> omega-rust/omega/backend/runtime/external-roots/src/program_local/program_local_roots/epoch_cohorts.rs:9:30
  |   ComponentEraEntryLedger, ComponentEraJournalRoster, ComponentEraLedgerId,
  |                              ^^^^^^^^^^^^^^^^^^^^^^^^^ no `ComponentEraJournalRoster` in the root
```

`ComponentEraJournalRoster` has no definition anywhere in the workspace; the
import arrived with `2d8c5136cc` ("backend: compose epoch aggregate snapshots
against the journal-replayed roster"). The file sits under
`external-roots/src/program_local/`, fenced to **ENTRY-CONTENT-ROOTS**
(Zergling-192, exp 2026-09-21T03:26Z) — attributed, not repaired here. The
same break blocks `cargo check --workspace` and every downstream gate leg.

## Rollout architecture gate

`cargo nextest run -p omega-architecture-test --test optimizer_rollout` —
**9/9 PASS**, including `exact_rule_rollout_is_complete_and_promotion_gated`:
the rules inventory, the `optimization_vocabulary!` selection vocabulary in
`optimization-core/src/selection.rs`, and the six staged records stay in
step, promotion requires the full evidence schema, and a staged record still
rejects a completed `Approved status` the inventory does not reflect.

## Constraint state

- `rules.md`: every rule row remains `Experimental` with required explicit
  opt-in `--disable-optimization <ExactName>` — the constraint itself holds.
- `promotions/`: six staged records (ControlFlowCleanup, CopyPropagation,
  DeadPureScalarElimination, GlobalValueNumbering, ProofCheckElision,
  SparseConditionalConstantPropagation) carry the complete schema with
  `Rollback evidence` completed. `Approved status`, `Owner approval`, and
  `Measurement evidence` are `PENDING` on all six: the measurement leg waits
  on the native-realization integer-comparison-occurrence gate that
  BENCHMARKS records, and approval is a product decision.

## Claim map this leg

- Held here: `optimization-core/src/selection.rs`, `optimization-core/rules.md`,
  `optimization-core/src/optimization_unit`, this file — WORKSPACE-ROLLOUT
  (exp 2026-09-21T03:39Z). Sibling catalog legs adding an `Optimization`
  member route through this claim per TASKS_OPTIMIZER.md.
- `optimization-core/promotions/` — RULE-PROMOTION-EVIDENCE (Zergling-43, exp 03:36Z).
- `packages/sources/acquisition` (clippy `permissions_set_readonly_false` at
  `tree/capture/traversal.rs`) — BUILD-PACKAGES-GATE.
- `tests/architecture/optimizer_rollout/mod.rs` (rollback legs noted at
  TASKS.md:10031) — PROMOTION-ROLLBACK-REJOIN-LEGS per board annotation.

## Residual

1. external-roots compile break (fenced) — re-measure the frozen-tree command
   once ENTRY-CONTENT-ROOTS lands; the floor prevents attributing any deeper
   failures this leg.
2. clippy / fmt drift / architecture and lib test failures — the z142 record
   attributes the measured set; none are re-measurable under the compile
   floor without duplication.

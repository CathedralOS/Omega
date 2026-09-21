# SELECTED-REWRITE-CATALOG-DISPOSITION — audit record (linux x86-64, e7c0099cb2)

Scope: the retain-or-delete decision over the 38 `Orphaned` rewrite modules in
`selected-instructions-to-selected-instructions/src/rewrites/module_catalog.rs`
(upstream bullet: `TASKS_OPTIMIZER.md` PIPELINE-OWNER-CONSOLIDATION — "Give each
retained rewrite a catalog entry executed by the stage entrance, or delete it").

## Audit result: retain all 38; no delete candidates; no drift

- **No Orphaned row gained a production caller.** Every public symbol of every
  orphaned module is referenced only by the module's own files, its own tests,
  and its `Validated*` impl in `analyses/selected_input.rs`. `optimize_selected_instructions`
  (`selected_optimization.rs`) still executes only the liveness → live-ranges →
  allocation-legality chain plus `run_selected_lowering_optimizations`
  (selected-lowering pair folds), so no row mislabeled Routed.
- **All Routed rows are live:** allocation_recovery, fixed_view, literal_folds,
  runtime_rematerialization, runtime_spill, selected_lowering — the reconcile
  test pins each `caller`/`evidence` pair.
- **All 8 Shared rows are read by other modules** (block_edges, catalog,
  commuting_accesses, condition_state, dead_path, module_catalog,
  place_storage, window_hazards) — no dead-shared residue.
- **No `Validated*` residue:** all 52 `Validated*` names in
  `analyses/selected_input.rs` map to live modules or analysis types;
  retired-module impls (`ValidatedLiteralMinuend`, `ValidatedLiteralCompare`,
  `ValidatedLiteralArithmetic`) are absent — the literal_minuend precedent
  (`66a6ea93f7`) removed them correctly.
- **Owner strings are real:** 35 rows name EXACT-MACHINE-SIMPLIFICATIONS and 3
  name ALIAS-AWARE-MEMORY; both items exist on the optimizer board.

## Retain-not-delete evidence (sampled module docs)

- `constant_boolean`: fills the flag-to-register materialization gap the literal
  compare pair folds stop short of ("the boolean forms are the only
  flag-to-register readers in the selected catalog").
- `dead_compare`: removes the dead compares the constant-flag cluster leaves
  behind — no executed pass does this.
- `copy_removal`, `redundant_extension`, `dead_store`, `load_forwarding`,
  `store_motion`, `address_fold`, `local_schedule`: real selected-CFG
  transformations with admission tables; none are second producers of the pair
  rules.
- The relocation/interchange families (arm, bypass, commuting, confluence,
  diamond, edge, fork, inflow, join, local, member_run, predecessor, run,
  triangle + `_run_*` variants): instruction-motion rewrites needing
  hazard/dead-path/commutation audits — not covered by any routed pass.

`relocation.rs` is the member-run *relocation* module under a short module name
(exports `ValidatedMemberRunRelocation`) — misnamed but live; renaming is a
cosmetic leg under MEMBER-RUN-RELOCATION-VALIDATOR-INDEPENDENCE's claim.

## Remaining dependency (not implementable here)

Wiring needs the `optimization-core` naming handoff — each retained rewrite
needs an `Optimization` member name (rules.md) plus a `SelectedStageRuleRows`
catalog arm + executor + evidence variant joined in `selected_optimization.rs`'s
`slice_executes_at_stage`. That handoff is WORKSPACE-ROLLOUT's lane per
zergling-z21's next-acceptance note on SELECTED-REWRITE-CATALOG-EXECUTION;
delete dispositions need a second-producer proof none of the 38 meet.

## Gate gap recorded

The module_catalog reconcile test pins only Routed rows (caller/evidence must
exist). Orphaned→gained-a-caller drift is silent today — an Orphaned module
that gains a production caller stays labeled Orphaned until audited. A
caller-existence check symmetric to the Routed check is a candidate
strengthening under a future catalog-owner item.

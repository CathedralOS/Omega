# z44 — task-activations join conversion (PROVIDER-SELECTION-AFTER-TERMINAL leg-4c)

Worktree: `.codex/worktrees/z44-task-activations`, branch `devin/z44-task-activations` @ b0bacbce9e.
Item text (TASKS.md ~L2900): "install-phase joins (selected dispatch, task activation, component progress,
intrinsic review, fused entry establishments) still read the typed program and checked records:
convert their inputs to Terminal Psi plus that target's Build rows".

## Problem
`elaborate_task_activation_plans` (provider-planning/src/task_plans/mod.rs) reads
`checked.program` (TypedTrees) + `program.facts` (CheckFacts) + `selected_provider_plan_facts`
+ `opaque_representation_selections` + `native_target`. Caller: selected-dispatch
execution_settlement.rs L272. Called from driver `install_selected_target`/`settle_product_target`
— the Check-product path runs it with NO produced artifact, so evidence must also be mintable
on the artifact-less path.

## Design (decided)
Mint a self-contained evidence bundle Psi-side; the join consumes bundle + Build rows.

1. **terminal-psi** — new `TerminalTaskActivationDemand` rows (module field
   `task_activation_demands`), with all typed-derived values pre-baked:
   - start operation (Machine, state, statement_index, call_ordinal, entry symbol name)
   - requirement owner trait path text + package identity + callable identity text (interned)
   - report_fingerprint, entry_report_fingerprint, contract fingerprint (u64s, same formulas)
   - specialization_commitment [u8;32] (same digest stream)
   - may_suspend / may_block / carry: CarryPolicy
   - signature arg types + outcome as `TerminalTaskValueType`:
     `Scalar(ScalarType) | Structural(StructuralTypeId) | TargetLayout(LayoutApplicationId)`
     + normalized identity text (interned) — text needed for signature layout fingerprint
   - frames: `Vec<TerminalTaskFrame { identity:u64, resident: Vec<TerminalTaskValueType>,
     calls: Vec<TerminalTaskFrameCall> }>` — frame closure + call resolution done upstream
   - crossings: canonical rows (identity u64, flags, live carry w/ minted place/type/claim ids)
   - structural_types needed + referenced TargetLayoutApplications ride in the bundle so the
     check path is self-consistent (id space is bundle-local; plans/fingerprints never hash ids)

2. **stage-05** — mint `task_activation_demands` after machines/structural/suspension are
   produced (id mappings in scope). Port (near-verbatim) start_selections,
   runtime_requirements identity pieces, specialization_commitments encoder,
   stack_graphs closure/call-resolution, carry_crossings canonicalization — output terminal
   rows instead of plan facts. Typed-reading helpers move Psi-side
   (candidate home: checked-trees evidence module or stage-05 crate-internal file).

3. **provider-planning** — `elaborate_task_activation_plans` new signature:
   (demands bundle, selected_provider_plan_facts, native_target, opaque selections,
    resolve_layout: FnMut(&TerminalTaskValueType)->Result<TypeLayout,Diagnostic>).
   Keeps: runtime-provider join over plan facts (Build rows), signature layout fingerprint,
   stack demand compose/project/validate over evidence frames.
   - product path: resolver walks bundle structural_types + applications + target name
     (realizations.find(r.target == target.display_name())). Port of structural_shape.
   - check path: driver supplies layouts via existing typed layout engine keyed by
     normalized identity text -> precomputed map from the minted demand rows.

4. **dep-edge cut last**: provider-planning loses checked_trees/typed reads only after the
   join is evidence-only; check-path bridge is a driver concern (driver keeps checked deps
   until the item closes).

## Landing sequence (each builds alone)
- C1 `psi:` terminal-psi row types + module field (+ codec if module is serialized — CHECK).
- C2 `psi:` stage-05 mint of task_activation_demands.
- C3 `omega:` join conversion + product-path resolver + check-path bridge at call sites.
- C4 `omega:` dead-delete typed reads in task_plans/* (start_selections, stack_graphs,
  carry_crossings, specialization_commitments become dead) + dep cut.
- C5 TASKS.md frontier bullet via --board-update.

## Open risk
- providers corpus dark upstream (2af8b1295c, z42 owns) — verify via capabilities group.
- TargetLayout-typed residents resolved via embedded applications (new vs typed engine) —
  fingerprint equality needs exact size/align parity.

slice_0034

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0256 | scalar_lowering.rs:2815-2818 | CLOSED | Non-fresh computed homes intentionally skip snapshot discard; `fresh_storage` is set from checked place geometry at emission. |
| A0257 | scalar_lowering.rs:793 | CLOSED | `Ok(None)` only for captured values or non-intrinsic-copy types; other completion paths still emit produce/cleanup from checked plans. |
| A0258 | shared.rs:308-321 | CLOSED | Missing flow row defaults to `FlowCallResult::Operand`; module/boundary scalar binds use the same coordinates stage 05 published with calls. |
| A0259 | shared.rs:2065-2079 | CLOSED | Missing per-field qualification row yields empty qualifications and base scalar type, matching an unqualified field snapshot. |
| A0260 | unit.rs:98-100 | CLOSED | No `contract_plans` row means no published crash interface; callable entry lowering requires a plan when contracts matter (attached_unit.rs:6240-6242). |
| A0261 | unit.rs:625-628 | CLOSED | `None` rows pad dynamic tables for conformances not selected in this closure; not a substitute for realization of selected rows. |
| A0262 | unit.rs:1074-1080, 1560-1561, 2061-2072 | CLOSED | Skips, no-op substitution, and store re-retain mirror stage-05 dispositions and checked cleanup schedules, not orphan acceptance. |
| A0263 | unit.rs:3759-3761 | CLOSED | Cleanups whose dispatcher machine is outside the emitted closure are skipped; terminal semantics rejects incomplete nominal cleanup designation later. |

### A0256

1. Read `effects::emit` in full (scalar_lowering.rs:2557-2853, including `CompleteSnapshots` at 2809-2852). Entry description matches read code at 2815-2818: `continue` when `!binding.fresh_storage`.
2. Search `rg 'effects::emit\\(|scalar_lowering::effects::emit'` under `omega-rust/**/*.rs`: 1 hit (unit_arguments.rs:3641). Read `emit_state` around that call (not full `emit_state`).
3. Caller chain: scalar branch lowering → `unit_arguments.rs` `emit_state` → `effects::emit` → `CompleteSnapshots` arm.
4. Input: `CompleteSnapshots` with `CompletedSnapshot::Produced(value)` where `operations.computed_values` has `fresh_storage: false` (reuse of pre-existing owned home, per unit_operation_frame.rs:7166-7169).
5. **Verdict: CLOSED.** Check at unit_operation_frame.rs:7166-7169 sets `fresh_storage`; skipping discard avoids dropping another binding’s storage. Comment at scalar_lowering.rs:848-849 ties disposal to Psi 05 admission.
6. Unsure: none.

### A0257

1. Read `snapshot_completion`, `complete_snapshot`, and `finish`/`finish_with_alias` in full (scalar_lowering.rs:749-845).
2. Search `rg '\\bsnapshot_completion\\b'` under `omega-rust/**/*.rs`: count 3 (scalar_lowering.rs:2, scalar_computations.rs:1). Read `complete_snapshot` caller in scalar_computations.rs:2444-2456 (not full expansion function).
3. Callers: `complete_snapshot` (scalar_computations.rs:2455); `snapshot_completion` alone (scalar_computations.rs:2450). Upstream: scalar dispatch expansion in stage 06 only after checked trees pass.
4. Input: `ValueRequest` with `captured: true`, or structural type lacking `copyable && no_code_affine_cleanup` match (scalar_lowering.rs:784-794).
5. **Verdict: CLOSED.** `Ok(None)` at 793-794 suppresses redundant snapshot completion; `finish_with_alias` (823-827) still emits `ProduceStructuralValue` when needed. Captured roots are excluded by design.
6. Unsure: none.

### A0258

1. Read `planned_result`, `discards_result`, `initializes_immutable_local`, and `checked_call` in full (shared.rs:275-337).
2. Search `rg '\\bplanned_result\\b'` under `omega-rust/**/*.rs`: count 3, all in shared.rs. Production uses: `discards_result` / `initializes_immutable_local` → `bind_scalar_result` (unit_operation_frame.rs:1212-1226).
3. Read `bind_scalar_result` (unit_operation_frame.rs:1206-1243) and scalar bind sites at 1122, 1133, 2281, 3095, 4937. `flow.control.call_at` (flow_ownership_and_roots.rs:390-399) returns `None` when no row; `map_or_else(Default::default, …)` yields `FlowCallResult::Operand` (flow.rs:308-311).
4. Input: `CheckedUnitCallCoordinate` with no matching `FlowCallFact` in flow control (wrong/missing row).
5. **Verdict: CLOSED.** Default `Operand` is the enum default (flow.rs:308-311). Module/boundary scalar results use coordinates from checked effect plans that stage 05 pairs with flow facts (published_facts.rs:10294-10311). `RegisterAccess` (unit_operation_frame.rs:4937) correctly keeps scalar results as operands.
6. Unsure: whether every non-module coordinate without a flow row is impossible on valid checked trees; not exhaustively traced for all effect kinds beyond register and module paths.

### A0259

1. Read `fresh_field_snapshot` in full (shared.rs:2065-2079).
2. Search `rg '\\bfresh_field_snapshot\\b'` under `omega-rust/**/*.rs`: count 5 (definition + unit_composed_control.rs:2 + emission.rs:1). Did not read full call-site functions.
3. Callers mint field snapshot `ValueDeclaration`s during structural lowering; missing `qualifications` entry uses `unwrap_or_default()` and base `scalar_type`.
4. Input: field id absent from the `qualifications` slice passed in.
5. **Verdict: CLOSED.** Absent row means no field-specific qualification overlay; plain scalar type is intentional.
6. Unsure: none.

### A0260

1. Read `effective_crash_routes` and `contribute` in full (unit.rs:94-124).
2. Search `rg '\\beffective_crash_routes\\b'` under `omega-rust/**/*.rs`: count 8 across stage 06 (attached_unit.rs, emission.rs, etc.). Read attached_unit.rs:10826-10828 and 6240-6242.
3. When `contract_plans.for_machine(machine)` is `None`, returns `Ok(Vec::new())` (unit.rs:98-100). Callable entry requires a plan (attached_unit.rs:6240-6242).
4. Input: machine symbol with no `MachineContractPlan` in checked facts.
5. **Verdict: CLOSED.** Absence of a plan is treated as no published crash interface; entry paths that need contracts fail without a plan (attached_unit.rs:6240-6242). `checked_state_contracts_have_plan` (execution_terminal_unit_types.rs:4416-4417) ties requires contracts to plans at stage 05.
6. Unsure: none for normal checked-tree pipeline; corrupt missing-plan trees not traced end-to-end.

### A0261

1. Read `lower_dynamic_dispatch_catalog` loop in full for table rows (unit.rs:454-653, focus 586-642).
2. Search `rg 'lower_dynamic_dispatch_catalog'` under `omega-rust/**/*.rs`: 1 caller (machine_lowering.rs:2035). Read that callsite only.
3. `selected_here` false when no selection’s `machine` is in `source_machines` (608-610); rows push `None` and `continue` (625-627) without `realizes_slot`.
4. Input: dynamic conformance for an implementation whose selecting machine is outside the current lowered closure.
5. **Verdict: CLOSED.** Placeholder `None` rows fill the catalog for unselected units; selected rows still get realization checks (629-641).
6. Unsure: none.

### A0262

1. Read `validate_runtime_memberships` (unit.rs:1009-1106), `substitute_runtime_requirement_scalar_values` (unit.rs:1454-1578), `outstanding_actions` (unit.rs:2001-2102).
2. Searches: `validate_runtime_memberships` → 1 production call (attached_unit.rs:6253); `outstanding_actions` → used from unit.rs / attached_unit (counts only).
3. Three accepting behaviors in one entry:
   - 1074-1079: `continue` when `restates_structural_parameter_qualification` — same skip as stage 05 (published_facts.rs:4052-4053).
   - 1560-1561: `StructuralCaseMembership` / `StructuralIdentity` unchanged — no scalar formal substitution needed.
   - 2061-2070: `Store` sets `*retained = true` — re-opens cleanup actions the checked plan still owes after a store into the same home.
4. Input: only checked runtime-requirement facts and cleanup action rosters from stage 05 terminal unit effects.
5. **Verdict: CLOSED.** Each branch mirrors checked disposition or proposition shape, not bypass of missing requirements.
6. Unsure: full proof that every `Store` re-retain matches checker intent; read `outstanding_actions` and matching `Store` arm only, not all producers of action lists.

### A0263

1. Read `bind_nominal_cleanup_subjects` and `selected_cleanups` iterator header (unit.rs:3302-3334, 3752-3774).
2. Search `rg '\\bbind_nominal_cleanup_subjects\\b'` under `omega-rust/**/*.rs`: count 2 (definition + attached_unit.rs:17131).
3. `lookup_machine_id` failure → `continue` (3759-3761); other cleanups in `selected_cleanups` still processed.
4. Input: `CheckedUnitNominalAffineCleanupPlan` whose `cleanup_machine` was not emitted in this closure’s `machine_ids`.
5. **Verdict: CLOSED.** Skip is for out-of-closure dispatchers; `terminal-psi/semantics/src/structural.rs:417-439` rejects missing/ambiguous nominal cleanup designation at the terminal trust boundary.
6. Unsure: whether lowering always reaches that verifier for every product path (read structural.rs site only).

### What I did not read

- Full `emit_state` in `unit_arguments.rs` (only the `effects::emit` callsite).
- Full `Expansion` / dispatch functions in `scalar_computations.rs` beyond the `complete_snapshot` callsite.
- Full `fresh_field_snapshot` caller functions in `unit_composed_control.rs` and `emission.rs`.
- Full `lower_unit_closure` and retention pipeline in `attached_unit.rs` beyond cited lines.
- Stage 05 execution modules beyond cited `published_facts.rs`, `execution_terminal_unit_types.rs` fragments.
- Corpus or interpreter runs (read-only assignment).

SLICE COMPLETE

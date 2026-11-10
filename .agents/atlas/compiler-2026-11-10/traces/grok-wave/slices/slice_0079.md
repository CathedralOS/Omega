slice_0079

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0602 | S00/lowering.rs:1624-1625; :189; :1571 | CLOSED | Stage-01 unit validation rejects mismatched witnesses, evidence surfaces, and non-empty cleanup obligations. |
| A0603 | S00/lowering_scalars.rs:264-266 | CLOSED | Operand admission is enforced in `validate_psi_optimization_unit` before optimization runs. |
| A0604 | S00/lowering_storage.rs:400-418 | UNDECIDED | Unlisted writers fall through to “no write” without tracing every `OperationKind` that mutates storage. |
| A0605 | S00/retained_terminal.rs:932-934 et al. | REACHABLE | Missing family or boundary matches are skipped, so provider rows can be omitted while native lowering continues. |
| A0606 | S01/control_flow_analyses.rs:303-323 | REACHABLE | CFG omits `StructuralCase`/`InvokeNoNormal`/`ExternalCompletion` edges, so case-arm blocks look unreachable to dominator consumers. |
| A0607 | S01/semantic_analyses.rs:2241-2249; S01/rules.rs:372-378 | UNDECIDED | Value liveness shares the CFG gap; whether `validate_psi_rewrite_candidate` blocks bad dead-scalar proposals was not fully traced. |
| A0608 | S01/control_flow_cleanup.rs:1141-1158 | UNDECIDED | Shared jump fusion skips the dominance predicate used by `merge_substitutions`; candidate validation after apply was not fully read. |
| A0609 | S01/ranked_cycles_validation.rs:428; 619-624 | REACHABLE | Non-countdown natural cycles get `None` from `project` and are dropped with no error when certificates also derive empty. |

Paths: `S00/` = `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/`; `S01/` = `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/`.

### A0602

1. Read `runtime_cleanup_actions`, `runtime_cleanup_receiver`, and terminator lowering in `lowering.rs` (read: `lowering.rs:1571-1641`, `:189`, `:157-191`).
2. Caller search: `rg '\bruntime_cleanup_actions\b' omega-rust/omega` → **7** hits, all in `S00/lowering.rs` (6 call sites in `lower_terminator`, 1 definition).
3. Upward: `runtime_cleanup_actions` ← terminator paths in `lower_machine` ← `lower_selected_module` (`lowering.rs:1795-1848`); `lower_selected_module` has **7** repository hits (`lowering.rs` + `artifact_admission.rs`).
4. Input on accepting branches: admitted `TerminalModule` terminators and `CallDynamic` with structural results lowered to abstract cleanup rows (obligations cleared, `cleanup_receiver` rewritten), `selected_evidence: Vec::new()`, `trivial_affine_locals: Vec::new()`.
5. **Verdict: CLOSED.** `VerifiedPsiOptimizationSession::new` runs full `validate_psi_optimization_unit_with_admitted_cycle_machines` (`pass_manager.rs:1313-1316`). `StructuralReturnTrivialAffineLocalsMismatch` fires when lowered empty locals disagree with terminal witnesses (`unit_validation.rs:18076-18098`, read). Non-payloadless structural calls require `selected_evidence.is_empty()` only on vetted surfaces (`unit_validation.rs:12516-12526`, `12624-12626`, read). Nominal cleanup with non-empty `requirement_obligations` fails ownership projection (`current_ownership.rs:3550-3552`, read).
6. Unsure: whether `runtime_cleanup_receiver` always matches the intended place when the cleanup machine has multiple structural parameters (read only first parameter at `lowering.rs:1637-1640`).

### A0603

1. Read `integer_bitwise::lower`, FMA lowering, and shift lowering in `lowering_scalars.rs` (read: `:264-274`, `:290-306`, `:562-577`).
2. Caller search: `integer_bitwise::lower` invoked from `lowering.rs:1227-1235` only (read).
3. Upward: `lower_operation` ← `lower_machine` ← `lower_selected_module` ← `build_verified_psi_optimization_unit` ← `optimize_abstract_operations` (`lib.rs:147-148`, read).
4. Input: terminal scalar ops whose **result** type is checked; operand `ValueId`s are copied without local type correspondence checks (read `lowering_scalars.rs`).
5. **Verdict: CLOSED.** Stage-00 lowering is permissive, but every production path hits `validate_psi_optimization_unit` before PSI rewrites (`pass_manager.rs:1313-1316`, read). Operand admission for FMA, bitwise, and shift is at `unit_validation.rs:12095-12205` (read).
6. Entry description matches the lowering code (read).

### A0604

1. Read `latest_write` and `reaching_fact` in `lowering_storage.rs` (read: `:318-421`, `:208-314`, `:146-154`).
2. Caller search: `rg '\breaching_fact\(' omega-rust/omega` → **2** hits in `lowering_storage.rs` (definition + atomic lowering call, read).
3. Upward: `reaching_fact` ← atomic coherence lowering in same module (read `:146`); not traced through full `lower_machine` admission path.
4. Input: `OperationKind` rows not in the explicit read/field-store arms; if `lends_root` is false, `latest_write` returns `None` (no write) (`lowering_storage.rs:407-420`, read).
5. **Verdict: UNDECIDED.** Some paths escalate to `Reaching::Unknown` → `LoweringError::UnsupportedAtomicCoherence` (`lowering_storage.rs:154`, read), but whether every mutating terminal kind is covered by the explicit arms or `lends_root` was not established (no exhaustive `OperationKind` audit).
6. Unsure: complete mutating-op inventory for Terminal Psi at this analysis site.

### A0605

1. Read `close_family_realizations`, `prepare_native_provider_demand`, `bind_direct_boundary_leaves` in `retained_terminal.rs` (read: `:910-972`, `:712-716`, `:932-950`, `:1224-1226`, `:632-717`, `:1208-1230`).
2. Caller search: `close_family_realizations` → **2** hits; `prepare_native_provider_demand` → **3** hits; all in `retained_terminal.rs` (read).
3. Upward: `close_family_realizations` ← `project_terminal_native_body` (`:1043`, read) ← native proposal path; `prepare_native_provider_demand` ← native execution preparation (`:584`, `:617`, read). `bind_selected_provider_coordinates` copies family rows afterward (`artifact_admission.rs:419-447`, read)—only rows present are bound.
4. Input: family `SelectedFamilyRealizationSource` entries with no matching boundary head (`continue` at `:932-934`); non-boundary selected members (`:948-950`); direct leaves with no declaration match (`:1224-1226`); family members with no `selected_requirement_row` (`:713-715`, read).
5. **Verdict: REACHABLE.** A configuration that needs a family realization row but supplies a stale head or lacks a selected requirement row can pass this stage with the row silently omitted while lowering continues into `VerifiedPsiOptimizationInput`.
6. Unsure: whether downstream provider-admission always errors when a required family row is absent (not read past stage-00 binding).

### A0606

1. Read `function_control_flow` in `control_flow_analyses.rs` (read: full `graph::control_flow` module `:284-362`).
2. Caller search: `rg '\bcontrol_flow\(' …/01_abstract-operations…` → **5** hits (`analyses.rs:1`, `control_flow_analyses.rs:4`, read).
3. Upward: dominators, SCC helpers, and loop detection consume this CFG (`control_flow_analyses.rs:157`, `:196`, `:373`, read). `dominators` filters to `block.reachable` from this incomplete BFS (`:219-224`, `:340-358`, read).
4. Input: blocks whose last operation is `StructuralCase`, `InvokeNoNormal`, or `ExternalCompletion` contribute **no** successor edges (`:321-322`, read); case-arm targets live in `node.successors` on the unit (`unit_validation.rs:2355-2369`, read) but not in this analysis.
5. **Verdict: REACHABLE.** Case-arm blocks can be marked unreachable/absent from dominator trees while still semantically reachable via `semantically_reachable_blocks` (`semantic_analyses.rs:2349-2373`, read)—a separate function not used by `control_flow()`.
6. Unsure: whether every dominator-dependent rule is gated on the broken reachability bit (only partial reads in `control_flow_cleanup.rs`).

### A0607

1. Read `value_liveness` in `semantic_analyses.rs` (read: `:2231-2268`) and dead-scalar proposal in `rules.rs` (read: `propose` `:344-381`).
2. Caller search: `rg '\bvalue_liveness\(' omega-rust/omega` → **2** hits (`semantic_analyses.rs`, `analyses.rs`, read).
3. Upward: dead-scalar rules require `AnalysisKind::ValueLiveness` (`rules.rs:351`, read) during PSI pass manager runs (`pass_manager.rs:603`, read for `validate_psi_rewrite_candidate`—body not fully read).
4. Input: blocks ending in `StructuralCase` etc. contribute empty successor lists in liveness fixpoint (`semantic_analyses.rs:2248`, read), same gap as A0606.
5. **Verdict: UNDECIDED.** A scalar only needed along a case-arm edge may show dead at the case block exit; whether `validate_psi_rewrite_candidate` (`pass_manager.rs:603`) rejects the resulting `DeadScalarNodeRewrite` was not traced end-to-end.
6. Entry matches the liveness code (read). Did not read full `validate_psi_rewrite_candidate`.

### A0608

1. Read `shared_jump_fusion` candidate construction (`control_flow_cleanup.rs:1141-1158`, read) and `merge_substitutions` (`:231-261`, read).
2. Caller search: `rg '\bmerge_substitutions\b' omega-rust/omega` → **6** hits in `control_flow_cleanup.rs` (read).
3. Upward: `SharedJumpFusionRule` registered in cleanup rule set (`control_flow_cleanup.rs:1065-1176`, read); candidates validated in pass manager (`pass_manager.rs:603`, read).
4. Input: terminal jump fusion with parameter/type match only—no `replacement_dominates_parameter_uses` (`control_flow_cleanup.rs:1145-1152` vs `:245-250`, read).
5. **Verdict: UNDECIDED.** The accepting branch is real (read), but I did not read whether candidate validation or rewrite reconstruction enforces dominance before commit.
6. Post-dominator analysis also uses incomplete `control_flow()` (read `control_flow_cleanup.rs:1093-1094`).

### A0609

1. Read `terminal::project`, `project_one`, and `rederive_exact_certificates` (`ranked_cycles_validation.rs:428-428`, `:431-599`, `:612-643`, read).
2. Caller search: `validate_exact_ranked_cycles` → `validation.rs:44` (read); `from_transformed` re-runs cycle context (`pass_manager.rs:1328`, read).
3. Upward: admission gate before PSI optimization session (`pass_manager.rs:1297`, read).
4. Input: each cycle component; `project_one` returns `None` unless terminal natural SCC matches unsigned countdown shape (`:435-481`, read); loop at `:619-623` skips without error when `None`.
5. **Verdict: REACHABLE.** A machine with a natural ranked cycle that is valid topology but not countdown-shaped is omitted from `recognized` and compared against an empty derived certificate list (`:626-639`, read)—no error when both sides are empty.
6. Topology still checked via `optimization_graph`, which **does** include `StructuralCase` edges (`ranked_cycles_validation.rs:754-760`, read)—distinct from A0606’s analysis graph.

### What I did not read

- Exhaustive Terminal Psi `OperationKind` mutator list for A0604.
- Full `validate_psi_rewrite_candidate` and shared-jump rewrite apply path (A0607, A0608).
- Downstream provider-admission errors for missing family rows (A0605 tail).
- `runtime_cleanup_receiver` multi-parameter edge cases (A0602 tail).
- Pass-manager internals beyond cited lines (`run_unit_inner_with_retention` full body).

SLICE COMPLETE

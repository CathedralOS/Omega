# slice_0079

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0602 | S00/lowering.rs:1624-1625, :189, :1571 | UNDECIDED | Later checks close some branches; cleanup obligation clearing not traced to admission. |
| A0603 | S00/lowering_scalars.rs:264-266, 290-306, 562-567 | UNDECIDED | Stage-00 lowers without operand-type checks; did not trace terminal decode closing every path. |
| A0604 | S00/lowering_storage.rs:400-418 | UNDECIDED | `latest_write` falls through to `None` for unlisted writers; impact on atomic admission not traced upward. |
| A0605 | S00/retained_terminal.rs:932-934, 1224-1226, 713-716 | UNDECIDED | Absent family head or missing member selection uses `continue`; harm to product admission not traced. |
| A0606 | S01/control_flow_analyses.rs:303-323 | UNDECIDED | CFG omits case/invoke/external edges; whether bad dominator consumers accept invalid units not traced. |
| A0607 | S01/semantic_analyses.rs:2241-2249 | CLOSED | Liveness analysis is incomplete but dead-node admission re-checks liveness via `node.successors`. |
| A0608 | S01/control_flow_cleanup.rs:1141-1158 | CLOSED | Proposer omits dominance; rewrite admission uses separate checks including post-dominance. |
| A0609 | S01/ranked_cycles_validation.rs:428, 619-624 | UNDECIDED | Unrecognized cycle shapes skip certificate pairing; whether invalid ranking must be rejected not read. |

## A0602

**Cited sites (re-located on current checkout):** `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/lowering.rs:1623-1626`, `:186-190`, `:1567-1572`.

**`runtime_cleanup_actions` (read):** In `lowering.rs:1615-1630`, `InvokeNominal` cleanup has `requirement_obligations.clear()` and `cleanup_receiver` replaced via `runtime_cleanup_receiver` (`1633-1640`). **Callers (search `\bruntime_cleanup_actions\b` in `omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations`, count mode):** 7 matches — 1 definition, 6 call sites at `1433`, `1467-1468`, `1521-1522`, `1546`, `1558`, `1572` (read).

**Dynamic structural `selected_evidence` (read):** `CallStructural` with `AbstractCallTarget::Dynamic` sets `selected_evidence: Vec::new()` at `:189`. **Closing check (read):** `optimization-unit/semantics/src/unit_validation.rs:12516-12520` requires `selected_evidence.is_empty()` for dynamic structural results — empty vector matches the intended dynamic surface.

**`ReturnStructural` / `trivial_affine_locals` (read):** Lowering sets `trivial_affine_locals: Vec::new()` at `:1571`. **Closing check (read):** `unit_validation.rs:18067-18098` rejects structural returns whose `trivial_affine_locals` do not match expected terminal witness when expectations are nonempty.

**`requirement_obligations.clear()` (read):** Obligations are removed before abstract cleanup is stored. **Inferred:** `current_ownership.rs:3550-3551` would reject nonempty cleanup obligations on the abstract side, but lowering clears them first. **Not traced:** Whether a canonical `TerminalModule` may carry cleanup actions with nonempty `requirement_obligations` that ought to fail compilation before stage 00.

**Input that would reach the branch:** Jump/case/return terminators with cleanup action lists on reachable machines (read lowering paths above).

**Verdict: UNDECIDED** — sub-claims for dynamic evidence and trivial locals are closed at S01 validation; obligation clearing vs terminal contract not traced.

## A0603

**Cited sites (read):** `lowering_scalars.rs:259-274` (FMA: only `result.scalar_type.as_ieee_float()`), `:289-306` (bitwise: only result integer type), `:562-577` (shift: `value_type` from result, `count_type` from `value_types.get(&count)`).

**Upstream (read):** `semantic_module.rs:4093-4119` `decode_module` runs `validate_structural_foundation` before returning a module. **Operand shapes (read):** `terminal-psi/semantics/src/scalars.rs:3597-3608` enforces matching integer types for binary integer leaves via `validate_operand_shape`; bitwise and wrapping shift rows register `goal_free_scalar_leaf` schemas in `terminal-psi/semantics/src/lib.rs:373-387`. **FMA (read):** `NearestIeeeFloatFusedMultiplyAdd` row has `goal_free_scalar_leaf: None` at `lib.rs:360` — no leaf operand-shape hook in that table.

**Not traced:** Whether every admitted machine operation is type-checked through `goal_free_scalar_leaf_semantics` or an equivalent foundation pass for bitwise, shift value operands, and FMA ternary operands (grep `goal_free_scalar_leaf_semantics` under `terminal-psi/codec`: 0 production uses, read).

**Verdict: UNDECIDED** — lowering plainly does not re-check operand types; closing guarantee at terminal admission not demonstrated for all listed ops.

## A0604

**Cited site (read):** `lowering_storage.rs:400-418` — read-only operation kinds match with empty arms; default arm returns `None` unless `lends_root`.

**Enclosing function (read):** `latest_write` (`318-421`), called from `reaching_fact` (`208-314`). **Consumer (read):** `reaching_fact` only drives atomic residency resolution in `atomic_events::lower` (`146-174`); unknown reach becomes `LoweringError::UnsupportedAtomicCoherence` when observation requires a source (`154-155`, read).

**Not traced:** Whether a writing operation kind omitted from the read-only list and not caught by `lends_root` can appear on a canonical terminal module and still yield a wrong `None` that becomes an accepted atomic edge instead of an error.

**Verdict: UNDECIDED**

## A0605

**Cited sites (read):** `retained_terminal.rs:932-934` — family source with no matching head: `continue`. `:1224-1226` — direct boundary leaf with no declaration: `continue`. `:712-716` — family member without `selected_requirement_row`: `continue`.

**Context (read):** Family realization rows are assembled during native realization admission (`704-728`); skipped members simply omit a family row.

**Not traced:** Whether a program with a stale or absent family head that should be rejected can still complete `lower_decoded_artifact` / artifact admission without a diagnostic.

**Verdict: UNDECIDED**

## A0606

**Cited site (read):** `control_flow_analyses.rs:303-323` — only `Jump`, `Conditional`, return patterns, and `Crash` add CFG edges or exits; `StructuralCase`, `InvokeNoNormal`, and `ExternalCompletion` fall through `_ => {}`.

**Enclosing function (read):** `function_control_flow` (`290-362`); reachability BFS at `340-346` uses incomplete successors.

**Callers (search `\bcontrol_flow\(unit\)` in `omega-rust`, count mode):** 4 matches — `analyses.rs:56`, `control_flow_analyses.rs:157`, `:196`, `:373` (read). **Dominators (read):** `dominance::dominators` (`195-211`) builds on this CFG.

**Contrast (read):** Optimization unit construction uses `expected_edges` including `StructuralCase` at `unit_validation.rs:2355-2369`; ranked-cycle terminal graph includes case edges at `ranked_cycles_validation.rs:708-713`.

**Not traced:** Whether any optimization rule using this CFG can lead `validate_psi_optimization_unit` to accept a unit that should be rejected.

**Verdict: UNDECIDED**

## A0607

**Cited site (read):** `semantic_analyses.rs:2238-2249` — block successors for liveness use only `Jump` and `Conditional`; `_ => Vec::new()` drops `StructuralCase` arms.

**Proposer (read):** `rules.rs:372-381` — `propose` skips removal when `live` says result is dead at exit (flawed analysis).

**Closing check on acceptance (read):** `candidates.rs:2826-2833` `validate_dead_scalar_node_candidate` rejects when `reconstruct_closed_scalar_node_boundary` reports `live_out` still contains the result or `function.node_uses()` still references it. **Reconstruct (read):** `candidates.rs:5774-5840` unions `node.successors` (populated from `expected_edges`, including case arms) — not the flawed `ValueLiveness` analysis.

**Verdict: CLOSED** — unsound liveness cannot admit a dead-node rewrite without failing candidate validation.

## A0608

**Cited site (read):** `control_flow_cleanup.rs:1141-1158` — `shared_jump_fusion` builds `ScalarSubstitution` lists with parameter/binding type equality only (no `replacement_dominates_parameter_uses`).

**Contrast (read):** `block_merge_substitutions::merge_substitutions` at `:231-260` requires `use_definitions.replacement_dominates_parameter_uses`.

**Closing check on rewrite admission (read):** `candidates.rs:1528-1580` `validate_shared_jump_fusion_candidate` requires `independently_post_dominates` (`1578-1579`), accounting reconstruction (`1595-1597`), and `checked_scalar_substitutions` with candidate substitution equality (`1581-1586`, `2032-2033`). Post-dominator CFG for validation uses `expected_edges` per block (`1649-1667`, read).

**Verdict: CLOSED** — dominance gap exists in proposal only; admitted fusions pass post-dominator and validator gates.

## A0609

**Cited sites (read):** `ranked_cycles_validation.rs:428` — `terminal::project` returns `None` when no countdown shape. `:619-624` — components without a certificate are skipped, not errors. **`derive` (read):** `current::derive` at `:203-216` requires a certificate for every component in the snapshot passed in; `rederive_exact_certificates` builds a reduced snapshot containing only recognized components (`626-629`) before comparing to terminal-derived certificates (`631-638`).

**Not read:** Language or product rule requiring rejection when a ranked SCC uses a legitimate non-countdown ranking scheme.

**Verdict: UNDECIDED** — code intentionally ignores unrecognized shapes; whether that permits accepting a program that should fail is unspecified here.

## What I did not read

- Full `validate_structural_foundation` per-operation typing for FMA and non-goal-free integer ops beyond cited fragments.
- Terminal cleanup `requirement_obligations` invariants on jump/case edges.
- End-to-end proof that `validate_psi_optimization_unit` fails when dominator analysis is wrong.
- Specification text for ranked-cycle obligations outside `ranked_cycles_validation.rs`.
- Remaining `retained_terminal.rs:948-950` family-member path beyond skimming adjacent `continue`/`push` logic.

SLICE COMPLETE

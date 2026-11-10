slice_0081

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0618 | `legalization.rs:234-243` | CLOSED | Duplicate obligation facts are rejected when the unit is built/validated; `accepted_fact` only runs on that index. |
| A0619 | `legalization_descriptor_inputs.rs:81-91` | CLOSED | `.ok()` affects descriptor catalog graphs only; custody `apply` still errors on selection failure. |
| A0620 | `legalization_reference_custody.rs:6004-6006` (producer `1100-1125`) | REACHABLE | If flow marks a live position `ProvedImpossible`, custody updates are skipped with `Ok(())`. |
| A0621 | `legalization_reference_custody.rs:6637` | CLOSED | Fallback arm is for non-reference ops; reference ops have arms and `validate_occurrences` runs after (6640). |
| A0622 | `LRC:3495-3503;5598-5617;5790-5796;7040` | REACHABLE | Several paths skip missing-leaf/custody/cleanup checks instead of failing. |
| A0623 | `LSG:228-230;2304-2309;1849-1853` | REACHABLE | Empty-path `MutableBorrow` windows authenticate without backing-root checks (1849-1853). |
| A0624 | `LSG:8451-8459;LSrc:691-727` | REACHABLE | `Exact` integer arithmetic skips operand-width checks in audit and projection. |
| A0625 | `LSG:8948-8953;8168-8176` | CLOSED | Skips are in `match_input` audit only; structural calls/results are checked in custody `apply`. |

### A0618

1. Read `legalization.rs:234-243` — `accepted_fact` filters `unit.accepted_obligation_facts` and returns `.next().map(|fact| fact.identity)` or `LegalizationError::custody()` (read).
2. Search `accepted_fact\(` in `omega-rust/`: **39 hit lines** across **~35 files** (read grep count); stage-03 production callers include `legalization.rs`, `legalization_source.rs` (14 call sites), `legalization_target.rs`, `legalization_scalar_graph.rs` (read).
3. Read `optimization-unit/src/proof.rs:519-538` — `attach_accepted_obligation_facts` sorts facts and returns `DuplicateOwner` when `(machine, owner, obligation)` repeats (read).
4. Read `unit_validation.rs:9697-9706` — `validate_psi_optimization_unit` rejects non-canonical or out-of-order accepted-fact rows via `AcceptedObligationFactIndexMismatch` (read); stage-01 `pass_manager.rs` invokes this validation (read grep hit, not full pass_manager body).
5. Input to accepting branch: a unit that already passed attach + validation; at most one fact per `(machine, owner, obligation)` tuple (inferred from attach sort + duplicate check).
6. **Verdict: CLOSED** — duplicates cannot reach `accepted_fact` on a unit that compiles through normal validation.
7. Unsure: whether any production entry bypasses `validate_psi_optimization_unit` (did not read full pipeline driver).

### A0619

1. Read `legalization_descriptor_inputs.rs:76-92` — `operation()` calls `selected_boundary_operation(...).ok()` and on `Err` uses `&node.operation` (read); entry description matches.
2. Search `self.operation(` — **2 hits**, both in `views()` / `references()` graph closures (lines 55, 69) (read).
3. Read `preserves_boundary` (94-154) — returns `false` when `node.operation` is not `BoundaryCall` or `row(...)` fails (read).
4. Read `legalization_reference_custody.rs:6007-6010` — custody `apply` uses `selected_boundary_operation(...)?`, not `.ok()` (read).
5. Input: optimization nodes where target-row selection fails; catalog builders see abstract `node.operation` instead of installed target row.
6. **Verdict: CLOSED** — swallowed error does not bypass custody simulation; worst case mis-builds descriptor catalog used for flow, while `apply` still fails closed on selection `Err`.
7. Unsure: full downstream effect of a wrong catalog on programs that never hit failing `apply` positions (did not read `optimized_catalog_graph` body).

### A0620

1. Read `legalization_reference_custody.rs:3840-3844` — `position_is_impossible` when flow entry is `ProvedImpossible` (read).
2. Read `6004-6006`, `6853-6855` — early `return Ok(())` in `apply` / `apply_terminator` (read).
3. Read producer `1100-1125` — `descriptor_backing::prepare` walks blocks, records `flow_feasibility`, rejects duplicate position/edge keys (read).
4. Read `block_entry_states:7178-7192` — still calls `apply` per node; impossible positions skip updates (read).
5. Input: nodes at positions the descriptor flow table marks `ProvedImpossible`.
6. **Verdict: REACHABLE** — if analysis wrongly marks a reachable reference operation impossible, custody is not updated and simulation returns `Ok(())` for that node.
7. Unsure: soundness proof of `DescriptorAnalysis::flow_feasibility` (did not read `terminal_semantics` analysis implementation).

### A0621

1. Read `apply` match `6016-6637` — arms cover reference-affecting ops including `CallStructural` with structural result (`6529-6580`) (read); `_ => {}` at 6637 (read).
2. Read `6640-6647` — `validate_occurrences()` always runs after the match (read).
3. Read `2691-2755` — `validate_occurrences` rejects duplicate/missing parent occurrences (read).
4. Input: any `AbstractOperation` variant without a dedicated custody arm (mostly scalar / no reference state).
5. **Verdict: CLOSED** — unhandled ops intentionally do not mutate custody; reference ops have arms; post-check enforces leaf consistency.
6. Unsure: whether any reference-bearing variant lacks an arm (did not enumerate entire `AbstractOperation` enum).

### A0622

1. Read `stored_calls::stage` loop `3495-3503` — missing leaf with `Case` segment → `continue` (read); inactive cases from `capture_predicate_feasibility` `1247-1255` / `path_is_impossible` `1258-1288` (read).
2. Read `call_results` `5598-5616` — `transfers_existing` call-result path with no moved/copied leaf → `continue` (read); empty `candidates` → `continue` at `5651-5652` (read).
3. Read `discard_subtree` `5790-5791` — no `source_contract` → `Ok(false)` (read); `discard_owned` `5772-5779` delegates there (read).
4. Read `complete_reference_outputs` `7040` — calls `discard_owned` after conflict check (read).
5. Input: stored-call effects on inactive case paths; call results expecting transferred references without custody records; cleanup on places with no contract in custody.
6. **Verdict: REACHABLE** — e.g. a call-result mapping with `transfers_existing` and no recorded leaf can be skipped (`continue`), and `DiscardRoot` on a place without contract returns success without discarding (`Ok(false)`).
7. Unsure: whether Psi earlier stages prevent those custody states on live paths (did not trace from Psi 05).

### A0623

1. Read `edge_bindings` `213-230` — non-address-join formals return `true` without accessibility checks (read); used from `legalization_target.rs:1684-1690`, `1744-1750` (read).
2. Read descriptor-origin `resolve` `2304-2318` — non-nonlinear multiplicity or empty slots → `Ok(Vec::new())` (read).
3. Read `borrowed_windows::extent` auth `1848-1854` — empty `path` authenticated when root is `Parameter(_)` **or** `required_access == MutableBorrow` without `window_backing_roots` (read, contrasts with `1817-1847` referent path).
4. Input: edge structural bindings to ordinary (non-join) parameters; descriptor calls without view slots; mutable-borrow windows at `[]` on non-parameter roots.
5. **Verdict: REACHABLE** — mutable borrow + empty path bypasses backing-root authentication (`1849-1853`).
6. Unsure: whether `extent` callers always preclude abusive roots (read callers partially, not all).

### A0624

1. Read `LSG:8451-8459` inside `match_input` node validation — operand `value_type` checks gated on `arithmetic.policy() != Exact` (read).
2. Read `LSrc:691-727` `project_exact_integer_arithmetic` — checks obligation + `accepted_fact` + `references_obligation`, not operand widths (read); contrast `project_saturating_integer_arithmetic` `442-446` (read).
3. Read `nodes::validate` admission `7654-7661` — admits `IntegerArithmetic` via `integer_arithmetic_admitted` only (read).
4. Search `IntegerArithmetic` + `value_type` in `omega-rust/`: **no matches** (read grep).
5. Input: `IntegerArithmetic` with `Exact` policy where `left`/`right` SSA types differ from `scalar_type`.
6. **Verdict: REACHABLE** — exact add/sub/mul/div can legalize without operand type match unless blocked earlier (no earlier guarantee found in this unit).
7. Unsure: Psi typed-tree checker enforcement before optimization unit (outside stage-03 files read).

### A0625

1. Read `match_input` `8168-8176` — skips nodes defining structural-result `CallStructural` (read).
2. Read `8914-8966` — validates unit/scalar `CallStructural` only; skips args containing `Referent`; uses `Custody::default()` in `argument_at` (read).
3. Read `legalization_reference_custody.rs:6529-6580` — structural-result `CallStructural` handled in custody `apply` (read).
4. Read `audit_target_operation_plan` `509-515` — `match_input` runs in audit path (read); production `construct_optimized_instruction_selection` calls `legalize_target_operations` without audit (`optimized.rs:98-107`) (read).
5. Input: structural calls during independent audit / `verify_target_operation_plan`.
6. **Verdict: CLOSED** for acceptance semantics of the skip — audit skips are compensated by full custody simulation in `block_entry_states` / `apply` when target legalization validation runs (`legalization_target.rs:1244`, read).
7. Unsure: whether production `legalize_target_operations` alone is intended as the final reference gate (read `legalization_source::validate_source_custody` — identity counts only, lines 17-31).

### What I did not read

- Full `AbstractOperation` enum vs `apply` match coverage inventory.
- `DescriptorAnalysis::flow_feasibility` / `case_feasibility` implementations in `terminal_semantics`.
- `optimized_catalog_graph` and `stored_calls::prepare` bodies end-to-end.
- Psi pipeline stages before stage 03 for integer type proofs and reference call contracts.
- Entire `call_results` / `stored_calls::stage` functions beyond cited regions.
- Production compiler driver beyond `optimized.rs` construction path.

SLICE COMPLETE

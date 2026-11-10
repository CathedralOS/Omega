slice_0042

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0317 | `omega-rust/psi/compilation/representations/checked-trees/src/flow_terminal_units.rs:55-73` | CLOSED | Fused state roster dedupes symbols before plans are built, so first-match `position` targets the sole row. |
| A0318 | `omega-rust/psi/compilation/representations/checked-trees/src/operators_and_proof.rs:477-486` | REACHABLE | `Missing` with zero candidates is treated like builtin fallback, and `check_operator_resolution` emits no diagnostic for `Missing`. |
| A0319 | `omega-rust/psi/compilation/representations/checked-trees/src/operators_and_proof.rs:1331-1337` | CLOSED | Published facts call `select_pending_domain_operator_meanings` before checks; `DomainPending` is not left in shipped `CheckFacts`. |
| A0320 | `omega-rust/psi/compilation/representations/checked-trees/src/operators_and_proof.rs:2659` | CLOSED | `build_contract_call_facts` appends at most one `ContractCallFact` per borrow call site, so the index has no duplicate keys in normal production. |
| A0321 | `omega-rust/psi/compilation/representations/checked-trees/src/operators_and_proof.rs:2037` | CLOSED | Transitional inputs and exact float literals are meant to skip direct-source provenance; publish and lowering apply other checks. |
| A0322 | `omega-rust/psi/compilation/representations/checked-trees/src/values_and_views.rs:1255-1256` | UNDECIDED | Cycle during `visit` returns `Normal`, but I did not trace whether the producer can build a cyclic structural value graph. |
| A0323 | `omega-rust/psi/compilation/representations/checked-trees/src/values_and_views.rs:1261-1272` | UNDECIDED | A `Call` node with no matching schedule row yields `Normal`; I did not prove that mismatch can occur after operand planning. |
| A0324 | `omega-rust/psi/compilation/representations/checked-trees/src/values_and_views.rs:2272-2300` | CLOSED | Executable lowering paths use `bound_expression_at`, which rejects duplicate rows; `expression_at` is mostly an existence probe. |

### A0317

1. Read `successor_position` and `state_position` in full at `flow_terminal_units.rs:55-73` (read).
2. Search `successor_position` in `omega-rust/`: 17 hits (`flow_terminal_units.rs`: 4, `unit_composed_control.rs`: 13); read `live_mask` caller at `flow_terminal_units.rs:601-619` and lowering uses in `unit_composed_control.rs` (not every lowering callsite read).
3. `target_occurrence: None` delegates to `state_position`, which uses `Iterator::position` (first matching `candidate.state == state`) at `flow_terminal_units.rs:72-73` (read).
4. Input on that branch: a `CheckedStructuralControlSuccessorPlan` with `target_occurrence == None` and a `states` slice passed from `CheckedComposedUnitControlStatePlan::live_mask` or `CheckedComposedUnitControlMachinePlan::successor_position`.
5. Upstream: `fused_states` refuses a second fused row with the same state symbol at `execution_terminal_unit_state_graph.rs:982-987` (read); `planned.push` runs once per fused entry at `9962-9988` (read); `fused_target_index` also uses first `position` at `1166-1167` (read).
6. **Verdict CLOSED:** A composed plan’s `states` vector should not contain two rows with the same `state` symbol when `live_mask` / lowering resolve successors with `target_occurrence == None`. Wrong-target resolution would need duplicate symbols in that vector; fusion blocks that before planning.

### A0318

1. Read `CheckedOperatorUseFact::resolved_through_builtin_fallback` at `operators_and_proof.rs:477-486` (read): true when spelling matches, `selected_operator_symbol` invalid, `candidate_count == 0`, and status is `Missing` or `BuiltinFallback`.
2. Search `resolved_through_builtin_fallback` in `omega-rust/`: 3 hits; read callers at `execution_terminal_unit_control.rs:5545-5549` and `published_facts.rs:6014-6016` (read).
3. Read `check_operator_resolution` at `checks_operators.rs:2835-2879` (read): `resolution_issues()` can include `Missing`, but `filter_map` only emits diagnostics for `is_ambiguous()` / `is_inadmissible()`; `Missing` yields `None` at `2847-2848`.
4. Input: operator fact rows with `CheckedOperatorResolutionStatus::Missing`, zero candidates, invalid `selected_operator_symbol`, on an expression that still reaches checked-tree execution or published semantic facts.
5. **Verdict REACHABLE:** A program whose operator use is stored as `Missing` with no candidates can pass `check_operator_resolution` with no operator-resolution error (read `checks_operators.rs:2841-2849`). Separately, `resolved_through_builtin_fallback` treats that `Missing` row like builtin fallback for the requested spelling (read `operators_and_proof.rs:481-485`), so index projections with `Missing`+0 candidates are accepted in array element preparation (read `execution_terminal_unit_control.rs:5545-5549`). I did not fully trace whether typed validation always rejects a `+` on a type with no declared operator before facts are published; the acceptance gap for `Missing` in the checker is established in code.

### A0319

1. Read `CheckedOperatorFacts::resolution_issues` at `operators_and_proof.rs:1326-1343` (read): filters `Missing`, `Ambiguous`, `Inadmissible` only; `DomainPending` excluded at `1332-1337`.
2. Search `resolution_issues` in `omega-rust/`: 2 hits; read consumer `checks_operators.rs:2839-2841`.
3. Read where `DomainPending` is set in production scratch at `checks_ranges.rs:6114-6118` (local replay arena only, read).
4. Read `select_pending_domain_operator_meanings` at `validation/operators.rs:1523-1544` and call site `published_facts.rs:11027` (read): runs on the published `operators` table before `CheckFacts` is assembled.
5. **Verdict CLOSED:** Shipped `CheckFacts.operators` should not still carry `DomainPending` when `check_operator_resolution` runs; omission from `resolution_issues` is harmless on the main path because pending domain uses are finalized at `published_facts.rs:11027` (read). Leftover `DomainPending` would only matter in the scratch replay in `checks_ranges.rs`, not in published facts.

### A0320

1. Read `contract_call_at` at `operators_and_proof.rs:2644-2661` (read): index callback `|rows| rows.last().copied()`.
2. Read sibling `unique_contract_call_at` at `2664-2684` (read): returns `None` unless exactly one row.
3. Search `contract_call_at(` in `omega-rust/`: 9 call sites; read `flow_calls.rs:57-62`, `checks_contracts.rs:7062-7068`, `checks_content.rs:2336-2341` (not every callsite read in full).
4. Read producer `build_contract_call_facts` / `append_contract_call` at `published.rs:887-1072` (read): one loop over `borrow.calls`, one `calls.append` per iteration when not early-returning.
5. **Verdict CLOSED:** Duplicate keys would require two `ContractCallFact` rows for the same `(caller_machine, caller_state, statement_index, call_ordinal)`; the only append path walks borrow calls once (read `published.rs:904-1061`). Obligation validation that needs uniqueness uses `unique_contract_call_at` at `contract_and_obligation_facts.rs:2855-2862` (read).

### A0321

1. Citation `operators_and_proof.rs:2037` matches `CheckedFloatMeaningProjection::validate` (read `1988-2043`); entry description of `TransitionalInput` / exact literal variants matches `CheckedFloatProjectionSource` at `1859-1868`.
2. `_ => {}` at `2037` (read) skips `InvalidSourceProvenance` for variants not covered by guarded `Direct*` arms, including `TransitionalInput`, `ExactBinary32Literal`, `ExactBinary64Literal`.
3. Read publish path `published.rs:3469-3480` (read): new projections call `validate()` before push.
4. Read lowering `proofs.rs:7163-7169` (read): lowering calls `validate()` again.
5. **Verdict CLOSED:** Those sources are accepted without direct machine/state provenance checks by design; format and contract checks still run (`operators_and_proof.rs:1990-1998`, `2039-2040`, read). Lowering applies separate carrier rules for direct sources (`proofs.rs:7175-7254`, read).

### A0322

1. Read `CheckedStructuralValuePlans::completion_with` / inner `visit` in full at `values_and_views.rs:1231-1340` (read).
2. At `1255-1256` (read), if `value` is already in `active`, return `CheckedValueCompletion::Normal` without error.
3. Read consumers `execution_terminal_unit_control.rs:6403-6447` and `execution_terminal_unit_control_statement_sequence.rs:2260-2261` (read): `Normal` vs `NoNormal` drives establish-local vs evaluate-structural-value effects.
4. **Verdict UNDECIDED:** The accepting branch is real (read `1255-1256`), but I did not trace structural-value construction to prove a cyclic graph can reach `completion`/`completion_with`. Without that, I cannot name a smallest rejecting program that would be accepted.

### A0323

1. Read `visit` `Call` arm at `values_and_views.rs:1261-1271` (read): no matching row in `calls` makes `call` `None`, so the branch returns `Normal` (not `NoNormal`).
2. Read call preparation in `execution_terminal_unit_control.rs:6178-6212` (read): `Call` nodes normally push a `CheckedStructuralValueCall` into `output` when planning operands.
3. **Verdict UNDECIDED:** Missing schedule row yields `Normal` (read `1267-1270`), but I did not prove a `Call` node can reach the final `completion_with` walk without a corresponding `output` entry. Wrong acceptance would need that mismatch; I did not establish it.

### A0324

1. Read `bound_expression_at` (`2221-2269`, read) and `expression_at` (`2272-2300`, read) in full.
2. `bound_expression_at` returns `None` on duplicate matches (`2254-2256`, `2265-2267`, read); `expression_at` uses `.find(...).map` with no duplicate guard (`2292-2299`, read).
3. Search `.expression_at(` in `omega-rust/`: 22 call sites (count); read `expression_bindings.rs:5373-5383` (read): lowering `expression_at` delegates to `bound_expression_at`.
4. Read representative 05 use `execution_terminal_unit_state_graph.rs:473-477` (read): `expression_at(...).is_some()` retention probe, with actual binding via `bound_expression_at` elsewhere in the same module (read `7010`, `7052` only).
5. **Verdict CLOSED:** Silent first-match in `expression_at` is not on the main binding/lowering path; production retrieval for lowering goes through `bound_expression_at`, which fails closed on duplicates (read `expression_bindings.rs:5381-5383`).

### What I did not read

- Every `successor_position` callsite in `unit_composed_control.rs` (13 hits; read 1).
- All nine `contract_call_at` call sites (read 3).
- Full typed-validation traversal for illegal `+` on non-builtin types before Psi 05 (inferred only via `validation/operators.rs:1728-1763` status rules and `checks_operators.rs:2847-2848`).
- Structural-value producer invariants ruling out cycles (A0322) or call/schedule mismatches (A0323).
- Entire `build_checked_scalar_expression_plans` planner surface for duplicate `(state, statement_ordinal, role)` rows.

SLICE COMPLETE

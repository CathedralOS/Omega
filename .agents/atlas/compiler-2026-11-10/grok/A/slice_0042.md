# slice_0042

Digest: area_11 (Proof, proof admission, psi-compiler, checked trees). Path shorthand `ct/` = `omega-rust/psi/compilation/representations/checked-trees/src/`.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0317 | ct/flow_terminal_units.rs:55-73 | CLOSED | Fused state roster dedupes by state symbol before `planned` is built, so `state_position` first-match equals the only match when `target_occurrence` is `None`. |
| A0318 | ct/operators_and_proof.rs:477-486 | UNDECIDED | `Missing` with zero candidates is treated as builtin only at two call sites (Index, Equal); whether a program with no `+` operator reaches that fact row was not traced. |
| A0319 | ct/operators_and_proof.rs:1331-1337 | CLOSED | `DomainPending` is cleared by `select_pending_domain_operator_meanings` before `check_operator_resolution` runs, so the omission does not affect acceptance. |
| A0320 | ct/operators_and_proof.rs:2659 | UNDECIDED | `rows.last()` is real, but whether two `ContractCallFact` rows share the same call key in production was not traced. |
| A0321 | ct/operators_and_proof.rs:2037 | CLOSED | Every checked float-meaning projection row is built in `published.rs` and must pass `validate()` immediately; skipped provenance arms match non-direct source shapes. |
| A0322 | ct/values_and_views.rs:1255-1256 | UNDECIDED | Cyclic revisit returns `Normal`, but whether a checked structural value graph with a cycle can be published was not traced. |
| A0323 | ct/values_and_views.rs:1261-1272 | REACHABLE | A `Call` node with no matching schedule row is classified as `Normal`, so a non-returning call missing from the schedule slice can be accepted as completable. |
| A0324 | ct/values_and_views.rs:2272-2300 | REACHABLE | Duplicate located expressions for the same state, statement, and role yield first-match via `expression_at` while `bound_expression_at` would reject them. |

## A0317

(read) Opened `successor_position` and `state_position` in `omega-rust/psi/compilation/representations/checked-trees/src/flow_terminal_units.rs:55-73 (matches digest cite; checkout later than `457adf4a11f`). When `successor.target_occurrence` is `None`, line 64 calls `state_position(states, successor.target_state)`, which uses `states.iter().position(|candidate| candidate.state == state)` at lines 68-72 — first index wins.

(read) Caller search: `\bsuccessor_position\b` in `omega-rust`, count mode → 17 matches in 2 files (`flow_terminal_units.rs`: 4, `unit_composed_control.rs`: 13). Production check-time use: `CheckedComposedUnitControlStatePlan::live_mask` at `flow_terminal_units.rs:601-619` calls `successor_position` at line 612 while building reachability over `planned`.

(read) `fused_states` in `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/execution_terminal_unit_state_graph.rs:958-988 refuses a second fused entry with the same state symbol (`candidate.symbol == owner_state.symbol` at lines 982-986). `planned.push` at `execution_terminal_unit_state_graph.rs:9962-9988` appends one plan row per fused entry, so the `states` slice passed to `live_mask` has at most one row per state symbol at check time.

(inferred) Input that would reach the branch: a composed-unit control graph whose terminator successor has `target_occurrence: None` (default at `flow_terminal.rs:1773`) and targets a state symbol; resolution uses symbol lookup only.

**CLOSED:** Duplicate symbols are not present in `planned` at Psi 05; first-match `state_position` is the unique match on that path.

## A0318

(read) `CheckedOperatorUseFact::resolved_through_builtin_fallback` at `operators_and_proof.rs:477-486`: returns true when spelling matches, `selected_operator_symbol` is invalid, `candidate_count == 0`, and status is `Missing` or `BuiltinFallback`.

(read) Caller search: `\bresolved_through_builtin_fallback\b` in `omega-rust`, count mode → 3 matches: 1 definition (`operators_and_proof.rs:477`), 2 production calls — `execution_terminal_unit_control.rs:5547` (`OperatorSpelling::Index`), `published_facts.rs:6016` (`OperatorSpelling::Equal`). No caller uses `Add` or other spellings.

(read) `execution_terminal_unit_control.rs:5543-5550`: array element preparation returns `None` when an index projection’s operator use does **not** satisfy `resolved_through_builtin_fallback(Index)` — absence skips the block and continues.

(read) `published_facts.rs:6003-6017`: `has_builtin_equality_meaning` sets `builtin &= resolved_through_builtin_fallback(Equal)` for each expression operator use.

(read) `checks_operators.rs:2835-2850`: `resolution_issues()` is invoked, but `filter_map` only emits diagnostics for ambiguous and inadmissible; `Missing` rows produce `None` (no diagnostic from this loop).

(read) Contrast `ct/facts.rs:244-254`: `has_builtin_operators` treats any use with status other than `BuiltinFallback` as not builtin (so `Missing` is not builtin there).

**UNDECIDED:** The digest’s `+` on a type with no operator was not traced through validation/operator publication to see whether `Missing` with zero candidates survives and whether any acceptance path depends on `resolved_through_builtin_fallback` for `Add`. What would decide it: tracing operator fact publication for unresolved binary `Add` and whether any gate besides the two call sites treats `Missing` as success.

## A0319

(read) `resolution_issues` at `operators_and_proof.rs:1326-1343` filters statuses to `Missing | Ambiguous | Inadmissible` only; `DomainPending` (defined at line 448) is excluded.

(read) Caller search: `\bresolution_issues\b` in `omega-rust`, count mode → 2 matches: definition `operators_and_proof.rs:1326`, sole use `checks_operators.rs:2841`.

(read) `validation/operators.rs:2227-2237`: domain-bearing candidate lists get initial status `DomainPending`.

(read) `published_facts.rs:11027`: `select_pending_domain_operator_meanings(program, &mut operators)` runs during fact assembly.

(read) `select_pending_domain_operator_meanings` at `validation/operators.rs:1523-1544` collects all `DomainPending` uses and rewrites status via `finalize_pending_use` → `select_scoped_candidates` (`operators.rs:1728-1769`), which never leaves `DomainPending`.

(read) `checks.rs:12162` calls `check_operator_resolution` after facts are built.

**CLOSED:** At check time, operator uses should no longer be `DomainPending`; omitting it from `resolution_issues` does not admit programs that still needed domain selection.

## A0320

(read) `contract_call_at` at `operators_and_proof.rs:2644-2661` uses `call_index.with_rows(..., |rows| rows.last().copied())`.

(read) `unique_contract_call_at` at `2664-2684` returns `None` when multiple rows share the key.

(read) Caller search: `\.contract_call_at\(` in `omega-rust`, count mode → 6 call sites in Psi 05 (`flow_call_phases.rs:2289`, `flow_calls.rs:57`, `checks_content.rs:2336`, `checks_contracts.rs:7062`, `checks_contracts_calls.rs:260`, `981`). `unique_contract_call_at` appears only at `contract_and_obligation_facts.rs:2856`.

(read) `published.rs:904-1001`: `build_contract_call_facts` loops borrow calls and calls `append_contract_call` once per flow call; `calls.append(ContractCallFact { ... })` at line 1061 is the only append site found (`grep calls.append(ContractCallFact` → this file only).

**UNDECIDED:** Taking the last duplicate row is observable if the arena holds multiple rows for one `(caller_machine, caller_state, statement_index, call_ordinal)` key. Whether borrow/call planning can publish two rows for that key was not traced.

## A0321

(read) `CheckedFloatMeaningProjection::validate` at `operators_and_proof.rs:1988-2043`. Direct-source arms reject invalid provenance (lines 2000-2036). Match arm `_ => {}` at 2037 covers `TransitionalInput`, `ExactBinary32Literal`, and `ExactBinary64Literal` — no extra provenance fields.

(read) Enum variants at `operators_and_proof.rs:1859-1868` (`TransitionalInput`, literals).

(read) Producer `push_float_meaning_projection` in `published.rs:3351-3480` constructs `MeaningProjection` and calls `projection.validate()` at 3478 before push; literals enter via `ProjectionSource::ExactBinary32Literal` / `ExactBinary64Literal`; transitional keys map to `TransitionalInput` at 3444-3447.

**CLOSED:** Acceptance on the publishing path always runs `validate()`; the `_ => {}` arm matches variants that intentionally carry no machine/state provenance to verify in this function.

## A0322

(read) `completion_with` / inner `visit` at `values_and_views.rs:1231-1271`. At lines 1255-1256, if `active.contains(&value)`, return `Some(Normal)` instead of error.

(read) `CheckedStructuralValueKind::Call` branch at 1261-1271 handles missing or non-normal schedule rows separately (A0323).

(read) Caller search: `.completion(` in `omega-rust/psi/compilation`, count mode → 10 call sites including `execution_terminal_unit_control_statement_sequence.rs:2260` and `execution_terminal_unit_control.rs:6438` (direct `completion` without the planner’s `visited` guard at 6165-6167).

**UNDECIDED:** Whether any published `CheckedStructuralValuePlans` graph can re-enter the same handle on the `active` stack during legitimate checking was not traced. What would decide it: tracing structural value construction for recursive/cyclic shapes into `completion` / `completion_with`.

## A0323

(read) `values_and_views.rs:1261-1271`: for `Call`, take first matching row in `calls`; duplicate matches return `None`. If `call` is `None`, the `if call.is_some_and(... !has_normal_continuation())` branch is false, so result is `Normal`.

(read) `execution_terminal_unit_control.rs:6429-6447` `establish_value`: uses `facts.values.structural_values.completion(value, &calls)?`; on `Normal`, emits `EstablishLocal` instead of `EvaluateStructuralValue` (lines 6438-6454).

(inferred) Input in plain words: a structural value node tagged as a `Call` included in establishment, but the `calls` slice passed into `completion` has no row for that value handle — the compiler treats the value as normally completable.

**REACHABLE:** Should reject non-completable or missing-schedule calls; the else branch at 1269-1270 accepts them as `Normal`.

## A0324

(read) `expression_at` at `values_and_views.rs:2272-2300` uses `.find(...)` (first match) with no duplicate check. `bound_expression_at` at 2221-2268 returns `None` when a second matching binding or expression exists (lines 2238-2239, 2265-2266).

(read) Caller search: `\.expression_at\(` in `omega-rust`, count mode → 26+ call sites in lowering and check pipelines (`06_checked-trees-to-lowered-psi` and `05_typed-trees-to-checked-trees`).

(read) `values_scalar.rs:5343-5366` `record_scalar_value_pair` appends one located expression per recorded role; did not prove global uniqueness of `(state, statement_ordinal, role)` across all pushes.

(inferred) Input in plain words: two stored scalar expression rows sharing the same state, statement ordinal, and role; lowering that uses `expression_at` reads the first row and proceeds.

**REACHABLE:** Should reject or fail closed on ambiguous keys; `expression_at` silently chooses the first row.

## What I did not read

- Full `checks.rs` driver beyond the `check_operator_resolution` call site.
- Borrow / flow-call construction proving uniqueness of `call_ordinal` keys for `ContractCallFact`.
- Validation diagnostics for unresolved non-primitive binary operators (e.g. `+`).
- Structural value graph construction for cycles.
- Entire `unit_078` record (digest preamble and area_11 digest sections through line 80 were read).

SLICE COMPLETE


slice_0041

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0309 | facts.rs:3440-3447 | CLOSED | `record_state_locals` fills one span per typed `state.symbol`; lowering runs only after checking succeeds |
| A0310 | flow.rs:1339-1346 | CLOSED | Checking appends one `FlowStateFact` per state; duplicates appear only in unit tests at flow.rs:1396-1427 |
| A0311 | flow.rs:2162-2166 | UNDECIDED | Fallback to state/statement `entry_constraints` when `state_call` misses may weaken loan pinning; borrower paths not fully traced |
| A0312 | flow_ownership_and_roots.rs:905-914 | UNDECIDED | `unwrap_or(0)` undercounts exit transfers; effect on `permits_disposal` / graph admission not fully traced |
| A0313 | flow_ownership_and_roots.rs:379-388 | CLOSED | Empty span is the intentional “natural” order mirrored from typed trees and `published_facts.rs:10347-10360` |
| A0314 | flow_terminal.rs:2044-2057; flow_terminal_units.rs:1520-1543 | REACHABLE | Equal `PartialEq` plans differing only in `source_declaration` merge silently with no `Err` |
| A0315 | flow_terminal.rs:1384-1410 | CLOSED | With empty `projection_evaluations`, only computed/runtime index segments need selector lowering steps |
| A0316 | flow_terminal_units.rs:972,1005 | REACHABLE | `statement_ordinal().unwrap_or(0)` maps `None` (e.g. cleanup operations) to ordinal 0 for disposal logic |

### A0309

1. Read `StateLocalFacts::locals`, `with_states`, `named` in full (`facts.rs:3423-3478`, read).
2. Search `\blocals\(` under `omega-rust` with `state_locals` context: 9 hits (3 definitions in `facts.rs`, 6 in `06_checked-trees-to-lowered-psi`, read grep counts).
3. Read caller `record_state_locals` (`published_facts.rs:10652-10682`, read in full); read one lowering use `unit_structural_values.rs:5569` (not every lowering caller).
4. Input: `SymbolHandle` for a state that was recorded in `with_states`; lookup uses `state.arena_index()` only (`facts.rs:3441-3446`, read).
5. **Verdict: CLOSED** — Recording walks `program.machine_states` once per state and keys spans by `arena_index()` (`published_facts.rs:10654-10682`, read). Consumers run in lowering after checking (`checking.rs:1266` sets `facts.state_locals`, inferred not opened). Wrong-index collision would require two live states sharing an arena index without a prior check failure; no production path creating that was found.
6. Unsure: whether the symbol allocator can assign two distinct state symbols the same arena index in one checked program.

### A0310

1. Read `state_by_symbol`, `unique_state_by_symbol` (`flow.rs:1339-1357`, read in full).
2. Search `state_by_symbol\(` in `omega-rust`: 294+ hits (count mode, read); did not read every caller.
3. Read test `unique_state_symbol_keeps_duplicates_and_generations_distinct` (`flow.rs:1396-1427`, read); read production append `05_typed-trees-to-checked-trees/src/flow.rs:4866-4898` (read).
4. Input: `SymbolHandle` for duplicate-keyed rows only in tests; production tables receive one append per typed state.
5. **Verdict: CLOSED** — Production flow build does a single `states.append(FlowStateFact { state_symbol: state.symbol, ...})` per state (`flow.rs:4866`, read). Duplicate rows with the same `state_symbol` are constructed in `#[cfg(test)]` (`flow.rs:1401-1425`, read). Critical validation helpers use `unique_state_by_symbol` (e.g. `reference_result_custody.rs:603`, read grep). Entry cites `:1396` for duplicates; that line is the duplicate-symbol test, not `state_by_symbol` behavior alone (read).
6. Unsure: whether any production `state_by_symbol` caller on a post-check path could observe duplicates without going through `unique_state_*` first.

### A0311

1. Read `state_call_entry_constraints` (`flow.rs:2146-2167`, read in full).
2. Search `state_call_entry_constraints\(`: 4 hits (`flow.rs`, `checks_borrows.rs:6345`, `checks_borrows_calls.rs:1694`, read).
3. Read `call_borrow_constraints` and `loans_live_at_call` entry (`checks_borrows_calls.rs:1683-1700`, `checks_borrows.rs:6336-6354`, read); did not read full `borrow_loan_constraints`.
4. Input: borrow check with `borrow_call` coordinates where `state_call(...)` returns `None`; chain uses statement then state `entry_constraints` (`flow.rs:2162-2166`, read).
5. **Verdict: UNDECIDED** — Fallback is real (read). Whether a checked program can reach it with a live borrow check that should reject but sees weaker constraints (and `loans_live_at_call` at `:6353-6354` returning empty when no constraints) was not traced to an acceptance decision.
6. Unsure: correspondence between `BorrowCallFact` rows and `FlowCallFact` keys at borrow-check time.

### A0312

1. Read `scalar_exits`, `scalar_exit_paths`, `scalar_destination` (`flow_ownership_and_roots.rs:881-914`, read in full).
2. Search `scalar_exits`: 2 definition-site hits in `flow_ownership_and_roots.rs`; module `scalar_exits` in `checks_contracts_exits.rs` is unrelated (read grep).
3. Read callers `definite_call_transfers` (`flow_ownership_and_roots.rs:795-813`, read) and consumer sketch `execution_terminal_unit_state_graph.rs:4825-4850` (partial read).
4. Input: guarded terminator with destinations where `scalar_destination` is `None` (Jump/Crash/InvokeNoNormal, `:899-902`, read); `min()` absent → `0`.
5. **Verdict: UNDECIDED** — Undercount is explicit (`unwrap_or(0)`, read). Whether that lets a program pass state-graph disposal when `definite_call_transfers == 1` is required, or safely pairs with `permits_disposal(Some(statement), ...)` (`state_graph.rs:4850`, partial read), was not proven either way.
6. Unsure: full `permits_disposal`/`validate` behavior for `transfers == 0`.

### A0313

1. Read `call_argument_evaluation_order` (`flow_ownership_and_roots.rs:379-388`, read in full).
2. Search `call_argument_evaluation_order\(`: 6 production hits (read grep).
3. Read publisher `published_facts.rs:10340-10360` (read) and typed-trees `CallArgumentEvaluationOrder::is_natural` (`calls_and_control_flow.rs:1029-1043`, read).
4. Input: `FlowCallFact` with empty `argument_evaluation_order` handle after checking records natural order as empty.
5. **Verdict: CLOSED** — Empty span reproduces authored 0..n order (`:380-381`, read), matching typed-trees natural iterator (`calls_and_control_flow.rs:1043`, read) and explicit recording rule (`published_facts.rs:10347-10348`, read). Not a missing-input acceptance branch.
6. Unsure: none material.

### A0314

1. Read `PartialEq for CheckedUnitStructuralTypePlan` (`flow_terminal.rs:2044-2057`, read) and `adopt_structural_types` (`flow_terminal_units.rs:1520-1543`, read in full).
2. Search `adopt_structural_types\(`: 3 hits; read error path `execution.rs:321-326`, `371-376` (read).
3. Conflict only when `*existing != shape` (`:1532`, read); equality ignores `source_declaration` and `source_application` (`:2044-2057`, read).
4. Input: second plan with same identity/shape fields but different `source_declaration`.
5. **Verdict: REACHABLE** — Second adoption hits `Some(_) => {}` (`:1538-1539`, read) without error even if declarations differ, because `PartialEq` treats plans equal. Smallest bad outcome: two registrations of the same structural identity attributed to different source declarations; checking continues and execution planning keeps the first plan.
6. Unsure: whether any producer ever emits differing `source_declaration` for the same identity in one program.

### A0315

1. Read `call_selector_positions` in full (`flow_terminal.rs:1384-1410`, read).
2. Search `call_selector_positions`: 6 hits; read `unit_arguments.rs:6961-6971` (read).
3. With empty `projection_evaluations`, `authored` is false for every index (`:1396-1397`, read); `FixedIndex` segments omitted unless computed/runtime (`:1399-1407`, read).
4. Input: structural argument path with literal `FixedIndex` and default-empty `projection_evaluations` (common in terminal planning, e.g. `execution_terminal_unit_state_graph.rs:217`, grep).
5. **Verdict: CLOSED** — Omission is conditional on empty evaluation span (read). Lowering iterates only reported selectors (`unit_arguments.rs:6961`, read); constant fixed indices need no scalar selector step. Not traced through every `FixedIndex` + dynamic mix edge case in lowering.
6. Unsure: paths mixing `FixedIndex` with nonempty evaluations where inherited count should surface selectors.

### A0316

1. Read `CheckedGuardedJumpFallbackPlan::statement_ordinal`, `CheckedClosedSumCasePlan::statement_ordinal`, and `CheckedConditionalReturnArm::statement_ordinal` (`flow_terminal_units.rs:968-1007`, `881-906`, read).
2. Search `statement_ordinal\(\)\.unwrap_or\(0\)`: 2 sites (`:972`, `:1005`, read).
3. Read `source_statement_index` on operations returning `None` for cleanup variants (`flow_terminal_units.rs:2253-2255`, read).
4. Input: return arm wrapping `ContinuationCleanup` / `Complete` / `ReleaseReference` → `statement_ordinal()` is `None` → `0` in fallback/case ordinals.
5. **Verdict: REACHABLE** — `unwrap_or(0)` (`:972`, `:1005`, read) feeds ordinals used by state-graph disposal (`execution_terminal_unit_state_graph.rs:4838`, partial read). A return arm without a retained ordinal is reported as statement 0, which can skew disposal checks vs the real site.
6. Unsure: whether planners emit such return arms on programs that reach checking success.

### What I did not read

- Full bodies of all 294+ `state_by_symbol` call sites.
- `borrow_loan_constraints` and complete `loans_live_at_call` borrow rejection logic.
- Full `permits_disposal`/`validate` in `execution_terminal_unit_state_graph.rs`.
- Remaining lowering callers of `call_selector_positions` beyond `unit_arguments.rs`.
- Symbol allocator rules for arena-index uniqueness across generations.

SLICE COMPLETE

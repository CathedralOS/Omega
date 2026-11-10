# slice_0041 (rows A0309–A0316)

Claim: ACCEPT, digest area_11 (`digests/area_11.md`, commit `457adf4a11f` preamble read). Path shorthand `ct/` = `omega-rust/psi/compilation/representations/checked-trees/src/`.

## Summary table

| id | file:line | verdict | one-line reason |
|---|---|---|---|
| A0309 | ct/facts.rs:3440-3447 | CLOSED | `record_state_locals` keys one row per `state.symbol`; lowering passes that same handle |
| A0310 | ct/flow.rs:1339-1346 | CLOSED | Checking appends one `FlowStateFact` per state; duplicate rows appear only in unit tests |
| A0311 | ct/flow.rs:2162-2166 | REACHABLE | Missing call row makes borrow checks use weaker statement/state entry constraints |
| A0312 | ct/flow_ownership_and_roots.rs:905-914 | UNDECIDED | `0` undercount vs `== 1` ownership in multiplicity; full caller soundness not traced |
| A0313 | ct/flow_ownership_and_roots.rs:379-388 | CLOSED | Empty span is the documented authored-argument order default for lowering |
| A0314 | ct/flow_terminal.rs:2044-2058 | UNDECIDED | Silent merge on `PartialEq` may hide `source_declaration`-only conflicts; consumers not fully traced |
| A0315 | ct/flow_terminal.rs:1384-1410 | CLOSED | `FixedIndex` is compile-time; runtime selectors get `projection_evaluations` before lowering |
| A0316 | ct/flow_terminal_units.rs:972,1005 | CLOSED | Those `statement_ordinal` methods have no production callers; planning rejects `None` ordinals |

## A0309

Digest #41: `StateLocalFacts::locals` indexes `spans` by `state.arena_index()` only (generation not used).

- **read** `ct/facts.rs:3423-3447` — `with_states` sets `spans[state.arena_index()]`; `locals` reads the same index.
- **read** `05_typed-trees-to-checked-trees/src/published_facts.rs:10652-10682` — `record_state_locals` walks `program.machines()` / `machine_states` and pushes one `(state.symbol, locals)` per state per compile.
- **Caller search** `grep '\bstate_locals\.' omega-rust` — **5** hits (4 in `06_checked-trees-to-lowered-psi`, 1 in `05_typed-trees-to-checked-trees/src/checking.rs`); production uses `state_locals.locals(state)` or `state_locals.locals(state.symbol)` with the state's own `SymbolHandle`.
- **Input reaching branch:** A checked `CheckedTrees` after stage 05; callee passes the `SymbolHandle` for the state whose locals are needed.

**Verdict CLOSED:** At publish time each live state appears once in the iterator, so each `arena_index()` slot in `spans` matches the handle lowering passes. Generation is omitted in the table key, but within one checked artifact the handle used at lookup is the same handle used at publish (`read`).

## A0310

Digest #42: `state_by_symbol` returns `rows.first()`; duplicates exist in tests (`ct/flow.rs:1396`).

- **read** `ct/flow.rs:1339-1346` — `KeyedRows::with_rows` + `rows.first()`.
- **read** `ct/flow.rs:1348-1357` — `unique_state_by_symbol` returns `None` when duplicate same `state_symbol` (test at `:1396-1426`).
- **read** `05_typed-trees-to-checked-trees/src/flow.rs:4866-4868` — `build.control.states.append(FlowStateFact { state_symbol: state.symbol, ... })` once per state body in flow construction.
- **Caller search** `grep '\bstate_by_symbol\(' omega-rust` — **~250** matches (including `05/checks_multiplicity.rs`, `validation/calls.rs`, many flow builders).

**Verdict CLOSED:** Production checking appends one flow row per authored state (`read`). Duplicate same-`state_symbol` rows are constructed in `ct/flow.rs` tests only (`read`). For accepted programs, `first()` is the sole row, so acceptance does not depend on arbitrary first-vs-last choice.

## A0311

Digest #43: missing call falls back to statement then state `entry_constraints`.

- **read** `ct/flow.rs:2146-2167` — `state_call(...).map(|call| call.entry_constraints).or_else(statement).unwrap_or(state.entry_constraints)`.
- **read** enclosing `FlowFactsView` methods — `state_call` (`:2130-2144`) `find`s on `statement_index`, `call_ordinal`, `target_symbol`, `receiver_symbol`.
- **Caller search** `grep 'state_call_entry_constraints' omega-rust` — **4** definition/use sites; production borrow checks at `05/checks_borrows.rs:6345` and `05/checks_borrows_calls.rs:1694`.

**Verdict REACHABLE:** A program where borrow checking runs against a `FlowCallFact` coordinate but `state_call` returns `None` (key mismatch or absent row) still enters `state_call_entry_constraints` and receives **state-level** entry constraints instead of call-specific ones (`read`). That is weaker than a call row that would add stricter entry constraints. Plain words: a body with a borrow call whose flow row is missing or keyed differently from the borrow check query can satisfy borrow loan constraints using only the state's generic entry facts.

## A0312

Digest #44: `scalar_exits` returns `0` when no exit paths contribute.

- **read** `ct/flow_ownership_and_roots.rs:905-914` — `min` over path transfer counts, else `0`.
- **read** `scalar_exit_paths` (`:916-969`) — `Crash` terminators add no paths; destinations that yield `None` from `scalar_destination` are skipped.
- **Caller search** `grep 'scalar_exits' omega-rust` — **2** hits in `flow_ownership_and_roots.rs`; reached via `definite_call_transfers` on completion terminators (`:812`).
- **read** `05/execution_terminal_unit_state_graph.rs:4688-4733,4825-4838` — consumers sum `definite_call_transfers`, compare to `0` / `1`, and call `permits_disposal`.

**Verdict UNDECIDED:** Whether reporting `0` transfers when all paths are absent lets a program be **accepted** depends on whether `permits_disposal` / `return_arm_owns` treat `0` as safe in those contexts (`read` partial). Did not read full multiplicity rejection paths for every caller.

## A0313

Digest #45: empty `argument_evaluation_order` means authored order.

- **read** `ct/flow_ownership_and_roots.rs:379-388` — if `call.argument_evaluation_order.is_empty()`, return `(0..authored_arguments.len())`.
- **Caller search** `grep 'call_argument_evaluation_order' omega-rust` — **6** hits (`06/scalar_computations.rs`, `06/shared.rs`, interpreter, validation bridge).

**Verdict CLOSED:** This is the explicit default for missing reorder data and feeds lowering order (`read`). It does not bypass a separate “program rejected” check in the sites opened; accepted programs get deterministic authored order.

## A0314

Digest #46: `PartialEq` for `CheckedUnitStructuralTypePlan` omits `source_declaration`; `adopt_structural_types` treats `==` as non-conflict.

- **read** `ct/flow_terminal.rs:2044-2057` — `eq` compares identity, presentation, natives, shape fields; not `source_declaration`.
- **read** `ct/flow_terminal_units.rs:1520-1542` — `Some(existing) if *existing != shape` errors; `Some(_)` same under `PartialEq` skips push.
- **Caller search** `grep 'adopt_structural_types' omega-rust` — **3** call sites in `05/execution.rs` and `05/execution_terminal_unit_cleanup.rs` (errors on `!= shape`).

**Verdict UNDECIDED:** Two plans differing only in `source_declaration` compare equal and the second is dropped (`read`). Whether that lets a program be accepted with wrong executable metadata depends on downstream use of `source_declaration` (e.g. `05/execution_terminal_unit_types.rs` — not fully read).

## A0315

Digest #47: with empty `projection_evaluations`, `FixedIndex` segments are not selector positions.

- **read** `ct/flow_terminal.rs:1384-1410` — `authored` selector requires non-empty `projection_evaluations`; `FixedIndex` is not in the `ComputedIndex`/`RuntimeIndex` `matches!` arm.
- **read** `05/execution_terminal_unit_calls.rs:6877-6893` — when authored projection selectors exist, `retain_projection_evaluations` binds `projection_evaluations`.
- **Caller search** `grep 'call_selector_positions' omega-rust` — **7** production uses in `06/` lowering; `call_index_selectors` (`06/attached_unit.rs:14151-14164`) requires evaluated count to match `call_selector_positions().count()`.

**Verdict CLOSED:** Compile-time `FixedIndex` segments are intentionally excluded from runtime selector receipts (`read`). Paths that need evaluated selectors must retain `projection_evaluations` during planning (`read`) or lowering fails the count check (`read`).

## A0316

Digest #48: return arm with no statement ordinal reports `0` via `unwrap_or(0)`.

- **read** `ct/flow_terminal_units.rs:968-974,999-1006` — `CheckedGuardedJumpFallbackPlan::statement_ordinal` and `CheckedClosedSumCasePlan::statement_ordinal` use `return_arm.statement_ordinal().unwrap_or(0)`.
- **read** `ct/flow_terminal_units.rs:881-906` — `CheckedConditionalReturnArm::statement_ordinal()` returns `None` for some `Call`/`Structural` shapes when `source_statement_index()` is `None`.
- **read** `05/execution_terminal_unit.rs:4173-4181` — `return_path` uses `.ok_or("completion cleanup: a return arm has no source coordinate")`, not `unwrap_or(0)`.
- **Caller search** `grep '\.statement_ordinal\(\)' omega-rust` — methods at `:968` and `:999` are **not** called elsewhere in `omega-rust` (only definitions at `:972`, `:1005`).

**Verdict CLOSED:** The `unwrap_or(0)` sits in helper methods with **no** production callers (`read` search). Acceptance paths that need a coordinate use `CheckedConditionalReturnArm::statement_ordinal()` and error on `None` (`read`).

## What I did not read

- Full `state_call` publication vs borrow-call key equality in `05/flow*.rs`.
- All `state_by_symbol` consumer branches for duplicate-row sensitivity beyond single-append invariant.
- Complete `execution_terminal_unit_state_graph` multiplicity rejection when `scalar_exits` is `0`.
- All `source_declaration` consumers after `adopt_structural_types`.
- Symbol allocator rules proving unique `arena_index()` per live state in one compile (relied on publish loop structure).

SLICE COMPLETE


# slice_0124

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0952 | omega-rust/psi/compilation/build-time-evaluation/src/evaluation_accounting.rs:499-501 | REACHABLE | Production `EvaluationDomain` uses default (non-recording) accounting while machine evaluation runs, so completion can succeed without receipts for those invocations. |
| A0953 | omega-rust/psi/compilation/build-time-evaluation/src/const_evaluation.rs:307-309 | REACHABLE | `Named` with invalid symbol is treated as a closed type argument, so generic instance realization can proceed on an unresolved name. |
| A0954 | omega-rust/psi/compilation/build-time-evaluation/src/const_evaluation.rs:1794-1807 | REACHABLE | `zero_gated` clearing inspects only the first variant’s `where_facts`, not later variants. |
| A0955 | omega-rust/psi/compilation/build-time-evaluation/src/const_evaluation.rs:4933 | CLOSED | Unfolded pending range endpoints are rejected at stage 04 `require_evaluated_constants`. |
| A0956 | omega-rust/psi/compilation/build-time-evaluation/src/const_generic_expressions.rs:1814-1821 | REACHABLE | Callee-body operator selections and same-machine `TransitionNamed` calls can skip custody recording while evaluation succeeds. |
| A0957 | omega-rust/psi/compilation/build-time-evaluation/src/const_generic_expressions.rs:2689-2695 | REACHABLE | In `Closed` context, `&&` `||` `&` `|` `^` `<<` `>>` bypass `require_operator` and accept builtin evaluation without overload checking. |
| A0958 | omega-rust/psi/compilation/build-time-evaluation/src/const_initializers.rs:940-942 | REACHABLE | Any expression passes `require_expression_carrier` when the destination type is primitive, with no shape check. |
| A0959 | omega-rust/psi/compilation/build-time-evaluation/src/const_initializers.rs:2188-2191 | CLOSED | Declarations without calls or float still run earlier materialized-root and literal-field checks; replay is only required when calls or float demand it. |

## A0952

**Cited (read):** `omega-rust/psi/compilation/build-time-evaluation/src/evaluation_accounting.rs:481-527` — `EvaluationAccounting` defaults with `state: None` (`:481-484`); `finish` at `:499-517` builds empty receipts and `validate_charges` succeeds on an empty list; `reserve_invocation` at `:525-527` returns `Ok(None)` when `state` is absent, so `evaluate` / `evaluate_terminal` at `:597-619` and `:548-594` run without recording.

**Callers (search `EvaluationAccounting` in `omega-rust/psi/compilation/build-time-evaluation`, count mode):** 4 files, many sites; production domain wiring uses `Default::default()` at `lib.rs:115,126` for `EvaluationDomain::selected` and `::all`; `evaluate_or_defer_recorded` swaps in `recording()` only at `lib.rs:601-610` for the batch `finish`, while `evaluate_work` at `lib.rs:720-777` passes `self.domain` (often `all()` with default accounting from `evaluate_pre_resolution` at `lib.rs:248-250`).

**Trace (read):** `BuildTimeAdmissionPlan::with_accounting` at `lib.rs:97-104` clones domain accounting into admission; machine runs during `PreCheckEvaluation::evaluate_work` therefore use non-recording accounting. `SourceCompletionAccounting` at `psi-compiler/src/source_compilation.rs:1067-1080` merges only receipts from `evaluate_or_defer_recorded`; `validate_charges` at `:331` cannot see unmetered invocations.

**Input that reaches the branch:** Any package whose pre-check runs `evaluate_work` under `EvaluationDomain::all()` (default accounting) before `evaluate_or_defer_recorded` records a separate receipt set.

**REACHABLE program (plain words):** A compilation unit that triggers substantial build-time machine execution during pre-check with the default domain accounting completes and passes source completion charge validation even though those particular evaluations never produced accounting receipts.

## A0953

**Cited (read):** `const_evaluation.rs:305-325` — `closed_argument` returns true for `TypeReferenceNode::Named` when `!symbol.is_valid()` (`:307-308`) or builtin/data symbol (`:309-312`).

**Callers (search `\bclosed_argument\b` in `omega-rust`, count mode):** 2 files — `const_evaluation.rs` (definitions and uses at `:250,317,384`) and `pipeline/05_typed-trees-to-checked-trees/src/shared.rs` (separate helper name collision at `:4260`; not this function).

**Enclosing functions (read):** `realize_folded_applications` (`:204-272`) and `realize_symbolic_applications` (`:347-405`) gate generic substitution on `closed_argument` for every argument before `realize_data_instance` (`:408-437`).

**Trace (read):** A generic use site whose type argument is still `Named { symbol: invalid, .. }` satisfies `closed_argument` and can enter realization; downstream checks in `realize_data_instance` (`:426-437`) do not re-validate that the name resolved.

**REACHABLE program (plain words):** A generic data application that still carries a mistyped or unresolved type name (invalid symbol) is treated as having closed arguments and may be realized as a synthetic instance instead of being rejected at this gate.

## A0954

**Cited (read):** `const_evaluation.rs:1703-1808` — `evaluate_selected_domain_facts`; ungated filter at `:1794-1802` uses `data_members(data).find_map` for the first `DataMember::Variant` only and requires `variant.where_facts.is_empty()` for that first variant; `zero_gated` cleared at `:1805-1807`.

**Callers (search `evaluate_selected_domain_facts` in `omega-rust`, count mode):** 8 matches in 4 files (`const_evaluation.rs`, `lib.rs`, `layouts.rs`, `preparation_const_evaluation.rs`).

**Trace (read):** `layouts.rs:997-1001` rejects layout work when `data.zero_gated` is still true; clearing at `const_evaluation.rs:1807` depends on first-variant facts only. `validation/value_custody_core.rs:4126-4127` uses `zero_gated` for establishment.

**REACHABLE program (plain words):** A multi-variant data type whose first variant has no qualifying facts but a later variant carries domain facts that fail at zero can have `zero_gated` cleared and proceed past gates that assume the whole type is zero-blocked.

## A0955

**Cited (read):** `const_evaluation.rs:4912-4937` — loop in range-endpoint evaluation; on `evaluate_round` error, `Err(_) if plan.calls.is_empty() => break` (`:4933`) exits without diagnostic; successful fold path emits errors at `:5027-5032` only when `required` (`:5020-5026`).

**Callers (search `evaluate_selected_range_endpoints` in `omega-rust`, count mode):** 5 matches in `const_evaluation.rs` and `lib.rs` (`evaluate_work` at `lib.rs:732-740`).

**Closing check (read):** `pipeline/04_typed-trees-to-typed-trees/src/preparation.rs:363-437` — `deferred_constants::require_evaluated_constants` scans `pending_const_range_endpoints` (`:415-432`) and errors with `"range endpoint has an unresolved build-time evaluation dependency"`; called from same file at `:1958` during stage 04 preparation.

**Why every path goes through it:** Endpoints left pending remain in `typed.pending_const_range_endpoints` (inserted at `const_evaluation.rs:4880-4883`); stage 04 monomorphization preparation invokes `require_evaluated_constants` before checked lowering.

**Input:** A range endpoint expression that fails a round with no pending calls still listed in `pending_const_range_endpoints` after BTE.

## A0956

**Cited (read):** `const_generic_expressions.rs:1349-1360` — `TransitionNamed` in a reflection body on the same machine returns `Ok(())` without custody; `:1814-1836` — `Kind::Operator` in closure walk records custody only when `context.initializer` or syntax owns the selection, otherwise `continue`; `:1837-1866` — resolved `Const` selections handled, other kinds fall through without custody.

**Callers:** Custody walk lives in closure validation for const initializer materialization (module around `:1320-1896`); invoked from const generic expression evaluation driver (not fully enumerated; read enclosing `selected_retained_call` and expression descent).

**Trace (read):** Operator evaluations inside callee machine bodies that are not classified as initializer-owned skip `custody.operators` push (`:1834-1836`); transitions between states of the same reflection machine skip selection attachment (`:1349-1360`).

**REACHABLE program (plain words):** A compile-time probe whose correct authored selections include operators or transitions executed only inside a generated callee body can evaluate to a folded constant while custody records omit those selections, so a mismatch with source selections would not be caught at this layer.

## A0957

**Cited (read):** `const_generic_expressions.rs:2660-2695` — `require_operator` returns `Ok(())` immediately for `And|Or|BitwiseAnd|BitwiseOr|BitwiseXor|ShiftLeft|ShiftRight` when context `is_closed()` (`:2667-2695`); other operators continue to builtin / trait checks at `:2697-2729`. `evaluate_integer_endpoint` uses `EvaluationContext::Closed` at `:5671-5676`; anonymous numeric fold at `:5680-5685` uses `|_| true`.

**Callers (search `require_operator` in `const_generic_expressions.rs`, read):** Used from closed evaluation paths in the same module’s evaluation context (`:2660`).

**Trace (read):** Logical and bitwise operators in closed const evaluation never reach the `"closed constant operator requires exact context-independent builtin meaning"` error (`:2726-2728`).

**REACHABLE program (plain words):** A closed compile-time integer endpoint expression that uses a user-overloaded logical or bitwise operator on scalar operands can be evaluated with builtin meaning and accepted without proving the overload matches authored selection.

## A0958

**Cited (read):** `const_initializers.rs:915-942` — `require_expression_carrier`; after `Match` handling, if `typed.primitive_type_reference(destination).is_some()` then `return Ok(())` (`:940-942`) before expression shape checks at `:944-1028`.

**Callers (search `require_expression_carrier` in `omega-rust/psi/compilation/build-time-evaluation`, count mode):** 6 call sites, all in `const_initializers.rs` (`:843,899,929,973,1011`).

**Trace (read):** Invoked from typed initializer probe validation (`:843,899`); primitive destination bypasses `expression_result_type_reference` matching (`:1024-1028`).

**REACHABLE program (plain words):** A const initializer probe declared to return a primitive carrier accepts a non-scalar expression shape (for example a struct or call result) at this check because the primitive destination triggers an unconditional pass.

## A0959

**Cited (read):** `const_initializers.rs:2109-2193` — `validate` replay loop; `validate_constant_literal_fields` at `:2123-2128`; `has_calls` at `:2188`; `continue` at `:2189-2190` skips replay when no retained/authored call and no floating component; replay path at `:2192+` when calls exist.

**Callers (search `const_initializers::validate` / module `validate` from checked initializers path, read):** Part of checked const initializer pipeline in same file (invoked from higher const initializer evaluation after materialization).

**Trace (read):** Paths with `retained_call || authored_call` or float destinations always hit `validate_replay_roots` (`:2171-2172,2192`); skipped declarations still required valid materialized root (`:2114-2122`) and literal field structure (`:2123-2128`).

**Closing paths:** No-call, non-float constants intentionally skip replay because they cannot carry call receipts (per slice note); custody for call-bearing constants still enters replay block.

## What I did not read

- Full `area_07.md` beyond the preamble and cited index lines (read: lines 1–78).
- `validate_replay_roots`, `contains_floating_component`, and the full `has_builtin_operator_selection` implementations (partial context only where cited).
- `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/shared.rs` `closed_argument` helper body (distinct from BTE `closed_argument`).
- Stage 02 symbol resolution paths that might reject invalid `Named` references before BTE sees them (not searched).
- Entire `evaluate_round` / `integer_root_plan` control flow beyond cited ranges for A0955.

SLICE COMPLETE

# slice_0023

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0173 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs:6880-6882, 7552 | REACHABLE | Empty membership predicate lists and zero-row exact-interval folds become `Constant(true)` and are returned as satisfied predicates. |
| A0174 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs:10938-10941 | REACHABLE | Symbol-invalid named references take the first `data_definition_positions_for_name` entry while homonymous defs can coexist. |
| A0175 | omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values_scalar.rs:5429 | REACHABLE | Planners return without recording rows; `build_checked_scalar_expression_plans` always completes and `build_check_facts` publishes anyway. |

## A0173

**Cited sites (read):** `values_scalar.rs:6880-6882` in `result_contract::lower_callable_scalar_membership_value` and `values_scalar.rs:7552` in `ContractPredicates::exact_predicate` (`values_scalar.rs:7523-7600`). Line numbers match the atlas cites at this checkout; not re-diffed with `git show 457adf4a11f:`.

At `:6880-6882`, after `validation::scalar_membership_value_predicates` returns `Some`, the iterator’s first element is `unwrap_or(CheckedBooleanExpression::Constant(true))`, so an empty vector folds to constant true before `And`-folding any remaining elements (`values_scalar.rs:6877-6888`). In `domains_and_predicates.rs:14502-14530`, `project` can return `Some(predicates)` with `predicates` still empty when the domain fact span yields no `Expression` or nested `Membership` rows.

At `:7552`, `exact_predicate` sets `predicate = CheckedBooleanExpression::Constant(true)` then conjoins interval rows (`values_scalar.rs:7549-7599`). `validation::exact_predicate_intervals` (`domains_and_predicates.rs:6836-6857`) returns `Some(rows)` only when `rows.len() == application.arguments.len()` and unfolding is decided true; with zero arguments, `rows` may be empty and the function still returns `Some(Constant(true))`.

**Enclosing functions (read):** Full `lower_callable_scalar_membership_value` (`values_scalar.rs:6850-6889`). Full `exact_predicate` (`values_scalar.rs:7523-7600`). `ContractPredicates::boolean` (`values_scalar.rs:7602+`) calls `exact_predicate` at `:7662`.

**Callers (read):** `rg '\blower_callable_scalar_membership_value\b' omega-rust` → 3 matches (definition plus call from `lower_callable_scalar_membership` at `values_scalar.rs:6841`, and `execution_terminal_unit_control.rs:140`). `rg '\blower_callable_scalar_membership\b' omega-rust` → 4 matches (definition at `values_scalar.rs:6832`; calls at `published_facts_boolean_predicates.rs:1847`, `execution_terminal_unit_types.rs:313`, `execution_terminal_unit_control.rs:142`). `rg '\bexact_predicate\b' omega-rust` → 2 matches, both in `values_scalar.rs` (definition and call from `boolean`).

**Input that reaches the branch (read):** A callable `ProofMembershipFact` whose domain projection yields zero boolean predicates still produces `Some(Constant(true))` and is pushed into closed contract rows in `execution_terminal_unit_control.rs:139-177`. A contract/exact-predicate expression whose interval application has zero arguments and passes `exact_predicate_intervals` stays `Constant(true)` at `:7552`.

**Verdict:** REACHABLE — a membership or exact-interval obligation with no substantive predicates is published as trivially true instead of failing closed.

## A0174

**Cited site (read):** `values_scalar.rs:10914-10943`, function `structural_fields::structural_data`. Atlas cite `10938-10941` matches the `symbol.is_valid()` else branch using `.data_definition_positions_for_name(name.as_str()).first()`.

**Enclosing function (read):** Full `structural_data` from `values_scalar.rs:10914` through `:10943` (peeled named/generic references, `closed_data_definition` fast path, then symbol or first-by-name).

**Callers (read):** `rg '\bstructural_data\(' omega-rust --glob '*.rs'` → 12 matches: 1 definition at `values_scalar.rs:10914`; 6 in-file uses (`values_scalar.rs:2757`, `:5831`, `:5859`, `:5924`, `:10685`, `:10713`); `published_facts_crash_entry_values.rs:570`; `execution.rs:470`, `:2154`; `execution_scalar_computations.rs:3758`, `:3759`, `:3977`. Re-export at `values.rs:3506` (read import list, not a call).

**Homonym evidence (read):** `declaration_tables_and_index.rs:96` (test) asserts `data_definition_positions_for_name("Shared")` is `[0, 3]`. `declarations_core.rs:1690-1705` emits `duplicate data` only when duplicate paths overlap a tracked set, not for every homonym.

**Downstream (read):** Some sites compare `structural_data(...)? .symbol` to an expected owner (`values_scalar.rs:2757-2758`, `:5831-5832`) and return `None` on mismatch; others use the resolved `DataDefinition` without a symbol cross-check (`values_scalar.rs:5859-5864`, `execution_scalar_computations.rs:3758-3759`).

**Input that reaches the branch (read):** A peeled `TypeReferenceNode::Named` or `Generic` with `symbol` invalid (spelling-only reference per area_09 digest `VS:4383-4389`, not opened) and a name shared by multiple data definitions resolves to index `positions[0]`.

**Verdict:** REACHABLE — the compiler can bind structural layout to the first homonymous data definition when the type reference carries no symbol, including paths that never re-check the owner symbol.

## A0175

**Cited site (read):** `values_scalar.rs:5417-5430` inside `build_checked_scalar_expression_plans` (`values_scalar.rs:5370-5452`). The `_ => {}` arm skips scalar planning for statement kinds not matched (including `StatementNode::Assembly` and `StatementNode::AssemblyFact` per `calls_and_control_flow.rs:3560-3567`, read).

**Enclosing function (read):** Full `build_checked_scalar_expression_plans` (`values_scalar.rs:5370-5452`).

**Planner failures (read):** `assignment::plan` returns without recording when target type, primitive type, or `lower_return_expression` is `None` (`values_scalar.rs:4681-4706`). `call_statement::plan` and `retain_expression_call_arguments` return when `lower_call_arguments` is `None` (`values_scalar.rs:4730-4753`, `:5309-5326`). `expression_statement::plan` only records when `lower_return_expression` succeeds (`values_scalar.rs:4767-4795`).

**Callers of build (read):** `rg 'build_checked_scalar_expression_plans' omega-rust` → 3 symbol hits: definition `values_scalar.rs:5370`, re-export `values.rs:3493`, sole production call `published_facts.rs:11033`.

**Accept path (read):** `build_check_facts` assigns the plans struct directly (no `Result`) at `published_facts.rs:11033-11039`, seals expressions, and continues the checking loop (`published_facts.rs:11040-11059`). No completeness check on scalar rows in this segment.

**Execution consumers (read):** `execution_scalar_computations.rs:9561-9597` `record_root` records a computation root only when `self.expression(...)` returns `Some`; otherwise it returns without recording. `execution_terminal_unit_calls.rs:8918-8919` uses `bound_expression_at(...)?`, which drops that sub-plan path when the pure row is missing, without surfacing a program-level diagnostic in the snippet read.

**Input that reaches the branch (read):** A state statement that type-checks but whose assignment value, call arguments, or return expression fails a scalar lowering step in the planner modules above, or an `Assembly` / `AssemblyFact` statement handled only by `_ => {}`.

**Verdict:** REACHABLE — statements can carry no scalar plan row while check-fact publication proceeds; downstream execution planning may omit roots or skip sub-plans rather than reject the program in the paths read.

## What I did not read

- Full fan-out of every `CheckedScalarExpressionPlans` consumer (flow, ETU, ESC `build_expression` ladder past `execution_scalar_computations.rs:9932`, corpus outcomes).
- Whether stage-04 typing always prevents assignment/call sites that fail `lower_return_expression` / `lower_call_arguments` from reaching stage 05.
- Language specification for whether empty domain fact lists or zero-argument exact predicates are intended to be vacuous.
- All homonym scenarios beyond the `Shared` index test and duplicate-data diagnostic scope in `declarations_core.rs`.
- `git show 457adf4a11f:` line drift check for every cited line.

SLICE COMPLETE

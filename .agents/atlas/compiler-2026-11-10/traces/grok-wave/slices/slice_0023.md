slice_0023

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0173 | values_scalar.rs:6880-6882, 7552 | REACHABLE | Empty membership or interval predicate lists become `Constant(true)` and publish as satisfied contract predicates. |
| A0174 | values_scalar.rs:10938-10941 | REACHABLE | Symbol-less named type references pick the first homonymous data definition with no ambiguity check. |
| A0175 | values_scalar.rs:5429 (+ planners) | REACHABLE | Planners silently skip failed scalar lowering; checking continues with no row for that statement site. |

### A0173

**Citation note (read):** Digest cites `VS:6880-6882` and `VS:7552`. At commit `457adf4`, `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/values.rs` is 3589 lines (read via `git show`). The same line numbers and code appear in `values_scalar.rs` (included from `values.rs` as the `scalar` module). Below cites use `values_scalar.rs`.

1. **read** `lower_callable_scalar_membership_value` in full at `values_scalar.rs:6850-6889`: after building `subject`, calls `validation::scalar_membership_value_predicates` (`6877-6878`), takes `.into_iter()`, then `predicates.next().unwrap_or(CheckedBooleanExpression::Constant(true))` (`6880-6882`), folds remaining with `And`.
2. **read** `validation::scalar_membership_value_predicates` / `project` in `domains_and_predicates.rs:14458-14530`: on success, `project` can return `Some(predicates)` with `predicates` still empty when the domain’s `proof_facts` span yields no `Expression`/`Membership` facts (`14502-14530`).
3. **Search:** `rg '\blower_callable_scalar_membership_value\b' /agent/repos/omega/omega-rust` → **3 hits** (definition + internal call at `values_scalar.rs:6841`; production reference at `execution_terminal_unit_control.rs:140`). **read** wrapper `lower_callable_scalar_membership` at `values_scalar.rs:6832-6844`. **Search:** `rg '\blower_callable_scalar_membership\b' /agent/repos/omega/omega-rust` → **4 hits** (`values_scalar.rs`, `published_facts_boolean_predicates.rs:1847`, `execution_terminal_unit_types.rs:313`, `execution_terminal_unit_control.rs:140-142`).
4. **read** caller `boundary_scalar_requires` (`execution_terminal_unit_control.rs:26-187`): membership facts call `project(...)` (`144-151`); a `Constant(true)` predicate is still pushed into `rows` (`167-177`). Failure is `None` from `?`, not empty success.
5. **read** `ContractPredicates::exact_predicate` in full at `values_scalar.rs:7523-7600`: after ordering/condition arms, loads `rows` from `validation::exact_predicate_intervals` (`7549`), sets `let mut predicate = CheckedBooleanExpression::Constant(true)` (`7552`), conjoins interval comparisons in the loop (`7593-7597`), returns `Some(predicate)` even when the loop never runs (`7599`).
6. **read** `validation::exact_predicate_intervals` at `domains_and_predicates.rs:6836-6857`: returns `Some(rows)` when `unfold(...).decided() == Some(true)`; `rows` may be empty (`interval_consequences` can push nothing, `6901`).
7. **read** `ContractPredicates::boolean` at `values_scalar.rs:7602-7664`: calls `self.exact_predicate` (`7662`) and returns the result unchanged. **read** `lower_callable_scalar_contract_predicate` in full at `6768-6790` (wraps `ContractPredicates::boolean`). **Search:** `rg 'lower_callable_scalar_contract_predicate\(' /agent/repos/omega/omega-rust` → **11 hits** in 9 production files (not fully read: all call-site handlers).
8. **Input reaching the branch:** (a) A `ProofMembership` whose carrier and subject lower successfully but whose domain theory projects to an empty `Vec` of boolean predicates (`6880-6882`). (b) A boolean contract expression for which `exact_predicate_intervals` returns `Some(rows)` with `rows` empty, so `exact_predicate` never adds comparisons (`7552-7599`).

**Verdict: REACHABLE.** A boundary or callable contract can retain a `Requires` predicate that is constantly true when the domain membership theory is empty or interval consequences contribute no rows but unfolding still decides true, so an obligation that should constrain the subject is published as satisfied (`execution_terminal_unit_control.rs:175-177`, `published_facts.rs:3161-3179` — read call sites only, not full check pass).

**Unsure:** Whether validation always requires non-empty domain `proof_facts` whenever a membership fact must carry semantic force (did not read domain admission in `program_validation.rs` / `proof_contracts_core.rs` end-to-end).

### A0174

1. **read** `structural_data` in full at `values_scalar.rs:10914-10943`.
2. **Entry accuracy (read):** For `TypeReferenceNode::Named` or `Generic` with `closed_data_definition(peeled)` absent, if `symbol.is_valid()` uses `program.data_definition_by_symbol(symbol)` (`10935-10936`); **else** uses `program.data_definition_positions_for_name(name.as_str()).first().map(|position| &program.data_definitions()[*position as usize])` (`10938-10941`).
3. **Search:** `rg '\bstructural_data\(' /agent/repos/omega/omega-rust` → **13 hits** (7 in `values_scalar.rs`, 3 in `execution_scalar_computations.rs`, 2 in `execution.rs`, 1 in `published_facts_crash_entry_values.rs`). **read** representative callers: enum equality at `execution_scalar_computations.rs:3758-3764`, case/path typing at `values_scalar.rs:5831`, `10685`, `10713`.
4. **read** contrast `find_data_definition` in `validation/value_custody_core.rs:1725-1734`: invalid symbol uses `data_definition_by_name_str` (first linear scan, `2929-2932` in `declaration_tables_and_index.rs`), not identical to `positions_for_name().first()` but same “first wins” pattern.
5. **read** `declaration_tables_and_index.rs:1870-1872` and test at `67-96` showing `data_definition_positions_for_name("Shared")` can return multiple positions — homonymous definitions exist in-table.
6. **read upward only for** `execution_scalar_computations.rs:3758-3765`: wrong definition can still fail later via `left_owner.symbol != right_owner.symbol` or identity mismatch; did **not** read all callers for whether mismatch always rejects the program.

**Input reaching the branch:** A peeled `Named`/`Generic` type reference with invalid `symbol` (and no closed instance on the handle), so resolution falls through to the first index in `data_definition_positions_for_name`.

**Verdict: REACHABLE.** A program can carry multiple data definitions under the same name and a symbol-less named type reference; structural lowering and enum/scalar checks use the first indexed definition (`10938-10941`) without requiring uniqueness, so checks keyed to the wrong carrier layout can proceed if later symbol/identity comparisons still align by accident.

**Unsure:** Whether every `CheckingInput` with invalid `symbol` on a value-carrying type reference is already rejected before stage 05 (did not trace full type-reference binding in validation).

### A0175

1. **read** `build_checked_scalar_expression_plans` in full at `values_scalar.rs:5370-5452` (includes `StatementPlanner` loop).
2. **read** `match statement` at `5417-5429`: handled kinds dispatch to planner modules; `_ => {}` (`5429`) skips all other `StatementNode` variants with no row and no error.
3. **read** `assignment::plan` in full at `4618-4720`: `lower_index_expression` failure `continue`s (`4666-4667`); `lower_return_expression` failure `return`s without `record_scalar_value_pair` (`4704-4705`).
4. **read** `expression_statement::plan` in full at `4767-4796`: records return scalar only when `lower_return_expression` returns `Some` (`4777-4794`).
5. **read** `lower_return_expression` header/logic at `8959-9002`: returns `None` when boolean/scalar lowering paths fail or primitive type does not match (`8998-9001`).
6. **Search:** `rg '\bbuild_checked_scalar_expression_plans\b' /agent/repos/omega/omega-rust` → **4 hits** (definition, re-export in `values.rs`, call at `published_facts.rs:11033-11038`). **read** caller: assigns plans and immediately `seal_expressions()` with no completeness guard (`11033-11039`).
7. **read** consumer `FlowBuildContext::selected_scalar_at` at `flow_context.rs:917-956`: missing expression row yields `Some(None)` (`949-950`), not `Err`.

**Input reaching the branch:** An admitted primitive return/assignment/expression statement where `lower_return_expression` (or related lowering) returns `None`, or a statement variant excluded from the `match` at `5417-5428`.

**Verdict: REACHABLE.** Validation-admitted statements can exit planners with zero scalar rows for that ordinal; publication continues (`published_facts.rs:11033-11039`), and flow treats the site as having no selected scalar (`flow_context.rs:949-950`) rather than failing the compile, so behavior can proceed with weakened scalar evidence.

**Unsure:** Whether `scalar_computations` / `structural_values` roots always cover gaps for completion-sensitive states (`execution_terminal_unit_control_statement_sequence.rs:9036-9045` — read snippet only).

### What I did not read

- Full `validation::unfold` and predicate-application admission (`domains_and_predicates.rs` beyond cited slices).
- Full validation pipeline ensuring unique data names or mandatory type-reference symbols before `CheckingInput`.
- All nine call sites of `lower_callable_scalar_contract_predicate`.
- Entire planner modules (`local_data::plan`, `call_statement::plan`, `transition::plan`, `array_elements::plan`) beyond cited regions.
- Corpus/fixture evidence for end-to-end compile outcomes.

SLICE COMPLETE

# Area 07 — accepts when it does not know

Read-only trace of every entry classed `open` in `.agents/atlas/compiler-2026-11-10/digests/area_07.md`. Citations are `file:line` at `457adf4a11f`. `git diff --stat 457adf4a11f HEAD --` the six crates in this area is empty, so those lines still match. Each claim is from lines I read unless it says inferred.

Paths below are under `omega-rust/`. `BTE/` is `psi/compilation/build-time-evaluation/src/`. `S0N/` is `psi/compilation/pipeline/0N_.../src/`. `V/` is `psi/compilation/validation/src/`.

| # | file:line | verdict | reason |
| --- | --- | --- | --- |
| 1 | BTE/evaluation_accounting.rs:499-501, 525-527 | CLOSED | The step budget and the terminal work meter run before the non-recording branch skips the receipt. |
| 2 | BTE/const_evaluation.rs:307-309 | REACHABLE | An unused type parameter instantiated with a name that resolves to nothing is treated as closed and never reported. |
| 3 | BTE/const_evaluation.rs:1794-1807 | CLOSED | Zero is the first variant, and a later variant's facts are judged when that case is constructed. |
| 4 | BTE/const_evaluation.rs:4933, 5019-5034 | UNDECIDED | A failed endpoint with no pending call is left unfolded, and the later pending-set diagnostic does not see it. |
| 5a | BTE/const_generic_expressions.rs:1814-1821 | UNDECIDED | An operator selection in a callee body is not recorded. Whether that body then applies an authored overload was not followed. |
| 5b | BTE/const_generic_expressions.rs:1837-1866 | UNDECIDED | Selection kinds other than call, operator, or resolved const are ignored. Whether one can disagree with the node was not followed. |
| 5c | BTE/const_generic_expressions.rs:1349-1361 | CLOSED | The early success is only for a toolchain `reflect.omg` helper whose transition stays inside that same machine. |

Entries 6 onward are not written yet.

## 1. `BTE/evaluation_accounting.rs:499-501, 525-527`

`EvaluationAccounting` (`evaluation_accounting.rs:481-653`, `finish`, `reserve_invocation`, `evaluate_terminal`, and `evaluate` read in full). Search `EvaluationAccounting::(default|recording)|with_accounting\( `: 16 hits, all in this crate (`const_initializers.rs` 3, `lib.rs` 3, `machine_execution.rs` 7, `const_evaluation.rs` 3). A second search `accounting: Default::default\(\)` finds the three constructors at `lib.rs:115`, `lib.rs:126`, and `machine_execution.rs:3450`.

`Default` leaves `state: None`. `finish` then builds an empty receipt list and `validate_charges` succeeds (`:499-518`). `reserve_invocation` returns `Ok(None)` (`:525-527`). Neither branch runs a machine.

The branches that do run a machine still meter it:

- `evaluate` always calls `checked_interpreter::evaluate_prepared_build_time_machine` (`:614-615`) before the `if let (Some(receipts), ...)` that writes a receipt (`:616-651`). That interpreter sets `CONST_EVAL_STEP_BUDGET` (`psi/interpreter/checked-interpreter/src/interpreter.rs:959`, read) and `tick` returns a trap when `work_units() > step_budget` (`execution.rs:2318-2324`, read).
- `evaluate_terminal` always builds `WorkMeter::with_budget(PURE_PSI_WORK_BUDGET)` (`:566`) and passes it to `execute` before the same `if let` (`:569-593`). `evaluate_checked_constant` turns `SponsorExhausted` into an error (`machine_execution.rs:4980-4988`, read).

Production does use the default. `EvaluationDomain::all` and `::selected` start with it (`lib.rs:115`, `:126`). `BuildTimeAdmissionPlan::infer` starts with it (`machine_execution.rs:3450`). Array-length evaluation passes `self.domain.accounting` into `evaluate_checked_constant` (`const_evaluation.rs:2810`, read). `evaluate_or_defer_recorded` replaces the domain's accounting with `recording()` before it runs (`lib.rs:601-602`, read), then puts `Default` back on a deferred domain (`:605-606`). The tests at `machine_execution.rs:5168-5345` pass `EvaluationAccounting::default()` on purpose. Those paths still go through `evaluate` or `evaluate_terminal`.

Digest is wrong that a non-recording accountant makes evaluation unmetered. The receipt is a record of a run the meter already bounded. Skipping the receipt does not admit a program the meter rejected.

Read in full: the four methods above, `evaluate_structured_return` through the budget configuration (`interpreter.rs:935-959`), `tick` and `configure_build_evaluation` (`execution.rs:2318-2344`), `evaluate_checked_constant` (`machine_execution.rs:4937-5003`). Not read: the five test bodies that pass `default()`, past the call. Not read: `validate_charge` on a terminal receipt past `:408-414`.

## 2. `BTE/const_evaluation.rs:307-309`

`const_applications::closed_argument` (`const_evaluation.rs:305-326`, read in full). Search `closed_argument\(` in this file: definition `:305`, recursive call `:317`, callers `:250` and `:384`. No other caller.

A `Named` argument is closed when its symbol is invalid, or when a valid symbol is a builtin or a data type (`:307-312`). Folded integers are stored as `Named` with an invalid symbol on purpose; that shape is the same as a name that never resolved.

Both callers instantiate when every argument is closed:

- `realize_folded_applications` (`:204-273`, read in full) also requires that some argument contains a just-folded site (`:242-250`), then calls `realize_data_instance`.
- `realize_symbolic_applications` (`:347-406`, read in full) requires one zero-argument const call and every other argument closed or such a call (`:381-384`).

`realize_symbolic_applications` is the defer path (`lib.rs:651`, read). `realize_folded_applications` runs after a successful const-application fold (`const_evaluation.rs:197`, called from `evaluate_selected_const_applications`, which `evaluate_work` calls at `lib.rs:754`).

An unresolved name does reach this. Stage 02 writes `SymbolHandle::invalid()` on `NotFound` or `Ambiguous` (`S02/symbols.rs:1758-1784`, read) and stores it on a `Named` type (`S02/symbols_type_references.rs:903-908`, read). Stage 03 copies that symbol into the typed node (`S03/type_reference.rs:2946-2967`, read) and does not reject an invalid symbol. Build-time evaluation runs in `complete_authored` before checking (`psi-compiler/src/source_compilation.rs:1064-1073`, read).

`realize_data_instance` (`:408-509`, read in full) builds the instance for a plain record: no `where` facts, not zero-gated, no lifetimes, no quotient, no attached machine, parameters only type/const/value, every member a field (`:426-450`). `substitute_member_type` (`:511-613`, read in full) copies a `Named` argument onto a field only when a field's type is that parameter (`:517-518`). A parameter that no field mentions is not copied.

Checking then walks field types (`V/value_custody_core.rs:3808-3825`, read) and reports an unknown data type (`V/type_references_and_wire.rs:5452-5458`, read). A stored unknown name is rejected. The use site itself is replaced by the new data symbol (`const_evaluation.rs:259-264`), so that walk no longer sees the argument. The origin generic is checked only for declared property bounds (`V/type_references_and_wire.rs:5049-5077` and `:3005-3064`, both read). No bound, no diagnostic. A search for an unused-type-parameter diagnostic in `psi/compilation` found none.

Verdict: a generic record whose type parameter is not stored in any field, applied to a name that resolves to nothing together with a const integer argument that folds or is a zero-argument const call. The application becomes a concrete data type and the unknown name is never reported.

Digest is right.

Not read: every `generic_instance` consumer. Opened and not a rejection of an unknown name: `applied_const_argument` (`V/domains_and_predicates.rs:11838-11856`), `data_predicates_admit_zero` (`V/contract_entailment.rs:12532-12546`), `validate_boundary_trait_value_placement` (`V/type_references_and_wire.rs:4988-5022`).

## 3. `BTE/const_evaluation.rs:1794-1807`

`evaluate_selected_domain_facts` (`const_evaluation.rs:1703-1809`, the ungating tail read; `data_fact_holds_at_zero` `:1105-1114` read). Search `evaluate_selected_domain_facts\(`: the call in this crate from `lib.rs:743` was opened. The same name in `layouts.rs` and in stage 02 was not opened.

The clear runs only for a generic instance whose membership evaluated to true, and only when every remaining data-level fact holds at zero (`:1782-1793`) and the first variant's `where_facts` is empty (`:1794-1801`). `data_fact_holds_at_zero` is true only for `Ok(Some(true))`.

Zero is that first variant. `zero_constructor` takes the first variant and zeroes its fields (`V/contract_entailment.rs:87765-87796`, read). The stage 02 setter also looks only at the first variant, and only to turn the gate on (`S02/symbols.rs:1006-1048`, read). A later variant's facts are judged when that case is constructed: `validate_literal_case_constraints` (`V/literals_and_struct_literals.rs:3439-3489`, read) calls `validate_literal_facts`, which reports a `requires` fact that evaluates false (`:3614-3618`, read).

Clearing the gate does not admit the later variant as zero, and it does not skip that construction check. Consumers opened that treat `zero_gated` as a permission on the zero value, not on later variants: `data_requires_establishment` (`V/value_custody_core.rs:4122-4128`), the sealed-data predicate (`:53-64`), and the recast path that returns when any member is not a field (`V/recasts.rs:3658-3661`).

Digest is right that only the first variant is inspected. That is the zero value, so the branch does not accept a later variant whose facts fail.

Not read: `zero_fold` past `S02/symbols.rs:1052`, and the two `layouts.rs` callers of this function.

## 4. `BTE/const_evaluation.rs:4933, 5019-5034`

`evaluate_selected_range_endpoints` (`:4888-4971`, read in full) and `evaluate_round` (`:4978-5041`, read in full). Search `evaluate_selected_range_endpoints\(`: 5 hits. Opened: `lib.rs:732` inside `evaluate_work`, which continues on `Ok`. `const_evaluation.rs:4781` is the wrapper `evaluate_const_range_endpoints`. Not opened: `:2422` and `:2766`.

When a round returns `Err` and `plan.calls` is empty, the loop breaks and the function returns `Ok(())` (`:4932-4933`). Inside a round, a root whose expression is not one of `plan.calls` drops the error and continues (`:5019-5026`). `plan.calls` are calls found under contract roots (`:5071-5087`, the loop read). A root with no such call is still a root. Its failure is not diagnosed and it is not inserted into `pending_const_range_endpoints`.

The later diagnostic only fires for an expression that is already in that set, still referenced, and without a recipe (`S04/preparation.rs:415-437`, read). The swallowed root is not in the set, so that diagnostic does not see it.

What I could not determine: whether proof discharge later rejects the unfolded integer, or accepts the fact as written. What would decide it is one contract integer root with no endpoint call, whose `evaluate_root` fails, followed through stage 05.

Digest is right that this failure is not diagnosed here.

Not read: `contract_expressions::roots` past the call-collection comment, the 11 uses of `pending_const_range_endpoints` in `S04/body_cloning_and_rewriting.rs`, and `endpoint_leaves` (the digest says it is never filled; I did not re-read that field).

## 5. `BTE/const_generic_expressions.rs:1814-1821, 1837-1866, 1349-1361`

Three sites in one digest bullet. They do not share a verdict.

### 5a. `:1814-1821`

`Collector::expression` (`const_generic_expressions.rs:1748-1867`, read through the selection loop). An operator selection is skipped, and not stored in custody, when the context is not an initializer and the syntax owner does not own that span (`:1814-1821`). The `continue` is before the builtin check at `:1822-1832`.

Callee bodies are the case the digest names. I did not read the code that queues callees with `initializer: false`, and I did not read the const interpreter's operator dispatch. Whether a skipped operator lets a callee apply an authored overload, or only a builtin the node already names, is what would decide this.

Digest is right that the operator is not recorded.

### 5b. `:1837-1866`

Same function, same read. After call and operator, only a resolved const is recorded (`:1837-1865`). Any other selection kind falls out of the loop with no error.

I did not read the code that attaches selections to nodes, nor the scalar evaluator's use of the node versus the selection. What would decide it is a selection whose target differs from the symbol on the node, followed into `evaluate_expression`.

Digest is right that those kinds are ignored.

### 5c. `:1349-1361`

`selected_retained_call` (`:1320-1396`, read in full) and `reflection_body_origin` (`:1398-1448`, read in full). Search `selected_retained_call\(`: definition `:1320` and three calls in this file (`:1513`, `:2095`, `:2125`). I opened the function; I did not open each caller's surrounding function past the call.

The early `Ok(())` requires no authored occurrence, `reflection_body_origin` returning `Some`, a `TransitionNamed` site, and a target whose owning machine is the machine being walked (`:1349-1360`). `reflection_body_origin` is `Some` only for a generated body whose source file is toolchain `reflect.omg` and whose declaration is one of `visit_runtime_fields`, `visit_runtime_fields_mut`, `inspect_record_fields`, `visit_active_fields`, `visit_active_fields_mut` (`:1436-1446`). Any other missing occurrence is the error at `:1362-1368`.

A user transition with no authored occurrence does not take this success. Digest is right about the early `Ok`. It does not apply to a user program.

Not read: the bodies of those five helpers, and the three callers above the call.

## Counts so far

| verdict | count |
| --- | --- |
| REACHABLE | 1 |
| CLOSED | 3 |
| UNDECIDED | 3 |

Entries not reached: 6 through 61 (the remaining digest bullets). Sub-sites 5a and 5b are counted above; 5c is the third closed row.

## What I did not read

Stage 01, stage 02 except the symbol assignment and the zero-gate setter named above, stage 03 except `lower_named_type_reference`, stage 04 except `preparation.rs:415-437`, and the rest of build-time evaluation outside the functions named in sections 1–5. No compiler was built or run.

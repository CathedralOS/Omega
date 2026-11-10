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
| 6 | BTE/const_generic_expressions.rs:2689-2695, 5680-5686 | CLOSED | Those seven tokens are not operator spellings, so no authored overload can be declared. |
| 7 | BTE/const_initializers.rs:940-942 | UNDECIDED | A primitive destination skips the carrier match. Whether the later probe rejects a non-primitive result was not read. |
| 8 | BTE/const_initializers.rs:2188-2191, 2238-2240 | UNDECIDED | Replay is skipped when the walker finds no call and no float. The walker does not enter a saved subject. |
| 9 | BTE/const_initializers.rs:3288, 3323 | UNDECIDED | The target-observation helpers keep only scalar destinations. Where an aggregate observation is evaluated was not read. |
| 10 | BTE/syntax_probes.rs:652-661 | CLOSED | Both constructors of the source context pass a source map, so the drop branch is not taken. |

Entries 11 onward are not written yet.

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

## 6. `BTE/const_generic_expressions.rs:2689-2695, 5680-5686`

`EvaluationContext::require_operator` (`const_generic_expressions.rs:2660-2696`, read through the early return) and `has_builtin` (`:2644-2658`, read). In the `Closed` context, `has_builtin` is unconditionally true. `require_operator` returns `Ok(())` for and, or, bitwise and, bitwise or, bitwise xor, shift left, and shift right (`:2689-2695`) before the overload test that the arithmetic spellings take (`:2697` onward, the start read). `evaluate_integer_endpoint` (`:5665-5688`, read) passes `|_| true` into anonymous evaluation, so that path does not consult an overload either.

An authored overload of those tokens cannot be parsed. `OperatorSpelling::from_symbol` accepts only `+ - * / % == != < <= > >= [] [..]` (`psi/foundation/language-core/src/lib.rs:289-305`, read). `parse_operator_spelling` rejects anything else (`S01/declarations.rs:2017-2026`, read). Search `parse_operator_spelling\(`: 4 hits, the definition and three callers, all in `S01/declarations.rs` (`:1608`, `:1957`, `:2779`). The machine-declaration caller was opened (`:1601-1609`). The other two callers were not opened; they receive the same error from `parse_operator_spelling`.

Digest is right that this pass does not check an overload. The overload it left open cannot be written.

Not read: the numeric table that computes each builtin result.

## 7. `BTE/const_initializers.rs:940-942`

`require_expression_carrier` (`const_initializers.rs:915-942`, read through the primitive return). Search `require_expression_carrier\(`: definition `:915` and calls `:843`, `:899`, `:929`, `:973`, `:1011`, all in this file. Opened: `owned_argument_probe` (`:824-845`) and `evaluate_owned_argument` (`:791-822`). The recursive calls at `:929`, `:973`, and `:1011` were not opened past the call.

A call is not subject to this early return. `evaluate_owned_argument` checks `result_carriers_match` and returns an error on a mismatch (`:800-806`) before it builds a probe. A non-call expression whose destination is a primitive returns `Ok(())` here without looking at the expression (`:940-942`), and the probe is then run by `evaluate_const_evaluable_machine_symbol_for_invocation` (`machine_execution.rs:4186-4200`, the wrapper read; the measured body was not).

What I could not determine: whether that invocation rejects a result that is not the primitive. What would decide it is the result check inside `evaluate_const_evaluable_invocation_measured`.

Digest is right that the carrier check succeeds for any expression into a primitive.

## 8. `BTE/const_initializers.rs:2188-2191, 2238-2240`

`replay::validate` (`const_initializers.rs:2077-2191`, read through the skip). `initializer_contains_call` (`const_generic_expressions.rs:6532-6537`, read) calls `contains_call` (`:699-712`, read), which walks `expression_children` (`:773-859`, read in full). The match has no wildcard. A `Call` node sets the flag. Nested calls in arguments, binary operands, casts, members, indexes, arrays, struct fields, and match arms are visited. `SavedSubject` adds no children (`:795`).

The skip continues when that walk finds no call and `contains_floating_component` is false (`:2188-2191`). I did not read `contains_floating_component` or the float arm at `:2164-2186`. `validate_replay_roots` (`:2495-2524`, read) checks that the declaration's initializer handles still match; it is not reached on the skip. `validate_constant_literal_fields` has already run for every declaration in the loop (`:2123-2128`).

Line `:2238-2240` was not the same skip when I opened the file; the float and call split is the block above. I did not re-find a second skip at `:2238`.

What I could not determine: whether a call can sit only behind a `SavedSubject`, so the walk reports no call and replay is skipped. What would decide it is the meaning of `SavedSubject` and whether an initializer root can be one.

Digest's inference holds for every call the walker can see. It is not confirmed for a saved subject.

## 9. `BTE/const_initializers.rs:3288, 3323`

`target_observations::retain_entries` (`:3278-3308`, read in full) and the start of `settle` (`:3310-3329`, read). Both keep a declaration only when `scalar_probe_destination` returns `Some` (`const_generic_expressions.rs:6059-6074`, read): an exact probe destination or `f32`/`f64`. An aggregate destination is `None` and is not given an evaluation entry.

A second loop refuses to demand a declaration whose `pending_target_observation` is set (`const_initializers.rs:3910-3914`, read). `has_pending_target_observations` (`const_evaluation.rs:3872-3890`, read) makes `evaluate_or_defer` defer (`lib.rs:643`, read earlier). The flag is also mentioned in a dependency-cycle message (`const_initializers.rs:4544-4548`, read).

What I could not determine: whether an aggregate with that flag is evaluated by the assignments at `:3786` and `:3868`, which I did not read, or left unevaluated and then accepted. What would decide it is those two assignments and the consumer of a const that still has the flag and no value after deferral ends.

Digest is right that these two helpers do not handle an aggregate.

## 10. `BTE/syntax_probes.rs:652-661`

`normalization_request` (`syntax_probes.rs:645-665`, read in full). When `sources` is `None`, `top_level_bindings` becomes empty and `retained_base` becomes `None` (`:652-661`). Search `normalization_request\(`: definition `:645` and three callers in this file (`:535`, `:556`, `:575`), all opened: `normalize_type_aliases`, `normalize_generic_data`, `normalize_initializer_probe_data`.

Each passes `context.sources` or a `sources` cloned from it. `desugar_access_views` sets `let sources = context.sources.clone()` (`layouts.rs:3450-3454`, read). Search `BuildTimeSources \{`: 2 hits, both in `psi-compiler/src/source_compilation.rs`, both read. Both set `sources: Some(...)` (`:799` and `:1141`). `evaluate_extension` clones that context (`lib.rs:312-326`, read). No assignment in this crate sets the field back to `None`.

The drop runs only for a missing source map, and every constructed source context has one. Digest is right about the branch. It is not taken for a program that enters pre-resolution.

Not read: every test that builds a `ResolutionRequest` with `sources: None` outside this struct.

## Counts so far

| verdict | count |
| --- | --- |
| REACHABLE | 1 |
| CLOSED | 5 |
| UNDECIDED | 6 |

Entries not reached: 11 through 61 (the remaining digest bullets). Sub-sites 5a and 5b are counted above.

## What I did not read

Stage 00, stage 01 except `parse_operator_spelling` and the machine-declaration caller, stage 02 except the symbol assignment and the zero-gate setter, stage 03 except `lower_named_type_reference`, stage 04 except `preparation.rs:415-437`, and build-time evaluation outside the functions named in sections 1–10. `evaluate_const_evaluable_invocation_measured`, `SavedSubject`, and the target-observation assignments at `const_initializers.rs:3786` and `:3868` were not read. No compiler was built or run.

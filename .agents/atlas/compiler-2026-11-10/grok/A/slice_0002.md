# slice_0002

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0009 | checking.rs:329-334 | REACHABLE | Acknowledgement mismatch is not diagnosed when the operational plan has no matching call row (or state summary is missing). |
| A0010 | checking.rs:995-997 | UNDECIDED | Fifteen of sixteen `CheckingRequest` constructions leave `declaration_authority` unset; whether authored declaration violations still fail on those paths was not traced. |
| A0011 | checking.rs:408-412 | UNDECIDED | Pending opaque Copy uses `PendingBuildSelection` and skips the strict receipt requirement; whether a later pass re-enforces it was not traced. |
| A0012 | checks.rs:2602-2603 | REACHABLE | Lifetime depth above sixteen yields `Region::Inferred`, and `compare` treats paired non-`Known` regions as equal at checks.rs:2849. |
| A0013 | checks.rs:4167-4169 | REACHABLE | `check_completed_value` returns without runtime index equality when the expression’s static type context is unreachable. |
| A0014 | checks.rs:4294-4305 | REACHABLE | Flow-recorded calls with no parameter roster or no resolvable call site skip runtime-index argument checking. |
| A0015 | checks.rs:4269 | REACHABLE | Assignment checking of the value runs only when `scope.actual` names the assignment target. |
| A0016 | checks.rs:3328-3330 | REACHABLE | An invalid symbol handle is treated as a stable index and, via `expression_is_const`, as a compile-time index name. |

## A0009

**Verdict:** REACHABLE (read)

**Cited site:** `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/checking.rs:329-334` matches commit `457adf4a11f` (read; verified with `git show 457adf4a11f:.../checking.rs`).

**Enclosing function:** `call_acknowledgements::validate_call` (checking.rs:318-365, read in full). After bumping `call_ordinal`, it looks up `operational_calls` by `(statement_index, call_ordinal)`; on miss it returns before comparing `acknowledgement` to `transitive_may_suspend` / `transitive_may_block` (checking.rs:344-358).

**Callers of `validate_call`:** search `\bvalidate_call\b` in `checking.rs` — count 4 (1 definition, 3 production call sites at checking.rs:108, :173, :283; read).

**Upstream:** `validate_call_acknowledgements` (checking.rs:39-77, read). Per state it resolves `operational_state` via `operational.states`; if none, `continue` (checking.rs:47-54, read). Then walks statements and calls `validate_call` / expression visitors.

**Caller of `validate_call_acknowledgements`:** search `validate_call_acknowledgements` in `omega-rust` — count 2 (1 definition checking.rs:39, 1 production caller `published_facts.rs:11230`; read).

**Admission path:** `build_check_facts` builds `operational`, then `validate_call_acknowledgements(program, &operational)` runs (published_facts.rs:11209-11230, read) before checked facts are returned to `check_program` (checking.rs:1139, read).

**Plain input:** A prepared program whose source lists a call with operational acknowledgements that disagree with the callee’s suspend/block envelope, while the `OperationalPlan` slice for that state has no `CallOperational` row for that statement index and ordinal (or the state has no operational summary). `validate_call` returns without pushing the envelope diagnostic; `validate_call_acknowledgements` still finishes with an empty diagnostic list if no other errors.

**Borrow.rs:** Not opened for this row (GROUP mentions borrow.rs; this row’s citation is only `checking.rs`).

## A0010

**Verdict:** UNDECIDED (read)

**Cited site:** `checking.rs:995-997` matches `457adf4a11f` (read).

**Enclosing function:** `admit_request` (checking.rs:991-1000, read in full). Runs `validate_authored_declaration_selections` only when `request.declaration_authority` is `Some`.

**Callers of `admit_request`:** search `admit_request` in `omega-rust` — count 5 (1 definition, 2 in checking.rs:923 and :979; 2 in `omega/build-system/build-settings`; read checking.rs call sites only).

**Construction sites:** search `CheckingRequest::new` in `omega-rust` — count 16 production uses (`source_checking.rs` 1, `const_initializers.rs` 2, `machine_execution.rs` 7, `const_evaluation.rs` 5, `psi_optimization_allocations.rs` 1; read grep listing).

**Authority wiring:** search `with_declaration_authority` in `omega-rust` — count 2 (definition checking.rs:770, sole use `source_checking.rs:702-703`; read). Comment at checking.rs:789-792 (read) states toolchain selections may stay late-bound; ordinary authored selections are “strict in both modes” at the request level, but `admit_request` does not run selection validation without `Some(authority)`.

**What would decide:** Tracing whether `validation::validate_authored_declaration_selections` (declarations_core.rs:905, not opened) or an earlier stage rejects the same authored declaration mismatches on each of the fifteen `CheckingRequest::new()`-only paths before `CheckedTrees` are consumed.

## A0011

**Verdict:** UNDECIDED (read)

**Cited site:** `checking.rs:408-412` matches `457adf4a11f` (read).

**Enclosing function:** `program_validation::prepare_typed_program` (checking.rs:399-419+, read through opaque branch). When `allow_pending_opaque_copy` is true, `opaque_properties` is `OpaquePropertyValidation::PendingBuildSelection` instead of `Required(opaque_property_receipts)` (checking.rs:408-412).

**Caller:** `prepare_typed_program` — single caller `check_program` at checking.rs:1047-1055 with `admits_pending_inputs` passed as `allow_pending_opaque_copy` (read).

**Downstream:** `program_validation.rs` prologue (read :546-557) passes `allow_pending_opaque_copy` into `properties::validate_data_properties`. In `validate_opaque_data_properties`, missing Copy receipt is diagnosed only when `!allow_pending_copy` (proof_contracts_core.rs:8189-8199, read).

**What would decide:** Whether any later invocation always supplies `Required` receipts and `allow_pending_opaque_copy == false` before a boundary-opaque `[copy]` program can ship, or whether some entry points stop after the pending path.

## A0012

**Verdict:** REACHABLE (read)

**Cited site:** `checks.rs:2602-2603` matches `457adf4a11f` (read).

**Enclosing function:** `runtime_indices::Scope::region_operand` (checks.rs:2578-2637, read in full). At `depth > 16` returns `Some(Region::Inferred)` instead of resolving binders (checks.rs:2602-2603).

**Uses of `region_operand`:** search `\bregion_operand\b` in `checks.rs` — count 5 (1 definition, uses at :2627, :2843, :2844; read).

**Closing check context:** `Scope::compare` (checks.rs:2742+, read :2839-2851). For generic lifetime operands, if both sides are `Some` but not both `Region::Known`, the closure returns `true` (checks.rs:2849).

**Caller path:** `compare` is used from runtime index application matching in the same module (e.g. `check_completed_value` via `compare` at checking.rs:4177, read).

**Entry:** `runtime_indices::check` (checks.rs:4186+) is called from `check_checked_facts_recording` (checks.rs:12070, read).

**Plain input:** Two applications whose generic lifetime operands would be unequal if resolved past depth sixteen; `region_operand` returns `Inferred` on both sides; `compare` accepts the pair without requiring equality.

## A0013

**Verdict:** REACHABLE (read)

**Cited site:** `checks.rs:4167-4169` matches `457adf4a11f` (read).

**Enclosing function:** `runtime_indices::Scope::check_completed_value` (checks.rs:4161-4183, read in full). First statement: if `static_type_context_is_unreachable(expression)`, return (checks.rs:4167-4168).

**Callers:** search `check_completed_value` in `checks.rs` — count 4 (1 definition, 3 call sites :3978, :4011, :4157; read listing).

**`static_type_context_is_unreachable`:** typed-trees `type_system.rs:1682-1684` delegates to static type contexts (read).

**Plain input:** A runtime-indexed value expression classified unreachable by static type contexts but present in the typed tree; the runtime-index subject-equality check at checks.rs:4177-4181 never runs.

## A0014

**Verdict:** REACHABLE (read)

**Cited sites:** `checks.rs:4205-4210`, `:4294-4305` match `457adf4a11f` (read).

**Enclosing function:** `runtime_indices::check` (checks.rs:4186-4320+, read :4186-4310). State loop: missing machine or state symbol → `continue` (checks.rs:4205-4210). Call loop: `call_target_parameters` or `find_call_site` returns `None` → `continue` (checks.rs:4294-4305) before `scope.check_binding` on arguments.

**Caller:** `check_checked_facts_recording` at checks.rs:12070 (read).

**Plain input:** `CheckFacts` flow records a call in a state, but the typed program yields no parameter roster or no call-site match for that `(statement_index, call_ordinal)`; runtime-index checks for that call’s arguments are skipped with no diagnostic.

## A0015

**Verdict:** REACHABLE (read)

**Cited site:** `checks.rs:4269` matches `457adf4a11f` (read).

**Enclosing function:** same `runtime_indices::check` statement `match` (checks.rs:4263-4271, read). `check_descriptor_store` always runs; `scope.check(assignment.value, required, …)` runs only inside `if let Some(required) = scope.actual(assignment.target)` (checks.rs:4269-4271).

**Plain input:** An assignment whose target place has no retained `actual` application; descriptor store may be checked, but the assigned value is not compared to the target’s required application for runtime indices.

## A0016

**Verdict:** REACHABLE (read)

**Cited sites:** `checks.rs:3328-3330`, `:1902-1903` match `457adf4a11f` (read).

**`stable_index_symbol`** (checks.rs:3327-3362, read in full): `if !symbol.is_valid() { return true; }` (checks.rs:3328-3329).

**Callers of `stable_index_symbol`:** search in `checks.rs` — count 4 (1 definition, 3 uses :3397, :3468, :3499; read).

**`expression_is_const`** (checks.rs:1896-1937, read :1901-1903): `is_const_symbol` returns true when `!symbol.is_valid()` (checks.rs:1902-1903). Used for call machine arguments at checks.rs:1926-1935 (read).

**Callers of `expression_is_const`:** search in `checks.rs` — count 6 (1 definition, 5 call sites; read grep).

**Plain input:** A proof or index position that names an invalid `SymbolHandle`; stability and const-index predicates succeed without tying the name to a const declaration, machine value parameter, or state parameter.

## What I did not read

- `borrow.rs` sites listed in the GROUP text for A0009.
- Full bodies of all fifteen `CheckingRequest::new()` call sites outside `source_checking.rs`.
- `validation::validate_authored_declaration_selections` and whether stage-04 admission already enforces authored selections.
- Operational-plan construction in flow-effects (`infer_operational_*`) to prove plan/source row alignment or mismatch.
- Full `prepare_typed_program` / prologue after checking.rs:419.
- Consumers of `Region::Inferred` beyond `compare` in `runtime_indices`.
- Definition of `static_type_contexts.is_unreachable` beyond the delegate at type_system.rs:1682.
- Full `find_call_site` / `call_target_parameters` failure modes.
- Full callers of `stable_proof_indices` / `expression_is_const` beyond grep counts.

SLICE COMPLETE

# slice_0046

Area digest preamble (read): Area 13 — validation first half; path `calls` = `omega-rust/psi/compilation/validation/src/calls.rs` (44,215 lines). Stage answers whether typed programs keep promises via diagnostics or `None`/`false`. Call validation (`calls:1-6400`), write frames, argument correspondence. Citations pinned at `457adf4a11f`; line numbers verified in current checkout (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0349 | calls:3057-3059 | CLOSED | Quotient operand checking lives in `quotients` admission/planning, not in `validate_expression_call_bounds`. |
| A0350 | calls:3171-3176 | REACHABLE | Admitted build/`embed` calls exit before `validate_call_arguments`; build value operands lack type correspondence here. |
| A0351 | calls:3809-3823 | REACHABLE | Type-parameter bound loop `continue`s when `declared_place_type` is `None`, skipping property checks. |
| A0352 | calls:4152-4155; calls:4709-4713 | REACHABLE | Conformance and const-selection loops skip arguments/bindings with no resolved type. |
| A0353 | calls:6059-6069 | REACHABLE | Discarded non-unit call accepted when target spelling matches first attached-data-free proof machine name. |
| A0354 | calls:2523; calls:2701-2702; calls:2716-2724 | REACHABLE | `scan_at` does not descend into `SavedSubject` or case-membership right operand; type-predicate binaries skipped entirely. |
| A0355 | calls:1795 | REACHABLE | `composes` wildcard `_ => true` treats unlisted expression kinds as boundary-composable. |
| A0356 | calls:10863-10868 | REACHABLE | Missing collected prefix runs origin logic with `prefix = None` and `preceding_effect` true at in-range write-origin callers. |

## A0349

**Cited site (read):** `calls.rs:3057-3059` — in `validate_expression_call_bounds`, `if call.quotient_operation.is_some() { return; }` runs before trait conformance, `validate_call_arguments`, and callee resolution.

**Enclosing function (read):** `validate_expression_call_bounds` (`calls.rs:3028-3322+`) validates value/statement/terminal call bounds.

**Callers (search):** pattern `\bvalidate_expression_call_bounds\b` in `omega-rust` — 1 definition (`calls.rs:3028`), 1 production call (`calls.rs:2668` inside `ScanContext::scan_at`); 0 other matches.

**Upward trace (read):** `scan_at` (`calls.rs:2429+`) handles `ExpressionNode::Call` at `calls.rs:2637-2699`, calls `validate_expression_call_bounds` then still `scan`s receiver/arguments (`calls.rs:2694-2698`). Entry: `validate_value_position_calls` (`calls.rs:2219-2232`) → `program_validation.rs:4400` (read).

**Closing checks (read):** All quotient calls are collected in `quotients.rs:7913-7918` (`request_admission::extract`). Each must be the unique fallthrough terminal result (`quotients.rs:7982-7985`) or admission fails. `admit_checked_quotient_requests` (`quotients.rs:8126-8154`) rejects non-admitted requests with diagnostics. `reject_quotient_operation_requests` (`quotients.rs:622+`) validates plans including representative argument arity/identity (`quotients.rs:6186-6193`). Programs with quotient operations not closed here do not compile through the quotient bridge.

**Input at cited branch:** Any call expression with `quotient_operation` set entering `validate_expression_call_bounds`.

**Smallest program (plain words):** Not a separate acceptance hole: quotient calls either fail quotient admission or are checked in `quotients`; skipping generic call-argument validation here does not leave them unchecked end-to-end.

## A0350

**Cited sites (read):** `calls.rs:3171-3176` — after unit-return checks, `admitted_build_declaration_call` or `is_exact_embed_call` causes `return` before operator/machine resolution and `validate_call_arguments` (`calls.rs:3294+`).

**Enclosing function (read):** `validate_expression_call_bounds` (`calls.rs:3028+`).

**Callers (search):** same as A0349 — single call from `calls.rs:2668`.

**Related admission (read):** `admitted_build_declaration_call` (`calls.rs:6411-6475`) checks static/value argument **counts**, nominal route, and intrinsic classification (`calls.rs:6458-6474`); it does not run `validate_call_arguments` on value operands. Build operand typing is handled later when selections resolve (`machine_calls_core.rs:929-1011`, read names only), not at this early return.

**Embed path (read):** `scan_at` runs `validate_total_specification_arithmetic` for exact embed before bounds (`calls.rs:2655-2666`). `is_exact_embed_call` is the `IntegerEmbed` builtin with fixed shape (`proof_contracts_core.rs:6433-6443`, read).

**Input:** Build declaration call passing `admitted_build_declaration_call` count/intrinsic gates, or an exact `IntegerEmbed` call, with value operands that would fail generic callee argument correspondence.

**Smallest program (plain words):** A `Build::…` declaration call with the expected number of static and value arguments but a value operand whose type does not match the build operation’s product role; validation returns before `validate_call_arguments` and may still compile if no later build pass rejects it.

## A0351

**Cited sites (read):** `calls.rs:3809-3823` — in `validate_type_parameter_instantiation_bounds`, for each argument/`StateParameter` pair, `declared_place_type(...)` `None` ⇒ `continue` before `type_satisfies_declared_property`.

**Enclosing function (read):** `validate_type_parameter_instantiation_bounds` (`calls.rs:3788-3841`); wrapper `validate_resolved_target_type_parameter_bounds` (`calls.rs:3765-3784`) and `validate_machine_call_type_parameter_bounds` (`calls.rs:3630-3657`).

**Callers of `validate_type_parameter_instantiation_bounds` (search):** pattern in `omega-rust` — definition `calls.rs:3788`, calls at `calls.rs:3647`, `calls.rs:3775`; 0 other production sites.

**Upward trace (read):** Bounds run after resolved machine calls via `validate_resolved_call_arguments` (`calls.rs:5017-5026`, `calls.rs:5259-5267`) and for value calls via `ValueCallSite::validate_callee` (`calls.rs:2989-2998`).

**Input:** Callee with non-empty type-parameter bounds; call argument expression at a parameter position where `declared_place_type(program, current_machine, current_state, argument)` is `None`.

**Smallest program (plain words):** A generic machine call whose static instantiation is attempted while a value argument has no declared place type at the call site; instantiation property bounds are not checked for that argument and the call may be accepted.

## A0352

**Cited sites (read):** 
- `calls.rs:4152-4155` — in `validate_named_conformance_arguments`, post-`validate_requirement_call_arguments` loop: `declared_place_type` `None` ⇒ `continue` before conformance/type match diagnostics. 
- `calls.rs:4709-4713` — in `scoped_callable_type_arguments` (`calls.rs:4581+`), const-selection loop: missing substitution binding for a const parameter ⇒ `continue` before `const_selection_matches`.

**Enclosing functions (read):** `validate_named_conformance_arguments` (`calls.rs:4073-4176`); `scoped_callable_type_arguments` used by `callable_type_arguments` / `requirement_call_type_arguments` (`calls.rs:4500+`, `calls.rs:4530+`).

**Callers of `requirement_call_type_arguments` (search):** count 3 in `calls.rs` (read: definition + internal uses).

**Upward trace (read):** Conformance path from `validate_expression_call_bounds` (`calls.rs:3101-3113`) and statement gates (`calls.rs:679+`). Type-argument path feeds `validate_resolved_call_arguments` (`calls.rs:4766-4779`): missing/invalid bindings yield diagnostic at `4775-4778`, but partial binding with skipped const rows can still return `Some(bindings)`.

**Input:** (1) Trait/requirement call with an argument lacking resolved place type after requirement argument validation. (2) Static machine call with const-parameter selections where inference left a const parameter unbound in `bindings`.

**Smallest program (plain words):** A conformance call whose argument type cannot be resolved at the site but still passes earlier argument validation, so trait-argument mismatch is not reported; or a call with incomplete const generic bindings where individual const selections are skipped instead of failing the whole correspondence.

## A0353

**Cited site (read):** `calls.rs:6057-6069` — in `validate_result_use`, when result is not unit and not discarded, empty receiver: find first machine with `attached_data.is_none()` and `name == call.target`, and if `classification.is_proof_machine`, `return` without error.

**Enclosing function (read):** `validate_result_use` (`calls.rs:6040-6076`).

**Callers (search):** pattern `\bvalidate_result_use\b` in `omega-rust` — 11 matches in `calls.rs` (read: count from ripgrep); production statement/expression call validation paths at `calls.rs:672`, `708`, `4910`, `4996`, `5116`, `5244`, etc.

**Upward trace (read):** Statement machine resolution (`calls.rs:4996-5002`) and conformance gates call `validate_result_use` before argument validation.

**Input:** Statement or expression call that discards its result (`discards_result` false is required — actually the check is for non-discarded: `if call.discards_result || ...` return early at `6047-6054`; cited block is when result is **not** discarded). Re-read...

Actually `6047`: if discards_result OR unit OR invalid return → return (no error). The citation is about **discarded** results - wait, if discards_result, function returns at 6047 without reaching 6057.

Re-read validate_result_use logic:
- 6047-6054: return if discards_result OR unit return type - so NO error for discarded unit/invalid
- 6057+: only reached when NOT discarding and non-unit return
- 6057: if call.receiver.is_empty() - citation exemption for proof machine name match

The digest says "discarded non-unit result" - that contradicts code flow unless discards_result is false meaning they're NOT discarding explicitly?

"discards its non-unit result" diagnostic at 6072 - so caller did NOT discard. Citation exemption when target name looks like proof machine - allows implicit discard for citation calls?

Actually when receiver empty and matches proof machine name, return without pushing error at 6072 - meaning a non-unit result used in statement position without assignment might be accepted as citation.

Digest: "discarded non-unit result is accepted when call's spelling equals the name of any proof machine"

So statement `ProofMachine(..);` without using result - might not set discards_result but still statement use. REACHABLE: executable call to machine X where X's spelling matches unrelated proof-only machine name, non-unit return, no explicit discard — no diagnostic.

**Smallest program (plain words):** A statement call whose target string equals the name of some proof machine without attached data (first name match), returning a non-unit type, with no explicit consume; the compiler treats it as a citation and emits no discard error even if the callee is not that proof machine.

## A0344

Wait - I used A0354 not A0344. Continue A0354 section.

## A0354

**Cited sites (read):** 
- `calls.rs:2523` — `ExpressionNode::SavedSubject(_) => {}` in `scan_at` match, no recursive `scan`. 
- `calls.rs:2701-2702` — `ExpressionNode::Binary(_)` when `static_type_predicate_operands(expression).is_some()` => empty arm. 
- `calls.rs:2716-2724` — after scanning binary left, `has_exact_case_membership_meaning` ⇒ `return` before scanning right.

**Enclosing function (read):** `ScanContext::scan_at` (`calls.rs:2429+`).

**Callers (search):** `scan`/`scan_at` internal; entry `validate_value_position_calls` (`program_validation.rs:4400`).

**Upward trace (read):** Skipped subtrees never reach `validate_expression_call_bounds` (`calls.rs:2668`) or operand type checks in this walk. Other files mention `SavedSubject` / type predicates (e.g. `value_custody_core.rs`, `expression_types_and_fields.rs`, read: names only); not traced whether every skipped form is enforced elsewhere.

**Input:** Value-position expression tree containing `SavedSubject`, a static type-predicate binary, or case-membership binary whose right operand should be checked under the short-circuit environment.

**Smallest program (plain words):** A machine state initializer or transition value holding a saved-subject or case-membership expression whose inner operand is ill-typed; this call-validation scan never visits that inner node and the program may be accepted.

## A0355

**Cited site (read):** `calls.rs:1795` — in nested helper `composes` inside `nested_boundary_applications_compose` (`calls.rs:1720-1801`), match wildcard `_ => true`.

**Enclosing function (read):** `nested_boundary_applications_compose` (`calls.rs:1720-1801`); `composes` (`calls.rs:1766-1797`) only recurses through Call, Atomic, Binary, Unary, Cast, Borrow, Indexed, Member.

**Callers (search):** pattern `\bnested_boundary_applications_compose\b` in `omega-rust` — definition `calls.rs:1720`, 1 production use `execution_scalar_computations.rs:10810` (read).

**Upward trace (read):** When `nested_boundary_applications_compose` returns true, scalar pipeline sets `argument_roots` true (`execution_scalar_computations.rs:10805-10813`), affecting which roots are recorded (`10837+`, read).

**Input:** Boundary scalar call whose arguments include expression kinds not listed in `composes` (e.g. `Match`, `SavedSubject`, `Name` leaves).

**Smallest program (plain words):** A nested boundary-declaration value call with a match or other unhandled expression inside an argument; `composes` returns true via the wildcard, so boundary-composition checking treats the tree as composable when it should not.

## A0356

**Cited site (read):** `calls.rs:10863-10868` — in `with_caller_origin_context`, when `collected_prefix_at(...)` is `None`, `resolve(&mut FrameInference::default(), None)` runs with no alias/prefix evidence.

**Enclosing function (read):** `with_caller_origin_context` (`calls.rs:10853-10879`); `with_retained_origin_context` fills isolated locals when state known (`calls.rs:10881-10913`).

**Callers in digest range (read):** `reference_operand_write_sources_with_captures` (`calls.rs:12573-12594`); `operator_write_origins` (`calls.rs:12648-12683`); `invocation_write_origins` closure (`calls.rs:12805-12813`). Count of `\bwith_caller_origin_context\b` in `calls.rs`: 7 (read).

**Upward trace (read):** When `prefix` is `None`, `preceding_effect = prefix.is_none_or(|prefix| prefix_may_change_origins(...))` is `true` (`calls.rs:12580-12581`, `12658-12660`, `12805-12807`, read: `Option::is_none_or`). Write-origin capture proceeds with default `FrameInference` and may record effects without alias context (`calls.rs:12583-12591`, `12672-12680`).

**Input:** Call-site write-origin query for a statement index with no collected prefix map entry (`collected_prefix_at` missing).

**Smallest program (plain words):** An expression-level call in a state where prefix collection did not run for that statement index; write-origin analysis assumes a preceding effect and may attribute write origins that should be unknown.

## What I did not read

Full remainder of `validate_expression_call_bounds` after `calls.rs:3322`; complete `relation_plan::derive_direct_terminal_plan`; all `validate_call_arguments` obligation kinds; every downstream validator for `SavedSubject` and type predicates; `git show 457adf4a11f:omega-rust/psi/compilation/validation/src/calls.rs` line-by-line diff; callers of `admit_checked_quotient_requests` outside `quotients.rs`; full `capture_operand_origins` with `prefix = None`.

SLICE COMPLETE

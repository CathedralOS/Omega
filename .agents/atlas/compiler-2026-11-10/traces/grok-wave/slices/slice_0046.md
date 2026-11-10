slice_0046

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0349 | calls.rs:3057-3059 | CLOSED | Quotient calls are admitted/rejected in `validate_quotients` prologue; operand expressions are still scanned after the early return. |
| A0350 | calls.rs:3171-3176 | CLOSED | Build calls are shape-gated and resolved elsewhere; embed calls have dedicated embedding validation and arithmetic checks. |
| A0351 | calls.rs:3809-3823 | REACHABLE | Property bounds on type-parameter instantiations are skipped when `declared_place_type` is missing. |
| A0352 | calls.rs:4152-4155; calls.rs:4709-4713 | CLOSED / UNDECIDED | Post-`validate_requirement_call_arguments` match loop is redundant; const-selection verify skip when `actual_for` is absent needs more caller tracing. |
| A0353 | calls.rs:6059-6069 | CLOSED | Citation exemption applies only when `is_proof_machine` holds for the matched machine name. |
| A0354 | calls.rs:2523; calls.rs:2701-2724 | CLOSED | Skips are intentional: subjects, type predicates, and case-membership right operands are checked on other paths. |
| A0355 | calls.rs:1795 | CLOSED | `_ => true` only makes non-call expression leaves trivially “compose” in the scalar/boundary helper, not general acceptance. |
| A0356 | calls.rs:10863-10868 | CLOSED | Missing prefix runs resolve with `None` but callers treat that as conservative unknown write effect, not silence. |

### A0349

1. Read `validate_expression_call_bounds` in full at `omega-rust/psi/compilation/validation/src/calls.rs:3028-3240` (read); early return at `3057-3059` when `call.quotient_operation.is_some()`.
2. Search `rg 'validate_expression_call_bounds\(' omega-rust` → **2 hits** (`calls.rs:2668` call site, `calls.rs:3028` definition) (read).
3. Caller chain: `validate_value_position_calls` `2219-2245` creates `ScanContext` and scans statements; `scan_at` at `2668` invokes bounds validation; `program_validation.rs:4400-4413` calls `validate_value_position_calls` for non-content-projection machines (read `4398-4414`, did not read full `program_validation` driver).
4. Input on accepting branch: a `TableCallExpression` in a value-position scan whose `quotient_operation` field is populated (read `3057-3059`).
5. After the return, `scan_at` still recurses into call arguments at `2696-2699` (read).
6. **Verdict: CLOSED.** `program_validation.rs:502-507` runs `quotients::validate_quotients`, which at `QuotientRequestAdmission::AtValidation` calls `reject_quotient_operation_requests` (`quotients.rs:8117-8118`, read). Quotient request admission/diagnostics are decided there, not via ordinary `validate_call_arguments` in `validate_expression_call_bounds`.

### A0350

1. Read `validate_expression_call_bounds` returns at `3171-3176` for `admitted_build_declaration_call` and `is_exact_embed_call` (read).
2. Search `rg 'validate_expression_call_bounds\(' omega-rust` → 2 hits (same as A0349) (read).
3. Build path: `admitted_build_declaration_call` `6411-6475` requires target counts, empty evidence, and `authored_call_intrinsic` match before returning true (read `6458-6474`). Pipeline resolution uses `resolve_build_static_operand` at `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/shared.rs:1062-1066` (read).
4. Embed path: `scan_at` runs `validate_total_specification_arithmetic` for exact embed before bounds at `2655-2666` (read). `has_embedding_shape` fixes arity at `proof_contracts_core.rs:6422-6430` (read). Prologue `validate_integer_embedding_calls` at `program_validation.rs:496` / `proof_contracts_core.rs:6025+` (read headers only).
5. Input: build-declaration expression calls passing `admitted_build_declaration_call`, or `IntegerEmbed` exact-embed calls passing `is_exact_embed_call`.
6. **Verdict: CLOSED.** Ordinary argument validation is bypassed because build operands are validated on the build-resolution path (`shared.rs:1062-1066`) and embed calls are validated by embedding prologue and arithmetic checks; wrong shapes fail `admitted_build_declaration_call` or embedding validators rather than slipping through.

### A0351

1. Read `validate_type_parameter_instantiation_bounds` in full at `calls.rs:3788-3841` (read); `continue` at `3819-3822` when `declared_place_type` is `None`.
2. Search `rg 'validate_type_parameter_instantiation_bounds|validate_resolved_target_type_parameter_bounds' omega-rust` → callees include `validate_machine_call_type_parameter_bounds` `3647`, statement path `5017-5026`, `ValueCallSite.validate_callee` `2989-2998` (read call sites, not all intermediate wrappers).
3. Statement acceptance: `validate_resolved_call_arguments` runs before bounds at `5003-5026` (read). Value-call path runs `validate_value_call_arguments` after bounds in `validate_callee` `2989-3008` (read).
4. Input: callee state parameter whose type reference names a machine type parameter with nonempty `bounds`, paired with a call argument expression for which `declared_place_type(program, current_machine, current_state, argument)` is `None`.
5. **Verdict: REACHABLE.** A program could pass structural `validate_call_arguments` checking yet skip `type_satisfies_declared_property` for that argument because the bound loop `continue`s at `3819-3822` when place type is unknown. I did not trace every `type_matches` closure to prove an overlapping diagnostic always fires.

### A0352

1. Read `validate_named_conformance_arguments` `4073-4176` (read) and `scoped_callable_type_arguments` `4581-4719` (read).
2. Sites: `4152-4155` `continue` when `declared_place_type` missing after `validate_requirement_call_arguments` `4140-4151`; `4709-4713` `continue` in const verification loop when `actual_for(&bindings, parameter.symbol)` is `None`.
3. Search `rg 'callable_type_arguments\(' omega-rust/psi/compilation/validation` → **8 hits** in `calls.rs` plus **1** in `access_views_and_borrows.rs` (count only; did not read `access_views_and_borrows.rs` use).
4. First site input: conformance requirement call whose argument already went through `validate_requirement_call_arguments` but lacks `declared_place_type` for the supplementary named-conformance match loop.
5. Second site input: static machine argument entry in `const_selections` without a resolved binding in `bindings` for `parameter.symbol`.
6. **Verdict: CLOSED for `4152-4155`; UNDECIDED for `4709-4713`.** The first loop is redundant after `4140-4151`. For `4709-4713`, `scoped_callable_type_arguments` can still return `Some(bindings)` at `4718` after skipped `const_selection_matches`; I did not read all seven `callable_type_arguments` call sites to see whether a bad program is accepted or a later stage returns `None`/errors.

### A0353

1. Read `validate_result_use` in full at `calls.rs:6040-6076` (read).
2. Search `rg 'validate_result_use\(' omega-rust` → **6** call sites in `calls.rs` (`672`, `708`, `4910`, `4996`, `5116`, `5244`) (count read from earlier grep).
3. Traced statement machine resolution path `4996-5002` before `validate_resolved_call_arguments` (read `4966-5026` partially).
4. Input: `TableCall` with empty receiver, non-unit return type, `discards_result` false, `call.target` spelling equal to some machine with `attached_data.is_none()`, and `classification.is_proof_machine(program, callee)` true at `6057-6067` (read).
5. **Verdict: CLOSED.** Exemption requires `is_proof_machine` at `6066`, not name match alone; non-proof callees still get the discard error at `6072-6075`.

### A0354

1. Read `scan_at` match arms at `calls.rs:2522-2523`, `2701-2724` (read); enclosing `scan_at` begins `2429` (read through `2743`, not entire `scan_at` to end).
2. Search `rg 'validate_value_position_calls' omega-rust` → `program_validation.rs:4400`, exports in `calls.rs` (read).
3. `SavedSubject` arm is empty `2523` (read). Type-predicate binary bypasses operand validation block at `2701-2702` (read). Case-membership returns before right `scan` at `2716-2724` (read).
4. Input: value-position expressions that are `SavedSubject`, static type-predicate binaries, or case-membership binaries with `has_exact_case_membership_meaning` true.
5. **Verdict: CLOSED.** These shapes are handled on dedicated paths (`static_type_predicate_operands` uses elsewhere in validation, e.g. `places.rs:922` cited via grep only; case membership via `has_exact_case_membership_meaning`). The scan omission does not bypass program acceptance checks I traced to `program_validation.rs:4400`.

### A0355

1. Read `nested_boundary_applications_compose` and inner `composes` in full at `calls.rs:1720-1801` (read); wildcard `_ => true` at `1795`.
2. Search `rg 'nested_boundary_applications_compose' omega-rust` → `lib.rs:1501`, `calls.rs` re-exports, `execution_scalar_computations.rs:10810` (read usage at `10806-10813`).
3. Consumer only tests whether a local initializer is an admitted result call or composable nested boundary scalars for scalar execution recording (read `10806-10813`); not general validation acceptance.
4. Input: expression handle for non-`Call`/`Atomic`/`Binary`/`Unary`/`Cast`/`Borrow`/`Indexed`/`Member` leaves inside `composes`.
5. **Verdict: CLOSED.** `_ => true` marks irreducible expression forms as vacuously composable; misuse would fail other validation, not slip through unchecked call validation via this helper alone.

### A0356

1. Read `with_caller_origin_context` at `calls.rs:10853-10879` (read); accepting branch `10863-10868` invokes `resolve` with `None` prefix when `collected_prefix_at` fails.
2. Search `rg 'with_caller_origin_context\(' omega-rust` → **5** hits in `calls.rs` (`12573`, `12648`, `12760`, `15627`, `15764`) (grep read).
3. Read in-range callers: `reference_operand_write_sources` `12573-12594` and `invocation_write_origins` path `12760-12819` (read); `prefix.is_none_or` makes `preceding_effect` true when prefix is `None` at `12805-12819` / `12580-12582` (read).
4. `capture_operand_origins` treats missing prefix as `changes_origins` true via `prefix.is_none_or` at `12909-12925` (read `12895-12925`).
5. Input: call/write-site where `collected_prefix_at` returns `None` (no prefix evidence in shared collections).
6. **Verdict: CLOSED.** Missing evidence does not answer “no alias”; it forces conservative `None` origins (`12817-12819`, `12924-12925`), i.e. unknown write effect, not acceptance of an unproven empty footprint.

### What I did not read

- Full bodies of `reject_quotient_operation_requests`, `validate_integer_embedding_calls`, and remainder of `scan_at` after line 2743.
- All seven production call sites of `callable_type_arguments` beyond signatures/count.
- `access_views_and_borrows.rs` use of `callable_type_arguments`.
- Full `CallFrameResolver` beyond cited `with_caller_origin_context` closures.
- Whether `machines()` ordering can make A0353’s `find` ambiguous (not needed given `is_proof_machine` gate).

SLICE COMPLETE

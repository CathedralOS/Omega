# Record: unit_061 (area 03-source-stage-05)

Crate root `<05>` = `source-rust/subsystems/compiler/source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/`.
All files below are `<05>/src/read/<name>.rs`. Every file is an `impl Program<'_>` extension (the reader's `Program`, `read.rs`) that turns typed trees into the closed `body` form; refusals are `refuse(id, Construct::Machine, Unserved::...)` (reason Unsupported) or `reject(id, source, text)` (reason Violation).

## casts.rs (921 lines)

1. Purpose: reads an `as` cast expression. Either a domain-only cast (`as +D -D =D`, "entry"), a reference recast, or a scalar carrier conversion, plus an optional arithmetic policy. Produces a body `Expression` (Cast, Requalify, Establish or a Call to a Core conversion).
2. Shape: `Program::cast` casts.rs:26; `exact_numeric_conversion` :339; `domain_entry` :403; `reference_entry_input` :450; `entry_applied` :494; `add_domain` :541 (pub(super)); `domain_entry_change` :612 (pub(super)); `domain_removal` :683 (pub(super)); `issue` :739 (pub(super)); `byte_literal_operand` :828, `array_literal_operand` :839. One test, :893.
3. Inputs/outputs: reads `TableCastExpression` (`form`, `target_type`, `result_type`, `domain`, `semantic_domain_symbol/id`) from `self.typed`; calls `self.expression`, `value_type`, `domain_theory`, `cast_predicates`, `record_cast_predicates`, `borrow_value`, `collection_recast`. Output `Checked<Expression>`.
4. How it decides: dispatch on `cast.form.entry()` (:35) then `recast_access()` (:38) then policy/scalar. Domain identity is `(declaration symbol, semantic id)` compared at :669-676 and :638-642, never a name. Float widening picks `can_exactly_widen_to` (:173), integer pair picks `can_widen_to || can_exact_cast_to` (:312). `issue` (:739) decides by `DomainIssuance` (Unrestricted / Only(routes) / Planned) and compares the current machine symbol to `DomainEstablishmentRoute::ExactMachine` (:779) or the machine's trait-conformance edge (symbol equality, :784-786). The only text-keyed judgment: none; `format!` is used only in rejection messages.
5. Cases by shape: refuses "exclusive storage view without establishment and handback evidence" for non-shared recast (:40); "cast to or from an address" (:310); "cast that adds a domain whose issuers are in an access plan" (:818); `BoundaryRequirement` route "is not followed here" (:787-789) and refuses (:813-815); only float<->float, float->int, int->float, int->int conversions are handled (:159-321), else "cast whose exact carrier conversion is not supported" (:305). Domain removal checks only access-retention, cleanup selection and mutable-view operands (:692-732).
6. Same job elsewhere: the contract-term cast reader `clause_cast` (contracts.rs:1771) re-uses `domain_entry_change` and `domain_removal` from here (contracts.rs:1851, 1855). Its integer carrier rule duplicates `cast` (contracts.rs:1815-1831 vs casts.rs:312), stated in contracts.rs:1784 ("as in a body (`Program::cast`)").
7. Unused/one-caller: from reading, `domain_entry_change` is called at casts.rs:516 and contracts.rs:1851; `add_domain` is called at casts.rs:523 and places.rs:1482 (`zero_value`); `domain_removal` at casts.rs:526 and contracts.rs:1855; `issue` at casts.rs:731. I did not grep these names crate-wide, so other callers outside the unit are unchecked.
8. Stated limits: "exclusive storage view without establishment and handback evidence" (:41); "a boundary requirement additionally requires provider selection and admission, which is not followed here" (:787-788, paraphrase); `DomainIssuance::Planned` refused (:817).
9. Fail-open risks: (a) `issue` `DomainIssuance::Unrestricted => {}` (:750) accepts any add; stated by the type: unrestricted domains need no issuer (traced by reading: `Unrestricted` is the declaration's own mode). (b) `domain_entry_change` Keep branch skips the "operand carries the domain" test when `domain_definition(symbol)` is `None` (:632-644); that case is the policy-domain path where `before != after` is already checked at :629; open (no comment, did not trace what other symbol kinds reach it). (c) `domain_removal` performs no issuer check for non-mutable-view operands (:702-732); stated at :680-682 ("Issuance restricts adding a domain, not forgetting"). (d) `add_domain` `_ => (Vec::new(), String::new())` (:587) adds a domain with no predicates and no Establish; correct only when `has_predicates` is false for the domain (the earlier arms catch predicates); traced by reading.

## clause_calls.rs (385 lines)

1. Purpose: unfolds a call of a declared, closed, contract-free scalar machine inside a contract clause or domain predicate into the proposition or term its single body expression states.
2. Shape: `Unfolded<R>` :45, `Unfolding` :54, `called_clause` :61, `called_term` :86, `within_unfolding` :125, `use_unfolded_argument` :151, `unfold` :174, `clause_call_machine` :206, `unfold_definition` :254, `bind_call_arguments` :338, `callee_scope` :371.
3. In/out: reads `TableCallExpression`, the callee's `Machine`/`State`, parameters via `formal`; produces `Proposition` / `(ScalarTerm, ValueType)`; mutates `self.unfolding_calls`, `admitted_declarations`, `admitted_bodies`, `clause_arguments`.
4. Decides: recursion keyed on the callee `SymbolHandle` equality against the `unfolding_calls` stack (:272-278); unfoldable only when `state.statement_nodes` is exactly one `StatementNode::Expression` or one unconditional value `Transition` (:289-305).
5. Cases by shape: "Only a machine whose meaning is exactly one body expression ... is unfolded" (header :18); parameters must be plain by-value unqualified scalars (:309-318); any call with receiver/evidence/selection fields refused (:212-229).
6. Same job: float meaning wrapper unfolding `unfold_meaning` (float_meaning.rs:498) re-uses `unfold_definition`, `bind_call_arguments`, `within_unfolding`, `callee_scope` from here.
7. Unused: `called_clause` callers: contracts.rs:474 (read). `called_term`: contracts.rs:1683 (read).
8. Stated limits: "A callee with `requires` would owe them at the call, which this reader does not yet discharge" (:24-25).
9. Fail-open: `use_unfolded_argument` does nothing when the top frame's machine differs from `owner` (:157-161, `if let ... filter`); effect: the used-flag stays false, which causes the argument to be retained for formation (conservative direction), so not fail-open (traced by reading :111-118).

## constant_values.rs (513 lines)

1. Purpose: checks that a materialized constant initializer (typed expression) agrees leaf by leaf with the constant's evaluated canonical value, and registers scalar-application ids for layout observations and deferred evaluation entries.
2. Shape: `ScalarApplicationSource` :20, `ScalarApplications` :29 (`register` :35, `layout` :55, `published` :71), `resolve_scalar_applications` :83, `scalar_evaluation` :108, `deferred_scalar_constant` :148, `subject_constant` :185, `constant_agrees` :312, `constant_aggregate_agrees` :429.
3. In/out: reads `ConstDeclaration`, `DecodedCanonicalConstValue` (application_identity), `self.records`, `self.record_sources`; writes `scalar_applications`; returns `Checked<()>` or `Checked<bool>`.
4. Decides: aggregate identity compares `declaration.name.as_str() != type_name` (:449) and `self.typed.symbols.name(*symbol) != name` for case (:476) and field name match `field.as_str() == name` (:503): these are spelling compares against the decoded value's `String` names (the decoded canonical value carries names, not ids). Integer leaf compares carrier and value (:351-365).
5. Cases by shape: leaf kinds Integer, Boolean, Float, Array, Record, Variant only (:343-426); everything else `_ => false`. Policy-qualified constants refused (:212, :326).
6. Same job: none seen.
7. Unused: `resolve_scalar_applications` called in lib.rs (2 matches with `publish_scalar_applications` in grep; lib.rs:2, constant_values.rs:1 = definition).
8. Stated limits: "Data with `requires` and policy-qualified leaves still need their checked obligations" (:309-311); "constant of data with requires needs its default-domain proof" (:467).
9. Fail-open: `constant_agrees` returns `Ok(false)` for unknown shapes (refusal side: caller raises "constant encoding and materialized carrier differ" at :298). Fail-closed. `subject_constant` accepts when `pending_target_observation && evaluated_value.is_none()` after `deferred_scalar_constant` (:217-220) without checking the value: stated at :148 by that function's checks of retained entry (traced by reading).

## contracts.rs (2968 lines)

1. Purpose: reads `requires`, `ensures`, `crashes` clauses and data/domain `requires` into canonical `Proposition`s and `ScalarTerm`s over formals, `result`, `old`, struct fields; also reads finite type-family alternatives for generic machines.
2. Shape: `ClauseScope` :34 (slots, values, bound, owner, result, subject, formal_fields, logical), `DataSubject` :70, `ResultTerm` :90; entry fns `contract_clause` :313, `clause` :347, `read_clause` :432, `term` :1255, `read_term` :1268, `identity_inputs` :732, `identity_clause` :900, `byte_predicate_clause` :948, `numeric_comparison_clause` :1089, `old_term` :1692, `clause_cast` :1771, `arithmetic_term` :1878, `mathematical_comparison` :1992, `read_mathematical_term` :2046, `field_term` :2158, `projected` :2221, `read_projection` :2240, `through_shared_referent` :2460, `family_alternatives` :2575 (pub), `mathematical_relation` :2940. `formal_place` :149. `needs_call_argument_formation` :101.
3. In/out: reads typed expression table, `Slot` roster, `ValueId` roster; produces `Proposition`, `(ScalarTerm, ValueType)`, `ClauseText`; mutates `clause_arguments`, completes operator selections (`complete_contract_operator`).
4. Decides: names resolve by symbol (`path.symbol`, `path.reserved_result_of`, `field_symbol`), explicitly "never by its spelling" (:1323-1326); builtin calls by `builtin_function_for_symbol` (:1956-1960); formal place identity by position count of record/borrowed formals (:149-163, `earlier + 1`), i.e. a position-derived identity that must match lowering's minting order (comment :145-148). Clause display text uses `display_name` (:1229) only for diagnostics (`ClauseText`).
5. Cases by shape: comparisons only (`unserved("clause that is not a comparison or conjunction")` :500); arithmetic directly as a clause refused (:620); ordering of Booleans refused (:708); IEEE comparison "without its selected declaration" (:703); mathematical terms: `+ - *` of embed/literal only (:2113-2126); `family_alternatives` accepts one clause listing `T == X || T == Y` closed scalar/record types, else walks static guards and forwarded calls (:2599-2931), "finite family listed by two clauses" refused (:2680).
6. Same job: `read_static_truth` in `types.rs` (suspected, not read) for static type equations; `mathematical.rs`/`mathematics.rs` same unit (mathematics.rs) provides `integer_embedding`, `mathematical_integer_literal` used at :2066, :2070. `fixed_bytes.rs` implements the same byte predicates for fixed arrays (fixed_bytes.rs:231), called from contracts.rs:965.
7. Unused: `family_alternatives` grep -> 4 matches in 3 files (README 1, lib.rs 2, contracts.rs 1 definition); so 2 call sites in lib.rs. `projected_arrival`: see logical.rs.
8. Stated limits: "memberships and named propositions are outside the subset" (:254); "Other formal projections need their own input or completion storage correspondence" (:1538); "no promotion of one into the mathematical integers is implied" (:2028-2029).
9. Fail-open: `contract_clause` returns `Ok(None)` (clause dropped) when `states_nothing` (:336-340): drops a clause decided true by static type equations; the false case is rejected in `states_nothing` (machine.rs:3893-3911), so traced: only truths are dropped. `read_projection` returns `Ok(None)` for non-storage expressions (:2246-2250, :2434) and callers turn it into a refusal ("term other than a field..." :2166, "case test of something that is not a sum" :570). None seen that accepts. `needs_call_argument_formation` (:101) treats Value/Boolean/Integer/IeeeFloatLiteral as already formed; stated at :99-100.

## expressions.rs (2583 lines)

1. Purpose: the expression reader: names to slots, literals, unary/binary operators, short-circuit operators, scalar and guarded `match` in value position, and `same_computation` (equality of two read expressions).
2. Shape: `slot_of` :50, `constant_value` :196, `erased_term` :231, `inferred_initializer` :287, `expression` :335, `read_expression` :347, `dereference` :846, `held` :891, `observed` :959, `place` :1000, `match_expression` :1075, `match_arms` :1153, `arm_value` :1266, `scalar_match` :1369, `guarded_match` :1495, `case_test_of` :1582, `case_condition` :1596, `binary_expression` :1720, `binary_selection` :1799 (pub(super)), `same_computation` :2018, `require_explicit_operand` :2245, `require_membership_carrier` :2283. Tests :2323-2583.
3. In/out: reads typed `ExpressionNode`s and `Slot`s; produces `body::Expression`; pushes `(span, label)` into `self.names` for arm labels; `self.captures` stack for match subjects.
4. Decides: slot lookup is by symbol, last match (`rposition`, :84-86), with comment "linear over the body's slots; bodies in the subset hold a handful of locals" (:82). Static truth `type_truth` from `read_static_truth` replaces a Binary/Member by a Boolean literal (:539-546). `label_arm` builds label text from `display_name` (:1338-1344) for warnings only (keyed by arm span). `same_computation` compares kinds, slots, literal values, callee identities; Block never equal; boundary Call equal only if `CallTarget::Machine` (:2085-2093).
5. Cases by shape: scalar match subject must be Integer or Boolean without domains (:1380-1399), arms must be literals (:1422-1432), only `true`+`false` together may omit `_` (:1446-1465); guarded match without `_` refused (:1542-1550); erased term only literal or binding (:231-273). "no ladder is judged that way yet" for complementary guards (:1491-1494). Operators on address types only Add/Subtract via `address_arithmetic_result` (:1653).
6. Same job: `binary_selection` (:1799) also selects operators for contract terms (contracts.rs:659, :1909); doc at :1796 states they select "the same operator from the same operand types".
7. Unused: `same_computation` 9 matches in 3 files within crate (expressions.rs 4, places.rs 1, transitions.rs 4) and 2 other files (typed-trees `references.rs` 4, `symbol_copies.rs` 1 are different functions of similar name, suspected unrelated). `binary_selection` callers: contracts.rs:659, :1909 and expressions.rs (read).
8. Stated limits: "Coverage is not decided here: lowering builds the terminator from the declaration's case table" (:1072-1074); complementary-guard coverage "no ladder is judged that way yet" (:1493); `ExpressionNode::UndeterminedNaN`, `Range`, `ZeroValue`, `TypeExpression` refused at :739, :788, :830, :836.
9. Fail-open: `match_arms` skips a later arm "whose earlier unconditional pattern already selects its values" (`continue`, :1238-1245) after reading and typing it; records it in `names.shadowed_patterns`; the arm is dropped from the lowered match (stated :1236-1237; a warning, traced). `scalar_match` for Boolean arms: when both `true` and `false` named, the last arm becomes default (:1464) (stated :1360-1363). `enclosing_value_capture` returns `Ok(())` when the name is unresolved but not an enclosing parameter/local (:104-115): wrong program (unresolved name) is handled elsewhere (`slot_of` refuses "name without a symbol", :71); traced by reading.

## fixed_bytes.rs (358 lines)

1. Purpose: states the primitive byte predicates `valid_utf8`, `ascii_only`, `no_nul`, `non_empty` of a fixed `[u8; N]` as an explicit proposition over `self[0..N]`, equivalent to the predicate (header :13-15).
2. Shape: `Bytes` :32 (`at`, `within`, `all`, `any`, `implies`, `continuation`, `valid_utf8`, `proposition` :205); `fixed_byte_predicate_clause` :231 (pub(super)); test :320.
3. In/out: reads array element/extent via `array_element`; produces `Proposition`.
4. Decides: element type must equal `u8` (:252-259), extent must be `CollectionExtent::Fixed` (:245-251).
5. Cases by shape: Unicode table 3-7 rules per byte position (:143-203); statement size is linear in N (one rule group per byte).
6. Same job: the view version is `ScalarTerm::byte_sequence_predicate` at contracts.rs:973; the choice is made at contracts.rs:960-971.
7. Unused: `fixed_byte_predicate_clause` 1 caller, contracts.rs:965 (read).
8. Stated limits: none seen beyond "An array whose elements are not `u8` is refused" (:229).
9. Fail-open: none seen. `within` returns `Falsehood` past the end (:65-67), the safe direction. The test (:320) checks all 1-2 byte arrays and edge-class 3-4 byte arrays against `predicate.holds_for`.

## float_meaning.rs (593 lines)

1. Purpose: reads `Identity<FloatMeaning>(a, b)` proof clauses into `Proposition::FloatMeaningEqual`, by evaluating proof-only IEEE "meaning" projections and semantic applications, unfolding ordinary meaning wrappers and verifying wrappers once.
2. Shape: `Primitive` :29, `intern` :38, `formed_wrapper_term` :53, `float_meaning_identity` :73, `meaning_term` :94, `meaning_primitive` :165, `meaning_format` :274, `meaning_application` :345, `meaning_definition` :414, `float_meaning_subject` :469, `returns_float_meaning` :492, `unfold_meaning` :498, `verify_meaning_wrapper` :520.
3. In/out: reads call expressions, sealed-intrinsic catalog (`FloatProjectionOperation::from_source_identity`, `FloatSemanticOperation::from_source_identity`); builds `FloatMeaningClause { values, left, right }`.
4. Decides: a primitive is recognized by `sealed_intrinsic(machine) == CompilerPrimitive::FloatProjection|FloatSemantics` (:172-177), then its namespace/name SPELLING: `display_path(machine,"::")` split at the last `::` and looked up in the sealed catalog (:190-206, :252-263). The format argument is read by decoding the selected constant's canonical value and comparing with `canonical_const_materialization_matches` (:302-318), and `format_of` uses `core_vocabulary(symbol)` (:319-323).
5. Cases by shape: float projection must have exactly one declared parameter of `f32`/`f64` (:207-229); semantic application parameters Format/Meaning/Integer only (:366-397); wrapper guarantees must each be an `Identity` call (:542-566).
6. Same job: `unfold_definition`/`bind_call_arguments` from clause_calls.rs (:506-510).
7. Unused: `float_meaning_subject` caller not grepped (inferred: reader entry for proof-only declarations). `returns_float_meaning` is called at machine.rs:490 and float_meaning.rs:183 and :479 (read); not grepped crate-wide.
8. Stated limits: "A proof-only declaration still owes its own body and guarantees in whole-package review" (:467).
9. Fail-open: `meaning_format` when `selected` is `None` (a struct literal with no matching constant) falls back to scanning all `const_declarations` for any format record that materialization-matches (:335-342); accepts an authored equivalent record by design (stated :270-273). `verify_meaning_wrapper` returns `Ok(())` immediately if `admitted_bodies.contains(machine)` (:525-527): a wrapper already admitted is not re-verified; traced: `within_unfolding` includes into `admitted_bodies` only after a successful read (clause_calls.rs:144-147), but `verify_meaning_wrapper` at :528 itself calls `within_unfolding`, so success marks it; a failure leaves it unmarked. Traced.

## logical.rs (1534 lines)

1. Purpose: builds the "logical invocation witness" for a callable arrival (a requirement entry calling a supplied implementation): mathematical snapshot inputs (leaves) from parameter types, nominal domain-membership atoms, premise theory, and a lowered abstract entry body that calls the supplied callee's signature.
2. Shape: `Leaf` :84, `Theory` :93, `NominalKey` :100, `Membership` :117, `Inputs` :126, `MathematicalAddressContext` :138, `MathematicalInputs` :176 (`new` :182, `validate` :279), `LogicalScope` :312 (`term` :377, `old_term` :398, `signature` :466), `scalar_type` :657, `Inputs::prepare` :702, `Inputs::input_type` :788, `Inputs::membership_clause` :1092, `projected_arrival` :1212 (pub(crate)).
3. In/out: reads `StateSignature`s, parameters, domain definitions; produces `CallableArrivalWitness { elaboration, required_inputs, supplied_inputs, nominal_premises, ... }`; calls `lowering::lower_generic` (:1504) with a two-machine roster.
4. Decides: paths ordered by `(arena_index, generation)` of symbol handles (:53-65) and looked up by binary search; membership correspondence by `(formal, field path, domain symbol, application id)` (`NominalKey` :100-115). No names except a `format!("{} in {}")` clause text (:531, :1203).
5. Cases by shape: refuses erased inputs (:199, :725), write-only references (:962), higher-order premises (:208), union/case input data (`DataMember::Field` only, :1031), generic/recursive/quotient data (:1007-1013), aliased/applied domains (:871-880), `Planned` issuance (:892), "outcome-specific logical guarantee" (:570), results (:1283-1291).
6. Same job: callable "arrival" refinement is also read in `callable_formations.rs`/`calls` (suspected; not read).
7. Unused: `projected_arrival` -> 2 matches (compilation.rs 1, logical.rs 1 definition): 1 call site (grep).
8. Stated limits: "Unread theory refuses the witness before proof search, so failure under an approximation cannot reject source" (:5-6).
9. Fail-open: `Inputs::input_type` for a non-nominal domain with `classification`, roles, cleanup, operators refuses (:905-917). `LogicalScope::source_leaf` returns `Ok(None)` if no leaf found (:449) and the caller `read_term` falls through to the other readers (contracts.rs:1293-1297); not an accept. None seen that accepts.

## machine.rs (4968 lines)

1. Purpose: the machine family: admission gate (`machine_shape`), signatures (formals, contracts, crash ceilings, termination), requirement/provider/witness machine identities, generic applications, and the body driver `read_body` that reads statements into `Body`.
2. Shape: `RequirementHeaderOrigin` :34, `Program::private_input_contracts` :173 (pub(crate)), `local_summary_bodies` :314 (pub(crate)), `machine` :461 (pub), `implementation` :566, `provider` :583, `requirement_body` :592, `read_machine` :642, `application` :769, `declared_requirement` :900, `witnessed` :939, `witnessed_signature` :1025, `callable_contract` :1093 (test/test-support only), `generic_requirement` :1126, `requirement_form` :1256, `served` :1432, `declaration` :1466, `machine_shape` :1518, `satisfied_requirements` :1734, `prepare_const_scope` :1775, `const_inputs` :1877, `value_binders` :1968, `assume_type_equations` :2012, `signature` :2093, `conformer_requirement` :2354, `inherit_requirement_contract` :2551, `requirement_signature` :2779, `byte_observation_signature` :2792, `numeric_primitive_signature` :2911, `callable_signature` :3188, `contract` :3396, `contract_parts` :3432, `termination` :3817, `states_nothing` :3859, `formal` :4004, `body` :4157, `read_body` :4205, `InputSymbolUses` :4612, `Declarer` :4896, `Reading` :4950, `PreparedFormalValues` :4916, `ValueBinder` :4943.
3. In/out: reads `Machine`/`State`/`StateSignature` rows from `self.typed`; appends to parallel tables on `Program` (`symbols`, `signatures`, `names`, `value_parameters`, `value_arguments`, `scopes`, `source_scopes`, `const_scopes`, `requirements`, `applied`, `fixed_contracts`, `conformance_witnesses`, `admitted_declarations`, `admitted_bodies`); returns `MachineId` (index+1) and `Body`.
4. Decides: machine identity is `MachineId::new(self.symbols.len()+1)` (:433-441) and lookup is a linear scan of `self.symbols` by `SymbolHandle` (:516-520). A machine is read once per symbol. Conformer detection through `typed.owned_conformance_realization` and trait-conformance rows (symbols). Public/private: `machine.is_public` picks crash default (:3699-3724). Numeric primitive recognition by `sealed_intrinsic(target) == FloatOperationRequirement` (:2917-2923). Byte-observation recognition by `compiler_primitive_schema` (:2812). One name-keyed spot: `parked_build_locals.iter().find` matches by `symbol_source_span(...) == Some(parked.entry)` (:4298-4307), a span compare.
5. Cases by shape (`machine_shape` :1518): refuses lexical region origin (:1548), assembly target (:1551), target selection (:1563), "supply mode other than a checked body" (:1587), provider-default purpose (:1589), "no complete body" (:1593), "pending type equations" (:1595), "owned data or conformances" (:1693), "`satisfies` clause on a generic machine" (:1706). `satisfied_requirements` handles only a trait without parameters (:1758-1764). `declaration` requires exactly one state (:1508). `contracts_restate_formal_domains` allows only `requires` domain-membership facts on a state (:1402-1415). `formal` refuses const, write-only, erased reference, mutable binding of a shared reference (:4016-4098). `termination` refuses published premises (:3825-3830).
6. Same job: reading a requirement signature (`requirement_signature`, :2779) vs callable signature (:3188) vs machine signature (:2093) share `contract`/`contract_parts` (:3396/:3432). `private_input_contracts` and `local_summary_bodies` repeat the same boundary-marking prologue over `entry`, `requirements`, `selections`, `fixed_contracts` (:179-201 vs :318-343).
7. Unused/one-caller: grep `\.callable_contract\(|\.private_input_contracts\(|\.local_summary_bodies\(` -> 3 matches in lib.rs (3 call sites; none counted in unit files). `callable_contract` is gated `cfg(any(test, feature = "test-support"))` (:1092).
8. Stated limits: "A `@write T` formal remains unsupported" (:3995-3996); "which a later slice serves" for erased record formals (:3988-3989); "Scalar result inference currently adds ordinary contract clauses, so its roster remains private-only" (:310-312); "An unchecked dependency body can hide a caller there" (:213-214).
9. Fail-open: (a) `served` returns `Ok(())` when no deferred-refusal row exists (:1443-1445): a declaration with no recorded refusal is served; stated by its doc (:1425-1431: rows are recorded in typing for unreached declarations and raised here; traced: "Every machine read passes here"). It filters out `DeferredRequest::LayoutPolicyStorage` rows (:1439-1441), those are deliberately not raised here (stated :1433-1434). (b) `private_input_contracts` `continue`s for any machine in an incomplete scope (:237-244), which keeps its input boundary open (the safe direction: it is not marked private). (c) `fact_restates_formal_domain` returns `false` for any unrecognised shape (:1329-1396), then the clause is read; fail-closed. (d) `contract_parts` `crashes`: public machine with no `crashes` clause becomes `Authored(canonical::routes([]))` i.e. no routes (:3710-3723); private -> `Inferred`. Stated by comment :3674-3679. (e) `requirement_termination` `NoGuarantee => Silent` (:3803): an omitted termination is not a promise (safe).

## mathematics.rs (923 lines; code 1-474, tests 476-923)

1. Purpose: reads transparent "mathematical definitions" (proof-only predicates over `embed`ded integers) used in callable premises, unfolds a call of one into the definition's body, and retains each definition once in `predicate_definitions`.
2. Shape: `mathematical_integer_literal` :29, `integer_embedding` :50, `mathematical_definition` :77, `address_bound` :93, `is_mathematical_term` :102, `mathematical_application` :125, `mathematical_clause` :220, `mathematical_predicate_definition` :251, `predicate_result` :334, `logical_mathematical_term` :389; three tests :703, :757, :821.
3. In/out: reads `typed.mathematical_definitions()`, `MathematicalParameter`s; fills `self.predicate_definitions[index]` with `PredicateDefinition { symbol, diagnostic, parameters, body }` (:324-329); returns `Proposition` / `IntegerMathTerm`.
4. Decides: a definition is found by `SymbolKind::MathematicalDefinition` plus linear scan for equal `symbol` (:77-91); `embed` by `builtin_function_for_symbol == IntegerEmbed` (:111-114); address bound by `builtin_type_member == AddressBound` (:99); result type must be `Strict<0>` by `fixed_core_declaration_symbol(Strict)` and a literal `0` (:334-387). A second read of a definition must equal the retained one (:314-322).
5. Cases by shape: only definitions without binders (:144, :275); arithmetic only `+ - *` with `Exact` policy (:451-466); recursion refused by `expanding` list (:290).
6. Same job: `read_mathematical_term` in contracts.rs:2046 does the non-logical version (`embed`, literals, `+ - *`); the two differ in how names resolve (contracts.rs refuses a bare name without embed, :2148; this file requires explicit embed too, :469).
7. Unused: `address_bound` callers: contracts.rs:2138 (and here :399). `is_mathematical_term` callers: contracts.rs:465, :1968 and here :452, :118.
8. Stated limits: "open mathematical premise application" (:149); "uninterpreted mathematical premise theory" (:288).
9. Fail-open: none seen. `is_mathematical_term` returns `false` for unknown shapes (:121), which routes the expression to the ordinary reader, where it is then refused or read as carrier arithmetic.

## nominal_applications.rs (534 lines)

1. Purpose: when a call passes a generic record whose type mentions const inputs, records which const arguments the actual and expected record types were built with, so the checker can compare complete nominal applications; publishes those rows into the module.
2. Shape: `nominal_argument_identity` :13; `NominalArgumentSource` :52; `NominalCallSource` :64; `nominal_constructor` :71; `nominal_const_inputs` :88; `nominal_call_argument` :186; `ConstInput` :311; `source_key` :316; `publish_nominal_calls` :331 (pub(crate)); `NominalArgumentSource::agrees` :506; `nominal_type_inputs` :512 (pub(crate)).
3. In/out: reads `record_sources[..].application`, `const_scopes`, type-reference table; writes into `terminal_psi::TerminalModule.identities` and returns `Vec<NominalCallArgument>`.
4. Decides: type identity is `NormalizedTypeIdentity` equality (`agrees` :506-508), not names. Calls are matched to emitted operations by `(machine, 0|1, handle.generation, handle.arena_index)` key (:316-327) and binary search. `visited` uses a `BTreeSet<NormalizedTypeIdentity>` (:96).
5. Cases by shape: a const argument is accepted only when it is a `Slot` of a const input of the caller, an `Integer` or a `Boolean` literal (:268-294), anything else returns `Ok(None)` (no row). `nominal_const_inputs` returns `None` for `ConstCall` lengths, open const expressions (not Integer/Boolean), and `Callable` types (:137, :174-178).
6. Same job: none seen.
7. Unused: grep (names `nominal_call_argument`, `nominal_type_inputs`, `publish_nominal_calls`): 1 call each: calls.rs:3207, lib.rs:1331, lib.rs:1333.
8. Stated limits: "An open expression needs its own exact input accounting before this route admits it" (:166-167).
9. Fail-open: `nominal_call_argument` returns `Ok(None)` (no nominal row, so no later identity check for that argument) in many cases: non-machine callee (:197-199), non-record types (:200-204), no application (:205-211), `nominal_const_inputs` None with empty `values` (:212-221), no used const inputs (:222-224), identity not computable (:225-229), a const argument that is not slot/literal (:293). Open: no comment says that dropping the row is safe; `publish_nominal_calls` later errors only for rows that exist but are not consumed (:477-485). I did not trace whether another check covers the dropped cases (so: "open" for the `_ => return Ok(None)` at :293 where a non-literal, non-slot const argument skips the row).

## numeric_contracts.rs (141 lines)

1. Purpose: for Core numeric primitive declarations, reads the source contract's requires and crashes clauses and compares them with the catalog's canonical ones; checks ensures clauses only for operand types.
2. Shape: `numeric_contract_sources` :20 (pub(super)); called from machine.rs:3011.
3. In/out: reads `IntrinsicRequirement`, the catalog `Signature`; returns `Checked<()>`.
4. Decides: `conjunction(requires) != conjunction(signature.requires)` (:120-126); `canonical::routes(routes) != expected_routes` (:134-138); both are structural equality of canonical propositions.
5. Cases by shape: only `Requires`, `Ensures`, `Crashes`; `EnsuresForResultCase` refused (:113).
6. Same job: signature building for the same primitive is `numeric_primitive_signature` machine.rs:2911.
7. Unused: called at machine.rs:3011 (read); not grepped crate-wide.
8. Stated limits: "does not prove that equality" for ensures (:3-5, paraphrase of header: reading an ensures clause checks operand types and selects operators).
9. Fail-open: the source `ensures` clauses are read (`complete_typed_contract_operator`, :90-96) but their meaning is not compared with the catalog's `ensures` (the `ensures` vector from the catalog is used as the signature: machine.rs:2933-2934, 2986). Stated at header :3-6 ("does not prove that equality"). A source declaration whose ensures disagrees with the catalog is not refused here (traced: only operand types are checked); the catalog's own ensures is what is published.

## places.rs (2311 lines; code 1-2080, tests 2082-2311)

1. Purpose: reads data declarations into storage-record rows (fields, sum cases, ownership, regions, `requires`, domain field predicates, cleanup selection), record/case literals, field and payload member access, case tests, destructuring patterns, zero values, and lifetime-binder checks.
2. Shape: `RecordSource` :53; `Member` :78; `record` :97, `record_at` :120, `occurrence_regions` :154, `record_definition` :280 (the record builder), `finish_cleanup_selections` :649, `declared_members` :686, `declared_field` :734, `data_type` :768, `member_of` :794, `require_intrinsic_tag_equality` :823, `case_position` :846, `record_pattern` :944, `record_arm` :1031, `unselected_member` :1085, `member` :1158, `record_literal` :1226, `zero_value` :1414, `named_case` :1500, `case_test` :1556, `case_selector` :1586, `case_pattern` :1640, `record_roster_coverage` :1728, `record_shape_for` :1788, `lifetime_binders` :1907, `lifetime_application` :1968, `reference_lifetime_declared` :2020. Free fn `storage_identity_for_inputs` :42.
3. In/out: reads `DataDefinition`, members, `SourceTypeApplication`; mutates `self.records` (an `IdTable<StructuralTypeId, RecordType>`), `self.record_sources`, `value_records` (sorted vec), `resolving`, `fields_minted`, `cases_minted`, `pending_cleanup`; returns `ValueType::Record`.
4. Decides: fields, cases and members are identified by `SymbolHandle` (`member_of` :801-806, `case_position` :852-855, `field_position` :777-789); record reuse by `declaration` symbol plus `same_application` (:289-308, :331-335, linear scan over `record_sources`). Text-keyed spots: the record's identity string is the rendered normalized application text (`typed.type_identity_text`, :394-400) with `" (checking machine N)"` appended by `format!` (:42-47) so rows of different checking machines differ; `unselected_member` compares `field.name.as_str() == name` (:1125) only to choose the diagnostic; `duplicate lifetime binders` compare `as_str()` (:1921-1923); `lifetime_application` compares tag strings (:1989, :1996). Ranges: `declared_field` uses `member_identity().spelling()` as the structural field id text (:753).
5. Cases by shape (`record_shape_for` :1788): refuses opaque data with members (:1797), generic data (:1812-1817 "generic data"), quotient data (:1818), retired identities (:1826), a variant with `requires` ("data requires", :1839), erased fields (:1845, :1851), and "mixed data" (fields beside cases, :1860, comment :1857-1859: "no slice of this lane lowers yet"). `record_pattern` serves only an owned record place pattern (:957-964). `record_literal` refuses opaque literals (:1300).
6. Same job elsewhere: the lifetime-tag checks here duplicate the typed-trees validator, stated at :1964-1967 ("being removed"). The `requires`-reading path uses contracts.rs clause reader (:549).
7. Unused: `finish_cleanup_selections` 2 call sites (lib.rs:1124, :1171; grep). `named_case` is called at contracts.rs:564, expressions.rs:631 and places.rs:1657 (read; not grepped crate-wide).
8. Stated limits: "no slice of this lane lowers yet" (:1858-1859, mixed data); "nominal cleanup on an applied data type" refused (:638); `Unserved::Type("copy data with an owned field")` (:485).
9. Fail-open: (a) `record_definition` `known` branch: a record found by `(declaration, application)` is reused (:331-382) after `record_shape_for` admission re-runs (:312-319) so a changed purpose cannot reuse; stated at :310-311. (b) `field_predicates` etc. in predicates.rs rather than here. (c) `record_arm` accepts a destructuring pattern as "takes every value like `_`" when `full_cases.is_empty()` (expressions.rs:1221-1224) after `record_roster_coverage` (:1075), stated at :1020-1025. (d) `lifetime_application` accepts any tag when `declares(tag)` is true and no retained resolution exists (:1995-2002); stated by doc :1958-1967. None found that accepts an unmatched case silently.

## predicates.rs (885 lines)

1. Purpose: turns the `requires` of every named domain a type carries into ordinary contract clauses: entry requires for a formal, ensures for a result, data `requires` for a field, and the predicate attached to a `value as +D` cast; also reads authored `T in D` membership facts in contracts.
2. Shape: `Predicate` :60; `runtime_membership_clause` :71; `runtime_membership` :93; `read_runtime_membership` :120; `predicates` :269; `of_value` :300; `signature_predicates` :339 (pub(super)); `subject_text` :435; `clause_subject` :455; `prepare_domain_predicates` :481 (pub(crate)); `cast_predicates` :539 (pub(crate)); `record_cast_predicates` :583 (pub(crate)); `stated` :628; `declared_predicates` :647; `storage_predicates` :726; `field_predicates` :824 (pub(crate)); `SignaturePredicates` :882.
3. In/out: reads `domain_theory(id, instance)` (`DomainDefinition`), proof facts, `ClauseScope`; returns `Vec<(Proposition, ExpressionHandle)>` and clause text; mutates `declared_domains[slot].predicate`/`complete_predicate_theory` (:502-533).
4. Decides: domains identified by `DomainInstance { declaration symbol, identity }`; a domain's `self` is the symbol `definition.subject_symbol` (:288, :309). "Nominal" evidence is decided by `access_policy.is_some() || issuance != Unrestricted` (:149-153). A predicate is dropped when `states_nothing` (:284). `clause_subject` finds the subject by the first valid `Name` symbol in the clause (:455-474).
5. Cases by shape: a nominal runtime membership must be about the exact normal result subject, three accepted subject forms (:203-224), else refused "routed contract membership needs its exact normal result subject"; byte-slice domains special-cased through `byte_slice_domain_predicate` (:659, :749); record cast predicates refuse a non-shared byte sequence (:592-601).
6. Same job: `contracts.rs::contract_clause` calls `runtime_membership_clause` (contracts.rs:332).
7. Unused: grep shows `signature_predicates` 1 call (machine.rs:3656); `prepare_domain_predicates` 5 matches outside this file's definition (lib.rs:980, 1133, 1170; read.rs:1041, 1111; some may be comments/wrappers; not individually opened).
8. Stated limits: "storage of a rigid type in a domain with predicates" (:744); "A case payload, an array element and a partial write ... retain their independent establishment restrictions" (:39-41).
9. Fail-open: (a) `signature_predicates` for a result of `ValueType::Rigid(_)` returns an empty clause list (:422): a result qualified by a domain with predicates on a type-parameter carrier states no guarantee; open (no comment; `predicates()` for Rigid is evaluated earlier only for formals at :350 and rigid formals return unserved at :375, but the result arm does not). (b) `field_predicates` returns `Ok(Vec::new())` for readable byte views whose domains are all primitive byte-slice predicates (:849-852): stated at :830-833 ("already owns proof of its primitive predicates at every store"). (c) `declared_predicates` for `ValueType::Record(..)` returns `Ok(())` (:714): stated :712-713 (read by Qualifications). (d) `prepare_domain_predicates` sets `predicate: None` for a record domain with no predicates (:522-524) which is correct for that case.

## provider_methods.rs (211 lines)

1. Purpose: finds, for a closed (applied) public requirement, the specialized method body prepared for those arguments; and reads a public header under the implementation's own type-parameter binders.
2. Shape: `conformance_method_bodies` :12, `with_method_type_parameters` :73, `provider_method_requirement` :139.
3. In/out: reads `typed.machine_specializations`, `machine_instantiation`, static applications; returns `Vec<(&StateSignature, &Machine)>`.
4. Decides: specialization receipts filtered by `receipt.template == supplied.symbol` (:39); members sorted and duplicate-checked by header symbol (:56-66); argument equality via `static_binding_matches_argument` (:197).
5. Cases by shape: public type parameters are matched to implementation parameters by position and kind (`Type` only), with a count equality check (:90-95); non-`Type` public parameters skipped (:99-101).
6. Same job: none seen.
7. Unused: grep: `conformance_method_bodies` 2 call sites (subjects.rs:333, traits.rs:844); `with_method_type_parameters` 2 (machine.rs:1015, :2588); `provider_method_requirement` 2 (machine.rs:2392, provider_methods.rs:42).
8. Stated limits: none seen.
9. Fail-open: `provider_method_requirement` returns `Ok(None)` when the symbol is not a specialization (:147-151) or has no closed requirement application (:161-163); callers treat `None` as "no method requirement" (provider_methods.rs:41-45 `continue`; machine.rs:2393-2399 `public_method.is_some_and`). A specialization with no closed requirement is skipped from the member list. Open: not traced whether that skipped specialization can be selected elsewhere.

## provider_selections.rs (245 lines)

1. Purpose: admits the exact provider records the source policy chose (`ProviderSelections`) and builds `SourceProviderSelections` rows plus supplied-requirement rows; infers nothing from names (header :1-2).
2. Shape: `prepare_provider_selections` :16 (pub(crate)).
3. In/out: reads `ProviderSelections { source, choices }`, conformance declarations, static contracts' conformance questions; pushes to `self.supplied` and `self.admitted_declarations`; returns `SourceProviderSelections`.
4. Decides: by symbol and identity (`same_application` :137, `root_application_identity() != Some(application)` :172); owner check `ptr::eq(typed, selected.source)` (:22). The compared `application` is the canonical identity TEXT (`type_identity_text`, :162-171), compared as a string at :172.
5. Cases by shape: a provider with type or lifetime parameters needs an explicit complete application (:114-121); question entries not `SourceImplementation::Machine` are skipped (:191-197, `continue`).
6. Same job: `provider_selections::publish` in `<05>/src/provider_selections.rs` (not read).
7. Unused: 1 call site (lib.rs:1115, grep).
8. Stated limits: "No candidate body, schema application or declaration is inferred from a name" (:1-2).
9. Fail-open: the loop over `questions` `continue`s for any question without a provider candidate, with a non-Machine implementation, or whose receiving application does not match (:191-203): those operations get no `SuppliedRequirement` row. Open: no comment says a missing row is harmless; the rows only add supplies, so a skipped row means a requirement may stay unsupplied (a refusal later is likely, not traced).

## providers.rs (580 lines; code 1-517, test 519-580)

1. Purpose: discovers native (externally realized) provider declarations that are demanded or selected, retains their source owners, reads their headers into `ExternalProviderSource` rows (signature, service-reach envelope, declaration, binding identity, sealed intrinsic), and registers them as boundary declarations.
2. Shape: `select_native_provider` :20, `prepare_external_providers` :47 (pub(crate)), `native_provider_source` :118, `read_native_boundary_declaration` :338, `has_native_provider_owner` :401, `numeric_catalog_supply` :435, `native_binding` :458.
3. In/out: reads machines with `satisfies` edges, `self.boundaries.rows`, `self.external_providers`; writes `external_providers.rows/selected`, `boundaries.rows`, `admitted_declarations`.
4. Decides: ownership by `machine.supply_mode` (`ExternalRealization{binding, mechanism}` or `Boundary` with sealed `FixedImageRealization`) (:405-429); the sealed intrinsic is looked up by SPELLING: `sealed_declaration(machine.symbol, machine.name.as_str())` (:408-410, :424-425, :493-494) and `sealed_member_intrinsic(owner, required.name.as_str())` (:496-497). Demand is by symbol: `demanded` is the sorted `requirement_symbol` set matched by binary search (:55-72, :83).
5. Cases by shape: native provider must be a closed non-generic machine with one state, no body and no state contract (:171-199); the catalog path requires `NativeFloatOperation` + `CompilerIntrinsic` binding + every edge naming the same binding (:444-455); fixed-image requires matching `FixedImageRealization(x)`/`FixedImageRequirement(x)` (:498-502).
6. Same job: boundary declaration reading in `boundaries.rs` (caller of `read_native_boundary_declaration` at boundaries.rs:285; suspected duplication of header reading, not read).
7. Unused: `select_native_provider` 3 callers (subjects.rs:157, :440, traits.rs:846); `prepare_external_providers` 1 (lib.rs:1159); `read_native_boundary_declaration` 1 (boundaries.rs:285).
8. Stated limits: "The authenticated sealed physical leaf has no outgoing Omega service calls. This is its catalog-owned inventory" (:235-237); a bodyless `satisfies` header "supplies neither a binding nor compiler custody" (:397-398).
9. Fail-open: (a) `select_native_provider` returns `Ok(false)` (not a native provider) when the machine is missing, has no native owner, or has no conformances (:25-33): the caller then reads it as an ordinary machine; traced by reading the predicate (:401-430): only external/fixed-image bodyless machines count. (b) `operational.service_reach_row` for a sealed intrinsic with a NULL row is set to `EMPTY_ROW` (:228-239), i.e. "no service calls" is assumed; stated at :235-237 and only reached when `sealed_intrinsic.is_some()`.

## rankings.rs (311 lines)

1. Purpose: reads a loop/machine's decreasing measure (ranking) over the body's formals into a `Ranking` with one measure and an optional range.
2. Shape: `ranking_view` :17, `ranking` :135 (pub(super)), `distance` :236, `catalog_natural_carrier` :286, `natural_subject` :290.
3. In/out: reads `machine.termination_plan.implementation_witness`, `ranking_expression_custody_for`, slot roster; returns `Option<Ranking>`.
4. Decides: the selected view is `RankingViewSelection` (Builtin SLICE_LENGTH / CanonicalDefault / Declaration(symbol) / Unresolved). For `Declaration`, admission needs `sealed_declaration(symbol, declaration.name.as_str()) == CompilerPrimitive::NaturalRanking` and then `RankingViewId::canonical(declaration.name.as_str())` (:34-50): a name-keyed catalog lookup after the sealed check. A tag mismatch is refused by `witness.ranking_view != selected.0` (:122-129).
5. Cases by shape: views NAT_BOUNDED_DISTANCE (two subjects), NAT_DESCENDING and SLICE_LENGTH (one subject) only (:153-188); everything else "ranking view has no normalized integer relation" (:183-187). The declaration header must be exactly a plain `u64 -> u64` bodyless compiler-provided machine (:51-113). Ranking carrier must be a closed finite non-address integer (:189-193).
6. Same job: `ranking` / `distance` here vs lowering's ranking edges (`<05>/src/lower/ranking.rs`, not read); header comment: "A familiar name does not turn an authored function into a compiler ranking theory" (:1-2).
7. Unused: `ranking` caller: machine.rs:4287 (read).
8. Stated limits: "ranking view arguments" refused (:146-148).
9. Fail-open: `ranking` returns `Ok(None)` when the machine has no `implementation_witness` (:141-143), i.e. no ranking is read for a machine without a witness; whether a loop then needs one is decided by the termination plan elsewhere (not traced): open.

---

## Coverage

All 18 files read in full, every line, in order (full Read calls; no skimming):

| file | lines read |
| --- | --- |
| read/casts.rs | 1-921 (921) |
| read/clause_calls.rs | 1-385 (385) |
| read/constant_values.rs | 1-513 (513) |
| read/contracts.rs | 1-2968 (2968) |
| read/expressions.rs | 1-2583 (2583) |
| read/fixed_bytes.rs | 1-358 (358) |
| read/float_meaning.rs | 1-593 (593) |
| read/logical.rs | 1-1534 (1534) |
| read/machine.rs | 1-4968 (4968) |
| read/mathematics.rs | 1-923 (923) |
| read/nominal_applications.rs | 1-534 (534) |
| read/numeric_contracts.rs | 1-141 (141) |
| read/places.rs | 1-2311 (2311) |
| read/predicates.rs | 1-885 (885) |
| read/provider_methods.rs | 1-211 (211) |
| read/provider_selections.rs | 1-245 (245) |
| read/providers.rs | 1-580 (580) |
| read/rankings.rs | 1-311 (311) |

Total read in full: 20,964 lines (sum of the unit's line counts). Skimmed: 0. Not read: none. Crate-wide caller greps were run only for the names listed in items 7; everything else marked "read" or "inferred" was not grepped.

## Five things a maintainer most needs to know about this unit

1. Every file is a slice of one `impl Program` (the reader); a refusal is either `refuse(...Unserved::X(&'static str))` (unsupported, e.g. contracts.rs:500, machine.rs:1587) or `reject(...)` (violation with an authored message). The `&'static str` detail text is the only stable name of most limits (tests assert it, e.g. casts.rs:899, places.rs:2163).
2. Contract clauses are read once into canonical `Proposition`s over `ValueId`s and `PlaceId`s; formal places are numbered by position (`formal_place`, contracts.rs:149-163: k-th record/borrowed formal is place k+1), which must match the order lowering mints them.
3. Machine identity is `symbols.len()+1` with a linear scan by `SymbolHandle` (machine.rs:433-441, :516-520); `read_machine` pushes parallel tables (`symbols`, `signatures`, `names`, `scopes`, ...) so all tables must stay the same length (machine.rs:736-761, :1300-1308, :1079-1086).
4. Several recognitions use a declaration's spelling after a sealed-catalog check: `sealed_declaration(symbol, name.as_str())` (providers.rs:408, :424, :493; rankings.rs:37), `sealed_member_intrinsic(owner, name)` (providers.rs:496), `FloatProjectionOperation::from_source_identity(namespace, name)` (float_meaning.rs:205). Constant agreement compares decoded names to declaration names (constant_values.rs:449, :476, :503).
5. Dropped or empty results that may hide a skipped obligation (open, not traced): `signature_predicates` Rigid result (predicates.rs:422), `nominal_call_argument` `Ok(None)` paths (nominal_applications.rs:197-293), provider-selection `continue`s (provider_selections.rs:191-203), `numeric_contract_sources` not comparing ensures (numeric_contracts.rs:88-97), `ranking` None (rankings.rs:141-143).

## What you are unsure about

- Whether the `Ok(None)` paths in `nominal_call_argument` are covered by another check (not traced; callers at calls.rs:3207 not read).
- Whether `domain_entry_change`'s Keep branch with `domain_definition == None` (casts.rs:632-644) can be reached by a non-policy domain.
- Call-site counts for functions I did not grep (`add_domain`, `issue`, `domain_removal`, `named_case`, `float_meaning_subject`, `returns_float_meaning`, `address_bound`, `is_mathematical_term`) are from reading only.
- Meaning of the `lib.rs:980` match for `prepare_domain_predicates` (possibly a comment or test); not opened.
- Where `read.rs:1041`/`:1111` use `prepare_domain_predicates` (wrapper vs call); not opened.

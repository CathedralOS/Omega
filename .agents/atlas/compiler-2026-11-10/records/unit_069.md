# Unit 69 notes

Files covered (all under `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/`):
1. `proofs.rs` (lines 1-8032)
2. `retention.rs` (lines 1-6094)
3. `scalar_computations.rs` (lines 1-7824)

Notes are written incrementally (one file, sections appended as read).

## proofs.rs

### proofs.rs 1-4600 (read in full)

**Purpose.** Lowers checked-tree proof material (content conservation plans, contract predicates, crash-route buckets and predicates, structural member/term paths) into Terminal Psi `Proposition`s, `ScalarTerm`s and `CrashRouteBucket`s. File header comment: "Proposition, contract, crash-route, content-conservation, float-meaning projection, integer predicate, and semantic evidence lowering into `TerminalModule`" (proofs.rs:2-4). It is a file of nested `pub(crate) mod` blocks, not a flat module.

**Shape (module map, read).**
- `mod content_conservation` (proofs.rs:59-807): `LoweredContentConservation` (79), `LoweredContentIdentityReshuffles` (86), `lower_structural_content_projection` (194), `lower_whole_content_entry_claims` (237), `lower_content_conservation_plan` (394), `lower_boundary_content_guarantees` (430), `lower_content_identity_reshuffles` (459), `lower_content_term` (717), `lower_content_place` (755), const `RESULT_STRUCTURAL_PLACE_ID = 4_294_967_297` (715), `MAX_CONTENT_TERM_DEPTH = 256` (714).
- `mod contract_predicates` (808-1303): `integer_math::lower` (821, budget + depth 64), `integer_math::validate_namespace` (904, budget 4096), trait `PredicateTerms` (936) with default methods `case_membership` (947, returns unsupported) and `strict_bound` (960, `LessThan`), `proposition` (965, budget 4096), `proposition_with_polarity` (1000), `ieee_float_proposition` (1179), `equality_from_polarities` (1205), `canonical_equality` (1231), `canonical_propositions` (1246), `canonical_connectives` (1265), `connective` (1288).
- `mod crash_routes` (1304-...): sub-mods `argument_prefix` (1306), `crash_predicates` (1640), `scalar_terms` (1978), `structural_arithmetic` (2553), `structural_members` (2765), `structural_route_buckets` (3557); top-level fns `lower_boundary_crash_routes` (4378), `lower_formal_crash_routes` (4412), `formal_crash_parameters` (4424), `normalize_formal_crash_routes` (4440), `lower_checked_crash_frontier` (4467), `lower_checked_crash_exit` (4485), `lower_crash_route_buckets` (4565), `lower_crash_cause` (4598).

**Inputs/outputs.** Reads `CheckedTrees` facts (`facts.qualifications.content`, `facts.flow`, `facts.borrow`, `facts.contract_plans`), plus Terminal Psi declarations (`StructuralTypeDeclaration`, `StructuralParameterDeclaration`, `ValueDeclaration`). Produces `Proposition`, `ScalarTerm`, `terminal_psi::CrashRouteBucket`, `ContentEntryClaim`, `ContentIdentityReshuffle`. Mutates the `TerminalIdentityTableBuilder` (interning identities) in several functions.

**How it decides (keys).**
- Content projection acceptance keyed on `carrier_identity` string equality against expected carrier text (proofs.rs:207, 346-348) and a nonzero/replayed `report_fingerprint` (208, 221-229, 353).
- Reshuffle grouping keyed on `(source_claim, output place)` (525-527); overlap check is segment-prefix equality on places (703-712).
- Content field lookup keyed on `symbols::SymbolHandle` scan of every data definition's members (97-109) — linear scan per lookup (read).
- Crash capture lookup keyed on `(statement_index, call_ordinal)` of the call (1338-1348) and `owner_operand` handle plus empty `capture_path` (1363-1366).
- Case membership matched by interned spelling: `case.identity == identities.intern(&case_identity.spelling())` (3544) — source-spelling-keyed (read).
- Crash exit: one covering bucket required (`let [covering_bucket] = ...` 4515); site predicates matched by `site_identities.contains(predicate)` (4531).
- Frontier claim mapping by linear `find_map` over `source_claims` (4474-4476).

**Cases by shape (fixed lists that refuse the rest).**
- `lower_boundary_content_guarantees` only takes plans with `owner_kind == TraitRequirement` and `callable == callable` (436-439).
- `direct_identity_projection` (666-701) accepts only a pair of `Projection` terms, one `Entry` and one `Current`, with input root < `RESULT_STRUCTURAL_PLACE_ID` and output root == it; `Separate` terms refused (`ContentIdentityNotDirectEquality`, 683-686).
- `proposition_with_polarity` fallthrough `_ => unsupported("result contract has an unsupported scalar predicate")` (1175). Handles StructuralIdentity, StructuralCaseMembership, IntegerMathComparison, IntegerComparison, ScalarIeeeFloatComparison, And/Or/Not, Constant, Parameter/StructuralParameterField/ByteSequencePredicate, Equal (1011-1174).
- `has_scalar_boolean_denotation` (973) whitelist: Constant, Parameter, StructuralParameterField, ByteSequencePredicate, Not, Equal; anything else returns `false` (994).
- `lowered_direct_scalar_term` (2166-2471): explicit list of refused `LoweredDirectExpression` variants (PrimitiveRead, IndexedPrimitiveRead, StructuralField, AtomicLoad, ImplementationSelection, ByteSequenceFieldLength/Read, ElementViewRead, RepresentationView) each `unsupported`. Trapping arithmetic refused: "Trapping arithmetic forms no crash predicate term" (2343).
- `ByteSequenceLength`/`ElementViewLength` only with empty path: "projected crash extent requires its canonical descriptor path" (2221).
- `structural_crash_route_prefix_from_type` (1504-1638): refuses AllElements, RepresentationView, FixedRange, RuntimeIndex, unfinished case, WriteOnlyBorrow reference; only `FixedArray` with `ArrayExtent::Literal` for FixedIndex (1615-1618).
- `argument_prefix::captured_crash_argument` (1317): only handles args whose path starts with `Referent` (1330); other args returned unchanged (1331).
- `RouteTerms::lower_term` (3695-3785): `Local`, `ErasedParameter`, `And`, `Or` => "structural crash route contains an unsupported Boolean term" (3778-3783).
- `structural_arithmetic` (2553-2764): fixed `requirements.contains(...)` pattern-matching for safe divisors/shifts (see fail-open below).

**Same job elsewhere.**
- Polarity-based Boolean lowering exists three times in this unit: `contract_predicates::proposition_with_polarity` (1000), `crash_predicates::checked_boolean_connective_polarity` (1816) and `RouteTerms::lower_polarity` (3838). All three call the same `equality_from_polarities` (1205) but each re-implements Constant/Not/And/Or handling (read). (Grep for call counts at the end of this file's section.)
- Scalar-term lowering for crash predicates: `scalar_terms::checked_boolean_scalar_term` (1993, checked form) vs `checked_boolean_scalar_term_from_lowered` (2473, lowered form) vs `RouteTerms::lower_term` (3695): three Boolean-to-`ScalarTerm` translators with overlapping variant lists (read).
- Read-projection selector lowering `lower_checked_read_projections` (3131) alongside `lower_structural_indexed_place` (3055) and `lower_sequence_extent_projection_term` (2956) repeat the "WriteOnlyBorrow => unsupported" readability test (2857, 2874, 3150) and the byte-carrier WriteOnly test (2940-2950, 3219-3227).
- `safe_exact_structural_divisor` (2559) and `safe_policy_structural_divisor` (2613) are near-copies that differ in handling of -1.

**Unused / one-caller** — see grep results appended at the end of the proofs.rs section.

**Stated limits (quoted <20 words).**
- "content conservation requires its own call correspondence" (4322).
- "structural crash predicates require structural signature lowering" (2026).
- "universal qualification is not a crash operand path" (1524).
- "byte window crash predicates require exact view extent custody" (1552).
- "runtime-indexed crash predicates have no canonical element prefix" (1575).
- "an explicit crash in the terminal-Psi source slice requires exactly one prechecked covering route bucket" (4516).

**Fail-open risks.**
- `safe_exact_structural_divisor` / `safe_policy_structural_divisor` / `safe_exact_structural_shift` (2559-2763) return `true` when the requirement list contains a matching `LessOrEqual` fact, and `true` for literal nonzero divisors. They do not fail open on missing requirements (return `false`). Status: none seen as fail-open; they are fail-closed on absence (read). Literal signed divisor `-1` is accepted by `safe_policy_structural_divisor` (2625-2626: only `!= 0`) — this is the "policy" variant, whose comment-free intent I did not trace: open.
- `lower_whole_content_entry_claims` `continue`s when `content.is_empty()` (306-308) or `projections.is_empty()` (368-370): an entry claim with no content qualification is silently dropped (no error). Which wrong program gets through: a parameter claim whose qualifications don't resolve to a content projection (via `filter_map` at 298-305 which silently drops `None` from `for_semantic_domain`) is dropped rather than rejected. Status: open.
- `lower_boundary_content_guarantees` (430): plans for other callables silently filtered (436-439); not a risk by itself.
- `normalize_formal_crash_routes` (4440-4465): drops a bucket guard that is `Constant(false)` (4452) and rewrites `Constant(true)` to `Truth` (4454). A bucket whose guards all vanish is dropped by `filter_map` if `CrashRouteBucket::new` returns `None` (4462-4463; I did not read `new`). Status: open for the all-false case.
- `lower_checked_crash_exit` site_guard: guards in the covering bucket not in `site_identities` are dropped (4542); if bucket has `Truth` the empty `site_guard` is accepted (4545-4549). That is by design for a covering Truth route (traced from the code: the `Truth` case is the explicit pass).
- `substitute_structural_crash_route_roots` `_ => {}` arms (4220, 4326): terms/propositions of unlisted kinds are left unsubstituted. Status: open (variants matched through macros I did not expand).
- `PredicateTerms::strict_bound` default (960) and `case_membership` default returns unsupported (952) — fail-closed.

(Section continues below.)

### proofs.rs 4601-8032 (read in full)

**Purpose.** Evidence lowering (proposition vocabulary, evidence terms, contract lanes, outcome-specific call evidence and ensure rows, proof-output calls with static requirement dispatch), the float-meaning projection resolver, and the structural integer-predicate lowerer. Installs results on `LoweredPsi.semantic_module`.

**Shape.**
- `mod evidence_lowering` (4620-7128): sub-mods `evidence_terms` (4623), `outcome_specific_calls` (5264), `outcome_specific_ensures` (5779), `proof_output_calls` (5996); entry `lower_and_install_evidence_artifacts` (6668); helpers `binder_argument_kind` (6792), `checked_evidence_requirement_identity` (6808), `evidence_projection_identity` (6818), `checked_requirement_schema` (6846), `checked_static_conformance_rows` (6946), `checked_requirement_family_rows` (6971), `exact_trait_requirement` (7061), `checked_evidence_machine_identity` (7090).
- `evidence_terms`: `lower_proposition_vocabulary` (4652), `lower_evidence_term_ids` (4823), `lower_evidence_terms` (4988), `lower_evidence_interface` (5085), `lower_evidence_contract_lanes` (5125), `selected_guarded_evidence_terms` (5189), `evidence_argument_subjects` (5207).
- `outcome_specific_calls`: `lower_outcome_specific_call_evidence` (5303), `lower_argument_subjects` (5650), `lower_argument_subject` (5679).
- `outcome_specific_ensures::outcome_specific_ensure_rows` (5789).
- `proof_output_calls`: `lower_proof_output_calls` (6016), `lower_static_requirement_dispatch` (6171), `proof_output_forwarded_source` (6408), `terminal_evidence_term_id` (6461), `terminal_proposition_application_id` (6473), `lower_proof_output_runtime_call` (6548).
- `mod float_meaning_projection` (7129-7747): `lower_float_meaning_equality` (7153), `lower_float_meaning_projection` (7163), `resolve_direct_float_source_binding` (7294), `resolve_direct_float_parameter` (7453), `resolve_direct_block_float_parameter` (7510), `resolve_float_operation_result_source` (7596), `rejoin_float_semantic_applications` (7656), `FloatMeaningProjectionLoweringError` (7733).
- `mod integer_predicates` (7748-8032): single `lower` (7758) that handles Widen, ExactCast, BitwiseNot, bitwise And/Or/Xor, shifts, arithmetic, and falls to a caller-supplied `leaf` closure (8029).

**Inputs/outputs.** Reads `checked.facts.proof` (evidence_terms, evidence_forwardings, proof_output_calls, outcome_specific_arms/guarantees, proposition_vocabulary, contract_facts), `checked.typed` (conformance applications, trait signatures, machine specializations), and the lowered `TerminalModule` (closed_conformance_applications, machines). Writes `lowered.semantic_module.{proposition_declarations, proposition_applications, evidence_terms, evidence_contract_lanes, proof_output_calls}` (6784-6788), per-machine `contract.outcome_specific_ensures` (6780), and the `selected_evidence` slot of call operations (5645).

**How it decides.**
- Evidence term identities: union-find over forwardings and proof-output forwards (4827-4874; `evidence_term_root` 4643), then ordered by lane key tuple `(machine_position, kind rank 0..4, position)` (4876-4964). Identity = canonical order, not source name.
- `lower_and_install_evidence_artifacts` renumbers terms by sorting `(proposition, interface, id)` and loops up to 4 rounds until stable (6689-6730): "evidence term identities reach no canonical order" if not.
- Proposition declarations matched to terminal declarations by `declaration.name == declaration_name` string compare (6494-6498) in `terminal_proposition_application_id`, while `lower_proposition_vocabulary` matches by symbol (4751-4753). Two keyings of the same relation (read).
- Applications matched by structural equality of `(declaration, binder_arguments, arguments, evidence_interface)` (6534-6542; 5042-5049).
- Static requirement dispatch (6171-6406): re-derives the expected closed conformance application from checked facts and searches `semantic_module.closed_conformance_applications` with an all-fields filter (6317-6344), then demands exactly one (6345). Keys are interned display-path strings (`checked.symbols.display_path(.., "::")`, 6207-6214, 6265-6270) and a specialization commitment (6362-6366).
- Runtime-call match by `(caller_state_symbol, statement_index, call_ordinal)` (6564-6570).
- Outcome-specific arm call occurrence matched by `(state symbol, statement_index)` plus `source_site == NominalMachineUseSite::Expression(..)` (5333-5341).
- Float projection source: `direct_source` supplied by caller must match `format` else `InvalidSourceCarrier` (7177-7185); when `None` falls back to `TransitionalInput` (7186, 7200, 7214, 7228, 7242, 7256).

**Cases by shape.**
- Outcome-specific selection supported only when the callee has only Owned/Unrestricted, unqualified, non-borrowed structural parameters, no entry claims, no content entry claims, no requires/ensures/crash routes, and no evidence lanes (5384-5412); error text: "outcome-specific selection from a callee with structural inputs or other contracts is unimplemented" (5410).
- Outcome-specific argument roots: only `CalleeResult` (once), `CalleeParameter*`, and `ClosedValue` with equal declared/instantiated args; else "outcome-specific substitution of a computed proposition argument is unimplemented" (5506-5533).
- `outcome_specific_ensure_rows` requires the guarded producer's FIRST state (`machine_states(machine).first()`, 5810-5813) to return a nominal sum whose terminal result is `StructuralTypeShape::Sum` (5851); mixed shapes refused ("guarded payloadless terminal result is not a sum").
- Static requirement dispatch bounded to runtime result Unit, exact i32 or exact bool: "outside the bounded runtime Unit, exact i32, or exact bool call" (6204); NoNormal sites return early (6590-6601).
- `lower_proof_output_runtime_call`: scalar result only (`primitive_type_reference`, 6602-6611) or none.
- `resolve_direct_float_source_binding`: `DirectOperationResult` returns `Ok(None)` (7417); only `DirectCallResult` resolves via call operations; `resolve_float_operation_result_source` with `call == false` accepts only `NearestIeeeFloatFusedMultiplyAdd` (7640-7645) — but its only visible caller passes `true` (7414), so the `false` arm is unreached from this file (grep below).
- `integer_predicates::lower`: five guarded arms by `CheckedIntegerBinaryKind` lists (7821-7946); every other expression delegates to `leaf` (8029).
- `checked_requirement_schema` kind: Nongeneric / Finite / Open (6854-6865); `checked_static_conformance_rows` returns an EMPTY row list for non-finite generic requirements (6954-6961).
- parameter-kind codes hard-coded 1..6 (6877-6884).

**Same job elsewhere.**
- Three "proposition id by declaration" lookups inside this file: 4751 (by symbol), 5005 (by symbol), 6494 (by name).
- Union-find forwarded-term walk at 4846-4874 repeats the forwarded-source determination of `proof_output_forwarded_argument` (6417) (read; it calls `proof_output_forwarded_source` at 4862).
- Outcome-specific position/index bookkeeping (`next_positions`, 5855-5892) duplicates the lane-key position scheme in 4876-4959 (inferred).
- Float meaning: six near-identical `match direct_source { Some(.. if format) => .., Some(_) => Err, None => transitional_input }` arms (7175-7258) (read).
- `selected_conformance_applications` + `checked_application_telescope` also used by retention.rs (see retention section).

**Unused / one-caller (grepped across omega-rust).**
- `lower_float_meaning_equality`: 1 external caller (`machine_lowering.rs:1152`) plus its import (1089).
- `lower_boundary_crash_routes`: 1 call (`attached_unit.rs:327`); `lower_formal_crash_routes`: 2 calls (`retention.rs:2955`, `retention.rs:3069`); `checked_scalar_term_with_views`: 1 call (`scalar_lowering.rs:2465`); `lower_structural_result_indexed_term`: 1 call (`scalar_lowering.rs:2418`); `lowered_erased_scalar_terms`: 8 calls in `unit_arguments.rs` (3749, 3797, 3959, 3964, 4055, 4065, 4164, 4209).
- Grep command: Grep pattern `checked_scalar_term_with_views|lowered_erased_scalar_terms|lower_structural_result_indexed_term|lower_float_meaning_equality|lower_formal_crash_routes|lower_boundary_crash_routes` over omega-rust (content mode, all hits listed). A second count-mode grep of the content-conservation names gave 53 total occurrences across 7 files (proofs.rs 16, attached_unit.rs 14, unit_arguments.rs 9, unit.rs 4, retention.rs 4, scalar_lowering.rs 3, unit_composed_control.rs 3) — not broken down per function.
- Not grepped (suspected single-use): `resolve_float_operation_result_source` `call=false` path; `lower_sequence_extent_projection_term` (re-exported at 4361, call sites not counted).

**Stated limits.** "outcome-specific selection from a callee with structural inputs or other contracts is unimplemented" (5410); "outcome-specific evidence interface reference scopes are unimplemented" (5543); "outcome-specific proposition dependencies beyond the call result are unimplemented" (5549); "outcome-specific substitution outside the proposition's arguments is unimplemented" (5490); "unnamed guarded guarantee is outside the bounded truth proposition" (5970); "structural crash Exact shift requires explicit terminal count and overflow safety evidence" (7898).

**Fail-open risks.**
- `lower_proposition_vocabulary` `continue`s when an application's declaration symbol is not in `declaration_ids` (4751-4756) and also drops an application whose evidence projection names a term not in `term_ids` (4763-4766, `belongs_to_emitted_bodies = false`). Wrong program that gets through: an application referencing an undeclared proposition is silently omitted from `proposition_applications` instead of erroring. Later lookups (`terminal_proposition_application_id`, 6543) fail closed if a retained term needs it, so the effect is bounded to unreferenced applications — traced for the lookups at 5050 and 6543 only; open for other consumers.
- `lower_evidence_terms` skips terms with no `term_ids` entry (4999-5001): intended ("belongs to emitted bodies"), by design of `lower_evidence_term_ids` which only assigns ids to roots reached from emitted machines (4877-4890). Traced.
- `lower_evidence_term_ids` `continue`s forwardings/terms whose machine is not in `source_machine_ids` (4847, 4882-4884, 4946-4948): such terms get `None` ids. Traced (consumers use `terminal_evidence_term_id` which errors on `None`, 6461-6471).
- `lower_float_meaning_projection` `None => transitional_input(...)` (7186-7256): when no `direct_source` is resolved the projection silently uses a `TransitionalInput` rather than the direct carrier. Name "transitional" indicates migration debt; the checker's `fallback` data is trusted. Wrong program: one whose direct source could not be resolved still lowers (to an unconstrained input). Status: open (I did not read callers to see when `direct_source` is `None`).
- `resolve_direct_float_source_binding` returns `Ok(None)` for unmapped owner machines (7319-7324), no occurrence (7400-7402), `DirectOperationResult` (7417) and `_` (7449): all fall back to the transitional input above. Status: open.
- `resolve_direct_float_parameter` returns `Ok(None)` when parameter scalar types differ between source and terminal machine (7479-7487). Status: open.
- `rejoin_float_semantic_applications` upgrades `TransitionalInput` rows to `SemanticApplication` (7699-7701) — transitional is accepted as the thing to be replaced; a row left `TransitionalInput` when no application names it is not rejected here (read; no check after the loop).
- `checked_static_conformance_rows` returns empty rows for open generic requirements (6954-6961): then `expected_rows` is empty and `application.rows.len() == expected_rows.len()` (6330) must hold — a lowered application with zero rows for such requirement passes. Stated by nothing; status: open.
- `lower_outcome_specific_call_evidence` `filter` selects arms with a selected term only (5318-5321); arms without selected terms are skipped (not an error).
- `lower_proof_output_calls` is a no-op when no invocations for the machine (iterator filter 6027-6035).

**Same-job note: static requirement verification.** `lower_static_requirement_dispatch` re-verifies, inside lowering, that the lowered module contains the application it expects (6345, 6380, 6391) — a verifier-style check in the lowering stage (read).

(proofs.rs section complete.)

## retention.rs

Header comment (retention.rs:1-5): "Checked-fact custody installed on the assembled `TerminalModule`: access views and lifecycles, closed reach and conformance applications, external provider candidates, operation crash contracts, payload memberships, restored reborrow call uses, retained borrow custody, and suspension call plans."

### retention.rs 1-1100 (read in full)

**Purpose.** Copies checked facts about placement/access (lifecycle plans, view inputs, view plans) and closed reach applications onto the already-built `TerminalModule`/`LoweredPsi`, resolving source symbols to terminal ids by identity text.

**Shape.**
- `mod access_lifecycles` (21-545): `retain` (33) writes `lowered.semantic_module.access_lifecycle_plans` (186); helpers `image_fields` (190), `execution_contract` (233), `profile_requirement` (295), `storage_contract` (399), `pub(crate) storage_fields` (495).
- `mod access_view_inputs` (546-673): `lower_access_view_input` (556) -> `TerminalAccessViewInput`.
- `mod access_view_plans` (674-854): `retain` (683) writes `module.access_view_plans` (828); `schema_type` (832) maps a data definition to a `StructuralTypeId`.
- `mod closed_reach_applications` (855-...): `ReachApplicationView` (876), `capture_view` (902), `application_views` (978), `closed_evidence_bindings` (1050), `parameter_kind` (1085).

**How it decides.**
- Plans: only `AccessLifecyclePlan::Evaluated` rows (42-56); a plan is emitted only per boundary found by scanning all `source_call_occurrences` whose `source_target == callable` (or the private declaration's state) and whose operation is `BoundaryCall` (68-87); if none, `continue` (90-92).
- `schema_type` (832-853) and `storage_contract::carrier` (406-420) find terminal structural types by comparing `module.identities.text(declaration.identity) == Some(identity_text)` — keyed on type-identity TEXT.
- `image_fields` (190-231) matches source fields to terminal fields by `field.name.as_str() == field_name` (211) then by interned member spelling (215-224).
- `profile_requirement` (295-397) selects a core `ResourceProfileSupply` call by: source file `origin == Toolchain`, `package_identity.is_none()`, path == `layout.omg`, and `source.source.as_bytes() == LAYOUT_SOURCE` (an `include_bytes!` of `source/library/core/layout.omg`, 302-321); name compare `normalized_hermetic_symbol_identity == "toolchain::ResourceProfileSupply"` (339) and `symbols.name(template).rsplit("::").next() == Some("profile")` (340). Source-spelling keyed (read).
- `storage_fields` (495-544): requires `symbols.name(extent) == "Extent"` (509) and picks fields by names "base"/"length" (543).
- `access_view_plans::retain` accessor kind by string: `"read"`, `"take"`, `"write"`; other => unsupported (760-769).
- `closed_evidence_bindings` builds identity strings via `format!("closed-reach-source-conformance-v1({}:{}|{})", ..)` (1074-1079).

**Cases by shape.**
- `execution_contract` profile_positions fixed per operation: Initialize `[2]`, MoveOut/View `[1]`, Relocate `[2, 3]` (271-275).
- `profile_requirement` requires exactly one supply role: `let [supply_role] = application.trait_arguments` (348).
- `parameter_kind` mapping (1085-1096): Type 0, Const/Value 1, Proposition 2, Machine(other) 3, Machine(RequirementIdentity) 4, Implementation 5.
- `capture_view` refuses RuntimeValue ("closed capture needs a static value operand", 948) and Conformance args ("closed capture has no conformance-parameter route", 951).

**Same job elsewhere.**
- Type-identity-text-to-terminal-structural-type lookups at 406-420 and 832-853 (both in this file, read); a third, `StructuralTypeDeclaration` lookup by text at 411-416. Another duplicate of the `proofs.rs:97-109` content_field_identity-style linear scans (inferred).
- `identity` closure `normalized_hermetic_symbol_identity(symbol).map(intern)` repeated at 100-108, 563-568, 802-821 (read).
- Parameter kind code mapping duplicates proofs.rs:6877-6884 with DIFFERENT numeric codes (retention 1085-1096: Type 0, Const 1, Prop 2, Machine 3/4, Impl 5; proofs 6877-6884: Type 1, Const 2, Value 3, Machine 4, Implementation 5, Proposition 6). Read both.

**Stated limits / unsupported messages.** "placement initialization requires its core profile supply call" (395); "placement boundary has competing lifecycle plans" (184); "closed capture has no conformance-parameter route" (951).

**Fail-open risks.**
- `access_lifecycles::retain` `continue`s for a plan with no emitted boundary call (90-92): a lifecycle plan with no emitted call site is silently dropped. Wrong program: none that I can identify — absence of a call means nothing to attach to. Status: traced-by-inspection only.
- `access_view_plans::retain` `continue`s when `declared_type(data)` is `None` (698-700): a view plan whose carrier type was not emitted is dropped, but the schema lookup after it (707) errors. Status: open.
- Accessor targets with no matching boundary machine are silently skipped (757-775, no else branch): an accessor target that matches no `boundary_machines` row yields an `operations` list missing that accessor. Status: open.
- `profile_requirement` `continue`s past occurrences whose role identity differs (351-356) — fine; error if none selected (394).

### retention.rs 1101-3600 (read in full)

**Purpose.** (a) closed reach applications: attaches to each emitted machine a `ClosedReachApplication` (template/specialization commitments, telescope of type/const/machine parameters, fixed services, dependencies, call sites) and marks consumer operations with `static_reach_binding`; (b) closed conformance applications: builds `ClosedConformanceApplication` rows with realization callables, schemas and static applications, then stamps `report_fingerprint` and `commitment`; (c) external provider candidates; (d) per-operation crash contracts for operator uses; (e) start of payload memberships.

**Shape.**
- `closed_reach_applications`: `requirement_arguments` (1098), `retain_closed_reach_applications` (1165-1571, one ~400-line function), `resolved_source_entry` (1573), `closed_call_application` (1584), `selected_static_machine_identity` (1666, pub(crate)), `exact_machine` (1689), `declaration_identity` (1701), `retain_services` (1712).
- `conformance_applications` (1768-2589): `lower_closed_conformance_applications` (1782), `collect_closed_conformance_applications` (1857-2298), `sort_applications` (2300), `checked_application_telescope` (2322, pub(crate)), `checked_method_arguments` (2435, pub(crate)), `checked_static_dispatches` (2496), `checked_method_return_reference` (2538).
- `external_providers` (2590-2789): `retain` (2602), `project` (2627) with caller-supplied `include` and `retain_declaration` closures.
- `operation_crash_contracts` (2790-3240): `retain_operation_crash_contracts` (2808), `validate_every_crash_qualified_use_has_a_site` (2841), `lower_site` (2901), `validate_named_site_carrier` (2989), `emitted_operation` (3082), `reindex_formal_telescope` (3116), `published_buckets` (3150), `validate_operator_signature` (3169), `positional_scalar_operands` (3210), `scalar_value_type` (3226).
- `payload_memberships` (3241-...): `membership_reachability::MembershipReachability` (3250; `new` 3254, `may_contain` 3333; one unit test 3342-3405), `operand_memberships::retained` (3411).

**How it decides (keys).**
- Reach call-site match: `occurrence.matches_statement(state.symbol, call.statement_index, call.call_ordinal)` plus `resolved_source_entry(checked, occurrence.source_target) == Some(call.target_state_symbol)` (1381-1390); then prefers matches that are module invocations of the owner (1392-1407).
- Reach binder names are strings: `format!("reach-slot({position})")` (1125) and `"closed-reach-source-conformance-v1({}:{}|{})"` (1074).
- `declaration_identity` builds `"package:{digest}::{path}"` or `"unmanaged::{path}"` (1701-1709) — path text of the symbol.
- `selected_static_machine_identity` formats `"{decl}|{overload}|selected={decl}"` (1681-1686).
- `retain_services` dedups by interned identity text of the service symbol and recurses into parents (1728-1742); service ids assigned `max+1` (1743-1753).
- Conformance: selected calls filtered by `application_report_fingerprint`/`application_commitment` equality (1917-1920) and field-wise equality of the source application (1935-1943).
- External providers: only machines whose conformance has an `external_binding` (2646-2651); Trait satisfaction retained only if `definition.is_boundary` (2656); `include` closure for `retain` is `boundary_machines` identity text == callable OR `boundary_application_demands` overload == callable (2612-2621) — string compare on callable identity text.
- Operator crash site joined to its emitted Terminal operation by `operator_use` handle (3094-3097) for selected-comparison operators, by `(source_site, state, source_target)` for named uses (3006-3013).

**Cases by shape.**
- `closed_reach` telescope: parameters limited to Type/Const/Value/Machine; `Proposition` => "closed reach executable proposition argument is unavailable" (1350-1354), `Implementation` => "retains a trait-only implementation binder" (1355-1359).
- `closed_call_application` returns `Ok(None)` for a `Proposition` parameter (1648) while the other function (1350) errors — same kind, different treatment (read).
- `checked_method_arguments`: `_ => Err("static method tuple contains an unsupported proposition or implementation argument")` (2480).
- `checked_application_telescope` (2322-2433): lifetime/Type/Const/Machine only; open const binder (`CallerExpression`/`CallerBinder`) refused (2366-2373); Proposition and Implementation refused (2405-2414).
- `operation_crash_contracts::positional_scalar_operands`: refuses call operations and operand-less operations (3211-3222). `validate_operator_signature`: all-scalar operator signature only (3186-3188).
- `lower_site`: only operators that are NOT named uses emit a contract; named uses are validated and return `None` (2908-2917).
- `operand_memberships::retained` handles exactly MoveStructuralField, EstablishReference, EstablishStructuralArray, EstablishRecord, EstablishStructuralCase; `_ => return None` (3556).

**Same job elsewhere.**
- `closed_call_application` (1584-1664), `checked_method_arguments` (2435-2489) and the telescope loop in `retain_closed_reach_applications` (1220-1361) each walk (types, constants, machines) iterators over a specialization in parallel — three copies of the same "zip arguments by parameter kind" loop in this file (read).
- `exact_machine` (1689) vs `lower_evidence_term_ids::body_position` (proofs.rs:4832-4836) vs `access view` lookups: linear scans of `(SymbolHandle, MachineId)` slices; same job (symbol to terminal machine id) done by at least these two helpers (read).
- `declaration_identity` (1701) vs `checked.symbols.display_path(.., "::")` used directly in proofs.rs:6207 and retention.rs:2272: two different spellings of a declaration's identity (package-qualified vs plain display path) (read).
- `checked_requirement_schema` / `checked_static_conformance_rows` are in proofs.rs (evidence_lowering) and consumed at retention.rs:2131 and 2188 (read).
- `lower_formal_crash_routes` is the shared formal-route lowerer (proofs.rs:4412) used at retention.rs:2955 and 3069 (grep earlier: 2 calls).

**Unused / one-caller / "single way" claims.**
- No comment in this range claims a canonical/single way. `checked_method_arguments`: called from proofs.rs:6368 and in retention (2090) — grep not run for exact count (suspected 2).
- `external_providers::project` takes an `include` and `retain_declaration` closure; `retain` passes a no-op `|_, _| {}` (2622). Other caller(s) of `project` not grepped (suspected: build/package side; unread).

**Stated limits.** "closed reach executable proposition argument is unavailable" (1352); "static method tuple contains an unsupported proposition or implementation argument" (2480); "operator crash contract requires an all-scalar operator signature" (3187); "operation crash contracts were already installed on this module" (2835). Test name `domain_reachability_follows_children_and_conservatively_keeps_unknowns` (3362).

**Fail-open risks.**
- `MembershipReachability::may_contain` (3333-3339): an id absent from `rows` returns `true` (keep); duplicate ids return `true`. Status: stated (test at 3362-3404 asserts `nested.may_contain(id(99))` is `true`, name says "conservatively keeps unknowns"). Wrong program: none; it keeps extra memberships rather than dropping.
- `MembershipReachability::new` marks `Atomic`, `BoundedInteger`, `RefinedScalar`, `ByteSequence`, `PredicateByteSequence` fields as always reachable (3295-3300) and Scalar/IeeeFloat fields reachable iff some carrier is `PrimitiveScalar` (3292-3293). Status: conservative (keeps), traced in this function.
- `retain_closed_reach_applications` `continue` when `exact_machine` returns `None` (1183-1185) and for calls with no matching occurrence (1408-1410): a closed-reach call that has no emitted occurrence gets no `ClosedReachCall` entry. Which wrong program passes: one where a static-machine call exists in checked facts but was never emitted — only fine if the emitter legitimately elided it (e.g. retired). Not traced; status: open.
- `lower_closed_conformance_applications` clears `module.closed_conformance_applications` and returns Ok if no selected applications (1795-1798) — a stale-table reset, by design (read).
- `collect_closed_conformance_applications` skips applications whose `source_owner` isn't in `owners` (1898-1901) and drops static calls whose source application mismatches (1935-1945, `continue`). Status: open.
- `checked_static_dispatches` skips `is_retired` calls (2517-2519). Status: open (not traced what retires a call).
- `external_providers::project` `continue`s when `resolve_satisfied_declaration` is `None` (2652-2655) or when `include` is false (2697-2699) — silently omits provider candidates; status: open.
- `lower_site` returns `None` for named-use sites after validation (2908-2917): fail-closed by `validate_named_site_carrier`.
- `operand_memberships::retained` returns `None` for any unlisted operation kind (3556) and for a failing helper (`?`): a membership contract is simply absent. Wrong program: one whose payload membership is needed on an unlisted operation kind loses its qualification silently. Status: open (consumer not traced).
- `retain_closed_reach_applications` `reach`/`schema` branch: `schema` is `None` if no specialization with source machine found (1274-1301) — optional by design.

**Duplicated-state note.** `collect_closed_conformance_applications` keeps two local `HashMap`s (`proof_dispatches_by_owner` 1877, `cached_realized_calls` 1889) and `validate_every_crash_qualified_use_has_a_site` three `HashSet`s (2855-2866). AGENTS.md says dense IDs use flat tables and ad hoc mutable caches are listed in `tools/cache_state.txt` (not checked here).

### retention.rs 3601-4850 (read in full)

**Purpose / shape.**
- `payload_memberships` (3241-3965): `retain(module)` (3741) runs a fixed-point loop per machine (3786-3953) that collects `PayloadMembershipContract`s from four sources (reference result memberships 3814-3829, `view_memberships::retained` 3623, `operand_memberships::retained` 3411, move/leaf-copy selection 3851-3893, descriptor contents query 3894-3931) and `merge`s them (3958) into each operation's structural result `qualifications` / `projected_qualifications` until no change. Early exits: no `structural_domains` (3742-3744), no candidate machine (3768-3770).
- `reborrow_restored_call_use` (3966-4557): `places::*` helpers produce identities (`identity` 3997, `storage_root_identity` 4020, `boundary` 4040, `owner_path` 4063, `Capture::selector_identity` 4095, `place_segments` 4121, `captured_root_identity` 4162, `place` 4207, `projected_place` 4219, `access` 4245); `retain_selected_reborrow_restored_call_uses` (4256-4515) emits `TerminalReborrowRestoredCallUse` rows; `restored_kind` (4517), `child_parent_weakening_handle` (4534).
- `retained_borrow_custody` (4558-): `lower_projection` (4568), `lower_custody` (4597-4746), `retain_foreign_borrow_custodies` (4748-4850) which either appends a `RetainedBorrow` guarantee to an existing boundary machine whose identity equals `callable_identity` (4795-4810) or creates a new `BoundaryMachineDeclaration` (4819-4847).

**How it decides.**
- Reborrow identities fall back to a digest of display path + arena index + generation when no hermetic identity exists (4001-4011); computed storage roots are digests of the expression arena handle (4026-4034) — identities keyed on arena index/generation values, not on source.
- Restored call use: matches the source call occurrence by `(state, statement_index, call_ordinal)` AND `source_target == call.target_symbol` (4386-4395) and requires exactly one (4396). Terminal call must be `OperationKind::CallUnit` with exactly one structural argument, callee with exactly one structural parameter (4423-4447).
- `retained_borrow_custody::lower_custody` replays fact fields against the signature and refuses on any disagreement (4618-4707): source must be a direct explicit-lifetime reference parameter at Entry with no segments (4626-4660); result must be `TypeReferenceNode::Generic` of `fact.result_data` with exactly one lifetime argument, no runtime generic arguments, Linear multiplicity (4674-4702).

**Cases by shape.**
- `restored_kind`: shared cohort size must be `1..=3` — "restored shared-freeze reborrow cohort is outside the retained bound" (4523-4526); dispositions other than Reactivate/RestoreSharedCohort refused (4530).
- Restored reborrow: parent must be `CheckedParentBorrowResource::DirectRoot` (4275-4278, 4300-4305); call must be a Unit call with exactly one structural argument (4423-4435).
- Retained borrow custody: only Shared/Mutable access (4644-4648, WriteOnly refused 4731); source place is a bare parameter (4643), result place is bare `Result` (4667); result lifetime ordinal must be 0 (4689). `result_lifetime_argument_count: 1` and `result_lifetime_slot_is_erased: true` are hard-coded (4739-4741).
- `view_memberships::retained` accepts only `EstablishElementView`, `EstablishMutableElementView`, `ElementViewSubslice` (3644-3649).
- `payload_memberships::retain` machine/ordinal as `u32`, errors on overflow (3808-3812).

**Same job elsewhere.**
- Identity digests with domain strings `omega-terminal-reborrow-symbol-v1`, `omega-terminal-computed-backing-v1`, `omega-terminal-captured-borrow-selector-v1`, `omega-terminal-incoming-borrow-root-v1` (4007, 4028, 4105, 4171): four digest-based identity constructors in one file (read).
- `places::access` (4245) maps `BorrowAccessKind` to `StructuralAccess`; the retained-borrow code at 4727-4732 and access_view_inputs at 569-573 do the same mapping for `ReferenceAccess` (read; three small copies).
- Boundary-id allocation `max+1` at 4784-4793 repeats the service-id allocation at 1743-1753 (read).

**Stated limits.** "restored shared-freeze reborrow cohort is outside the retained bound" (4524); "retained-borrow result is not the bounded nominal lifetime carrier" (4682); "restored reborrow call use does not name a Unit call" (4429).

**Fail-open risks.**
- `payload_memberships::retain` early-returns `Ok(())` when `module.structural_domains.is_empty()` (3742) and for machines with no candidate (3783-3785); `operand_memberships::retained` returns `None` for unlisted kinds (3556). Memberships are an enrichment fixed-point; absent membership means the result carries fewer qualifications (that is conservative for a requirement check, not an accept) — inferred, consumers not traced. Status: open.
- `retain` ignores `Err` from `reference_memberships.for_operation` (3814 `if let Ok`), from `descriptors::reference_backing_memberships` (3474 `if let Ok`) and `history.current_selected_payload_contract_in` (3490 `let Ok`): errors there silently drop that source of membership. Status: open.
- `retain_selected_reborrow_restored_call_uses` skips events from other machines or without `restored_call` (4268-4270) by design.
- `places::identity` (3997-4012) never fails for a valid symbol: if no hermetic identity exists it hashes display path + arena index + generation. Wrong program: two distinct symbols with equal display path still differ by index; no accept issue. Status: traced locally.

### retention.rs 4851-6094 (read in full)

**Purpose.** `mod suspension_call_plan` (4852-6090): for every checked suspension crossing (a call that may suspend the activation), builds a `TerminalSuspensionCallPlan` listing the live values saved across the call and a loan graph (`TerminalSuspensionLoans`) recording what each retained borrow is captured from, backed by and supplied by. Stamps `operation.suspension_crossing` on the Terminal operation and sets `module.suspension_call_plan_count`, `suspension_call_sites` (with `frontier_commitment`) and `suspension_call_plans` (5080-5095). Last line re-exports `checked_application_telescope` and `checked_method_arguments` (6092-6094).

**Shape.** `retain_suspension_call_plans` (4863-5097), `retain_loan_graph` (5099-5653, ~550 lines, one `match row.source` with five arms: `Incoming` 5153, `LifetimeDependency` 5180, `OutputInput` 5207, `Stored` 5291, `CapturedActual` 5417), `incoming_reference_binding` (5655), `source_binding` (5742), `call_argument_binding` (5769), `scalar_argument_binding` (5898), `fused_component_transfer` (5911), `owning_callable_symbol` (5941), `terminal_call_target` (5955), `scalar_declaration` (5977), `scalar_value_available_before` (5993), `captured_call_leaf` (6032).

**How it decides.**
- Which crossings are processed: those whose `machine` is in `source_machines` OR whose `state` appears in some `state_blocks` row (4881-4886).
- Occurrence join: `matches_statement(crossing.state, statement_index, call_ordinal)` (4888-4894); duplicate or `owning_callable_symbol(source_target) != owning_callable_symbol(crossing.target)` => "suspension crossing call join is duplicate or redirected" (4910-4914).
- Missing occurrence: `continue` only if `fused_component_transfer` holds (4896-4898), else error "possibly-suspending checked call has no exact Terminal operation join" (4899).
- Live-value place by origin: Parameter/Local use `source_binding(&occurrence.frontier_before_call, position)` keyed on `source_position` (5742-5767); CallArgument by lane position counting same-kind (scalar vs structural) non-erased parameters before it (5788-5797).
- Scalar frontier availability: value declared before the operation in machine/block, or defined in a dominating block (5993-6030, uses `terminal_semantics::control_graph::dominators`).
- Loan capture selection in the `Stored` arm: `UnretainedDerived` lineage with suppliers or retained call inputs => `DescriptorTransfer` (5301-5305); else `SuppliedOutput` => `CompletedOutput`; else unique formation capture => `Reference`; else `Unresolved` (5353-5370). `Unresolved` is a representable result (5369, 5393, `TerminalSuspensionValueLoans::Unresolved` 5026, 5647).
- Loan resource: DirectRoot/Reborrow/SuppliedOutput by valid handle; fallback DescriptorTransfer or `Unresolved` (5371-5394).

**Cases by shape.**
- `call_argument_binding` handles exactly: direct calls (`direct_call_parts`), `BoundaryCall`, `CallDynamic`; anything else => "suspension call argument is not a call" (5798-5874). `terminal_call_target` handles the same three (5955-5975).
- For `CallDynamic`, scalar lane 0 is the selector (5847-5849), then lane shifted by 1 (5850-5856).
- Backing: Unresolved / StateValue / CompletedOutput / InputDependencies / IncomingReferent (5546-5595).
- `fused_component_transfer` (5911-5939): a missing join is tolerated only when the composed-machine plan has a successor `(statement_ordinal == crossing.statement_index, target_state == call target entry)` and every holder block of the crossing state has a block for the target in the same machine.

**Same job elsewhere.**
- `Reborrow`/`DirectRoot` parent classification in `retain_loan_graph` (5371-5394 and 5503-5541) and in `reborrow_restored_call_use` (4272-4340) both classify borrow resources by checked handle (read).
- `owning_callable_symbol` (5941) vs `resolved_source_entry` (1573) vs `exact_machine` (1689): three symbol-to-owning-machine lookups by linear scan of `checked.machines()` (read).
- `BorrowAccessKind` to `StructuralAccess` mapping at 5619-5623 and 5720-5728 duplicates `places::access` (4245-4251) (read; three copies in the file, plus the `ReferenceAccess` ones at 569-573 and 4727-4732).
- Operation lookup by scanning all machines/blocks/operations for an `OperationId` is repeated at 4916-4939, 6071-6079 and `scalar_declaration` 5977 (read); also in proofs.rs:5357-5364, 5628-5637 (read).

**Unused / one-caller (grepped).**
- `checked_method_arguments`: call sites outside its definition: retention.rs:2090, proofs.rs:6368, attached_unit.rs:409 (3), plus re-export at retention.rs:6093 and imports at proofs.rs:6013, attached_unit.rs:134. (Grep: `checked_method_arguments|external_providers::project|access_lifecycles::storage_fields|retention::access_lifecycles`, content mode.)
- `external_providers::project`: 1 caller outside the file (`projections.rs:772`) plus the in-file `retain` (2608) = 2.
- `access_lifecycles::storage_fields`: 1 caller (`attached_unit.rs:15425`).
- `access_lifecycles::retain` imported at `machine_lowering.rs:8` (not counted further).
- `checked_application_telescope`: re-exported 6092-6094; callers inside this file 1914, 1942, 2268 and `proofs.rs:6241`; not counted elsewhere.

**Stated limits.** "suspension call plans must be produced exactly once" (4872); "suspension loan has multiple formation captures" (5355). `TerminalSuspensionLoanCapture::Unresolved`/`TerminalSuspensionValueLoans::Unresolved`/`TerminalSuspensionLoanBacking::Unresolved`/`TerminalSuspensionLoanResource::Unresolved` are accepted outputs (5369, 5393, 5547, 5648) — the Terminal vocabulary has an explicit unresolved state.

**Fail-open risks.**
- `retain_loan_graph` emits `Unresolved` capture/resource/backing/value-loans rather than failing (5369, 5393, 5547, 5647): a loan the checked facts cannot classify still produces a plan. Which wrong program passes: a suspended activation with a borrow whose origin the lowerer cannot resolve gets a plan row marked unresolved. Whether a later validator rejects `Unresolved` was not traced. Status: open.
- `retain_suspension_call_plans` skips a crossing with no emitted occurrence when `fused_component_transfer` is true (4896-4898). Status: traced above (condition requires a planned successor and holder blocks), consumer not traced.
- Crossings filtered out by the `source_machines.contains || state_blocks` test (4881-4886) are silently not planned. Status: open.
- `retain_loan_graph` filters `rows` by `retained_for_crossing` (5132); loans not retained are absent and `dependency()` of a missing loan returns `Handle::invalid()` (5141-5149), which most callers check (5194, 5264, 5402, 5506) but not all: in the `CapturedActual` / `CallAccess` arm the parents list is checked with `any(!valid)` (5524) then `first().map_or(invalid)` (5527). Traced: all five arms check validity before use.

**retention.rs coverage note.** Whole file read: 1-1100, 1101-2400, 2401-3600, 3601-4850, 4851-6094.

**retention.rs: five facts for a maintainer** are listed in the final summary.

(retention.rs section complete.)

## scalar_computations.rs

Header comment (scalar_computations.rs:1-4): "Expansion of checked scalar computations (fields, cases, calls, arrays, subslices, byte comparisons, and normal-result completions) into typed lowered blocks." Line 6: `pub(crate) use crate::scalar_lowering::*;` (glob re-export of the whole `scalar_lowering` module into this one).

### scalar_computations.rs 1-3600 (read in full)

**Purpose.** Turns each checked scalar computation node (a DAG of values, calls, case constructions/observations, dispatches, field reads, byte comparisons) into a chain of `LoweredScalarBranchState`s (a small CPS-style state graph). The unit is `Expansion` (struct defined later in the file; methods are added by `impl Expansion<'_>` blocks inside each sub-module). Each sub-module owns one kind of node.

**Shape (sub-modules of `mod computations`, 8).**
- `arrays::declarations` (31-45): selects `EstablishScalarArray` operation results.
- `byte_comparisons` (46-479): `Expansion::descriptor_bindings` (73), `byte_sequence_observation` (102-325; comment at 95: "The one byte-sequence operand observation"), `byte_literal_comparison` (328), `byte_predicate` (367), `OperandSelectors` (395-465), `observation_path` (467).
- `call_signature::structural_parameters` (487-519): finds the structural parameter plan for a call target from boundary machines, dynamic slots, or a scalar callee.
- `calls` (521-1510): `source::call_source` (558-637, two origins: `Statement(handle)` and `Declaration{application,call}`), `enum Operand` (641-658, seven variants), `consume_pure_call_argument` (660), `consume_pure_call_arguments` (712), `Expansion::call` (782-1508, ~730 lines; builds a `LoweredScalarBinding` of kind `DynamicCall` / `DescriptorTable` / `BoundaryCall` / `DirectCall` by callee: 1217, 1306, 1332, 1350).
- `cases` (1511-2307): `observed_values::lower` (1550), `unique_root` (1677), `Slot` (1708), `ReservedField` (1719), `ConstructedField` (1736), `Construction` (1743), `prepare` (1752), `reserve` (1782), `type_roots` (1921), `constructions` (1959), `Expansion::case_membership` (1990), `emit` (2205), `declarations` (2270), `structural_case_identity` (2285).
- `comparisons::comparison_binding` (2315).
- `dispatch` (2334-2861): `dispatch_route` (2351), `structural_dispatch` (2375-2611), `dispatch` (2616-2859).
- `fields` (2862-...): `structural_field` (2890), `Binding` (3039), `CapturedPlaceSubject` (3063), `planned_place_loans` (3078), `loan_subject` (3096), `prepare_place_observations` (3113), `captured_place_subject` (3171-3433), `LoanedStore` (3435), `loaned_store` (3443), `loaned_primitive_store` (3502), `captured_store_path` (3534), `checked_path_segment` (3562), `prepare` (3580).

**Inputs/outputs.** Reads `checked.facts.values.scalar_computations` (nodes, operands, structural_arguments, dispatch_arms, claim_transfers), `checked.facts.flow.control` (calls), `flow_facts(checked).terminal_unit_effects` (boundary_machines, dynamic_slots, place_loans), `checked.facts.termination.rank_arrivals`. Mutates the `Expansion` (`self.push(..)` appends branch states, `self.calls`, `self.established_values`, `self.completed`, `self.next_place`). Output: an entry state index per node.

**How it decides (keys).**
- Call occurrence uniqueness keyed on `SourceInvocation` coordinate: `if self.calls.contains(&coordinate) { unsupported("scalar computation repeats a call occurrence") }` (1370-1373).
- Statement-origin calls validated by `(state membership, !is_retired, statement_index, call_ordinal, target_symbol == callee.called_signature(), authored_expression == expression)` (580-590).
- Callee kind chosen by enum variant (`Dynamic`, `DescriptorTable`, `signature.boundary`, else direct) (1217-1369).
- Formal lane classification: erased => none; position 0 with selector => none; structural if `parameters[structural_ordinal].position == position`; else scalar (874-888).
- Case observation by symbol: `slot.cases.iter().find(|(symbol, _)| *symbol == observed_case)` (2112-2119); case identity lookup scans every data definition (`structural_case_identity` 2285-2306) — linear scan keyed on `variant.symbol`.
- Case/field matching to terminal declarations by interned spelling: `identities.intern(&identity.spelling())` (1825, 1857, 3240) — source-spelling keyed.
- Byte-overlay detection: a `u8` unsigned element (`integer.sign()==Unsigned && bits()==8`, 3363-3367) plus a terminal Record whose `identity == intern(normalized_type_identity_text(referee))` (3368-3371).
- Scalar dispatch coverage: needs a `Wildcard` arm, or both Boolean constants `true` and `false` as `Value` patterns (2677-2727).
- Structural dispatch arm patterns: `Wildcard` or `Predicate`; a `Value` pattern is refused (2539-2545).

**Cases by shape.**
- `cases::reserve`: "computed case requires a plain sum" (1804); every payload field must be scalar: "computed case field is not scalar" (1864) and relevant (1869); retained field count must equal case/authored field counts (1840).
- `case_membership` for computed (non-place) subject: requires `plain_owned_with_numeric_constraints` AND multiplicity `Affine | Unrestricted`: "computed case observation requires its nontrivial cleanup plan" (2093-2103).
- Place observation of case with a `RuntimeIndex` in the path goes through `element_case_observation` (2035-2059).
- `byte_comparisons::observation_path`: only Referent/Field/Case/FixedIndex/RuntimeIndex (467-478).
- `captured_place_subject`: path segments Field, FixedIndex, RuntimeIndex only; `_ => "record loan's captured place selects a case, a byte window or a stored reference"` (3351-3355); at most one runtime index (3324-3326); returns `Ok(None)` when the referee is neither primitive nor a `Named` type (3179-3186).
- `loaned_store`: root must be `CheckedStoreRoot::Local` else `Ok(None)` (3454-3456); must be MutableBorrow (3461).
- `Expansion::call`: operand kinds Structural/Range/ByteLiteral/Scalar/Snapshot/DispatchSelector/Produced (641-658).

**Same job elsewhere.**
- `fields::structural_field` (2890) and `cases::observed_values::lower` (1550) both construct an `ObserveStructuralValue` state with `complete_observation_receiver` and `ValueRequest::prepare(...).finish(..)`; the two bodies are near-copies (read both).
- `dispatch::dispatch` (2616) and `structural_dispatch` (2375) share the same arm loop skeleton (normal-result arms vs plain arms, `dispatch_route`, `claim_continuation`, `argument_with_normal_result`) with different pattern handling (read both).
- `structural_case_identity` (2285) is another linear scan from symbol to `CheckedMemberIdentity`; compare `proofs.rs:5861-5873` (`case_identity` by `variant.symbol == guarantee.result_case`) and `proofs.rs:97-109` (`content_field_identity` by `field.symbol == symbol`) — three symbol-to-member-identity scans in the unit (read).
- `fields::checked_path_segment` (3562) converts terminal path segments back to checked ones through `identities.text(*field).unwrap_or_default()` and `CheckedMemberIdentity::of_spelling` — a reverse of the spelling intern; unknown identity text becomes the empty string (3570).

**Unused / one-caller.** Not individually grepped in this chunk (see summary list at the end).

**Stated limits (quoted).** "scalar dispatch has no independently covered terminal arm" (2725); "saved case observation has no completed receiver" (1589); "computed enum receiver has no checked normal disposition" (1627); "byte record store requires separate semantic field-range evidence" (3477); "record store requires an immutable captured selector" (3550).

**Fail-open risks.**
- `consume_pure_call_arguments` (712-778) returns `Ok(())` without checking anything when the root is not `CheckedScalarComputationKind::Call` with a `Body` callee (725-734), when the call has structural arguments (741-743), or when any operand is not a plain `Value` (755-758). Which wrong program: a call whose operand expression disagrees with its retained pure argument, but whose callee is not `Body` (dynamic, boundary) or which carries any structural argument, is not cross-checked here. Status: open (callers not traced).
- `fields::prepare_place_observations` (3113-3169) converts `Ok(None)`/`Err` to a stored `Err` in `PlaceObservation.capture` (3153-3159) rather than failing; the failure surfaces only when that observation is consumed. Status: traced locally (error preserved), consumer not traced.
- `loan_subject` returns `Ok(None)` when no planned place loan matches the symbol (3104-3110) and `loaned_store` then returns `Ok(None)` (3457-3459): the store falls back to the non-loan path. Status: open.
- `checked_path_segment` maps an unknown field identity to an empty spelling (3568-3572, `unwrap_or_default`). Status: open.
- `fields::captured_place_subject` `Ok(None)` for unsupported referee types (3179-3186) leads to the "place loan has no scalar or record observation" error stored at 3155; fail-closed deferred.
- `dispatch` falls through `Some(..)`/`None` for `normal_result` — both paths build full chains; no accept-on-missing seen.

### scalar_computations.rs 3601-7824 (read in full)

**Purpose / shape.** The rest of the `computations` module: field bindings (`fields::prepare` 3580-3956, `receiver_request` 3958, `record_view_source_walk` 3972, `field_path_identity` 4027, `type_roots` 4051, `emitted_observation` 4133, `copied_observation` 4169, `completed_observation` 4204, `field_binding` 4264, `observation_source` 4288, `resolved_observation_path` 4331, `atomic_observation` 4353, `observe` 4394-4592); `literals` (4594-4691); `observation_completion::complete_observation_receiver` (4697); `observations` (4719-5282: `Slot`, `prepare` 4748, `Expansion::storage_observation` 4830-5184, `completed_storage_observation` 5189); `operand_scope` (5283-5400); `operand_types::type_requirements` (5417-5489); `produced_values` (5491-5608); `returns` (5609-5926: `NormalCompletionTarget`, `NormalResultTarget`, `NormalResultScope`, `NormalResultPath`, `NormalDispatchScope`, `Expansion::normal_return` 5733, `normal_result_scope` 5872); `selection` (5928-6185: `claim_continuation` 5939, `selection` 5975, `condition_order` 6125); `structural_arguments::lower` (6195-6338, `borrowed_access` 6340); `subslices` (6357-6612: `Slot`, `prepare` 6383, `reserve` 6457, `Expansion::subslice` 6487); `value_types::computation_value_type` (6620) / `value_type` (6639); then the engine: type alias `Computation` (6751), `enum Argument` (6753), `Site` (6758), `EstablishedValue` (6765), `PreparedExpansion` (6777), `PreparedClaimEdge` (6782), `struct Expansion` (6788-6815, 21 fields), `impl Expansion` (6817-7798) with builder `with_*` methods (6861-6950), `call_arguments` (6953), `retained_pure_value` (7003), `retained_value` (7034), `retained_root` (7065), `retained_root_argument` (7093), `qualification_edge` (7121), `push` (7141), `argument_type` (7148), `sequence`/`sequence_arriving` (7168/7189), `direct_value` (7241), `binding` (7256), `argument` (7276), `argument_with_normal_result` (7297-7797, the central `match node.kind` with 17 arms), free fns `parameter` (7800), `parameters` (7807), `machine_computation_handles` (7816).

**Inputs/outputs.** Same as 1-3600. `Expansion::finish` returns `PreparedExpansion { states, claim_edges }` (6880-6885): a vector of `LoweredScalarBranchState` and claim edges keyed by state position. Slots (`produced_values`, `subslices`, `literals`, `cases`, `observations`) are prepared before expansion by each module's `prepare` and passed in through `with_*` builders (6901-6950).

**How it decides (keys).**
- `argument_with_normal_result` dispatches on `CheckedScalarComputationKind` (7388-7794): StorageAddress, ByteSequenceLiteralComparison, ByteSequencePredicate, StructuralField, CaseMembership, SelectedComparison, Qualification, AuthoredCast, Dispatch, RetainedConstant, Value, Select, BooleanToInteger, Call, StorageObservation, WithOperands, Apply. All variants appear in the match; no `_ =>` arm (read).
- Cycle check by handle membership in `active` (7322); shared sub-computations reuse a carried parameter by `self.completed` lookup keyed on computation handle (7334-7344).
- Value completion looked up by `(machine, state, statement, expression)` in `cleanup_control.value_completion(..)` (7349-7358).
- `RetainedConstant`: `checked.landed_integer_constant(expression)` first, else `scalar_applications.expression_in(expression, machine)` (7565-7586).
- Return completion destination: `qualifications.return_completion(..)` then `target_state.parameter_types` compared to `source_types + [destination]` (5772-5800).
- Slot reuse keyed on `source_call` + `argument_ordinal` (literals 4664-4666, produced_values 5584-5586) or on `expression` (subslices 6430-6432), with conflicting-type rejection.
- Storage observation selects readable authority by trying `MutableBorrow` then `SharedBorrow` through `descriptor_source_projection_accessible` (5082-5090).
- Scalar value availability in `fields::prepare` atomic path: `carrier.operation == AtomicAccessOperation::Load(*ordering)` and `ordering.valid_for_load()` (3610-3620).

**Cases by shape.**
- `fields::prepare` (3580-3956): record read path segments limited to Field, Case, Referent, FixedIndex (3707-3786); `_ => "record read requires declared record field projections"` (3783). `FixedArray` only with `ArrayExtent::Literal` (3758). Endpoint type loop crosses only `SharedBorrow | MutableBorrow` references and must end at a `PrimitiveScalar` of `leaf_scalar_type` with at least one crossing: "scalar observation endpoint is not a referenced primitive of its scalar type" (3913).
- `observe` (4394-4592): handles atomic loads, established case payloads, runtime-index reads (byte overlay / element view / indexed primitive), byte overlay at fixed index, primitive/structural leaf by `leaf_type` (Boolean, Integer, IeeeFloat) (4548-4586).
- `atomic_observation`: requires no runtime index, no byte overlay, no bounded leaf, exact scalar types (4360-4366).
- `selection::condition_order` (6125-6183): walks Apply/StorageObservation operands, SelectedComparison left/right, cast/qualification operands; explicitly lists the leaf kinds with no children (6169-6179) — no `_`.
- `value_type` (6639-6747): all `CheckedScalarComputationKind` variants listed (no `_`); `Dispatch` requires all arms to have the same qualified type (6728-6736); result scalar type must equal `terminal_scalar_type(node.primitive_type)` (6743).
- `structural_arguments::lower`: declaration inputs (`source_call.is_none()`) must be `Owned | SharedBorrow` and from a `Parameter` source: "declaration input lacks the required loan or temporary custody" (6235-6238); owned or `Unrestricted` multiplicity only (6242-6252); literal arguments `Owned` or `SharedBorrow` only (6286-6290).
- `storage_observation` handles only `StructuralParameterIndexedRead` and `StructuralSequenceLength` expression shapes specially; any other expression is just lowered as an expression with the operands as prefix (4934-4955).
- `subslice`: endpoints must be non-address integers (6549-6555).
- `Expansion::call_arguments` (6953): `Pure` or `Computation` operands.

**Same job elsewhere.**
- Every `match`-on-`CheckedScalarComputationKind` in this file (`value_type` 6667, `condition_order` 6151, `argument_with_normal_result` 7388, `constructions` 1963, `observations::prepare` 4760, `produced_values::prepare` 5524, `literals::prepare` 4629, `subslices::prepare` 6394, `type_requirements` 5433, `fields::type_roots` 4080, `fields::prepare` 3592, `cases::type_roots` 1929) re-selects the variants it cares about by hand: no shared visitor. 12 sites in this file (counted by reading, not grepped).
- Reachable-node traversal: `reachable_nodes(checked, &roots)` is imported from `expression_preparation::computation_graph` (used 1762, 1928, 3590, 4079, 4756, 5523, 6393, 7822) while `condition_order` (6125) has its own explicit stack traversal for a different purpose (read).
- Producing a `LoweredScalarBranchState` literal with all `Vec::new()` fields is repeated at least ten times (e.g. 1379-1389, 1396-1409, 1455-1471, 1481-1492, 2139-2168, 2580, 3025-3033, 6579-6590, 6596-6609, 7265-7273) although `LoweredScalarBranchState::continuing/jumping` constructors exist and are used elsewhere in the file (read).
- `NormalResultTarget`/`NormalDispatchScope` carry the same arm list through dispatch in both `structural_dispatch` (2422-2427) and `dispatch` (2661-2666) (read).

**Unused / one-caller (grepped).**
- `retained_pure_value`: 1 call (`unit_arguments.rs:3297`). `with_completed_values`: 1 call (`unit_arguments.rs:4833`). `type_requirements`: 1 call (`attached_unit.rs:3227`; the definition is `operand_types::type_requirements`). `retained_value(`: 3 call sites (`unit_arguments.rs:3307`, `unit_arguments.rs:6118`, `scalar_lowering.rs:335`) plus the inner call at 7054.
- Grep: `retained_pure_value|with_completed_values|retained_value\(|type_requirements\(` (content mode over omega-rust). A count-mode grep over `retained_pure_value|\.retained_value\(|with_completed_values|\.call_arguments\(|operand_types::type_requirements|machine_computation_handles|computation_value_type|prepare_place_observations|loaned_primitive_store|loaned_store\(` returned 32 total occurrences in 8 files (scalar_computations.rs 11, unit_arguments.rs 9, attached_unit.rs 3, references.rs 3, unit_operation_frame.rs 2, unit_composed_control.rs 2, scalar_lowering.rs 1, function_fragments.rs 1); not split per name. (Some of those hits are unrelated `loaned_store(` strings in other crates: references.rs and function_fragments.rs are outside this crate — suspected, not read.)
- Comment claiming single path: "The one byte-sequence operand observation" (95): `byte_sequence_observation` has 2 in-file callers (`byte_literal_comparison` 340, `byte_predicate` 377) and none outside (read).

**Stated limits (quoted).** "saved call operand has no dominating completed subject" (929); "computed place frontier precedes its saved address" (1050); "declaration structural call has no retained initialized input home" (820); "computed constructor did not retain its structural value" (6229); "computed owner requires transfer or lending" (6267); "record view carrier change needs a static source leaf" (4445); "record loan's runtime element read needs a scalar leaf at the view root" (4472).

**Fail-open risks.**
- `argument_with_normal_result` has no catch-all arm in the `match node.kind` (7388-7794), so a new computation kind forces a compile error rather than a silent pass (read).
- `selection` skips emitting the `when_false` arm when `identical_values` (6060-6069: both nodes are `Value(left) == Value(right)` and neither is already completed): the conditional is replaced by the `when_true` path. Which program: a `select` whose arms are the same expression — both branches would compute the same value, but the `claim_continuation` for `selected: false` (6072-6082) is also skipped, so a claim edge for the false route is not emitted. Status: open (whether destinations can differ per route when values are equal was not traced).
- `retained_root_argument` (7093-7119) runs `consume_pure_call_arguments` only for role `LocalInitializer`; other roles are not cross-checked (7107-7117). Status: open.
- `returns::normal_return` `Selected` target uses `continuation = 0` (5847) as a placeholder state index while the real targets are mapped inside `NormalResultTarget`; state index 0 is relative to `self.base`. Status: open (not traced).
- `fields::observe` returns the established case payload early when `established_case_payload` yields `Some` (4432-4441), skipping later carrier checks. Status: open.
- `subslice`: missing `start` defaults to literal 0 (6565-6568) — by design for open-ended ranges; missing `end` is passed as `None` to `exclusive_end` (6564), not read here.
- `operand_types::type_requirements` only collects identities; no accept/reject.

## Coverage

- `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs`: lines 1-8032 read in full (8032 lines). Read in chunks 1-1600, 1601-3100, 3101-4600, 4601-6300, 6301-7300, 7301-8032.
- `.../src/retention.rs`: lines 1-6094 read in full (6094 lines). Chunks 1-1100, 1101-2400, 2401-3600, 3601-4850, 4851-6094.
- `.../src/scalar_computations.rs`: lines 1-7824 read in full (7824 lines). Chunks 1-1100, 1101-2350, 2351-3600, 3601-4850, 4851-6100, 6101-7100, 7101-7824.
- Skimmed: none. Not read: none. Total read 21950 of 21950 lines.
- Grepped but not read: other call sites of functions listed in "Unused / one-caller" (only the matching lines were seen).

## Five things a maintainer most needs to know about this unit

1. All three files are nested `pub(crate) mod` trees inside one source file each. `proofs.rs` holds six top-level modules (`content_conservation` 59, `contract_predicates` 808, `crash_routes` 1304, `evidence_lowering` 4620, `float_meaning_projection` 7129, `integer_predicates` 7748); `retention.rs` holds ten (`access_lifecycles` 21 ... `suspension_call_plan` 4852); `scalar_computations.rs` is one `computations` module (8) with about 20 sub-modules and a single `Expansion` engine (6788). Names are found by module path, not by file.
2. Boolean-predicate lowering with explicit positive/negative polarity exists in three parallel copies inside `proofs.rs` (`contract_predicates::proposition_with_polarity` 1000, `crash_predicates::checked_boolean_connective_polarity` 1816, `structural_route_buckets::RouteTerms::lower_polarity` 3838), all sharing `equality_from_polarities` (1205) but each re-handling Constant/Not/And/Or/Equal.
3. Evidence lowering mixes keying schemes: evidence term identity is canonical order from union-find plus a sorted renumbering loop capped at 4 rounds (`proofs.rs:6689-6730`); propositions are matched by symbol in one place (4751) and by declaration *name string* in another (6494-6498); static requirement dispatch re-searches the lowered module for the conformance application it expects and demands exactly one match (6317-6349).
4. The float-meaning projection resolves a "direct" Terminal source when it can and silently falls back to `FloatMeaningSource::TransitionalInput` otherwise (`proofs.rs:7186, 7200, 7214, 7228, 7242, 7256`); `rejoin_float_semantic_applications` (7656) then upgrades `TransitionalInput` rows that have a semantic application (7699-7701). A row that stays `TransitionalInput` is not rejected in this file.
5. `retention.rs` and `scalar_computations.rs` are mostly lookup-and-join code: they find a lowered operation or Terminal declaration for a checked fact by `(state, statement_index, call_ordinal)` coordinates, interned type-identity text, or interned member spelling, and error when the join is not unique. Where a join is allowed to be absent (`continue` / `Ok(None)` / `unwrap_or_default`) the code silently drops that item; the sites are listed under each section's "Fail-open risks".

## What I am unsure about

- Whether the `Unresolved` suspension capture/resource/backing/value-loan states (`retention.rs:5369, 5393, 5547, 5648`) are rejected by a later validator; I did not read the Terminal Psi validators.
- Whether `FloatMeaningSource::TransitionalInput` is rejected downstream when left in a published module; I did not trace the callers of `lower_float_meaning_projection` beyond `machine_lowering.rs:1152`.
- Whether the empty row list returned by `checked_static_conformance_rows` for open generic requirements (`proofs.rs:6954-6961`) can let a lowered conformance application with zero rows match; not traced.
- Whether the HashMap/HashSet uses in `retention.rs` (1877, 1889, 2855-2866) conflict with the repository's `tools/dense_id_maps.txt` / `tools/cache_state.txt` lists; I did not open those files.
- Numeric parameter-kind codes differ between `retention.rs:1085-1096` and `proofs.rs:6877-6884`; I did not check whether both feed a shared consumer that expects one encoding.
- Macro-expanded patterns (`facts::checked_boolean_triple_pattern!`, `semantic_vocabulary::scalar_term_*` macros used at `proofs.rs:1694, 4149, 4201-4216, 4236`, `terminal_psi::call_structural_pair_pattern!` at 5368) were not expanded; their exact variant coverage is unverified.
- Per-function external call counts were grepped only for the functions named in the "Unused / one-caller" paragraphs; other `pub(crate)` functions in the unit were not counted.

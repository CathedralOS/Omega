# Unit 103

Files covered (both read in full, line 1 to last line):

- `omega-rust/psi/compilation/validation/src/quotients.rs` (1-8176, 8176 lines)
- `omega-rust/psi/compilation/validation/src/recasts.rs` (1-5438, 5438 lines)

Checkout: `<checkout>/`. Both files are plain `.rs` files mounted by
`validation/src/lib.rs:60-61` (`pub(crate) mod quotients; pub(crate) mod recasts;`) and re-exposed under the aliases
`crate::proof_contracts::quotients` (`lib.rs:118`) and `crate::value_custody::recasts` (`lib.rs:132`). Public re-exports are at
`lib.rs:1829-1833` (quotients) and `lib.rs:1876-1885` (recasts). All counts below come from the Grep tool over
`omega-rust` (`--glob *.rs`) unless noted.

---------------------------------------------------------------------------------------------------------------------

# File 1: quotients.rs (8176 lines)

## 1. Purpose

The file does three jobs for the `quotient` data feature (a type formed by `%` over a carrier type and an equivalence relation):
(a) checks that each quotient data declaration names one exact, sealed, toolchain `Equivalence` conformance and a relation over
two carrier values (`validate_quotient_formations`); (b) for each `Quotient::lift` / `Quotient::define` call, derives a
"direct-terminal relation plan" (which parameters are quotient-typed, the representative operation, the congruence theorem, optional
forward-precondition transport theorem, precondition partitions, runtime correspondence) and verifies the theorem's shape; (c) turns a
fully verified plan into a normalized, hermetic `QuotientCorrespondenceJudgment` (type `language_semantics::quotient_correspondence`)
that later stages consume (`admit_checked_quotient_requests`). At validation time every quotient operation request is still rejected
with a long diagnostic that lists what was and was not checked (`reject_quotient_operation_requests`).

## 2. Shape (all nested modules live inside the file; line ranges)

- `equivalence_selection` 5-515
  - `validate_equivalence_selection` :17 (formation check for one quotient definition).
  - `row_has_exact_equivalence_premises` :222, `fact_is_exact_relation_pair` :256, `exact_relation_application_matches` :328,
    `exact_generic_argument_symbols` :383, `is_sealed_core_equivalence` :416, `type_reference_symbol` :452,
    `checked_proof_dependency` :463 (transitive "no admitted/boundary machine" walk), `base_data_symbol` :487, `quotient_for_type` :505.
- `formation_collection` 516-916
  - `collect_validated_quotient_formations` :532 (loop over `program.data_definitions()`; pushes a `ValidatedQuotientFormation` only if no new diagnostic was added, :611).
  - `reject_quotient_operation_requests` :622 (always emits an error for every quotient-operation call; success path builds the long "plus exact ..." text :873-878).
- `relation_plan` 917-7902 (the bulk; sub-modules):
  - `correspondence_certificate` 930-1695: `QuotientCorrespondenceCertificate` :1014, `QuotientCorrespondenceEvidence` {DirectLift, DirectLiftWithTransport, Define} :997,
    `derive_direct_lift_precondition_implication` :1055, `derive_fixed_representative_call_preconditions` :1179, `proof_value_substitutions` :1274,
    `proof_value_for_literal` :1307, arithmetic binding helpers :1369-1552, `compose_lift_correspondence_certificate` :1596,
    `compose_lift_transport_correspondence_certificate` :1621, `compose_define_correspondence_certificate` :1673.
  - `normalized_judgment` 1696-2612: `normalized_direct_correspondence` :1729, `normalized_correspondence` :1761 (the long list of `require_*` shape checks),
    `canonical_forwarded_result_flow` :2100, `theorem_correspondence` :2200, `relation_identity` :2441, hermetic identity helpers `canonical_symbol` :2570 / `canonical_type` :2577.
  - `precondition` 2613-2997: `RepresentativePreconditionPartition` {dependent, fixed} :2635, `derive_*_precondition_partition` :2652-2754,
    `derive_define_precondition_correspondence` :2802, `pair_precondition_facts` :2861, `precondition_fact_at` :2922, `expression_depends_on_any` :2967.
  - `proof_fact_identity` 2998-3739: `ProofValueSubstitution` {symbol, rendered:String, trace:String} :3014, `proof_facts_match` :3351, `proof_expression_identity` :3462,
    `expression_symbol_trace` :3495, `static_argument_identity` :3707.
  - `representative` 3740-3816: `RepresentativeTelescope` :3761, `derive_representative_telescope` :3771.
  - `result_flow` 3817-4135: `fallthrough_result_root` :3860, `immutable_alias_fallthrough_root` :3880, `complete_single_state_result_flow` :3925,
    `complete_state_forwarding_result_flow` :3966, `complete_result_flow` :4065.
  - `runtime_correspondence` 4136-5193: `ClosedLiftLiteral` (13 variants) :4199, `derive_define_runtime_correspondence` :4272,
    `derive_direct_lift_runtime_correspondence` :4293, `closed_lift_literal_for_representative` :4492, `exact_integer_landing` :5019, `exact_float_landing` :5044,
    `direct_public_parameter_symbol` :5096, `quotient_carrier_matches_type` :5148.
  - `static_application` 5194-5575: `substituted_type_matches` :5207, `validate_static_application` :5440, `validate_closed_type_argument` :5537.
  - `theorem` 5577-5660: `derive_selected_theorem_telescope` :5606.
  - `theorem_schema` 5661-5896: `ExpectedTheoremSchema` :5766, `derive_expected_theorem_schema` :5775, `collect_contract_facts` :5702.
  - `theorem_schema_verification` 5897-6255: `verify_selected_theorem_schema` :5937, `verify_parameters` :6209.
  - `transport_schema` 6256-6538: `verify_forward_precondition_transport_schema` :6294.
  - Plan types and enums 6554-6711 (`DirectTerminalRelationPlan` :6602, `PlannedQuotientTheoremEvidence` :6625, `RepresentativeStaticBinding(Kind)` :6634-6645,
    `RelationPlanError` ~58 variants :6654, `Display` impl :6713-6909).
  - `derive_direct_terminal_plan` :6911 (orchestrator), `unconditional_termination_fence` :7242, `pure_effect_fence` :7255, `require_checked_proof_closure` :7277,
    `derive_relation_and_representative` :7293, `render_failed_builtin_implication` :7379, `validate_theorem_role_collection` :7503, `exact_quotient_relation` :7536,
    `impl DirectTerminalRelationPlan` render_* methods :7565-7873.
- `request_admission` 7903-8030: `extract` :7909, `extract_one` :7963.
- Top level 8032-8176: `CORE_EQUIVALENCE_SOURCE = "relation.omg"` :8046, trait `CheckedTerminationOracle` :8048 (impl for `TypedTrees` :8060 and for any `Fn(SymbolHandle)->Option<TerminationGuarantee>` :8071),
  `ValidatedQuotientFormation` (alias of `checked_trees::CheckedQuotientFormation`) :8080, `ValidatedQuotientOperation` :8083, `validate_quotient_formations` :8091,
  `QuotientRequestAdmission` {AtValidation, AfterCheckedFacts} :8105, `validate_quotients` :8110, `admit_checked_quotient_requests` :8126, `checked_quotient_relation_identity` :8157.

## 3. Inputs and outputs

- Reads everything through `&dyn crate::CheckingInput` (`program`): data definitions, type-reference table, expression/statement tables, proof facts, signature contracts,
  machines/states, traits/conformances, `proposition_definition_by_symbol`, `normalized_type_identity`, `render_proof_expression`, `symbol_toolchain_relative_path`.
  Termination comes from a `CheckedTerminationOracle`; effects from `crate::infer_operational_may` / `infer_service_reaches` (:6927-6928) and `machine_calls::denotational_calls::{has_pure_effect_closure, has_no_crash_routes}` (:6962, :7263).
- Outputs: `Vec<Diagnostic>` (formations, reject path), `Vec<ValidatedQuotientFormation>`, `Vec<ValidatedQuotientOperation>` (each holding a `QuotientCorrespondenceJudgment`).
  It does not mutate program tables.
- Entry callers (grep, whole `omega-rust`, excluding this file):
  `validate_quotients(` 1 call site (`program_validation.rs:502`); `validate_quotient_formations` 1 (`review-evidence/.../projection.rs:91`);
  `admit_checked_quotient_requests` 1 (`05_typed-trees-to-checked-trees/src/checking.rs:1102`, passes a closure as the oracle);
  `checked_quotient_relation_identity` 2 (`06_checked-trees-to-lowered-psi/src/attached_unit.rs:12256`, `:12348`);
  `exact_relation_application_matches` 2 external (`contract_entailment.rs:23885`, `:23894`) plus its own use at :265.

## 4. How it decides (what each judgment is keyed on)

- Sealed `Equivalence` identity (`is_sealed_core_equivalence` :416-450): keyed on spelling and path: `name.as_str() != "Equivalence"` (:421),
  `symbol_toolchain_relative_path(symbol) == Path::new("relation.omg")` (:424-426, constant :8046), parameter kinds (Type, Proposition) :431-436, and the
  sorted parent trait NAMES equal `["Reflexive","Symmetric","Transitive"]` (:437-449). Nothing here keys on a symbol identity.
- Law-premise check (`row_has_exact_equivalence_premises` :222-254): `match requirement { "reflexive" | "symmetric" | "transitive" }` string compare of the requirement name (:228-233);
  expects fact-parameter index pairs `[]`, `[(0,1)]`, `[(0,1),(1,2)]`.
- `fact_is_exact_relation_pair` :256-326: for a `ProofFact::Expression`, builds `render_proof_expression` strings of the relation body and the actual fact with parameters
  replaced by the literal text `"$left"`, `"$right"`, `"$other"` and compares the strings (:286-322). For `Proposition` facts compares proposition symbol, argument symbols
  (`expression_is_symbol`, :405) and binder symbols (:328-381). `Membership` facts are rejected (:324).
- Proof-fact equality (`proof_facts_match` :3351, `proof_expression_identity` :3462): identity is a pair `(rendered_string, trace_string)`; equality requires both strings equal.
  `trace` is built by `expression_symbol_trace` :3495-3637 using `format!` with `{:?}` of symbol handles, `{:?}` of `call.quotient_operation`, `private_layout_operation`,
  `evidence_arguments`, `operational_acknowledgement` (:3549-3572) and `static_argument_identity` (:3707, `{:?}` of `const_literal`, `evidence_projection`).
  The trace for `Binary` is `"{left}|{right}"` (:3586-3588) and for `Unary` is just the operand trace (:3616); neither includes the operator; `Range` omits inclusivity (:3602); `Borrow` omits access (:3601).
  So operator identity comes only from the first (rendered) string.
- Relation/representative/theorem selection: all by symbol handle (`machine_states_named`, `representative_machine_state` :3803, `selected_theorem_machine_state` :5647). A request must resolve to exactly one state or fails (:3811, :5655).
- Partition into "dependent" vs "fixed" preconditions (`derive_precondition_partition` :2756, `expression_depends_on_any` :2967): by whether a `Name` expression's `symbol` or `head_symbol` is in the list of quotient-bearing parameter symbols.
- Quotient detection on a type (`quotient_for_type` :505): `base_data_symbol` (:487; looks through `Reference`, `Constrained`, `Named`, `Generic.base_symbol`) then linear search of `data_definitions()` for `symbol == && quotient.is_some()`.
- Arithmetic entailment of representative preconditions uses `crate::strict_arithmetic_expression_implication` (:1124, :1233) with atoms named `"$theorem_parameter_{n}"` / `"$runtime_parameter_{n}"` (strings).
- Literal-argument admission (`closed_lift_literal_for_representative` :4492): by the representative parameter's TYPE shape (fixed arrays of literal length, `Named` primitive by `PrimitiveType::from_name(name)` spelling :4740-4745), not by the literal's own spelling, except `Integer` which must `integer_literal_fits` (:5065).
- Termination/purity/crash fences: `unconditional_termination_fence` :7242 (oracle), `pure_effect_fence` :7255, `has_no_crash_routes` :6962 (all keyed on machine symbol).

## 5. Cases by shape (fixed lists that refuse the rest)

- Equivalence premises: only the three names `reflexive`, `symmetric`, `transitive` (:228-233); anything else -> `false`.
- `validate_theorem_role_collection` :7503-7528: Define -> exactly `[Congruence]`; Lift -> one evidence `[Congruence]` or two `[Congruence, ForwardPreconditionTransport]`; any other count -> `NonCanonicalTheoremRoleCollection`.
- `normalized_correspondence` :1761-2098 (judgment production): public op, representative and every selected theorem must be single-state machines (`require_single_entry` :2508), have an empty static telescope
  (`require_empty_owner_telescope` :2523), have immutable parameters (`require_plain_immutable_parameters` :2536), closed static applications (":1831 'the proof-only bridge requires empty closed static applications'"),
  Define admits no public/representative preconditions and no legality premises (:1888-1892, :1932-1945), generic quotient declarations excluded (:2450-2453).
- `closed_lift_literal_for_representative` :4492-4775 handles: bool 3-D tensor (:4512-4558), arrays of depth >= 3 recursive (:4559-4575), 2-D arrays of u8 / landed integers / f32,f64 / bool (:4576-4661),
  1-D bool / u8 / integers / floats (:4662-4717), byte strings only when the target is a shared `&[u8]` slice or a bounded byte buffer (:4719-4732), and scalar bool/integer/float (:4746-4774). Everything else -> `DirectLiftLiteralTargetMismatch`.
- `derive_relation_and_representative` :7321-7349: a literal argument with no declared place type is accepted only for `Lift` and only for `Boolean|Integer|Float|String|ArrayLiteral` expression nodes.
- Result flow (`result_flow` module): a request must be (a) the last statement's value (direct) or (b) reached through a chain of immutable local aliases ending in the request at statement position 0 (`immutable_alias_fallthrough_root` :3880-3923),
  and the machine must be single-state (`complete_single_state_result_flow` :3952) or multi-state where every non-result state is a single unguarded ordinary `Transition` to a named state and all chains reach the result state with no cycle (:4006-4049).
- Static application (`validate_static_application` :5440-5535): lifetime parameters on the callee refused (:5446), `Proposition` and `Implementation` parameter kinds refused (:5510-5523).
- `theorem_schema::collect_contract_facts` :5702: contracts with a `binding`, or kinds other than Requires/Ensures, refused (:5718-5727).

## 6. Same job elsewhere

Within this unit:
- Identify a primitive type reference: `quotients.rs:4803 exact_primitive_type` (spelling only: `PrimitiveType::from_name` plus `name == primitive.name()`), `quotients.rs:4740`, `quotients.rs:5551-5553` (spelling),
  vs `recasts.rs:4512 exact_primitive_type` (requires the builtin-type ATOM of the symbol and the spelling to agree) and `recasts.rs:4540 exact_scalar_representation_type`. Same job, different key (spelling vs symbol+spelling).
- Look up a machine/state by symbol: `quotients.rs:2484 exact_machine_state` (scan of all machines and states, must be exactly one), `:3934-3950`, `:3975-3999` (repeated `program.machines().iter().filter(symbol ==)`),
  `:3803 representative_machine_state` and `:5647` (`machine_states_named`), `:5944` (`typed_trees::machine::machine_by_symbol`), `:7282` (`program.machine`), `:6506 exact_theorem_state`.
- Fetch a `Requires` fact by (owner, contract position, fact position): `precondition_fact_at` :2922 and `representative_fact` :6082 do the same lookup; contract-fact iteration by owner (Machine, State) x `SignatureContractKind::Requires`
  repeats at :238-245, :2766-2797, :5708-5747, :5851-5885.
- Literal-array leaf discovery: `literal_fixed_array_leaf` :4816 here vs `recasts.rs:276 literal_fixed_array_target_terminal`.
- `base_data_symbol` :487 vs `type_reference_symbol` :452 (peeled + `type_head_symbol`) in the same file.
Outside the unit (read only the cited lines):
- `base_data_symbol` is re-implemented in `expression_types_and_fields.rs:189` (read :189-201): uses `unconstrained_type_reference` + `type_head_symbol`, does not look through `Reference` or take `Generic.base_symbol`. Different behaviour, same name.
- `integer_literal_fits`: `quotients.rs:5065` and `03_symbol-resolved-trees-to-typed-trees/src/equality_and_exhaustiveness.rs:2416` (read :2416-2439, a per-primitive width table). Suspected duplicate of the same fit test.

## 7. Unused or one-caller code

- `theorem_correspondence` :2200 is called once (:2048) with `retain_legality = true`; the `false` branch (:2299 `Vec::new()`) has no caller in this file (read; grep of the name shows only this site and the definition).
- `static_argument_matches_type` takes `_substitutions` (:5383) and only passes it through recursively (:5421); it is never read.
- `fact_is_exact_relation_pair` (pub(crate)) :256: callers are :252 and :5982 (both in this file). 0 outside.
- `checked_proof_dependency` (pub(crate)) :463: callers :203 and :7285 (both in this file), recursion :481. 0 outside.
- `quotient_for_type` (pub(crate)) :505: callers :2719 and :7541 (this file). 0 outside.
- `base_data_symbol` (pub(crate)) :487: callers inside this file only (:214, :545, :593, :5172, :8162, :497-499, :509). 0 outside (the same name in `expression_types_and_fields.rs:189` is a separate private fn).
- `exact_relation_application_matches` re-exported at :8032; 2 external call sites (`contract_entailment.rs:23885`, `:23894`).
- `render_*` methods on `DirectTerminalRelationPlan` (:7566-7872) are called only from `reject_quotient_operation_requests` (:873-878 and the lines before it): they only produce diagnostic text.
- "Shared/canonical" claims: the file header (:1-4) and `normalized_judgment` use "canonical" for the identity text; no comment declares a single shared relation. None to report.
- `HashSet` (std) is used at :8, :202, :7288 (against the dense-ID rule in AGENTS.md; noted, not judged).

## 8. Stated limits (quoted from the file, under 20 words each)

- :6718 "adapted lift arguments require later expression typing" (`UnresolvedArgumentType`).
- :6749 "has no closed application boundary yet" (`RepresentativePropositionApplicationUnsupported`).
- :6745 "require the ordinary call-site elision judgment" (`RepresentativeLifetimeApplicationRequiresElision`).
- :6828 "does not yet substitute a generic quotient owner" (`DirectLiftOwnerRequiresSubstitution`).
- :6874 "requires exact owner-telescope substitution" (`DefineOwnerRequiresSubstitution`).
- :869 and :904 "executable quotient operations are not admitted until ... are independently checked" (the validation-time admission is a refusal for every request).
- :1831 "the proof-only bridge requires empty closed static applications" and :2451 "the proof-only bridge excludes generic quotient declarations".
- :571-577 "the initial quotient surface cannot identify affine or linear occurrences" (carrier fence).

## 9. Fail-open risks

1. `checked_proof_dependency` :477-479: `let Some(callee) = program.machine_by_own_or_state_symbol(dependency) else { continue; };` A call-dependency symbol that does not resolve to a machine is skipped, i.e. treated as checked.
   A formation whose conformance row calls something that resolves to no machine (I did not trace `machine_call_dependency_symbols`, `call_cycles.rs:3458`, which collects contract and statement dependency symbols) would not trip the "admitted or boundary proof machine" rule. Status: open.
   Same function is used for representative and theorem closure (:7285).
2. `expression_symbol_trace` :3586-3588, :3616: trace key omits the operator of `Binary` / `Unary`. Equality of two facts relies on the first (`render_proof_expression`) string also being equal (:3486-3492, tuple compare in `proof_facts_match` :3359-3362).
   I did not read `render_proof_expression`; if it includes operators the pair is sound. Status: open (inferred, not traced).
3. `validate_closed_type_argument` :5551-5556: an argument whose symbol has no data definition is accepted if it is a single-segment path whose spelling is a primitive name. Keyed on spelling. Status: open.
4. `exact_quotient_relation` :7540-7549: a type for which `quotient_for_type` finds no data definition with quotient metadata is treated as `NotQuotient` and the position becomes `ExactEquality`.
   A quotient type whose definition is not in `data_definitions()` would be treated as an ordinary type. Status: open.
5. `is_sealed_core_equivalence` :424-426: authority is path text `"relation.omg"` (toolchain-relative), name text and parent names. Whether a non-toolchain symbol can return `Some("relation.omg")` from `symbol_toolchain_relative_path` was not traced. Status: open.
6. `push_exact_position_arithmetic_bindings` :1456-1459 returns silently if the parameter is not an exact integer: fewer bindings, so entailment is harder to prove, not easier (fails closed; noted so it is not mistaken for a risk).
Not a risk (checked): `admit_checked_quotient_requests` :8142-8146 falls through to the rejecting path if any request fails; `transport_gated_derivation` returning `None` when the theorem schema failed (:7043-7062) leads to `correspondence_certificate = None` and `normalized_direct_correspondence` errors on `None` (:1746); `extract_one` also requires `representative_purity` (:8003).

---------------------------------------------------------------------------------------------------------------------

# File 2: recasts.rs (5438 lines)

## 1. Purpose

Validates "recasts" (`&x as &T` / `&mut x as &mut T`): re-viewing a place's bytes under another shape. It decides whether a recast is allowed (size, representation facts both directions for writable views,
byte-region footprint in range, qualification preservation, atomic authority), classifies the source of any reference formation (`reference_formation_source`), computes the byte layout of record/array types as a flat list of scalar "leaves"
(`MutableRecordRepresentation`), and exposes layout/projection data (`RecastScalarLayout`, `RecastScalarProjection`, source-leaf matching) that later stages (checking, lowering) use to emit loads/stores through a recast view.
It also handles "pending views" (mutable recast of a record with `where` facts whose predicate is not yet established).

## 2. Shape (nested modules; line ranges)

- `literal_indexed_footprints` 5-307: `ValidatedLiteralIndexedRecastFootprint` {collection,start,end} :21, `validate_literal_indexed_recast_footprint` :116, `validate_runtime_indexed_recast_source` :163,
  `maximum_recast_footprint` :215, `literal_indexed_recast_target_size` :238, `literal_fixed_array_target_terminal` :276.
- `offset_bounds` 308-582: `composite_offset_upper_bound` :428 (Add/Subtract/Multiply/ShiftLeft), `offset_expression_is_multiple_of` :498.
- `pending_views` 583-966: `pending_view` :601, `formation_obligation` :625, `formation_owner` :679, `pending_origins` :731, `withholds_default_predicate` :772, `report_consumption` :809, `report_whole_view_write` :885, `validate_operand_consumptions` :913.
- `qualification_views` 967-1051: `qualification_view_compatible` :972, `lender_qualifications_implied` :987, `qualification_carrier` :1019, `declared_qualifications` :1028.
- `raw_byte_region` 1052-1425: `indexed_byte_region` :1070, `SliceRecastByteRegion` :1138, `slice_recast_byte_region` :1147, `byte_region_footprint` :1229 -> {NotByteRegion, Contained, Rejected(String)},
  `interior_byte_region_source` :1292 -> {NotInteriorShape, OffsetUnproven, Bounded}, `proved_offset_upper_bound` :1332.
- `recast_judgments` 1426-2348: `judge_scalar_recast` :1448 (entry), `form_scalar_recast` :1521 (formation checks), `judge_scalar_recast_alternative` :1742 (per layout alternative), `judge_slice_recast` :2043, `report_unspelled_reference_pun` :2298, `strip_mutable` :2339.
- `record_eligibility` 2349-2661: `closed_fact_free_record_type_is_eligible` :2359 (iterative walk, limits :2355-2357), `phantom_lifetime_record_symbol_shape` :2473, `direct_phantom_lifetime_record_symbol` :2530, `direct_record_view_type_is_fact_free` :2538.
- `record_representation` 2662-4264: `MutableRecordRepresentation` {size, align, leaves} :2799, `MutableRecordLeaf` {offset,size,facts,path,primitive,stored_integer,byte_projection,bit_fragments,arrays} :2856,
  `RepresentationBudget` :2775 (depth 256 :2772, work 16384 :2773), `initialized_storage::InitializedStorageBits` :2674, public layout API `RecastScalarLayout` :2931 / `recast_scalar_layout` :2994 / `recast_scalar_projection` :3090 /
  `recast_scalar_path_projection` :3101 / `recast_scalar_path_geometry` :3110, `recast_record_source_leaf` :3222 / `recast_scalar_source_leaf` :3238 / `common_scalar_source_leaf` :3262 / `scalar_source_leaf` :3286,
  `type_representation` :3390, `pending_record_fields` :3397, `pending_record_definition` :3412, `normalized_representation` :3471, `array_region_representation` :3522, `mutable_record_representation_inner_body` :3605 (uses `plan_laid_layout_for_data` or a local fallback layout :3813-3823),
  `representation_values_imply` :3997, `mutable_record_representations_equivalent` :4027, `recast_representations_equivalent` :4037, `repeat_representation` :4082, `representation_is_exactly_tiled` :4105, `record_representation_implies` :4121,
  four `#[cfg(test)]` tests :4130-4263 (the only unit tests in this unit).
- `slice_views` 4265-4298: `slice_recast_source` :4278.
- `source_regions` 4299-4399: `source_layout_root` :4306, `source_representation` :4331.
- `read_admission` 4411-4491: `established_reference_formation` :4416 (re-judges a retained view from its original body).
- `representation_types` 4492-4569: `scalar_is_atomic` :4496, `exact_primitive_type` :4512, `exact_scalar_representation_type` :4540.
- `scalar_representation` 4570-5018: `MutableScalarRepresentationFacts` {atomic, domains, values} :4576, `ScalarRepresentationSet` {ExactBitPatterns(Vec<(u64,u64)>), FloatInterval} :4583, `mutable_scalar_representation_facts` :4639,
  `encode_stored_integer_facts` :4696, `narrow_to_integer_interval` :4780, `narrow_by_field_representation_bounds` :4831, `scalar_representation_facts_imply` :4963.
- `storage_interpretation` 5019-5070: `reference_view_storage` :5024.
- Top level 5086-5438: `ReferenceFormationSource` :5086, `ReferenceFormationInterpretation` {Declared, RepresentationView, Access, ResidentAccess} :5094, `access_borrow_has_empty_write_frame` :5105, `access_storage` :5131,
  `reference_formation_source` :5185, `validate_recasts` :5292 (pass over all machines/states/statements), `statement_expression_roots` :5417.

## 3. Inputs and outputs

- Reads through `&dyn CheckingInput` (expression table, type-reference table, statement table, symbols incl. `builtin_type_atom`, `plan_laid_layout_for_data`, `layout_checking_targets`, `access_view_plans`, `access_borrow_factory`),
  plus `crate::value_custody::places::*`, `crate::proof_contracts::{arithmetic_domains, contract_entailment, default_domains}` for offset/interval/predicate facts, and `crate::CallFrameResolver` for lender declarations.
- Produces diagnostics (`validate_recasts`, `report_*`), `Option` witnesses (`ValidatedLiteralIndexedRecastFootprint`, `RecastScalarProjection`, `RecastRecordSourceLeaf`, `ReferenceFormationSource`), and `Result<(), &'static str>` (`established_reference_formation`).
  Nothing is stored; layout representations are recomputed per call.
- Callers outside this file (grep counts):
  `validate_recasts` 1 (`program_validation.rs:573`); `reference_formation_source(` 51 occurrences in 19 files, 4 in recasts.rs, so 47 outside (13 in `calls.rs`, 6 in `borrow.rs`, 4 in `values.rs`, and others);
  `pending_views::report_consumption` 5 external (`program_validation.rs:1640, :2839, :3657`; `literals_and_struct_literals.rs:4289`; `calls.rs:40216`) plus 1 internal (:953);
  `report_whole_view_write` 1 (`program_validation.rs:1631`); `withholds_default_predicate` 1 (`domains_and_predicates.rs:12459`); `established_reference_formation` 1 (`calls.rs:30604`);
  `reference_view_storage` 4 (`reference_result_custody.rs:5362`, `calls.rs:13733`, `:19700`, `values.rs:2122`);
  `recast_record_source_leaf` 3 (`arithmetic_domains.rs:23456`, `scalar_computations.rs:3988`, `execution_terminal_unit_structural_scalar_store.rs:1695`);
  `recast_scalar_layout` 1 (`scalar_computations.rs:3135`); `recast_scalar_projection` 1 (`expression_bindings.rs:687`); `recast_scalar_path_geometry` 1 (`expression_bindings.rs:701`);
  `recast_scalar_source_leaf` 1 (`calls.rs:19717`); `recast_scalar_source_leaf_in_array_region` 1 (`calls.rs:19708`); `recast_representations_equivalent` 1 (`arithmetic_domains.rs:23575`);
  `unrestricted_scalar_read_projection` 1 (`calls.rs:30560`); `validate_literal_indexed_recast_footprint` 2 (`borrow.rs:561`, `execution_terminal_unit_control_statement_sequence.rs:2177`);
  `validate_runtime_indexed_recast_source` 2 (`borrow.rs:590`, `...statement_sequence.rs:2187`); `slice_recast_source` 2 outside (`published.rs:6871`, `execution_scalar_computations.rs:6157`) plus internal 4282;
  `slice_recast_byte_region` 1 outside (`execution_scalar_computations.rs:6179`) plus internal 106; `access_borrow_has_empty_write_frame` 3 (`calls.rs:39248`, `flow_mutation.rs:4048`, `:4199`).

## 4. How it decides

- Recast admission (`form_scalar_recast` :1521-1739): in order: view access must equal receiving access and the spelled borrow access (:1547-1572; keyed on `ReferenceAccess` values); access-view plans on source or target type refuse (:1585-1598, keyed on `access_view_plan_for_type_reference`);
  qualification compatibility from `qualification_view_compatible` (:1602; domain constraints compared by `symbol` and `semantic_id`, or by `domain_membership_implies`, :1004-1015); the let's declared type must restate the target (:1657-1672:
  by primitive when the target is scalar, else by `normalized_type_identity`); for byte-region sources, writable views need `byte_region_allows_arbitrary_writes`, and the target must be fact-free (:1673-1731).
- Per layout alternative (`judge_scalar_recast_alternative` :1742): slice targets go to `judge_slice_recast`; non-scalar targets compare `MutableRecordRepresentation`s (`mutable_record_representations_equivalent` for writable, `record_representation_implies` for shared, :1836-1848);
  scalar targets require equal byte size (:1908) and `scalar_representation_facts_imply` (shared) or `mutable_scalar_representation_facts_equivalent` (writable) (:1969-1983). Shared recasts must preserve atomic status (:1951-1963).
- Footprint (`byte_region_footprint` :1229): `Bounded` => `offset + footprint <= region_length` (:1250); offset bound proven by literal, declared interval, composite expression, occurrence bounds, relational fit, or completed-value interval (`proved_offset_upper_bound` :1332-1397).
- Representation facts: bit-pattern interval sets over `u64` pairs; implication = every source interval inside some target interval (:4990-4994). Facts per leaf come from the type's spelling (`PrimitiveType::from_name(name)` :4669-4672), its domain constraints (symbols), and `where` bounds (`data_where_field_representation_bounds` :3637).
- Pending-view decisions (`pending_origins` :731): decided per machine; recasts issued in other machines are skipped (:757-759).
- `reference_formation_source` :5185: structural on the expression node: value-cast to a reference type, access-borrow factory call (looked up by `access_borrow_factory(selected.symbol)`, requires exactly one `access_view_plans` row with `data_symbol == symbol` :5223-5229), recast, or plain borrow.
- `validate_recasts` :5292-5415: judges every `let x: &T = <recast>` (:5306-5337), every other recast found in statement expression roots (:5364-5393), and flags any remaining recast expression in the table as an error (:5398-5414). It also calls `report_unspelled_reference_pun` for non-recast reference lets (:5339).

## 5. Cases by shape

- Record eligibility (`closed_fact_free_record_type_is_eligible` :2359-2471): allowed: layout-storage types (follow semantic type), non-bool primitives with a scalar size, fixed arrays of literal length, closed checked-shape record data with no quotient/where facts/zero gate, only live fields. Refused: `Constrained`, bool, builtin non-primitive atoms, cycles (state 1), more than 4096 nodes (:2355) or 16384 edges (:2356).
- Literal-indexed targets (`literal_indexed_recast_target_size` :238): primitives except bool, fixed arrays whose terminal is an exact primitive or closed record (nonzero length), closed fact-free records.
- `judge_slice_recast` :2043-2296: interior slice of a fixed byte region (requires fact-free, exactly tiled element; offset in range; exact tiling by remainder or by multiple-of proof :2133-2158); whole-descriptor slice recast only for shared access and padding-free exactly-tiled elements (:2174-2225); fixed-layout place sources (:2227-2295).
- `indexed_byte_region` :1070-1102: only `FixedArray` with literal length or `Slice`, with u8 elements.
- `composite_offset_upper_bound` :428-496: only `Add`, `Subtract`, `Multiply`, `ShiftLeft` (with literal shift); other operators -> `None`. `offset_expression_is_multiple_of` :498-581: literal, Multiply/Add/Subtract/ShiftLeft, or a place with a one-point range constraint.
- `phantom_lifetime_record_symbol_shape` :2473-2528: a `Generic` with lifetime arguments and no type arguments over a unique data instance whose origin is a closed generic record; `MAX_RECAST_PHANTOM_LIFETIME_SHELL_DEPTH = 2` (:2357).
- `mutable_record_type_representation_body` :3866-3948: scalars, fixed arrays (literal length), `Named`, closed `Generic`, phantom-lifetime shells; `Constrained`, `Reference` and everything else -> `None` (:3945-3946).
- `narrow_by_field_representation_bounds` :4831-4899: float or integer `where` bounds only; operators `<=`, `>=`, `==` for floats (:4854-4861).
- `representation_types::exact_scalar_representation_type` :4540-4568: bool, atomic bool, i8..i64, u8..u64, atomic u32/u64, address, f32, f64 only.

## 6. Same job elsewhere

Within the unit:
- Wrappers of one function: `record_view_type_is_fact_free` :1419, `direct_record_view_type_is_fact_free` :2538 and `closed_fact_free_record_type_is_eligible` :2359; the first two have a body that only calls the third (:1423, :2542).
- Primitive identification: `exact_primitive_type` :4512 and `exact_scalar_representation_type` :4540 (here) vs `quotients.rs:4803` (spelling only); see quotients section 6. `mutable_scalar_representation_facts` :4669 also uses spelling.
- Fetching a primitive's leaf descriptor: `literal_fixed_array_target_terminal` :276 vs `quotients.rs:4816 literal_fixed_array_leaf`.
- Local fallback layout (offset/align rounding) in `mutable_record_representation_inner_body` :3812-3823 and `checked_align_up` :3950 vs the planned layout path `plan_laid_layout_for_data` :3712 in the same function.
- Strip borrows: `strip_mutable` :2339 (strips nested `Borrow` repeatedly) vs `quotients.rs:5096 direct_public_parameter_symbol` (strips one `Borrow`).
- Same "re-judge a recast" call is made from four sites: `literal_indexed_recast_preamble` :67, `established_reference_formation` :4465, `validate_recasts` :5324 and :5376.
Outside the unit (suspected; only names grepped):
- `align_up` / `checked_align_up` helpers exist at `access_views_and_borrows.rs:836`, `build-time-evaluation/src/layouts.rs:7016`, `foundation/layout-plans/src/lib.rs:161`, `foundation/extents/src/loans_and_claims.rs:273` and several backend files (suspected duplicates of `recasts.rs:3950`; bodies not read).
- `strip_borrow` at `arithmetic_domains.rs:23259` (suspected same job as `strip_mutable`; not read).

## 7. Unused or one-caller code

- `literal_indexed_recast_target_size` (pub(crate)) :238: 1 call site (:232), 0 outside.
- `qualification_view_compatible` (pub(crate)) :972: 2 call sites (:1603, :1614), both in this file; `lender_qualifications_implied` :987: 1 call (:1639); `qualification_carrier` :1019: 1 call (:1766).
- `recast_scalar_path_projection` (pub, re-exported at `lib.rs:1882`): 1 call site, internal (:5038); 0 outside the file (grep of the name across `omega-rust`).
- `validate_operand_consumptions` (pub(crate)) :913: 1 call site (:5351), same file.
- `direct_phantom_lifetime_record_symbol` :2530: 1 call (:3487). `phantom_lifetime_record_symbol_shape` :2473: 2 calls (:2534, :3933).
- `exact_interior_byte_region_offset` :1185: 1 call (:2116). `byte_region_allows_arbitrary_writes` :1113: 1 call (:1675). `is_indexed_byte_region` :1104: 1 call (:1673).
- `interior_offset_unproven_rejected` :2003: 1 call (:2085); it has `#[allow(clippy::too_many_arguments)]` but four parameters (read).
- Unused-looking field: `RecastFormation.access` etc. are used (:1761); `RepresentationBudget.lifetime_shell_depth` used (:3930-3942).
- `pending_origins` re-evaluates `has_pending_formation` every call (:738), which walks every statement of the machine (:709-729).
- No comment in the file says a function is "the single/shared/canonical" way; nothing to count.

## 8. Stated limits

- :1871 "deeper shapes land with the byte-view rung" (recast target not scalar or eligible aggregate).
- :1896 "record sources and temporaries land with the byte-view rung".
- :1595 "access-view recast ... is unavailable; retain the underlying qualified extent borrow".
- :5406 "proof-only fact positions (`requires`/`ensures`) produce no runtime borrow and stay fenced".
- :2355-2357, :2772-2773 hard budgets (4096 nodes, 16384 edges, depth 256, work 16384): exceeding returns `None`/`false` (refuses).
- :850 "target padding is not initialized field storage" (mutable recast escape needs complete handback).

## 9. Fail-open risks

1. `judge_scalar_recast` :1483-1490: `if program.defers_pending_layout_geometry() && pending_layout_geometry(...) { return; }` returns with no diagnostic when layout geometry is pending.
   `defers_pending_layout_geometry` is true only for the frozen-program wrapper that carries `defer_layout_geometry` (`frozen_program_and_queries.rs:979-981`; others forward or return false :602-604). `established_reference_formation` refuses in that mode (:4420-4422).
   I did not trace which pass re-runs the judgment with geometry known. Status: open.
2. `judge_scalar_recast_alternative` :1795-1800: for a non-scalar target, `qualification_compatible.is_none() && pending_view(...).is_some()` returns with no diagnostic. A pending view is accepted here and checked later at consumption (`report_consumption`, 5 external call sites).
   I did not verify that every consumption path of a pending-view reference reaches one of those sites. Status: open (design visible, coverage untraced).
3. `pending_origins` :731-770 with `has_pending_formation` :709: if the current machine has no mutable-recast of a pending record, the result is `Ok(Vec::new())` (no obligations); recasts whose issuer is in another machine are skipped (:757-759).
   A reference to a pending view received from another machine is not charged here. Status: open.
4. `judge_scalar_recast_alternative` :1902-1907: `let (Some(source_size), Some(target_size)) = ... else { return; }` returns silently if `scalar_byte_size()` is `None` for either exact scalar primitive. Which primitives return `None` was not checked. Status: open.
5. `report_unspelled_reference_pun` :2307-2321: returns without error when the source or referee is not an exact scalar; only scalar vs scalar mismatches are reported. Status: open (other shapes are presumably caught elsewhere; not traced).
6. `type_has_atomic_storage` :4818-4820: a type whose normalized identity has no storage index yields `false` ("no atomic storage"), which disables the writable/atomic qualification-erasure check at :1601, :1611-1613. Status: open.
7. `qualification_views::declared_qualifications` :1028-1050 returns `None` for any non-`Domain` constraint, and `qualification_compatible == None` is then treated as "no qualification issue" (:1605, :1611, :1796). The later representation checks return `None`/reject for constrained records (:3945) and for `Named` constraints on scalars (:4662), but an `ArithmeticDomain` constraint is ignored (:4660) in representation facts. Status: open.
8. `source_regions::source_representation` :4353-4354 uses `.unwrap()` on `indexed_node` / `range_node` after `has_builtin_subslice_meaning`; panic, not accept. Noted only.
Not a risk (checked): `withholds_default_predicate` :804 uses `map_or(true, ...)` so an error withholds; `report_whole_view_write` :904 treats error as reportable; `report_consumption` :854-873 treats a missing place or a non-expression `where` fact as unestablished; `validate_literal_indexed_recast_footprint` returns `None` on any failure.

---------------------------------------------------------------------------------------------------------------------

## Coverage

- `omega-rust/psi/compilation/validation/src/quotients.rs` lines 1-8176: read in full (8176 lines), in chunks of 1000.
- `omega-rust/psi/compilation/validation/src/recasts.rs` lines 1-5438: read in full (5438 lines), in chunks of 1000/500.
- Skimmed: none. Not read: none. Total read = 13614 lines.
- Outside-unit reading (for "same job elsewhere" and caller counts only): `expression_types_and_fields.rs:155-244`, `equality_and_exhaustiveness.rs:2410-2439`, `call_cycles.rs:3450-3489`, `lib.rs:1825-1894`, and Grep hits.

## Five things a maintainer most needs to know about this unit

1. `quotients.rs` is mostly verification plumbing for ONE feature path; at validation time (`QuotientRequestAdmission::AtValidation`, :8117) every quotient operation is rejected by `reject_quotient_operation_requests` (:622), even when the plan fully verifies. Success is only reachable through `admit_checked_quotient_requests` (:8126, 1 caller in `checking.rs:1102`), which needs `request_admission::extract` to produce a `QuotientCorrespondenceJudgment`.
2. Equivalence authority and law shapes are decided by text: name `"Equivalence"`, toolchain path `"relation.omg"`, parent names, requirement names `reflexive/symmetric/transitive`, and `$left/$right/$other`-substituted rendered expression strings (:228-233, :286-322, :416-450).
3. Proof-fact equality is a `(rendered, trace)` string pair built with `format!` and `{:?}` of handles (:3462-3637); the `trace` half omits operators for `Binary`/`Unary`, so soundness leans on `render_proof_expression` (outside this unit).
4. `recasts.rs` has two halves: (a) judgment code (`judge_scalar_recast` and helpers, :1448-2348, `validate_recasts` :5292), and (b) a layout inventory (`MutableRecordRepresentation`, :2662-4264) that is also exported as the public API later stages use for projections and source leaves (`recast_*`, 14 external call sites in total across validation, checking and lowering). `reference_formation_source` (:5185) has 47 callers outside the file.
5. Several accept-without-diagnostic exits exist in recast judgment (defer pending layout geometry :1483-1490, pending-view early return :1795-1800, machine-local pending origins :731-770); each relies on a later check that I did not trace.

## What I am unsure about

- Whether `render_proof_expression` encodes operators, access and range inclusivity, which decides whether the `trace` omissions at `quotients.rs:3586-3616` matter.
- What `machine_call_dependency_symbols` (`call_cycles.rs:3458`) collects (calls inside contracts and statements only? trait method calls? evidence calls?), which decides whether the `continue` at `quotients.rs:477-479` skips anything real.
- Whether any non-toolchain symbol can have `symbol_toolchain_relative_path == "relation.omg"`.
- Which pass re-runs recast judgment when `defers_pending_layout_geometry()` is true, and whether every pending-view consumption reaches `report_consumption`.
- Whether `PrimitiveType::scalar_byte_size()` is `None` for any primitive that `exact_scalar_representation_type` can return (affects `recasts.rs:1902-1907`).
- Whether the spelling-only primitive checks in `quotients.rs` (:4740-4745, :4803-4814, :5551-5553) can be reached by a user type named like a primitive.
- `derive_precondition_partition`'s classification (`quotients.rs:2756-2800`): I did not trace whether `subtree_for_each` with `SAVED_SOURCES` edges reaches every operand form, which affects whether a quotient-dependent fact could be filed as "fixed".

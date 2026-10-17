//! Semantic validation API. Start at `program_validation.rs` for the validation lifecycle.
//!
//! That file owns `validate_specialized_program` and the order of the passes.
//! The remaining modules are either one validator each (calls, traits,
//! domains, literals, transitions, wire) or reusable semantic queries (places,
//! expression types, definition facts) that checking and lowering also read.
//!
//! The root modules are grouped by what they validate: `machine_calls.rs`,
//! `proof_contracts.rs`, `value_custody.rs` and `declarations.rs` each open
//! one folder of validators and the queries they share.
//! `execution_projection.rs` selects executable declaration contexts without
//! granting a checked judgment; compile-time evaluation and checking share it.

pub use declarations::traits::conformance_applications;
pub mod execution_projection;
pub use declarations::operators::{
    build_operator_facts, derive_checked_operator_crash_contracts,
    derive_pre_flow_operator_selections,
};
pub use declarations::traits::refinement_fit;
pub use declarations::traits::{ProviderRegionCorrespondence, provider_region_correspondence};
pub use machine_calls::attached_method_bounds::validate_selected_attached_method_bounds;
pub use machine_calls::machine_parameters::validate_callable_parameter_refinement;
pub use machine_calls::{compiler_operations, contract_identities};
pub use proof_contracts::qualification_evidence::boundary_qualification_authorization;
pub use proof_contracts::qualification_evidence::{
    BoundaryQualificationObligation, boundary_provider_qualification_obligations,
};
pub use proof_contracts::{contract_structure, expression_owners, mathematical_applications};
pub use value_custody::partial_affine_residuals::{
    CaseSelectedCleanupRegion, CleanupCaseCondition, CleanupCaseReachability, reconstruct,
    reconstruct_case_selected_regions,
};
pub use value_custody::places::collection_operations;
pub use value_custody::places::declaration_type_reference;

pub use declarations::selection_resolution;
pub use declarations::source_completion;
pub use declarations::standard_declarations::{
    boundary_has_explicit_calling_policy, standard_calling_vocabularies,
};
use declarations::state_signatures::StateSignatureOwner;
pub use machine_calls::boundary_calls::validate_boundary_calls;
pub use machine_calls::calls::{
    call_type_arguments, closed_requirement_dispatch_is_valid,
    named_conformance_target_requirement, nested_boundary_applications_compose,
    result_initializer_call_is_admitted, selected_requirement_signature,
    unit_result_initializer_call_is_admitted, unit_return_call_is_admitted,
    unit_statement_call_is_admitted,
};
pub use machine_calls::fact_only_citations::{
    fact_only_theorem, is_fact_only_citation, is_fact_only_citation_call,
};
pub use machine_calls::static_machine_call_contracts::{
    static_machine_call_binding_bytes, static_machine_parameter_contract_bytes,
    static_machine_template_reach_contract_bytes,
};
pub use machine_calls::structural_call_custody::{
    DeclarationPredicateRow, StateScalarContractRow, StructuralResultContract,
    borrowed_slice_view_result, declaration_facts_carried, finite_arrival_parameter,
    machine_declaration_predicates, reconstruct_structural_parameter_return_claims,
    restates_structural_parameter_qualification, result_membership_carrier,
    state_declaration_predicates, state_finite_predicate, structural_call_custody,
    structural_call_result_claims, structural_claim_path, structural_owned_path,
    structural_qualification_contract, structural_result_contract,
    structural_result_projected_qualifications, structural_result_qualification_contract,
    structural_result_qualifications, structural_state_contract_scalar_predicates,
    view_result_carrier_qualifications, zeroed_byte_projected_qualifications,
};
pub use proof_contracts::arithmetic_domains::domain_intervals::{
    closed_interval_from_comparison, declared_domain_predicate_bounds,
    exact_declared_domain_carrier_interval, exact_declared_domain_interval,
};
pub use proof_contracts::arithmetic_domains::float_return_postcondition;
pub use proof_contracts::arithmetic_domains::normal_return_float_predicates;
pub use proof_contracts::arithmetic_domains::plain_exact_widening_cast;
pub use proof_contracts::arithmetic_domains::{BigIntInterval, Interval, WideInterval};
pub use proof_contracts::arithmetic_domains::{
    SelectedScalarIntervalSource, carrier_integer_range, checked_integer_binary_kind,
    frozen_declared_integer_range, integer_range_contains, selected_boolean_decision,
    selected_scalar_interval,
};
pub use proof_contracts::collection_predicates::{length_member, premises_prove_expression};
pub use proof_contracts::contract_entailment::{
    CapturedScalarObservation, captured_collection_length, captured_integer_value,
    captured_scalar_observations, closed_collection_length, closed_collection_length_at_place,
    closed_collection_length_under_matches, completed_value_proves_domain_predicate,
    completed_value_refutes_domain, unique_literal_field,
};
pub use proof_contracts::contract_results::{
    ReservedResultPlace, reserved_result_owner, reserved_result_place,
};
pub use proof_contracts::domains::{
    integer_domain_membership, scalar_domain_predicate, scalar_field_domain_predicate,
    scalar_field_restriction, scalar_membership_carrier, scalar_membership_predicates,
    scalar_predicate_domains, scalar_state_contracts_are_qualifications, scalar_type_tags,
};
pub use proof_contracts::parameter_domains::parameter_domain_constraints;
pub use proof_contracts::parameter_expression_meaning::has_builtin_parameter_bound_expression_meaning;
pub use proof_contracts::signature_ensures::{
    collect_parameter_bound_comparisons, ensures_contract_expressions,
};
use typed_trees::types::TypeReferenceHandle;
pub use value_custody::claim_frontier::{
    ClaimFrontierClaim, linear_claim_frontier, owned_receiver_claims,
    type_carries_linear_obligation,
};
pub use value_custody::constants::validate_constant_literal_integer_ranges;
pub use value_custody::content_classes::classify_program_content_classes;
pub use value_custody::content_conservation::{data_variant_name, push_structural_field};
pub use value_custody::dispatch_extraction::DispatchExtraction;
pub use value_custody::field_domains;
pub use value_custody::owned_value_source::{
    affine_owned_value_source, linear_owned_value_source, plain_owned_value_source,
    whole_owned_value_source,
};
pub use value_custody::permission_provenance::expression_permission_provenance;
pub use value_custody::placed_atomic_observing_results;
pub use value_custody::receiving_values::receiving_value;
pub use value_custody::record_local_disposition::record_local_disposition;
pub use value_custody::reference_shells::{
    ReferenceShell, SliceViewShell, byte_sequence_carrier, reference_shell, slice_view_shell,
    stored_view_shell,
};
pub use value_custody::scalar_case_constructor::{
    ScalarCaseConstructor, StructuralCaseConstructor, is_scalar_case_value,
    scalar_case_constructor, scalar_case_value_source, structural_case_constructor,
};
pub use value_custody::scalar_representation_range::scalar_representation_range;
pub use value_custody::source_liveness::statement_uses_symbol;
pub use value_custody::storage_contents::{
    ContentsRequirement, data_has_contents, has_contents, has_owned_or_shared_view_fields,
};
pub use value_custody::struct_literals::{
    ArrayWindowShape, array_window_shape, case_predicates_admit_zero,
    selected_construction_field_type, validate_value_construction,
};
pub use value_custody::structural_identity::{
    stored_structural_type_identity, stores_reference_shape, structural_identity_type,
    structural_type_identity,
};
pub use value_custody::transition_operand_moves::transition_operand_moves;
pub use value_custody::type_shapes::{
    exact_constrained_shells, exact_shells_symbolic_range_maximum,
};
pub use value_custody::wire::validate_wire_protocol;
pub use value_custody::{
    collect_executable_value_origins, visit_executable_value_occurrences,
    visit_statement_value_occurrences,
};

/// The sealed operation an [`AtomicCarrierAccess`] names.
pub use access_plans::AtomicAccessOperation;
pub use value_custody::atomic_operations::{
    AtomicCarrierAccess, atomic_assignment_carrier, atomic_load_carrier, place_is_atomic_storage,
};
pub use value_custody::places::{
    AttachedStorageRoot, StructuralScalarField, StructuralSource, assignment_target_indexes,
    assignment_target_selectors, attached_storage_root, canonical_storage_label,
    collection_length_receiver, declaration_storage_label, declared_collection_extent,
    exact_attached_field, exact_attached_storage_field, exact_data_member_field, exact_self_field,
    expression_names_attached_storage, fixed_array_type_length, has_builtin_subslice_meaning,
    mutable_byte_slice_parameter_element, place_has_builtin_coordinates, saved_dispatch_call,
    saved_dispatch_subject, structural_scalar_field, structural_scalar_field_at_coordinate,
    structural_scalar_field_in_parameters, structural_source, subslice_expression,
};

pub use proof_contracts::bound_expression_meaning::{
    exact_case_reference_owner, exact_domain_subject_type, has_builtin_binary_expression_meaning,
    has_builtin_bound_expression_meaning, has_builtin_decomposed_guard_meaning,
    has_builtin_domain_decomposed_guard_meaning, has_builtin_or_sealed_float_comparison_meaning,
    has_exact_case_membership_meaning, has_exact_data_case_membership_meaning,
    has_exact_domain_case_membership_meaning, has_exact_integer_domain_subject,
    has_exact_parameter_case_membership_meaning, sealed_float_comparison,
    sealed_float_comparison_declaration,
};
pub use proof_contracts::contextual_identity::{
    contextual_identity_subject_type, contextual_proposition_children,
};
pub use value_custody::intrinsic_boundaries::boundary_requirement_completion;
pub use value_custody::intrinsic_boundaries::checked_shape_data_declaration;
pub use value_custody::intrinsic_boundaries::exact_byte_read_result_type;
pub use value_custody::intrinsic_boundaries::exact_compiler_intrinsic_boundary_requirement;
pub use value_custody::intrinsic_boundaries::exact_external_leaf_boundary_requirement;
pub use value_custody::intrinsic_boundaries::is_canonical_process_exit_requirement;
pub use value_custody::intrinsic_boundaries::valid_boundary_completion_declaration;
pub use value_custody::intrinsic_boundaries::validate_boundary_completion_declarations;

pub use crate::declarations::declaration_visibility::validate_declaration_visibility;
pub use crate::declarations::selection_authority::{
    DeclarationSelectionAuthority, validate_authored_declaration_selections,
};
pub use crate::machine_calls::call_cycles::{
    RankRelevance, RuntimeRankBodyWitness, RuntimeRankComponents, ValidatedProofRankingRelation,
    ValidatedProofRecursiveCallSite, ValidatedProofRecursiveComponent, ValidatedProofRecursiveEdge,
    ValidatedProofRecursiveMember, ValidatedProofRecursiveTransitionLane,
    ValidatedRuntimeNaturalComponent, ValidatedRuntimeNaturalEdge, ValidatedRuntimeNaturalState,
    activation_target_state_index, internal_state_index, machine_call_dependency_symbols,
    prepare_runtime_rank_components, rank_relevance, retain_runtime_rank_component,
    retained_internal_rank_nonincrease, retained_runtime_natural_components,
    retained_runtime_recursive_components,
};
pub use crate::machine_calls::calls::{
    AssignmentStorageOrigins, AssignmentWriteTarget, CallFrameResolver, CallWriteOrigins,
    LocalWriteOrigin, LocalWriteSource, RecastSource, ReferenceResultLeafOrigin,
    ScalarStorageCoordinate, ValidatedSelfCallDescent, assignment_stored_value_type,
    frame_coordinate_path, frame_length_coordinate_path, frame_paths_overlap, operand_call_plans,
    state_reference_parameter_binding_is_stable,
};
pub use crate::machine_calls::denotational_calls::{
    admit_specification_calls, checked_value_call_candidate, expression_values_have_call_identity,
    normal_return_call_candidate, selected_operator_has_closed_value_denotation,
};
pub use crate::machine_calls::fact_call_projections::has_checked_denotational_call_identity;
pub use crate::proof_contracts::slice_ranking::slice_tail_strictly_decreases;
pub use crate::value_custody::cleanup::{
    nominal_cleanup_machine_for_type, nominal_cleanup_owner, validate_reserved_cleanup_selections,
};
pub use crate::value_custody::content_conservation::{
    ContentConservationSourcePlan, build_content_conservation_plans, content_conservation_plans,
};
pub use crate::value_custody::content_projections::build_content_projection_plans;

pub use crate::declarations::symbols::TopLevelSymbols;
pub use crate::value_custody::cleanup::{
    data_requires_nominal_drop, type_graph_requires_nominal_drop,
};
pub use crate::value_custody::data::{data_requires_establishment, type_requires_establishment};
pub use crate::value_custody::type_references::{
    StaticTypeArgumentOwner, scalar_type_refinement_predicates, scalar_value_domain_predicate,
    validate_closed_const_argument, validate_exact_const_value_encoding,
    validate_static_type_argument,
};
pub use declarations::definition_facts::build_definition_fact_plan;
pub use declarations::operators::{
    ValidatedBoundaryOperatorApplication, ValidatedBoundaryOperatorApplicationArgument,
    ValidatedBoundaryOperatorApplicationUseSite, canonical_closed_operator_realization_bytes,
    checked_operator_application_matches_realization, landed_integer_literal_type_reference,
    validate_closed_operator_application, validate_named_operator_application,
};
pub use declarations::operators::{
    build_operator_uses, checked_operator_candidate, collect_named_requirement_uses,
    collect_pre_specialization_operator_selections, comparison_operator_spelling,
    expression_type_reference_for_origin, indexed_operand_types,
    select_pending_domain_operator_meanings, token_bound_machine_call_target,
};
pub use declarations::traits::{
    DynamicImplementationSelection, DynamicSlotDemand, collect_dynamic_implementation_selections,
    collect_dynamic_slot_demands, compose_forwarded_trait_arguments, dynamic_call_demand,
    dynamic_expression_call_demand, generic_bound_operator_requirement, qualified_trait_scope,
    resolve_closed_requirement_application, resolve_dynamic_call_targets,
    resolve_selected_dynamic_call_targets, revalidate_top_level_requirement_realization,
};
pub use machine_calls::effect_inference::{
    boundary_signature_service_reach, declared_machine_invocations, declared_signature_invocations,
    fixed_installation_boundary_service_reach, has_self_forwarded_boundary_parameter,
    infer_operational_may, infer_operational_may_for_cleanup_dependencies, infer_service_reaches,
    infer_synchronous_invocations, invocation_target_label,
};
pub use machine_calls::effects::{
    AsmAuthorityAdmission, validate_asm_discharge, validate_behavior_plan,
};
pub use machine_calls::machine_parameters::{
    ValidatedNominalMachineUse, ValidatedNominalMachineUseSite,
    ValidatedRequirementCallMachineSelection, ValidatedRequirementCallSpecialization,
    ValidatedRequirementCallTypeBinding, ValidatedStaticMachineSelections,
    closed_static_call_type_bindings, validate_static_machine_selections,
};
pub use machine_calls::machine_specialization_identity::machine_specialization_matches_template_identity;
pub use machine_calls::result_overloads::{
    bind_float_conversion_casts, resolve_named_result_overloads,
};
pub use machine_calls::static_arguments;
pub use machine_calls::static_bindings;
pub use proof_contracts::default_domains::{
    OpenInvariantCrashSite, data_float_predicate, data_predicates_admit_zero,
    standing_facts_prove_requirement,
};
pub use proof_contracts::float_projection_bindings::semantic_operations::{
    exact_toolchain_float_semantic_contract, float_semantic_application_for_parameters,
};
pub use proof_contracts::float_projection_bindings::semantic_values::closed_float_semantic_integer_operand;
pub use proof_contracts::float_projection_bindings::semantic_values::exact_toolchain_float_format_const;
pub use proof_contracts::float_projection_bindings::{
    exact_toolchain_float_projection_contract, exact_toolchain_float_projection_primitive,
    float_boolean_semantic_contract, float_format_conversion_contract,
    float_integer_conversion_bounds, float_integer_conversion_contract,
    float_result_semantic_contract, float_saturation_divisor_parameter,
    integer_float_conversion_bounds, is_exact_toolchain_float_meaning_type,
    numeric_float_conversion_contract,
};
pub use proof_contracts::float_projection_invocations::exact_projection_operation as exact_toolchain_float_projection_operation;
pub use proof_contracts::properties::{
    DeclaredPropertyRequirement, OpaqueDataPropertyReceipt, OpaqueDataPropertyReceiptKind,
    TypeCarryRequirements, declared_property_requirements, effective_data_carry_policy,
    effective_type_carry_policy, effective_type_carry_requirements,
    type_satisfies_declared_property,
};
pub use proof_contracts::proposition_entailment::select_named_evidence_implementation;
pub use proof_contracts::quotients::{
    CheckedTerminationOracle, QuotientRequestAdmission, ValidatedQuotientFormation,
    ValidatedQuotientOperation, admit_checked_quotient_requests,
    checked_quotient_relation_identity, validate_quotient_formations,
};
pub use value_custody::expression_types::argument_matches_type_reference_handle as checked_argument_matches_type_reference;
pub use value_custody::expression_types::bounded_byte_buffer_capacity;
pub use value_custody::expression_types::match_subject_primitive_type;
pub use value_custody::expression_types::validate_match_dispatch;
pub use value_custody::expression_types::{
    MatchCaseDispatch, MatchCaseDispatchArm, MatchCaseSubject, match_case_dispatch,
};
pub use value_custody::expression_types::{
    arithmetic_result_type_reference, expression_result_type_reference,
    join_result_type_references, parameter_expression_result_type_reference,
};
pub use value_custody::literals::builtin_constant_array_projection_type;
pub use value_custody::literals::declared_constant_array_type;
pub use value_custody::literals::evaluate_anonymous_numeric_comparison;
pub use value_custody::literals::evaluate_anonymous_numeric_expression;
pub use value_custody::literals::has_anonymous_numeric_results;
pub use value_custody::literals::has_anonymous_operator_meaning;
pub use value_custody::literals::land_anonymous_integer_expression;
/// The declared type of a simple place argument (bare name / `self.field`,
/// through the `&mut` marker), WITH its Constrained shells -- exposed for the
/// typed-trees machine-monomorphization pass's param-position inference.
pub use value_custody::literals::land_float_literal_destinations;
pub use value_custody::literals::land_integer_value;
pub use value_custody::literals::select_anonymous_numeric_match_arm;
pub use value_custody::literals::{
    ScalarArrayElements, closed_array_store_type, closed_constant_array_elements,
    closed_literal_array_elements, closed_record_scalar_projection, is_closed_primitive_array_type,
    peel_arithmetic_policy, scalar_array_elements, scalar_array_preserves_memberships,
};
pub use value_custody::places::{assignment_value_type, declared_place_type_raw};
pub use value_custody::places::{
    declared_carriers_match, peel_declared_constraints, unwrapped_type_reference,
    zeroed_structural_parts,
};
pub use value_custody::recasts::{
    RecastLeafSegment, RecastRecordSourceLeaf, RecastRuntimeIndexGeometry, RecastScalarGeometry,
    RecastScalarLayout, RecastScalarProjection, ReferenceFormationInterpretation,
    ReferenceFormationSource, SliceRecastByteRegion, SliceRecastSource,
    ValidatedLiteralIndexedRecastFootprint, recast_record_source_leaf, recast_scalar_layout,
    recast_scalar_path_geometry, recast_scalar_path_projection, recast_scalar_projection,
    reference_formation_source, reference_view_storage, slice_recast_byte_region,
    slice_recast_source, validate_literal_indexed_recast_footprint,
    validate_runtime_indexed_recast_source,
};
pub use value_custody::type_references::normalize_open_index_expressions;
pub use value_custody::type_references::{
    closed_float_range_endpoint, closed_integer_range_bound, closed_integer_range_maximum,
    constrained_range_bounds, declared_integer_range, ieee_float_range_ordered,
    is_arithmetic_policy_only_integer, is_plain_scalar_storage,
    scalar_result_contract_is_represented, unrestricted_builtin_primitive,
};

pub use proof_contracts::arithmetic_domains::CollectionBoundsObservations;
pub use proof_contracts::arithmetic_domains::arrival_integer_expression_bounds;
pub use proof_contracts::arithmetic_domains::builtin_monotonic_integer_update_bounds;
pub use proof_contracts::arithmetic_domains::enforced_integer_type_bounds;
pub use proof_contracts::arithmetic_domains::guarded_call_requires_proven;
pub use proof_contracts::arithmetic_domains::immutable_integer_expression_bounds;
pub use proof_contracts::arithmetic_domains::integer_widen_is_total;
pub use proof_contracts::arithmetic_domains::normal_return_integer_bounds;
pub use proof_contracts::arithmetic_domains::normal_return_length_collection;
pub use proof_contracts::arithmetic_domains::occurrence_integer_expression_bounds;
pub use proof_contracts::arithmetic_domains::occurrence_integer_expression_fits_range;
pub use proof_contracts::arithmetic_domains::state_parameter_integer_interval;
pub use proof_contracts::arithmetic_domains::validate_ordered_requirement_call_totality;
pub use proof_contracts::arithmetic_domains::{
    standing_integer_bounds, stored_integer_bounds, stored_integer_source_may_require_fit,
    stored_integer_source_projection, stored_integer_source_projections,
};
pub use proof_contracts::contract_entailment::integer_embedding_sources_equal;
pub use proof_contracts::contract_entailment::is_arm_pattern_marker;
pub use proof_contracts::contract_entailment::selected_value_call_requirement_entailed;
pub use proof_contracts::contract_entailment::structural_call_requirement_entailed;
pub use proof_contracts::contract_entailment::{
    ArithmeticCallObservation, RankArrivalCompatibility, RankArrivalCorrespondence,
    RankArrivalDemand, RankCoordinateBinding, RankCoordinateTransport, RankLocalCapture,
    RankPremiseCoordinate, RankPremiseInput, RankPremiseObservation, RankProtectedPath,
    RankReceivingCoordinate, RankStateBindings, RankSubjectCoordinate, RankValueDerivation,
    RankingRangeEdgeProof, RankingRangeMeasure, RankingRangePremises, RankingRangeState,
    StateEntryCorrespondence, arithmetic_entry_requirement_is_covered,
    discover_rank_arrival_correspondence, frame_preserves_rank_premises, protect_rank_premise,
    prove_arithmetic_call_requirement, prove_captured_arithmetic_call_requirement,
    prove_ranking_range_edge, prove_ranking_range_entry, prove_ranking_range_transition,
    rank_protected_path, ranking_coordinate_type, ranking_range_arrival_preserves_roles,
    ranking_range_premise_symbols, ranking_range_required_coordinates,
    ranking_range_required_symbols, ranking_range_state_premise_coordinates,
    ranking_slice_length_observation, select_rank_arrival_correspondence,
};
pub use proof_contracts::contract_entailment::{
    ComputationBodyShape, DeclaredIdentityView, DeclaredScalarView, MeasureBodyShape,
    ProjectionStep, ScalarViewComputation, computation_body_shape, declared_scalar_view,
    find_declared_measure, identity_subject_matches, measure_body_shape,
    measure_constraints_cover_subject,
};
pub use proof_contracts::contract_entailment::{
    InheritedRequirementApplication, inherited_requirement_proposition_application,
    inherited_requirement_proposition_label, inherited_satisfier_parameters,
};
pub use proof_contracts::contract_entailment::{
    InheritedRequirementEdge, transparent_call_requirement_entailed,
    transparent_proposition_application_entailed,
};
pub use proof_contracts::contract_entailment::{
    MatchedLawGuarantee, matched_machine_law_guarantees,
};
pub use proof_contracts::contract_entailment::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticComparison,
    ScopedArithmeticExpression, ScopedArithmeticHypothesis, ScopedArithmeticValue,
    StrictArithmeticBindingValue, StrictArithmeticExpressionBinding,
    StrictArithmeticImplicationJudgment, StrictArithmeticSymbolBinding,
    scoped_arithmetic_comparisons_unsatisfiable, scoped_arithmetic_implication,
    scoped_arithmetic_value_bounds, scoped_arithmetic_values_relation,
    strict_arithmetic_expression_implication,
};
pub use proof_contracts::contract_entailment::{
    anonymous_integer_math_literal, proof_integer_expression,
};
pub use proof_contracts::contract_entailment::{
    call_contract_parameter_position, instantiated_value_call_requires_proven,
};
pub use proof_contracts::default_domains::{
    case_where_field_intervals, data_where_field_intervals, impossible_variant,
};
pub use proof_contracts::float_projection_invocations::{
    ValidatedFloatMeaningEqualityProposition, ValidatedFloatMeaningProjectionInvocation,
    float_static_call_resolution,
};
pub use proof_contracts::immutable_integer_bounds::{
    ImmutableBoundLookup, immutable_integer_bound_sum, immutable_integer_bound_symbol_offset,
    immutable_integer_bound_value_symbol, indexed_integer_bound_root,
    mutable_integer_bound_storage_symbol, normalize_immutable_integer_bound_expression,
    normalize_immutable_integer_bound_to_usize, projected_integer_bound_root,
    projected_integer_bound_subject_type,
};
pub use proof_contracts::proof_embeddings::{
    ValidatedIntegerEmbeddingCall, integer_embedding_argument,
};

pub mod borrow_contracts;
mod declarations;
mod type_regions;
pub(crate) use optimization::digest_encoder;
pub mod expression_traversal;
mod frozen_program;
mod layout_checking;
pub use layout_checking::{LayoutCheckingContext, layout_checking_targets};
mod machine_calls;
mod program_validation;
mod proof_contracts;
pub mod substitutions;
mod type_applications;
mod value_custody;

// The front-end pipeline the tests run, shared with the `suite` integration
// target that includes the same file; see its module documentation.
pub use frozen_program::{
    CheckingContext, CheckingInput, CheckingMemos, CheckingProjection, proof_only_classification,
};
pub use machine_calls::effect_inference::PreparedEffectPlans;
pub use machine_calls::reference_result_custody;
pub use program_validation::{
    ExactIntegerCastFact, ExpressionInterval, OpaquePropertyValidation, ProgramValidation,
    ProgramValidationFacts, ProvenContractExpressions, admit_prepared_program_effects,
    checked_operator_contract_snapshot, expression_interval, prepare_specialized_program,
    validate_checked_operator_realization_contract, validate_generic_machine_contract_entailment,
    validate_program,
};

pub use proof_contracts::scalar_boundary_guarantees::derive_scalar_boundary_guarantees;

// Exact typed-program queries are independently owned here so preparation
// and immutable checking share declaration identity and occurrence order.
pub use expression_traversal::{
    append_fact_expression_roots, collect_contract_facts, collect_expression_tree,
    collect_statement_expression_trees, collect_transition_target_expression_trees,
    is_single_segment_name_path, single_segment_name_symbol,
};
pub use machine_calls::calls::{
    CallSite, call_is_unconditional, call_receiver_parts, call_site_argument_expressions,
    call_site_evidence_arguments, call_target_parameters, call_target_return_type,
    call_target_state, call_target_symbol, call_target_type_parameters,
    collect_call_sites_in_statement, find_call_site, find_call_site_in_state, find_machine,
    find_machine_by_entry_state, find_machine_head, find_state, find_state_for_exit,
    find_state_in_machine, find_state_on_machine, machine_parameter_signature_symbol,
    machine_state_by_symbols, machine_state_count, receiver_can_dispatch_to_machine,
    resolve_name_path_member_symbol, resolve_state_call_target,
    statement_call_can_dispatch_to_machine, statement_call_receiver_members,
    statement_call_receiver_path, terminal_callable_target, trait_machine_signature_symbol,
    transition_call_target,
};
pub use value_custody::places::{
    CanonicalPlace, RuntimeFieldSubject, canonical_place_from_expression,
    canonical_place_from_expression_in_state, canonical_place_source_type_application,
    canonical_place_type_reference, collection_element_type_reference,
    contextual_canonical_place_from_expression, data_member_symbol_by_name, declared_data_type,
    effective_member_symbol, effective_member_symbol_from_type_symbol,
    expression_place_type_reference, expression_root_symbol, expression_type_reference_in_state,
    expression_type_symbol, first_valid_name_path_symbol, index_place_segment,
    loan_carrier_has_owned_storage, machine_symbol_from_type_reference_handle,
    nearest_scoped_local, project_type_reference_from_segments, projected_statement_receiver_place,
    projected_statement_receiver_type_reference, push_field_place_segments, receiver_projections,
    resolve_member_symbol_from_type_symbol, runtime_field_subject, state_scoped_name_symbol,
    structural_member_identity, symbol_type_symbol, unique_data_definition_by_symbol,
    whole_collection_view_source,
};

/// Walk a type-reference table once into a positional row vector: row 0 is
/// the classification of the invalid handle, and every named index answers
/// its own row's classification.
pub(crate) fn classify_type_references<R: Clone, F>(
    program: &dyn crate::CheckingInput,
    classify: impl Fn(&dyn crate::CheckingInput, TypeReferenceHandle) -> R,
    with_rows: impl FnOnce(Vec<R>) -> F,
) -> F {
    let table = &program.type_reference_table;
    let highest = table
        .type_reference_rows()
        .map(|(handle, _)| handle.arena_index())
        .max()
        .unwrap_or(0);
    let mut rows = vec![classify(program, TypeReferenceHandle::invalid()); highest as usize + 1];
    for (handle, _) in table.type_reference_rows() {
        rows[handle.arena_index() as usize] = classify(program, handle);
    }
    with_rows(rows)
}

pub use value_custody::atomic_fences::{
    AtomicFenceContract, exact_toolchain_atomic_fence_contract,
};

pub use proof_contracts::instruction_effects::{
    InstructionEffectContract, instruction_effect_contract, instruction_effect_model,
};

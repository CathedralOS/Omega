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

pub use declarations::standard_declarations::standard_calling_traits;
use declarations::state_signatures::StateSignatureOwner;
pub use machine_calls::calls::{
    named_conformance_target_requirement, result_initializer_call_is_supported,
    unit_result_initializer_call_is_supported, unit_return_call_is_supported,
    unit_statement_call_is_supported,
};
pub use machine_calls::static_machine_call_contracts::{
    static_machine_call_binding_bytes, static_machine_parameter_contract_bytes,
    static_machine_template_reach_contract_bytes, validate_static_machine_call_contracts,
    validate_static_machine_parameter_contracts,
};
pub use machine_calls::structural_call_custody::{
    reconstruct_structural_call_custody, reconstruct_structural_parameter_return_claims,
    structural_claim_path, structural_result_projected_qualifications,
    structural_result_qualifications, structural_state_contract_scalar_predicates,
    structural_state_contracts_are_parameter_qualifications,
};
pub use proof_contracts::contract_results::{
    ReservedResultPlace, reserved_result_owner, reserved_result_place,
};
pub use proof_contracts::domains::{scalar_state_contracts_are_qualifications, scalar_type_tags};
pub use value_custody::claim_frontier::{ClaimFrontierClaim, linear_claim_frontier};
pub use value_custody::owned_value_source::{
    affine_owned_value_source, linear_owned_value_source, plain_owned_value_source,
};
pub use value_custody::permission_provenance::expression_permission_provenance;
pub use value_custody::record_local_disposition::record_local_disposition;
pub use value_custody::scalar_case_constructor::{
    ScalarCaseConstructor, is_fresh_scalar_case_value, is_scalar_case_value,
    scalar_case_constructor, scalar_case_value_source,
};
pub use value_custody::scalar_representation_range::scalar_representation_range;
pub use value_custody::storage_contents::{
    has_cleanup_owned_contents, has_linear_owned_contents, has_plain_owned_contents,
    has_plain_owned_contents_with_numeric_constraints, has_plain_owned_contents_with_substitutions,
    has_stable_observable_contents,
};

pub use value_custody::locals::receiver_allows_mutation;
pub use value_custody::places::{
    LocalScalarRecordField, collection_length_receiver, exact_attached_field,
    exact_data_member_field, exact_self_field, has_builtin_subslice_meaning,
    local_scalar_record_field, place_has_builtin_coordinates,
};

pub use proof_contracts::bound_expression_meaning::{
    exact_case_reference_owner, has_builtin_binary_expression_meaning,
    has_builtin_bound_expression_meaning, has_builtin_decomposed_guard_meaning,
    has_exact_case_membership_meaning, has_exact_data_case_membership_meaning,
    has_exact_domain_case_membership_meaning, has_exact_parameter_case_membership_meaning,
};
pub use value_custody::intrinsic_boundaries::exact_byte_read_result_type;
pub use value_custody::intrinsic_boundaries::exact_compiler_intrinsic_boundary_requirement;

pub use crate::declarations::declaration_visibility::validate_declaration_visibility;
pub use crate::machine_calls::call_cycles::{
    ValidatedProofRankingRelation, ValidatedProofRecursiveCallSite,
    ValidatedProofRecursiveComponent, ValidatedProofRecursiveEdge, ValidatedProofRecursiveMember,
    ValidatedProofRecursiveTransitionLane, machine_call_dependency_symbols,
    validated_runtime_recursive_components,
};
pub use crate::machine_calls::calls::{
    AssignmentWriteTarget, CallFrameResolver, LocalWriteOrigin, frame_paths_overlap,
    generic_bound_call_requirement, generic_bound_value_call_requirement,
    state_reference_parameter_binding_is_stable,
};
pub use crate::proof_contracts::slice_ranking::slice_tail_strictly_decreases;
pub use crate::value_custody::cleanup::validate_reserved_cleanup_selections;
pub use crate::value_custody::content_conservation::{
    ContentConservationSourcePlan, build_content_conservation_plans,
};
pub use crate::value_custody::content_projections::build_content_projection_plans;

pub use crate::declarations::symbols::TopLevelSymbols;
pub use crate::value_custody::cleanup::{
    data_requires_nominal_drop, type_graph_requires_nominal_drop,
};
pub use crate::value_custody::data::data_requires_establishment;
pub use crate::value_custody::type_references::{
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
pub use declarations::traits::{
    DynamicConformanceSelection, DynamicDescriptorStorage, collect_dynamic_conformance_selections,
    collect_dynamic_descriptor_storages, compose_forwarded_trait_arguments,
    generic_bound_operator_requirement, resolve_dynamic_call_targets,
    revalidate_top_level_requirement_realization,
};
pub use machine_calls::effect_inference::{
    declared_machine_invocations, declared_signature_invocations,
    fixed_installation_boundary_service_reach, has_self_forwarded_boundary_parameter,
    infer_operational_may, infer_service_reaches, infer_synchronous_invocations,
    invocation_target_label,
};
pub use machine_calls::effects::{validate_asm_discharge, validate_behavior_plan};
pub use machine_calls::machine_parameters::{
    ValidatedNominalMachineUse, ValidatedNominalMachineUseSite,
    ValidatedRequirementCallMachineSelection, ValidatedRequirementCallSpecialization,
    ValidatedRequirementCallTypeBinding, ValidatedStaticMachineSelections,
    closed_static_call_type_bindings, validate_static_machine_selections,
    validate_static_machine_selections_with_facts,
};
pub use machine_calls::machine_specialization_identity::{
    machine_specialization_matches_template_identity,
    recompute_checked_machine_specialization_commitment,
    validate_checked_machine_specialization_commitments,
};
pub use machine_calls::result_overloads::resolve_named_result_overloads;
pub use proof_contracts::default_domains::{
    OpenInvariantCrashSite, build_open_invariant_crash_sites,
};
pub use proof_contracts::float_projection_bindings::{
    exact_toolchain_float_projection_contract, exact_toolchain_float_projection_primitive,
    is_exact_toolchain_float_meaning_type,
};
pub use proof_contracts::properties::{
    DeclaredPropertyRequirement, OpaqueDataPropertyReceipt, OpaqueDataPropertyReceiptKind,
    declared_property_requirements, effective_data_carry_policy, effective_type_carry_policy,
    type_satisfies_declared_property,
};
pub use proof_contracts::proposition_entailment::select_subjectless_evidence_conformance;
pub use proof_contracts::quotients::{
    NonExecutableQuotientCorrespondenceBatch, ValidatedQuotientFormation,
    extract_non_executable_quotient_correspondences, validate_quotient_formations,
};
pub use value_custody::expression_types::argument_matches_type_reference_handle as checked_argument_matches_type_reference;
pub use value_custody::expression_types::bounded_byte_buffer_capacity;
pub use value_custody::expression_types::match_subject_primitive_type;
pub use value_custody::expression_types::validate_match_dispatch;
pub use value_custody::expression_types::{
    arithmetic_result_type_reference, expression_result_type_reference, join_result_type_references,
};
pub use value_custody::expression_types::{
    fresh_payloadless_case, is_fresh_payloadless_structural_value,
};
pub use value_custody::literals::builtin_constant_array_projection_type;
pub use value_custody::literals::declared_constant_array_type;
pub use value_custody::literals::evaluate_anonymous_numeric_comparison;
pub use value_custody::literals::evaluate_anonymous_numeric_comparison_with_selected_match_arms;
pub use value_custody::literals::evaluate_anonymous_numeric_expression_with_selected_match_arms;
pub use value_custody::literals::has_anonymous_numeric_results;
pub use value_custody::literals::has_anonymous_operator_meaning;
pub use value_custody::literals::land_anonymous_integer_expression;
pub use value_custody::literals::land_anonymous_integer_expression_with_selected_match_arms;
pub use value_custody::literals::land_anonymous_integer_expression_with_warning;
/// The declared type of a simple place argument (bare name / `self.field`,
/// through the `&mut` marker), WITH its Constrained shells -- exposed for the
/// typed-trees machine-monomorphization pass's param-position inference.
pub use value_custody::literals::land_float_literal_destinations;
pub use value_custody::literals::land_integer_value;
pub use value_custody::literals::select_anonymous_numeric_match_arm;
pub use value_custody::literals::{
    ScalarArrayElements, closed_constant_array_elements, closed_literal_array_elements,
    closed_record_scalar_projection, is_closed_primitive_array_type, scalar_array_elements,
};
pub use value_custody::placed_views::{
    CheckedAtomicResidentAccess, CheckedAtomicResidentAccessRejection,
    bind_checked_atomic_resident_access,
};
pub use value_custody::places::declared_place_type_raw;
pub use value_custody::places::unwrapped_type_reference;
pub use value_custody::recasts::{
    ValidatedLiteralIndexedRecastFootprint, validate_literal_indexed_recast_footprint,
};
pub use value_custody::type_references::normalize_open_index_expressions;
pub use value_custody::type_references::{
    closed_float_range_endpoint, closed_integer_range_bound, closed_integer_range_maximum,
    closed_scalar_result_range, declared_integer_range, ieee_float_range_ordered,
    is_arithmetic_policy_only_integer,
};

pub use proof_contracts::arithmetic_domains::arrival_integer_expression_bounds;
pub use proof_contracts::arithmetic_domains::builtin_monotonic_integer_update_bounds;
pub use proof_contracts::arithmetic_domains::enforced_integer_type_bounds;
pub use proof_contracts::arithmetic_domains::immutable_integer_expression_bounds;
pub use proof_contracts::arithmetic_domains::integer_widen_is_total;
pub use proof_contracts::arithmetic_domains::validate_ordered_requirement_call_totality;
pub use proof_contracts::contract_entailment::integer_embedding_sources_equal;
pub use proof_contracts::contract_entailment::is_arm_pattern_marker;
pub use proof_contracts::contract_entailment::transparent_proposition_application_entailed;
pub use proof_contracts::contract_entailment::{
    ComputationBodyShape, DeclaredIdentityView, DeclaredScalarView, MeasureBodyShape,
    ProjectionStep, ScalarViewComputation, computation_body_shape, declared_identity_view,
    declared_scalar_view, find_declared_measure, identity_subject_matches, measure_body_shape,
    measure_constraints_cover_subject, unwrap_constraint_shells,
};
pub use proof_contracts::contract_entailment::{
    InheritedRequirementApplication, inherited_requirement_proposition_application,
    inherited_requirement_proposition_label, inherited_satisfier_parameters,
};
pub use proof_contracts::contract_entailment::{
    MatchedLawGuarantee, matched_machine_law_guarantees,
};
pub use proof_contracts::contract_entailment::{
    RankingRangeEdgeProof, RankingRangeMeasure, RankingRangePremises, RankingRangeState,
    arithmetic_entry_requirement_is_covered, discover_state_entry_mappings,
    discover_state_entry_mappings_preferring, prove_arithmetic_call_requirement,
    prove_ranking_range_edge, prove_ranking_range_entry, prove_ranking_range_transition,
    ranking_range_premise_symbols, ranking_range_required_symbols,
};
pub use proof_contracts::contract_entailment::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticBindingValue,
    StrictArithmeticExpressionBinding, StrictArithmeticImplicationJudgment,
    StrictArithmeticSymbolBinding, scoped_arithmetic_implication,
    strict_arithmetic_expression_implication,
    strict_arithmetic_expression_implication_with_arguments,
};
pub use proof_contracts::float_projection_invocations::{
    ValidatedFloatMeaningEqualityProposition, ValidatedFloatMeaningProjectionInvocation,
};
pub use proof_contracts::immutable_integer_bounds::{
    immutable_integer_bound_sum, immutable_integer_bound_symbol_offset,
    immutable_integer_bound_value_symbol, normalize_immutable_integer_bound_expression,
    normalize_immutable_integer_bound_to_usize,
};
pub use proof_contracts::proof_embeddings::{
    ValidatedIntegerEmbeddingCall, integer_embedding_argument,
};

mod declarations;
mod machine_calls;
mod program_validation;
mod proof_contracts;
mod value_custody;
pub use machine_calls::reference_result_custody;
pub use program_validation::{
    ContractEntailmentStandDown, ContractEntailmentStandDownReason, ExactIntegerCastFact,
    OpaquePropertyValidation, ProgramValidation, ProgramValidationFacts,
    checked_operator_contract_snapshot, collect_contract_entailment_stand_downs,
    proven_machine_contract_expressions, validate_checked_operator_realization_contract,
    validate_generic_machine_contract_entailment, validate_program, validate_specialized_program,
};

//! Semantic validation API. Start at `program_validation.rs` for the validation lifecycle.

mod arithmetic_domains;
mod bound_expression_meaning;
mod call_cycles;
mod callable_overloads;
mod standard_declarations;
pub use standard_declarations::standard_calling_traits;
mod calls;
pub use calls::{
    named_conformance_target_requirement, result_initializer_call_is_supported,
    unit_result_initializer_call_is_supported, unit_return_call_is_supported,
    unit_statement_call_is_supported,
};
mod cleanup;
mod constants;
mod content_conservation;
mod content_projections;
mod contract_entailment;
mod contract_results;
pub use contract_results::{ReservedResultPlace, reserved_result_owner, reserved_result_place};
mod data;
mod declaration_visibility;
mod default_domains;
mod definition_facts;
mod denotational_calls;
mod destructure;
mod domain_weakening;
mod domains;
pub use domains::{scalar_state_contracts_are_qualifications, scalar_type_tags};
mod effect_inference;
mod effects;
mod expression_types;
mod fact_call_projections;
mod float_projection_bindings;
mod float_projection_invocations;
mod immutable_integer_bounds;
mod intrinsic_boundaries;
mod invocations;
mod literals;
mod locals;
mod machine_data;
mod machine_parameters;
mod machine_specialization_identity;
mod operators;
mod owned_value_source;
pub use owned_value_source::plain_owned_value_source;
mod record_local_disposition;
pub use record_local_disposition::record_local_disposition;
mod permission_provenance;
pub use permission_provenance::expression_permission_provenance;
mod scalar_case_constructor;
mod storage_contents;
pub use scalar_case_constructor::{
    ScalarCaseConstructor, is_fresh_scalar_case_value, is_scalar_case_value,
    scalar_case_constructor, scalar_case_value_source,
};
pub use storage_contents::{
    has_plain_owned_contents, has_plain_owned_contents_with_numeric_constraints,
    has_plain_owned_contents_with_substitutions, has_stable_observable_contents,
};
mod placed_views;
mod places;
mod plan_laid;
mod proof_embeddings;
mod proof_facts;
mod proof_only_faces;
mod properties;
mod proposition_entailment;
mod qualification_evidence;
mod quotients;
mod recasts;
mod relevance;
mod result_overloads;
mod scalar_representation_range;
mod slice_ranking;
mod state_signatures;
use state_signatures::StateSignatureOwner;
mod static_machine_call_contracts;
pub use static_machine_call_contracts::{
    static_machine_call_binding_bytes, static_machine_parameter_contract_bytes,
    static_machine_template_reach_contract_bytes, validate_static_machine_call_contracts,
    validate_static_machine_parameter_contracts,
};
pub mod reference_result_custody;
mod struct_literals;
mod structural_call_custody;
pub use structural_call_custody::{
    reconstruct_structural_call_custody, reconstruct_structural_parameter_return_claims,
    structural_claim_path, structural_result_projected_qualifications,
    structural_result_qualifications, structural_state_contracts_are_parameter_qualifications,
};
mod symbols;
mod traits;
mod transitions;
mod type_references;
mod wire;
pub use scalar_representation_range::scalar_representation_range;
mod write_only_borrows;

pub use locals::receiver_allows_mutation;
pub use places::{
    LocalScalarRecordField, collection_length_receiver, exact_attached_field,
    exact_data_member_field, exact_self_field, has_builtin_subslice_meaning,
    local_scalar_record_field, place_has_builtin_coordinates,
};

pub use bound_expression_meaning::{
    exact_case_reference_owner, has_builtin_binary_expression_meaning,
    has_builtin_bound_expression_meaning, has_builtin_decomposed_guard_meaning,
    has_exact_case_membership_meaning, has_exact_data_case_membership_meaning,
    has_exact_domain_case_membership_meaning, has_exact_parameter_case_membership_meaning,
};
pub use intrinsic_boundaries::exact_byte_read_result_type;
pub use intrinsic_boundaries::exact_compiler_intrinsic_boundary_requirement;

pub use crate::call_cycles::{
    ValidatedProofRankingRelation, ValidatedProofRecursiveCallSite,
    ValidatedProofRecursiveComponent, ValidatedProofRecursiveEdge, ValidatedProofRecursiveMember,
    ValidatedProofRecursiveTransitionLane, machine_call_dependency_symbols,
    validated_runtime_recursive_components,
};
pub use crate::calls::{
    AssignmentWriteTarget, CallFrameResolver, LocalWriteOrigin, frame_paths_overlap,
    generic_bound_call_requirement, generic_bound_value_call_requirement,
    state_reference_parameter_binding_is_stable,
};
pub use crate::cleanup::validate_reserved_cleanup_selections;
pub use crate::content_conservation::{
    ContentConservationSourcePlan, build_content_conservation_plans,
};
pub use crate::content_projections::build_content_projection_plans;
pub use crate::declaration_visibility::validate_declaration_visibility;
pub use crate::slice_ranking::slice_tail_strictly_decreases;

pub use crate::cleanup::{data_requires_nominal_drop, type_graph_requires_nominal_drop};
pub use crate::data::data_requires_establishment;
pub use crate::symbols::TopLevelSymbols;
pub use crate::type_references::{
    validate_closed_const_argument, validate_exact_const_value_encoding,
};
pub use default_domains::{OpenInvariantCrashSite, build_open_invariant_crash_sites};
pub use definition_facts::build_definition_fact_plan;
pub use effect_inference::{
    declared_machine_invocations, declared_signature_invocations,
    fixed_installation_boundary_service_reach, has_self_forwarded_boundary_parameter,
    infer_operational_may, infer_service_reaches, infer_synchronous_invocations,
    invocation_target_label,
};
pub use effects::{validate_asm_discharge, validate_behavior_plan};
pub use expression_types::argument_matches_type_reference_handle as checked_argument_matches_type_reference;
pub use expression_types::bounded_byte_buffer_capacity;
pub use expression_types::expression_result_type_reference;
pub use expression_types::match_subject_primitive_type;
pub use expression_types::validate_match_dispatch;
pub use expression_types::{fresh_payloadless_case, is_fresh_payloadless_structural_value};
pub use float_projection_bindings::{
    exact_toolchain_float_projection_contract, exact_toolchain_float_projection_primitive,
    is_exact_toolchain_float_meaning_type,
};
pub use literals::builtin_constant_array_projection_type;
pub use literals::declared_constant_array_type;
pub use literals::evaluate_anonymous_numeric_comparison;
pub use literals::evaluate_anonymous_numeric_comparison_with_selected_match_arms;
pub use literals::evaluate_anonymous_numeric_expression_with_selected_match_arms;
pub use literals::has_anonymous_numeric_results;
pub use literals::has_anonymous_operator_meaning;
pub use literals::land_anonymous_integer_expression;
pub use literals::land_anonymous_integer_expression_with_selected_match_arms;
pub use literals::land_anonymous_integer_expression_with_warning;
/// The declared type of a simple place argument (bare name / `self.field`,
/// through the `&mut` marker), WITH its Constrained shells -- exposed for the
/// typed-trees machine-monomorphization pass's param-position inference.
pub use literals::land_float_literal_destinations;
pub use literals::land_integer_value;
pub use literals::select_anonymous_numeric_match_arm;
pub use literals::{
    ScalarArrayElements, closed_constant_array_elements, closed_literal_array_elements,
    closed_record_scalar_projection, is_closed_primitive_array_type, scalar_array_elements,
};
pub use machine_parameters::{
    ValidatedNominalMachineUse, ValidatedNominalMachineUseSite, closed_static_call_type_bindings,
    validate_static_machine_selections, validate_static_machine_selections_with_facts,
};
pub use machine_specialization_identity::{
    machine_specialization_matches_template_identity,
    recompute_checked_machine_specialization_commitment,
    validate_checked_machine_specialization_commitments,
};
pub use operators::{
    ValidatedBoundaryOperatorApplication, ValidatedBoundaryOperatorApplicationArgument,
    ValidatedBoundaryOperatorApplicationUseSite, canonical_closed_operator_realization_bytes,
    checked_operator_application_matches_realization, landed_integer_literal_type_reference,
    validate_closed_operator_application, validate_named_operator_application,
};
pub use placed_views::{
    CheckedAtomicResidentAccess, CheckedAtomicResidentAccessRejection,
    bind_checked_atomic_resident_access,
};
pub use places::declared_place_type_raw;
pub use places::unwrapped_type_reference;
pub use properties::{
    DeclaredPropertyRequirement, OpaqueDataPropertyReceipt, OpaqueDataPropertyReceiptKind,
    declared_property_requirements, effective_data_carry_policy, effective_type_carry_policy,
    type_satisfies_declared_property,
};
pub use proposition_entailment::select_subjectless_evidence_conformance;
pub use quotients::{
    NonExecutableQuotientCorrespondenceBatch, ValidatedQuotientFormation,
    extract_non_executable_quotient_correspondences, validate_quotient_formations,
};
pub use recasts::{
    ValidatedLiteralIndexedRecastFootprint, validate_literal_indexed_recast_footprint,
};
pub use result_overloads::resolve_named_result_overloads;
pub use traits::{
    DynamicConformanceSelection, DynamicDescriptorStorage, collect_dynamic_conformance_selections,
    collect_dynamic_descriptor_storages, compose_forwarded_trait_arguments,
    generic_bound_operator_requirement, resolve_dynamic_call_targets,
    revalidate_top_level_requirement_realization,
};
pub use type_references::normalize_open_index_expressions;
pub use type_references::{
    closed_integer_range_bound, closed_integer_range_maximum, closed_scalar_result_range,
    declared_integer_range, is_arithmetic_policy_only_integer,
};

pub use arithmetic_domains::arrival_integer_expression_bounds;
pub use arithmetic_domains::builtin_monotonic_integer_update_bounds;
pub use arithmetic_domains::enforced_integer_type_bounds;
pub use arithmetic_domains::immutable_integer_expression_bounds;
pub use arithmetic_domains::integer_widen_is_total;
pub use arithmetic_domains::validate_ordered_requirement_call_totality;
pub use contract_entailment::integer_embedding_sources_equal;
pub use contract_entailment::is_arm_pattern_marker;
pub use contract_entailment::transparent_proposition_application_entailed;
pub use contract_entailment::{
    InheritedRequirementApplication, inherited_requirement_proposition_application,
    inherited_requirement_proposition_label, inherited_satisfier_parameters,
};
pub use contract_entailment::{MatchedLawGuarantee, matched_machine_law_guarantees};
pub use contract_entailment::{
    RankingRangeEdgeProof, RankingRangeMeasure, RankingRangePremises, RankingRangeState,
    arithmetic_entry_requirement_is_covered, prove_arithmetic_call_requirement,
    prove_ranking_range_edge, prove_ranking_range_entry, prove_ranking_range_transition,
};
pub use contract_entailment::{
    StrictArithmeticBindingValue, StrictArithmeticExpressionBinding,
    StrictArithmeticImplicationJudgment, StrictArithmeticSymbolBinding,
    strict_arithmetic_expression_implication,
    strict_arithmetic_expression_implication_with_arguments,
};
pub use float_projection_invocations::{
    ValidatedFloatMeaningEqualityProposition, ValidatedFloatMeaningProjectionInvocation,
};
pub use immutable_integer_bounds::{
    immutable_integer_bound_symbol_offset, immutable_integer_bound_value_symbol,
    normalize_immutable_integer_bound_expression, normalize_immutable_integer_bound_to_usize,
};
pub use proof_embeddings::{ValidatedIntegerEmbeddingCall, integer_embedding_argument};

mod program_validation;
pub(crate) use program_validation::finish_diagnostics;
pub use program_validation::{
    ContractEntailmentStandDown, ContractEntailmentStandDownReason, ExactIntegerCastFact,
    OpaquePropertyValidation, ProgramValidation, ProgramValidationFacts,
    checked_operator_contract_snapshot, collect_contract_entailment_stand_downs,
    proven_machine_contract_expressions, validate_checked_operator_realization_contract,
    validate_generic_machine_contract_entailment, validate_program, validate_specialized_program,
};

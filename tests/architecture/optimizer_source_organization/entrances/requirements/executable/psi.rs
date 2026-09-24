use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "omega-rust/psi/pipeline/lowered-psi-to-lowered-psi/src/psi_optimization.rs",
        coordination_marker: "pub fn run_psi_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/native-realization/src/native_pipeline/abstract_operation_optimization/mod.rs",
        coordination_marker: "pub fn optimize_artifact_sections",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/optimization-unit/src/optimization_unit/identity/operation_encoding/mod.rs",
        coordination_marker: "match operation",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/sparse_conditional_constant_propagation/mod.rs",
        coordination_marker: "pub(super) fn validate_scalar_evaluation_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/optimization-core/src/optimization_core/manifest/mod.rs",
        coordination_marker: "DECISION_WIRE_FORMAT",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/optimization-core/src/optimization_core/decisions/external_schema/mod.rs",
        coordination_marker: "impl ExternalDecisionPoint",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/pass_manager/external_policy/mod.rs",
        coordination_marker: "pub(super) fn validated_candidate_features",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/ranked_rewrites/countdown_invariant_constant_relocation/mod.rs",
        coordination_marker: "apply::validated(session, validated)",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/ranked_rewrites/loop_invariant_scalar_motion/mod.rs",
        coordination_marker: "apply::validated(session, validated)",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/optimization-unit/src/optimization_unit.rs",
        coordination_marker: "pub struct PsiOptimizationUnit",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/optimization-unit/src/optimization_unit/rewrite/candidate/mod.rs",
        coordination_marker: "fn new(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/optimization-unit/src/optimization_unit/construction/mod.rs",
        coordination_marker: "pub fn reconstruct_psi_optimization_unit_seed",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/optimization-unit/src/optimization_unit/ledger/mod.rs",
        coordination_marker: "pub fn new(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/control_flow_cleanup.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/copy_propagation.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/copy_propagation/redundant_block_parameter.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/constant_conditionals/mod.rs",
        coordination_marker: "pub fn validate_constant_conditional_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/empty_block_threading/linear/mod.rs",
        coordination_marker: "pub fn validate_linear_empty_block_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/empty_block_threading/path_qualified/mod.rs",
        coordination_marker: "pub fn validate_path_qualified_empty_block_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/block_merging/adjacent/mod.rs",
        coordination_marker: "pub fn validate_adjacent_block_merge_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/shared_jump_fusion/mod.rs",
        coordination_marker: "pub fn validate_shared_jump_fusion_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/unreachable_private_machines/mod.rs",
        coordination_marker: "pub fn validate_unreachable_private_machines_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/block_merging/non_adjacent/mod.rs",
        coordination_marker: "pub fn validate_non_adjacent_block_merge_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/dead_scalar_elimination.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/dead_scalar_elimination/dead_scalar_literal_elimination.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/dead_scalar_elimination/dead_unconditionally_total_scalar_elimination.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/proof_check_elision/proof_certified_dead_scalar_elimination.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/global_value_numbering.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/proof_check_elision.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/boolean_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/boolean_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/boolean_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/boolean_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/boolean_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_binary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_cast_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_unary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/integer_unary_constants.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/range_against_constant.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/range_against_constant.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/range_against_constant.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/range_against_constant.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/range_against_constant.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/range_against_constant.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/range_against_range.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/range_against_range.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/sparse_conditional_constant_propagation/range_against_range.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/validation/prephysical_manifest/mod.rs",
        coordination_marker: "pub fn project_pre_physical_optimization_manifest",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/rewrite_accounting/mod.rs",
        coordination_marker: "fn preserve_edge_custody",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/global_value_numbering/mod.rs",
        coordination_marker: "fn validate_candidate_origin",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/global_value_numbering/total_scalar_identity/classification/mod.rs",
        coordination_marker: "match identity",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/sparse_conditional_constant_propagation/integer_evaluation/mod.rs",
        coordination_marker: "pub(crate) fn evaluate_integer_operation",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/sparse_conditional_constant_propagation/boolean_evaluation/mod.rs",
        coordination_marker: "pub(super) fn evaluate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/analyses/semantic/value_ranges/mod.rs",
        coordination_marker: "pub(in crate::analyses) fn value_ranges",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/analyses/control_flow/countdown_induction/mod.rs",
        coordination_marker: "pub(crate) fn analyze_counted_loops",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/analyses/control_flow/countdown_invariant_constants/mod.rs",
        coordination_marker: "pub(crate) fn analyze_countdown_invariant_constants",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/analyses/control_flow/countdown_invariant_constant_placement/mod.rs",
        coordination_marker: "pub(crate) fn analyze_countdown_invariant_constant_placement",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/dead_scalar_elimination/mod.rs",
        coordination_marker: "fn validate_candidate_contract",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/proof_check_elision/mod.rs",
        coordination_marker: "pub(super) fn validate_proof_check_elision_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/copy_propagation/mod.rs",
        coordination_marker: "fn validate_candidate_contract",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/unit_validation/operation_contracts/mod.rs",
        coordination_marker: "fn validate_values_and_bindings",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/unit_validation/function_structure/mod.rs",
        coordination_marker: "pub(crate) fn validate_function",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/unit_validation/derived_metadata/mod.rs",
        coordination_marker: "pub(crate) fn validate_places_and_claims",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/validation/context/mod.rs",
        coordination_marker: "fn validate_psi_optimization_unit_with_context",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/validation/context/ranked_cycles/mod.rs",
        coordination_marker: "fn validate_exact_ranked_cycles",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/validation/context/ranked_cycles/countdown_ranking/mod.rs",
        coordination_marker: "fn rederive_exact_certificates",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/unit_validation.rs",
        coordination_marker: "pub fn validate_psi_optimization_unit",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/current_value_ranges/mod.rs",
        coordination_marker: "pub fn validate_current_value_range_fact",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/validation/projection/mod.rs",
        coordination_marker: "pub fn validate_optimized_abstract_plan_projection",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/unit_validation/structural_catalog/mod.rs",
        coordination_marker: "fn index_structural_catalogs",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/semantics/optimization-unit-semantics/src/current_ownership/mod.rs",
        coordination_marker: "fn validate_current_ownership_frontier",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/global_value_numbering/bitwise_absorbing_literal_identity.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/global_value_numbering/bitwise_neutral_literal_identity.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/global_value_numbering/saturating_multiply_zero_annihilation.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/global_value_numbering/saturating_neutral_arithmetic_identity.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/global_value_numbering/wrapping_multiply_zero_annihilation.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/global_value_numbering/wrapping_neutral_arithmetic_identity.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/abstract-operations-to-abstract-operations/src/rules/global_value_numbering/wrapping_shift_zero_count_identity.rs",
        coordination_marker: "fn propose(",
    },
];

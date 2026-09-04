use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "omega-rust/psi/pipeline/psi-checked-trees-to-terminal/src/preterminal_optimization/mod.rs",
        coordination_marker: "pub fn run_psi_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/psi_optimization/mod.rs",
        coordination_marker: "pub fn optimize_artifact_sections",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/omega-optimization-unit/src/identity/operation_encoding/mod.rs",
        coordination_marker: "match operation",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/mod.rs",
        coordination_marker: "pub fn validate_scalar_evaluation_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/omega-optimization-core/src/manifest/mod.rs",
        coordination_marker: "DECISION_WIRE_FORMAT",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-policy/src/external_schema/mod.rs",
        coordination_marker: "impl ExternalDecisionPoint",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/pass_manager/external_policy/mod.rs",
        coordination_marker: "pub(super) fn validated_candidate_features",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/ranked_rewrites/countdown_invariant_constant_relocation/mod.rs",
        coordination_marker: "apply::validated(session, validated)",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/omega-optimization-unit/src/model.rs",
        coordination_marker: "pub struct PsiOptimizationUnit",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/omega-optimization-unit/src/rewrite/candidate/mod.rs",
        coordination_marker: "fn new(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/omega-optimization-unit/src/construction/mod.rs",
        coordination_marker: "pub fn reconstruct_psi_optimization_unit_seed",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/omega-optimization-unit/src/ledger/mod.rs",
        coordination_marker: "pub fn new(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/control_flow_cleanup/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/copy_propagation/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/copy_propagation/redundant_block_parameter/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/constant_conditionals/mod.rs",
        coordination_marker: "pub fn validate_constant_conditional_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/empty_block_threading/linear/mod.rs",
        coordination_marker: "pub fn validate_linear_empty_block_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/empty_block_threading/path_qualified/mod.rs",
        coordination_marker: "pub fn validate_path_qualified_empty_block_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/block_merging/adjacent/mod.rs",
        coordination_marker: "pub fn validate_adjacent_block_merge_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/shared_jump_fusion/mod.rs",
        coordination_marker: "pub fn validate_shared_jump_fusion_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/unreachable_private_machines/mod.rs",
        coordination_marker: "pub fn validate_unreachable_private_machines_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/control_flow_cleanup/block_merging/non_adjacent/mod.rs",
        coordination_marker: "pub fn validate_non_adjacent_block_merge_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/literal/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/unconditionally_total/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/proof_check_elision/dead_scalar/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/proof_check_elision/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/boolean/boolean_not_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/boolean/boolean_equal_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/boolean/integer_equal_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/boolean/integer_less_than_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/boolean/integer_less_or_equal_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/exact_integer_add_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/exact_integer_subtract_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/exact_integer_multiply_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/wrapping_integer_add_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/wrapping_integer_subtract_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/wrapping_integer_multiply_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/saturating_integer_add_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/saturating_integer_subtract_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/saturating_integer_multiply_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/exact_integer_divide_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/exact_integer_remainder_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/wrapping_integer_divide_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/wrapping_integer_remainder_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/saturating_integer_divide_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/saturating_integer_remainder_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/exact_integer_shift_left_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/exact_integer_shift_right_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/wrapping_integer_shift_left_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/wrapping_integer_shift_right_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/integer_bitwise_and_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/integer_bitwise_or_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/binary/integer_bitwise_xor_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/exact_integer_cast_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/unary/integer_widen_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/integer/unary/integer_bitwise_not_constants/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/range_comparisons/against_constant/integer_equal_range_constant/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/range_comparisons/against_constant/integer_equal_constant_range/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/range_comparisons/against_constant/integer_less_than_range_constant/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/range_comparisons/against_constant/integer_less_than_constant_range/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/range_comparisons/against_constant/integer_less_or_equal_range_constant/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/range_comparisons/against_constant/integer_less_or_equal_constant_range/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/range_comparisons/against_range/integer_equal_range_range/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/range_comparisons/against_range/integer_less_than_range_range/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/range_comparisons/against_range/integer_less_or_equal_range_range/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/prephysical_manifest/mod.rs",
        coordination_marker: "pub fn project_pre_physical_optimization_manifest",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/rewrite_accounting/mod.rs",
        coordination_marker: "fn preserve_edge_custody",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/mod.rs",
        coordination_marker: "fn validate_candidate_origin",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/total_scalar_identity/classification/mod.rs",
        coordination_marker: "match identity",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/integer_evaluation/mod.rs",
        coordination_marker: "pub(crate) fn evaluate_integer_operation",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/boolean_evaluation/mod.rs",
        coordination_marker: "pub(super) fn evaluate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/analyses/semantic/value_ranges/mod.rs",
        coordination_marker: "pub(in crate::analyses) fn value_ranges",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/analyses/control_flow/countdown_induction/mod.rs",
        coordination_marker: "pub(crate) fn analyze_counted_loops",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/analyses/control_flow/countdown_invariant_constants/mod.rs",
        coordination_marker: "pub(crate) fn analyze_countdown_invariant_constants",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/analyses/control_flow/countdown_invariant_constant_placement/mod.rs",
        coordination_marker: "pub(crate) fn analyze_countdown_invariant_constant_placement",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/dead_scalar_elimination/mod.rs",
        coordination_marker: "fn validate_candidate_contract",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/proof_check_elision/mod.rs",
        coordination_marker: "pub fn validate_proof_check_elision_candidate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/copy_propagation/mod.rs",
        coordination_marker: "fn validate_candidate_contract",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/operation_contracts/mod.rs",
        coordination_marker: "fn validate_values_and_bindings",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/function_structure/mod.rs",
        coordination_marker: "pub(crate) fn validate_function",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/derived_metadata/mod.rs",
        coordination_marker: "pub(crate) fn validate_places_and_claims",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/context/mod.rs",
        coordination_marker: "fn validate_psi_optimization_unit_with_context",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/context/ranked_cycles/mod.rs",
        coordination_marker: "fn validate_exact_ranked_cycles",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/context/ranked_cycles/countdown_ranking/mod.rs",
        coordination_marker: "fn rederive_exact_certificates",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/core/mod.rs",
        coordination_marker: "pub fn validate_psi_optimization_unit",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/current_value_ranges/mod.rs",
        coordination_marker: "pub fn validate_current_value_range_fact",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/projection/mod.rs",
        coordination_marker: "pub fn validate_optimized_abstract_plan_projection",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/structural_catalog/mod.rs",
        coordination_marker: "fn index_structural_catalogs",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/current_ownership/mod.rs",
        coordination_marker: "fn validate_current_ownership_frontier",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/bitwise_absorbing/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/bitwise_neutral/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/saturating_multiply_zero/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/saturating_neutral/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/wrapping_multiply_zero/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/wrapping_neutral/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/wrapping_shift_zero_count/mod.rs",
        coordination_marker: "fn propose(",
    },
];

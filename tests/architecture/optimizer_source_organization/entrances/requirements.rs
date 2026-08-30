//! Required executable joins and semantic ladders.

pub(super) struct RequiredCoordinationEntrance {
    pub(super) path: &'static str,
    pub(super) coordination_marker: &'static str,
}

/// Entrances that must visibly own a real stage join or catalog route. Merely
/// keeping these paths small is insufficient: deleting the coordination seam
/// and leaving a re-export wall must fail this architecture test.
pub(super) const REQUIRED_COORDINATION_ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/mod.rs",
        coordination_marker: "pub(crate) fn settle_provider_executions",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/replay/mod.rs",
        coordination_marker: "pub(super) fn replay_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/representations/omega-optimization-unit/src/identity/operation_encoding/mod.rs",
        coordination_marker: "match operation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/mod.rs",
        coordination_marker: "pub fn validate_scalar_evaluation_candidate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/representations/omega-optimization-core/src/manifest/mod.rs",
        coordination_marker: "DECISION_WIRE_FORMAT",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/representations/omega-legalized-operations/src/validation/mod.rs",
        coordination_marker: "impl LegalizedCallUnit",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/mod.rs",
        coordination_marker: "impl FixedViewCopyPlan",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/selected/mod.rs",
        coordination_marker: "fn decode_selected_plan",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-policy/src/external_schema/mod.rs",
        coordination_marker: "impl ExternalDecisionPoint",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/pass_manager/external_policy/mod.rs",
        coordination_marker: "pub(super) fn validated_candidate_features",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/backend/images/omega-image-emission/src/ranked_u32_countdown/mod.rs",
        coordination_marker: "pub(super) fn replay_ranked_u32_countdown",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/representations/omega-optimization-unit/src/model.rs",
        coordination_marker: "pub struct PsiOptimizationUnit",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/representations/omega-optimization-unit/src/rewrite/candidate/mod.rs",
        coordination_marker: "fn new(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger/mod.rs",
        coordination_marker: "pub fn new(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/constant_evaluation/mod.rs",
        coordination_marker: "fn integer_evaluation_contract",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate.rs",
        coordination_marker: "pub fn validate_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/compute.rs",
        coordination_marker: "pub(crate) fn compute_terminal_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/liveness/validate/mod.rs",
        coordination_marker: "pub fn validate_liveness",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/home_assignment/mod.rs",
        coordination_marker: "compute::compute_terminal_register_homes(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/home_assignment/compute/mod.rs",
        coordination_marker: "compute_function(index, legality, ranges, physical)",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/home_assignment/validate/mod.rs",
        coordination_marker: "replay::validate_function(function_index, actual, legality, ranges, physical)",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/post_allocation_manifest/mod.rs",
        coordination_marker: "pub fn project_post_allocation_optimization_manifest",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/artifact/mod.rs",
        coordination_marker: "pub fn lower_artifact_sections",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/optimization/mod.rs",
        coordination_marker: "pub fn build_verified_psi_optimization_unit",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/provider_installation/mod.rs",
        coordination_marker: "pub fn admit_provider_installation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/lowering/mod.rs",
        coordination_marker: "pub(crate) fn lower_decoded_verified_module",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/lowering/machine.rs",
        coordination_marker: "pub(super) fn lower_machine",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/lib.rs",
        coordination_marker: "pub fn project_optimization_run",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/replay/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/replay/candidate_decisions/mod.rs",
        coordination_marker: "manifests::validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/replay/candidate_decisions/mod.rs",
        coordination_marker: "declarations::validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/replay/candidate_decisions/mod.rs",
        coordination_marker: "baseline::validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-optimization-run-to-abstract-operations/src/source/mod.rs",
        coordination_marker: "pub(super) fn project_plan",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/mod.rs",
        coordination_marker: "pub fn assign_registers",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/function/mod.rs",
        coordination_marker: "pub(super) fn assign_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/function/operation_routes.rs",
        coordination_marker: "pub(super) fn assign_operation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/mod.rs",
        coordination_marker: "pub fn lower_to_target_operations_with_provider_executions_and_installation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/mod.rs",
        coordination_marker: "pub fn validate_abstract_to_target_translation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/mod.rs",
        coordination_marker: "const ENABLED_TRANSLATION_FAMILIES",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/mod.rs",
        coordination_marker: "fn reconstruct_parameter_return",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/mod.rs",
        coordination_marker: "pub(super) mod integer",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/mod.rs",
        coordination_marker: "mod unary",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/mod.rs",
        coordination_marker: "mod bitwise",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/mod.rs",
        coordination_marker: "fn parameter(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/source/integer/bitwise/mod.rs",
        coordination_marker: "fn reconstruct_bitwise_xor",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/comparison/mod.rs",
        coordination_marker: "fn reconstruct_equal",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/comparison/mod.rs",
        coordination_marker: "fn reconstruct_less_or_equal",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/bitwise/mod.rs",
        coordination_marker: "fn reconstruct_bitwise_and",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/bitwise/mod.rs",
        coordination_marker: "fn reconstruct_bitwise_or",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/bitwise/mod.rs",
        coordination_marker: "fn reconstruct_bitwise_xor",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/straight_line_parameter/integer/bitwise/replay.rs",
        coordination_marker: "pub(super) fn reconstruct",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/dispatch/parameter/mod.rs",
        coordination_marker: "mod unary",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/catalog/dispatch/parameter/mod.rs",
        coordination_marker: "mod bitwise",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/error/parameter/bitwise/mod.rs",
        coordination_marker: "mod schema",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/error/parameter/bitwise/mod.rs",
        coordination_marker: "mod bitwise_xor",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/receipt/parameter/bitwise/mod.rs",
        coordination_marker: "mod bitwise_or",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/receipt/parameter/bitwise/mod.rs",
        coordination_marker: "mod bitwise_xor",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/error/mod.rs",
        coordination_marker: "AbstractToTargetTranslationFamilyError",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/validation/model/receipt/mod.rs",
        coordination_marker: "AbstractToTargetFunctionTranslationReceipt",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/function/mod.rs",
        coordination_marker: "pub(super) fn lower_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/mod.rs",
        coordination_marker: "pub(super) fn lower_scalar_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/straight_line/mod.rs",
        coordination_marker: "pub(super) fn lower_straight_line",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/conditional_scalar/mod.rs",
        coordination_marker: "pub(super) fn lower_conditional_scalar_operation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/structural/mod.rs",
        coordination_marker: "pub(super) fn lower_structural_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source/mod.rs",
        coordination_marker: "pub(crate) fn derive_source_function_rosters",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source/structural/mod.rs",
        coordination_marker: "pub(super) fn derive_source_structural_unit_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay/mod.rs",
        coordination_marker: "pub(crate) fn replay_terminal_legalized_plan",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay/structural/mod.rs",
        coordination_marker: "pub(super) fn replay_structural_unit_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay/leaf/mod.rs",
        coordination_marker: "pub(super) fn replay_leaf",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/mod.rs",
        coordination_marker: "pub fn select_instructions",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/mod.rs",
        coordination_marker: "pub(super) fn build_plan",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/scalar/mod.rs",
        coordination_marker: "let body = catalog::build(&context)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/structural_unit/mod.rs",
        coordination_marker: "let call = call::build(function, source, plan, layout, keys, catalog)?",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/validation/structural_unit/mod.rs",
        coordination_marker: "pub(super) fn reconstruct_structural_unit_contract",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/control_flow_cleanup/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/copy_propagation/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/proof_check_elision/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/sparse_conditional_constant_propagation/mod.rs",
        coordination_marker: "fn built_in_registrations",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/selected_lowering/literal_fold/mod.rs",
        coordination_marker: "compute::compute_terminal_literal_fold(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/selected_lowering/literal_fold/compute/mod.rs",
        coordination_marker: "derive_function_folds(selected, recovery, &rows)",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/selected_lowering/literal_fold/validate/mod.rs",
        coordination_marker: "reconstruct_literal_fold(selected, recovery, &rows)",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/pressure_rematerialization/mod.rs",
        coordination_marker: "pub fn rematerialize_selected_active_resident",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/prephysical_manifest/mod.rs",
        coordination_marker: "pub fn project_pre_physical_optimization_manifest",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/rewrite_accounting/mod.rs",
        coordination_marker: "fn preserve_edge_custody",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/mod.rs",
        coordination_marker: "fn validate_candidate_origin",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/total_scalar_identity/classification/mod.rs",
        coordination_marker: "match identity",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/expression_keys/mod.rs",
        coordination_marker: "type ScalarExpressionRow",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/phi_translated/mod.rs",
        coordination_marker: "fn phi_translated_contract",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/local/mod.rs",
        coordination_marker: "fn same_block_contract",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/dominating/mod.rs",
        coordination_marker: "fn dominating_contract",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/dead_scalar_elimination/mod.rs",
        coordination_marker: "fn validate_candidate_contract",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/copy_propagation/mod.rs",
        coordination_marker: "fn validate_candidate_contract",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/operation_contracts/mod.rs",
        coordination_marker: "fn validate_values_and_bindings",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/function_structure/mod.rs",
        coordination_marker: "pub(crate) fn validate_function",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/derived_metadata/mod.rs",
        coordination_marker: "pub(crate) fn validate_places_and_claims",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/context/mod.rs",
        coordination_marker: "fn validate_psi_optimization_unit_with_context",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/core/mod.rs",
        coordination_marker: "pub fn validate_psi_optimization_unit",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/current_value_ranges/mod.rs",
        coordination_marker: "pub fn validate_current_value_range_fact",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/projection/mod.rs",
        coordination_marker: "pub fn validate_optimized_abstract_plan_projection",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/unit_validation/structural_catalog/mod.rs",
        coordination_marker: "fn index_structural_catalogs",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/current_ownership/mod.rs",
        coordination_marker: "fn validate_current_ownership_frontier",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/register_environment/mod.rs",
        coordination_marker: "pub fn baseline_target_register_environment",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/selection/selection/mod.rs",
        coordination_marker: "pub fn stage_optimized_instruction_selection",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/selection/optimized_target_operations/mod.rs",
        coordination_marker: "validate_abstract_to_target_translation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/selection/assignment/mod.rs",
        coordination_marker: "fn stage_optimized_assignment",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/allocation_legality/mod.rs",
        coordination_marker: "pub fn stage_optimized_allocation_legality_with_availability",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/register_homes/mod.rs",
        coordination_marker: "pub fn stage_optimized_register_homes",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/liveness/mod.rs",
        coordination_marker: "pub fn stage_optimized_liveness",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/live_ranges/mod.rs",
        coordination_marker: "pub fn stage_optimized_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/fixed_view_copies/mod.rs",
        coordination_marker: "pub fn stage_optimized_fixed_view_copies",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/allocation/selected_reanalysis/mod.rs",
        coordination_marker: "pub fn stage_optimized_selected_reanalysis",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/post_allocation_optimizations/mod.rs",
        coordination_marker: "OptimizedPostAllocationMachineOptimizationError",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/post_allocation_machine_effects/mod.rs",
        coordination_marker: "fn seal_staged_post_allocation_machine",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/active_resident_rematerialization/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/machine_effects/mod.rs",
        coordination_marker: "fn admit_machine_effects",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/literal_folds/mod.rs",
        coordination_marker: "pub fn run_selected_lowering_optimizations",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/literal_fold_homes/mod.rs",
        coordination_marker: "pub fn stage_optimized_register_homes_after_literal_folds",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/encoding/active_resident_selected_form_encoding/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization_selected_form_encoding",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/encoding/post_allocation_selected_form_encoding/mod.rs",
        coordination_marker: "stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/active_resident_resolved_selected_form_layout/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization_resolved_selected_form_layout",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/resolved_selected_form_layout/mod.rs",
        coordination_marker: "stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/whole_function_exit_contract/mod.rs",
        coordination_marker: "stage_whole_function_exit_contract_with_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/artifacts/function_fragment_emission/mod.rs",
        coordination_marker: "stage_optimized_function_fragment_emission",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/artifacts/function_fragment_text_section/mod.rs",
        coordination_marker: "stage_optimized_relocation_free_text_section",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/artifacts/function_fragment_object_container/mod.rs",
        coordination_marker: "stage_optimized_relocation_free_object_container",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/artifacts/object_artifact/mod.rs",
        coordination_marker: "stage_validated_optimized_object_artifact",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/ordinary_callable_entry/mod.rs",
        coordination_marker: "stage_validated_optimized_ordinary_callable_entry",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/structural_unit_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_optimized_structural_unit_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/active_resident_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/allocation_recovery_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_allocation_recovery_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/physical_pipeline/routes/allocation_recovery/mod.rs",
        coordination_marker: "fn stage_allocation_recovery_pipeline",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/physical_pipeline/routes/composition/mod.rs",
        coordination_marker: "fn resolve_physical_phase_composition",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/unit_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_optimized_unit_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/entry_settlement/mod.rs",
        coordination_marker: "pub fn validate_native_program_entry_settlement",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/mod.rs",
        coordination_marker: "pub fn realize_native_artifact",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/mod.rs",
        coordination_marker: "pub(crate) fn admit_native_providers",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_encoding/mod.rs",
        coordination_marker: "pub fn select_optimized_program_storage_semantic_wrapper_encoding",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/mod.rs",
        coordination_marker: "pub fn stage_validated_optimized_program_storage_semantic_wrapper_object",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/validation/mod.rs",
        coordination_marker: "pub fn validate_optimized_program_storage_semantic_wrapper_object",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/object/mod.rs",
        coordination_marker: "pub(crate) fn construct_object",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/validation/mod.rs",
        coordination_marker: "pub fn validate_selected_instructions",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/mod.rs",
        coordination_marker: "pub fn analyze_pre_allocation_machine_effects",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/planning/post_allocation/mod.rs",
        coordination_marker: "pub fn analyze_post_allocation_machine_plan",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/compare_zero_branch_nonzero/mod.rs",
        coordination_marker: "pub fn optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/mod.rs",
        coordination_marker: "pub fn optimize_aarch64_materialize_i64_with_shortest_movn_seed",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/x86_64/materialize_i64_mov_r32_imm32/mod.rs",
        coordination_marker: "pub fn optimize_x86_materialize_i64_with_mov_r32_imm32",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/x86_64/materialize_i64_xor_zero/mod.rs",
        coordination_marker: "pub fn optimize_x86_materialize_i64_zero_with_xor",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/physical_pipeline/mod.rs",
        coordination_marker: "pub(super) fn stage_optimized_verified_physical_pipeline",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/encoding/post_allocation_selected_form_encoding/validation/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/resolved_selected_form_layout/validation/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/layout/resolved_selected_form_layout/validation/ordinary/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/assembly/mod.rs",
        coordination_marker: "pub(super) fn build_realization",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/bitwise_absorbing/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/bitwise_neutral/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/saturating_multiply_zero/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/saturating_neutral/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/wrapping_multiply_zero/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/wrapping_neutral/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/wrapping_shift_zero_count/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/spill_choice/mod.rs",
        coordination_marker: "pub fn choose_spill_victims",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/allocation_legality/mod.rs",
        coordination_marker: "pub fn analyze_allocation_legality",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/allocator_availability/mod.rs",
        coordination_marker: "pub fn materialize_allocator_availability",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/mod.rs",
        coordination_marker: "pub fn analyze_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/liveness/mod.rs",
        coordination_marker: "pub fn analyze_liveness",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/recovery_classification/mod.rs",
        coordination_marker: "pub fn classify_pressure_recovery",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/mod.rs",
        coordination_marker: "pub fn materialize_fixed_view_copies",
    },
];

pub(crate) fn is_required_coordination_entrance(path: &str) -> bool {
    REQUIRED_COORDINATION_ENTRANCES
        .iter()
        .any(|entrance| entrance.path == path)
}

/// The v4 codec remains a visible semantic ladder below its protocol entrance.
pub(super) const REQUIRED_FIXED_VIEW_COPY_CODEC_LEAVES: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/content.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/copy.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/primitives.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/values.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/selected/function.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/selected/register.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/selected/block.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/selected/instruction.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec/selected/provenance.rs",
];

/// The optimization-manifest entrance routes directly to one leaf per stable
/// record concern; restoring the former mixed leaf must not erase this ladder.
pub(super) const REQUIRED_MANIFEST_LEAVES: &[&str] = &[
    "source/omega-rust/omega/representations/omega-optimization-core/src/manifest/codec.rs",
    "source/omega-rust/omega/representations/omega-optimization-core/src/manifest/decision.rs",
    "source/omega-rust/omega/representations/omega-optimization-core/src/manifest/error.rs",
    "source/omega-rust/omega/representations/omega-optimization-core/src/manifest/fact_reference.rs",
    "source/omega-rust/omega/representations/omega-optimization-core/src/manifest/pass.rs",
    "source/omega-rust/omega/representations/omega-optimization-core/src/manifest/work_usage.rs",
];

pub(super) const REQUIRED_OPERATION_ENCODING_LEAVES: &[&str] = &[
    "source/omega-rust/omega/representations/omega-optimization-unit/src/identity/operation_encoding/calls_and_effects.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/identity/operation_encoding/control.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/identity/operation_encoding/scalar.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/identity/operation_encoding/scalar_shapes.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/identity/operation_encoding/structural.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/identity/carrier_encoding.rs",
];

pub(super) const REQUIRED_SCCP_VALIDATION_LEAVES: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/boolean_candidate.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/integer_candidate.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/observation.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/range_comparisons.rs",
];

pub(super) const REQUIRED_LIVE_RANGE_REPLAY_LEAVES: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/replay/architectural_units.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/replay/canonical.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/replay/comparison.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/replay/constraints.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/replay/fragments.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/replay/function.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/replay/tests.rs",
];

pub(super) const REQUIRED_GVN_EXPRESSION_KEY_LEAVES: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/expression_keys/mod.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/expression_keys/model.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/expression_keys/total.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/expression_keys/proof_certified.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/expression_keys/compatible_policy.rs",
];

pub(super) const REQUIRED_STRUCTURAL_CATALOG_TEST_LEAVES: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/tests/structural_catalog/mod.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/tests/structural_catalog/catalog.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/tests/structural_catalog/type_declarations.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/tests/structural_catalog/content_projection.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/tests/structural_catalog/function_catalog.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/tests/structural_catalog/structural_roots.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/tests/structural_catalog/provider_specialization.rs",
];

pub(super) const REQUIRED_LEDGER_LEAVES: &[&str] = &[
    "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger/model.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger/error.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger/validation.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger/codec/mod.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger/codec/encoding.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger/codec/decoding.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger/codec/cursor.rs",
    "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger/tests.rs",
];

pub(super) const REQUIRED_REGISTER_ALLOCATION_TEST_LEAVES: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/mod.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/allocation_legality.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/fixed_view_copies.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/live_ranges.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/liveness.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/register_homes.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation/selected_input.rs",
];

pub(super) const REQUIRED_SELECTED_LOWERING_TEST_LEAVES: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/machine/selected_lowering/mod.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/machine/selected_lowering/exact_immediates.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/machine/selected_lowering/exit_contract.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/machine/selected_lowering/named_suite.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/machine/selected_lowering/pressure_recovery.rs",
];

pub(super) const REQUIRED_CONDITIONAL_CONTROL_LEAVES: &[&str] = &[
    "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/conditional_control/mod.rs",
    "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/conditional_control/bindings.rs",
    "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/conditional_control/boolean.rs",
    "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/conditional_control/integer.rs",
];

pub(super) const REQUIRED_PROVIDER_SETTLEMENT_LEAVES: &[&str] = &[
    "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/mod.rs",
    "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/boundary.rs",
    "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/exact_plan.rs",
    "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/normalized_foreign_call.rs",
    "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/mod.rs",
    "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/exact_evidence.rs",
    "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/fixtures.rs",
    "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/literal_arguments.rs",
    "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/tests/zero_argument_import.rs",
];

pub(super) const REQUIRED_PRE_ALLOCATION_EFFECT_CODEC_LEAVES: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/mod.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/cursor.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/error.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/mod.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/framing.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/instruction.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/ownership.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/structural.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec/v6/values.rs",
];

pub(super) const REQUIRED_MOVN_COMPUTE_LEAVES: &[&str] = &[
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/mod.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/budget.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/materialization.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/recipe.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/selection.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/source.rs",
    "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute/tests.rs",
];

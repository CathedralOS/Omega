use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/coordination/psi_optimization/mod.rs",
        coordination_marker: "pub fn optimize_artifact_sections",
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
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-policy/src/external_schema/mod.rs",
        coordination_marker: "impl ExternalDecisionPoint",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/pass_manager/external_policy/mod.rs",
        coordination_marker: "pub(super) fn validated_candidate_features",
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
        path: "source/omega-rust/omega/representations/omega-optimization-unit/src/construction/mod.rs",
        coordination_marker: "pub fn reconstruct_psi_optimization_unit_seed",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger/mod.rs",
        coordination_marker: "pub fn new(",
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
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/literal/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/unconditionally_total/mod.rs",
        coordination_marker: "fn propose(",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/dead_scalar_elimination/proof_certified/mod.rs",
        coordination_marker: "fn propose(",
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
        path: "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/integer_evaluation/mod.rs",
        coordination_marker: "pub(crate) fn evaluate_integer_operation",
    },
    RequiredCoordinationEntrance {
        path: "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/analyses/semantic/value_ranges/mod.rs",
        coordination_marker: "pub(in crate::analyses) fn value_ranges",
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
];

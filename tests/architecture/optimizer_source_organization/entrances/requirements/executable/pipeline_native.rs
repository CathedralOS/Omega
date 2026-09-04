use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_entry/mod.rs",
        coordination_marker: "pub fn bind_optimized_program_storage_semantic_entry_contract",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/plans/omega-program-entry-plan/src/optimized_semantic_wrapper/mod.rs",
        coordination_marker: "pub fn plan_optimized_program_storage_semantic_wrapper",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements/mod.rs",
        coordination_marker: "pub(crate) fn settle_provider_executions",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-to-register-environment/src/lib.rs",
        coordination_marker: "pub fn baseline_target_register_environment",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/optimized/mod.rs",
        coordination_marker: "pub fn stage_optimized_instruction_selection",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/optimized.rs",
        coordination_marker: "validate_abstract_to_target_translation",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-live-ranges-to-allocation-legality/src/lib.rs",
        coordination_marker: "pub fn stage_optimized_allocation_legality_with_availability",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-allocation-legality-to-register-homes/src/lib.rs",
        coordination_marker: "pub fn stage_optimized_register_homes",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-liveness/src/lib.rs",
        coordination_marker: "pub fn stage_optimized_liveness",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-liveness-to-live-ranges/src/lib.rs",
        coordination_marker: "pub fn stage_optimized_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-allocation-legality-to-fixed-view-copies/src/fixed_view_copies/mod.rs",
        coordination_marker: "pub fn stage_optimized_fixed_view_copies",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-fixed-view-copies-to-reanalyzed-legality/src/lib.rs",
        coordination_marker: "pub fn stage_optimized_selected_reanalysis",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/machine/post_allocation_optimizations/execution/mod.rs",
        coordination_marker: "pub fn stage_optimized_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/machine/post_allocation_optimizations/aarch64_same_view_copy/mod.rs",
        coordination_marker: "use execution::{stage_with_inputs, validate_with_inputs};",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/machine/post_allocation_machine_effects/construction/mod.rs",
        coordination_marker: "fn analyze_and_seal",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-allocation-legality-to-active-resident-rematerialization/src/lib.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/machine/machine_effects/mod.rs",
        coordination_marker: "fn admit_machine_effects",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-allocation-legality-to-literal-folds/src/lib.rs",
        coordination_marker: "pub fn run_selected_lowering_optimizations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/machine/literal_fold_homes/mod.rs",
        coordination_marker: "pub fn stage_optimized_register_homes_after_literal_folds",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/encoding/active_resident_selected_form_encoding/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization_selected_form_encoding",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/encoding/post_allocation_selected_form_encoding/mod.rs",
        coordination_marker: "stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/layout/active_resident_resolved_selected_form_layout/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization_resolved_selected_form_layout",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/layout/resolved_selected_form_layout/mod.rs",
        coordination_marker: "stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/layout/whole_function_exit_contract/mod.rs",
        coordination_marker: "stage_whole_function_exit_contract_with_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/artifacts/function_fragment_emission/mod.rs",
        coordination_marker: "stage_optimized_function_fragment_emission",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/artifacts/function_fragment_emission/compute/mod.rs",
        coordination_marker: "pub(super) fn compute",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/artifacts/function_fragment_text_section/mod.rs",
        coordination_marker: "stage_optimized_relocation_free_text_section",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/artifacts/function_fragment_text_section/placement/mod.rs",
        coordination_marker: "pub(super) fn place_fragments",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/artifacts/function_fragment_object_container/mod.rs",
        coordination_marker: "stage_optimized_relocation_free_object_container",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/artifacts/object_artifact/mod.rs",
        coordination_marker: "stage_validated_optimized_object_artifact",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/realization/ordinary_callable_entry/mod.rs",
        coordination_marker: "stage_validated_optimized_ordinary_callable_entry",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/realization/structural_unit_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_optimized_structural_unit_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/realization/active_resident_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/realization/allocation_recovery_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_allocation_recovery_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/coordination/physical_pipeline/routes/allocation_recovery/mod.rs",
        coordination_marker: "fn stage_allocation_recovery_pipeline",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/coordination/physical_pipeline/routes/composition/mod.rs",
        coordination_marker: "fn resolve_physical_phase_composition",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/realization/unit_function_relative_realization/mod.rs",
        coordination_marker: "pub fn stage_optimized_unit_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/entry_settlement/mod.rs",
        coordination_marker: "pub fn validate_native_program_entry_settlement",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/mod.rs",
        coordination_marker: "pub fn realize_native_artifact",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_review.rs",
        coordination_marker: "pub(crate) fn review_terminal_authority_closure",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/terminal_authority_policy/mod.rs",
        coordination_marker: "pub fn terminal_authority_policy_with_rows",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/mod.rs",
        coordination_marker: "pub(crate) fn admit_native_providers",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_encoding/mod.rs",
        coordination_marker: "pub fn select_optimized_program_storage_semantic_wrapper_encoding",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/mod.rs",
        coordination_marker: "pub fn stage_validated_optimized_program_storage_semantic_wrapper_object",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/validation/mod.rs",
        coordination_marker: "pub fn validate_optimized_program_storage_semantic_wrapper_object",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/optimized_semantic_wrapper_object/object/mod.rs",
        coordination_marker: "pub(crate) fn construct_object",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/coordination/physical_pipeline/mod.rs",
        coordination_marker: "pub fn stage_optimized_verified_physical_pipeline",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/encoding/post_allocation_selected_form_encoding/validation/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/layout/resolved_selected_form_layout/validation/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/layout/resolved_selected_form_layout/validation/ordinary/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/realization/function_relative_realization/assembly/mod.rs",
        coordination_marker: "pub(super) fn build_realization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec/mod.rs",
        coordination_marker: "impl FunctionRelativeOptimizationRealizationManifest",
    },
];

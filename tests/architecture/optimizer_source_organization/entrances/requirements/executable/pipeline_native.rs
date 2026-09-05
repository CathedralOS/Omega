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
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/legality/mod.rs",
        coordination_marker: "pub fn stage_optimized_allocation_legality_with_availability",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/baseline/mod.rs",
        coordination_marker: "pub fn stage_optimized_register_homes",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/liveness/staging/mod.rs",
        coordination_marker: "pub fn stage_optimized_liveness",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/live_ranges/staging/mod.rs",
        coordination_marker: "pub fn stage_optimized_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/fixed_view/fixed_view_copies/mod.rs",
        coordination_marker: "pub fn stage_optimized_fixed_view_copies",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/reanalysis/mod.rs",
        coordination_marker: "pub fn stage_optimized_selected_reanalysis",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/execution/mod.rs",
        coordination_marker: "pub fn stage_optimized_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-optimized-machine/src/aarch64_same_view_copy/mod.rs",
        coordination_marker: "use execution::{stage_with_inputs, validate_with_inputs};",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-register-homes-to-post-allocation-machine/src/construction/mod.rs",
        coordination_marker: "fn stage_optimized_post_allocation_machine_plan",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-frame-layout/src/lib.rs",
        coordination_marker: "pub fn stage_target_frame_layout",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/frame_protocol/mod.rs",
        coordination_marker: "pub fn stage_target_frame_protocol_encoding",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/rematerialization/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-machine-effects/src/lib.rs",
        coordination_marker: "pub fn analyze_machine_effects",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/literal_folds/mod.rs",
        coordination_marker: "pub fn run_selected_lowering_optimizations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/transformed/mod.rs",
        coordination_marker: "pub fn stage_optimized_register_homes_after_literal_folds",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-selected-form-encoding/src/lib.rs",
        coordination_marker: "stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-form-encoding-to-resolved-layout/src/resolved_selected_form_layout/mod.rs",
        coordination_marker: "stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/exit_contract/mod.rs",
        coordination_marker: "stage_whole_function_exit_contract_with_post_allocation_machine_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/exit_contract/validation/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/fragment_emission/mod.rs",
        coordination_marker: "stage_optimized_function_fragment_emission",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/text_placement/mod.rs",
        coordination_marker: "place_fragment_text_section",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/text_placement/production/mod.rs",
        coordination_marker: "mod structural_unit",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/text_placement/validation/mod.rs",
        coordination_marker: "pub(super) fn check",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/fragments/mod.rs",
        coordination_marker: "emit_resolved_function_fragments",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/frame_application/mod.rs",
        coordination_marker: "apply_frame_protocol_to_fragments",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/fragments/validation/mod.rs",
        coordination_marker: "pub(super) fn check",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/fragments/production/mod.rs",
        coordination_marker: "pub(super) fn emit",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/fragment_emission/compute/mod.rs",
        coordination_marker: "pub(super) fn compute",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/text_placement/custody/mod.rs",
        coordination_marker: "stage_optimized_relocation_free_text_section",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/text_placement/custody/placement/mod.rs",
        coordination_marker: "pub(super) fn place_fragments",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/object/omega-object-file/src/fragment_container/mod.rs",
        coordination_marker: "stage_optimized_relocation_free_object_container",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/object/omega-object-file/src/artifact_custody/mod.rs",
        coordination_marker: "stage_validated_optimized_object_artifact",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/artifacts/omega-native-artifact/src/callable_entry/mod.rs",
        coordination_marker: "stage_validated_optimized_ordinary_callable_entry",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/function_realization/structural_unit/mod.rs",
        coordination_marker: "pub fn stage_optimized_structural_unit_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/function_realization/allocation_recovery/mod.rs",
        coordination_marker: "pub fn stage_allocation_recovery_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/native_pipeline/physical_pipeline/routes/allocation_recovery/mod.rs",
        coordination_marker: "fn stage_allocation_recovery_pipeline",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/native_pipeline/physical_pipeline/routes/composition/mod.rs",
        coordination_marker: "fn resolve_physical_phase_composition",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/function_realization/unit/mod.rs",
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
        path: "omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/native_pipeline/physical_pipeline/mod.rs",
        coordination_marker: "pub fn stage_optimized_verified_physical_pipeline",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-selected-form-encoding/src/validation/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-form-encoding-to-resolved-layout/src/resolved_selected_form_layout/validation/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-form-encoding-to-resolved-layout/src/resolved_selected_form_layout/validation/ordinary/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/function_realization/assembly/mod.rs",
        coordination_marker: "pub(super) fn build_realization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/omega-machine-emission/src/function_realization/codec/mod.rs",
        coordination_marker: "impl FunctionRelativeOptimizationRealizationManifest",
    },
];

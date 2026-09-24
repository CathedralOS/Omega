use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_entry/mod.rs",
        coordination_marker: "pub fn bind_optimized_program_storage_semantic_entry_contract",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_wrapper/mod.rs",
        coordination_marker: "pub fn plan_optimized_program_storage_semantic_wrapper",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/native-realization/src/native_realization/providers/settlements/mod.rs",
        coordination_marker: "pub(crate) fn settle_provider_executions",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/register-environment/src/lib.rs",
        coordination_marker: "pub fn baseline_target_register_environment",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/03_target-operations-to-selected-instructions/src/optimized.rs",
        coordination_marker: "pub fn stage_optimized_instruction_selection",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/02_abstract-operations-to-target-operations/src/lowering/optimized.rs",
        coordination_marker: "validate_abstract_to_target_translation",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/07_selected-instructions-to-selected-instructions/src/analyses/legality/mod.rs",
        coordination_marker: "pub fn stage_optimized_allocation_legality_with_availability",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/08_selected-instructions-to-register-homes/src/assignment/baseline/mod.rs",
        coordination_marker: "pub fn stage_optimized_register_homes",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/07_selected-instructions-to-selected-instructions/src/analyses/liveness/staging/mod.rs",
        coordination_marker: "pub fn stage_optimized_liveness",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/07_selected-instructions-to-selected-instructions/src/analyses/live_ranges/staging/mod.rs",
        coordination_marker: "pub fn stage_optimized_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/07_selected-instructions-to-selected-instructions/src/rewrites/fixed_view/fixed_view_copies/mod.rs",
        coordination_marker: "pub fn stage_optimized_fixed_view_copies",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/07_selected-instructions-to-selected-instructions/src/analyses/reanalysis/mod.rs",
        coordination_marker: "pub fn stage_optimized_selected_reanalysis",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/09_register-homes-to-post-allocation-machine/src/post_allocation_machine.rs",
        coordination_marker: "fn stage_optimized_post_allocation_machine_plan",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/frame_layout/mod.rs",
        coordination_marker: "pub fn stage_target_frame_layout",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/frame_protocol/mod.rs",
        coordination_marker: "pub fn stage_target_frame_protocol_encoding",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/08_selected-instructions-to-register-homes/src/rewrites/rematerialization/mod.rs",
        coordination_marker: "pub fn stage_optimized_active_resident_rematerialization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/07_selected-instructions-to-selected-instructions/src/analyses/machine_effects/mod.rs",
        coordination_marker: "pub fn analyze_machine_effects",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/07_selected-instructions-to-selected-instructions/src/rewrites/literal_folds/mod.rs",
        coordination_marker: "pub fn run_selected_lowering_optimizations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/08_selected-instructions-to-register-homes/src/assignment/transformed/mod.rs",
        coordination_marker: "pub fn stage_optimized_register_homes_after_literal_folds",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/10_post-allocation-machine-to-selected-form-encoding/src/selected_form_encoding.rs",
        coordination_marker: "stage_optimized_layout_independent_selected_form_encoding",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/11_selected-form-encoding-to-resolved-layout/src/resolved_selected_form_layout.rs",
        coordination_marker: "stage_optimized_resolved_selected_form_layout",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/12_resolved-layout-to-resolved-layout/src/phase.rs",
        coordination_marker: "pub fn execute_resolved_layout_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/12_resolved-layout-to-resolved-layout/src/phase/validation.rs",
        coordination_marker: "pub fn validate_resolved_layout_optimization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/exit_contract/layout_optimization.rs",
        coordination_marker: "stage_whole_function_exit_contract_for_layout",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/exit_contract/validation/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/fragment_emission/mod.rs",
        coordination_marker: "stage_optimized_function_fragment_emission",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/text_placement/placement/mod.rs",
        coordination_marker: "place_fragment_text_section",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/text_placement/placement/production/mod.rs",
        coordination_marker: "mod fixed_frame",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/text_placement/placement/validation/mod.rs",
        coordination_marker: "pub(super) fn check",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/fragment_emission/projection/mod.rs",
        coordination_marker: "emit_resolved_function_fragments",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/frame_application/mod.rs",
        coordination_marker: "pub fn stage_function_fragment_frame_application",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/frame_application/insertion/mod.rs",
        coordination_marker: "apply_frame_protocol_to_fragments",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/fragment_emission/projection/validation/mod.rs",
        coordination_marker: "pub(super) fn check",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/fragment_emission/projection/production/mod.rs",
        coordination_marker: "pub(super) fn emit",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/fragment_emission/compute/mod.rs",
        coordination_marker: "pub(super) fn compute",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/text_placement/mod.rs",
        coordination_marker: "stage_optimized_fixed_frame_text_section",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/text_placement/assembly.rs",
        coordination_marker: "pub(super) fn compute_fixed_frame",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/object/object-file/src/fragment_container/mod.rs",
        coordination_marker: "stage_optimized_relocation_free_object_container",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/object/object-file/src/artifact_custody/mod.rs",
        coordination_marker: "stage_validated_optimized_object_artifact",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/artifacts/native-artifact/src/callable_entry.rs",
        coordination_marker: "stage_validated_optimized_ordinary_callable_entry",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/artifacts/native-artifact/src/semantic_wrapper_object/composition.rs",
        coordination_marker: "pub fn compose_optimized_program_storage_semantic_wrapper_object",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/function_realization/fixed_frame.rs",
        coordination_marker: "pub fn stage_fixed_frame_function_relative_realization",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/native-realization/src/entry_settlement/mod.rs",
        coordination_marker: "pub fn validate_native_program_entry_settlement",
    },
    RequiredCoordinationEntrance {
        coordination_marker: "pub fn realize_native_artifact",
        path: "omega-rust/omega/compiler/native-realization/src/native_realization.rs",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_review.rs",
        coordination_marker: "pub(crate) fn review_terminal_authority_closure",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/native-realization/src/native_realization/terminal_authority_policy/mod.rs",
        coordination_marker: "pub fn terminal_authority_policy_with_rows",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/native-realization/src/native_realization/providers/mod.rs",
        coordination_marker: "pub(crate) fn admit_native_providers",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/plans/program-entry-plan/src/optimized_semantic_wrapper/encoding.rs",
        coordination_marker: "pub fn select_optimized_program_storage_semantic_wrapper_encoding",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/native-realization/src/optimized_semantic_wrapper_object/mod.rs",
        coordination_marker: "pub fn stage_validated_optimized_program_storage_semantic_wrapper_object",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/native-realization/src/optimized_semantic_wrapper_object/validation/mod.rs",
        coordination_marker: "pub fn validate_optimized_program_storage_semantic_wrapper_object",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/native-realization/src/optimized_semantic_wrapper_object/object.rs",
        coordination_marker: "pub(crate) fn construct_object",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/compiler/native-realization/src/native_pipeline/physical_pipeline/mod.rs",
        coordination_marker: "pub fn stage_optimized_verified_physical_pipeline",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/10_post-allocation-machine-to-selected-form-encoding/src/validation/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/11_selected-form-encoding-to-resolved-layout/src/resolved_selected_form_layout/validation/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/11_selected-form-encoding-to-resolved-layout/src/resolved_selected_form_layout/validation/ordinary/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/backend/machine-emission/src/function_realization/codec/mod.rs",
        coordination_marker: "impl FunctionRelativeOptimizationRealizationManifest",
    },
];

use super::super::RequiredCoordinationEntrance;

pub(crate) const ENTRANCES: &[RequiredCoordinationEntrance] = &[
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/omega-selected-instructions/src/selected_instructions/effects/catalog.rs",
        coordination_marker: "pub fn validate_machine_effect_catalog",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/live_ranges/validate/replay/mod.rs",
        coordination_marker: "pub(super) fn replay_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/representations/omega-legalized-operations/src/legalized_operations/validation/mod.rs",
        coordination_marker: "impl LegalizedCallUnit",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/allocation_recovery/fixed_view_copy/codec/mod.rs",
        coordination_marker: "impl FixedViewCopyPlan",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/allocation_recovery/fixed_view_copy/codec/selected/mod.rs",
        coordination_marker: "fn decode_selected_plan",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/live_ranges/validate.rs",
        coordination_marker: "pub fn validate_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/live_ranges/compute.rs",
        coordination_marker: "pub(crate) fn compute_terminal_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/liveness/validate/mod.rs",
        coordination_marker: "pub fn validate_liveness",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/home_assignment/mod.rs",
        coordination_marker: "compute::compute_terminal_register_homes(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/fixed_precolored_segment_homes/mod.rs",
        coordination_marker: "pub fn assign_fixed_precolored_segment_homes",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/fixed_view/fixed_precolored_segment_homes/mod.rs",
        coordination_marker: "pub fn stage_optimized_fixed_precolored_segment_homes",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/home_assignment/compute/mod.rs",
        coordination_marker: "compute_function(index, legality, ranges, physical)",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/home_assignment/validate/mod.rs",
        coordination_marker: "replay::validate_function(function_index, actual, legality, ranges, physical)",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/post_allocation_manifest/mod.rs",
        coordination_marker: "pub fn project_post_allocation_optimization_manifest",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/mod.rs",
        coordination_marker: "pub fn assign_registers",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/function/mod.rs",
        coordination_marker: "pub(super) fn assign_function",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/function/unit/mod.rs",
        coordination_marker: "pub(super) fn assign",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/function/unit/foreign_call/mod.rs",
        coordination_marker: "pub(super) fn assign",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-assigned-target-operations/src/assignment/function/operation_routes.rs",
        coordination_marker: "pub(super) fn assign_operation",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source/mod.rs",
        coordination_marker: "pub(crate) fn derive_source_function_rosters",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source/conditions/mod.rs",
        coordination_marker: "pub(super) fn derive",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source/scalar_call_unit/mod.rs",
        coordination_marker: "pub(super) fn derive_source_scalar_call_unit_function",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay/conditions/mod.rs",
        coordination_marker: "pub(super) fn replay",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay/scalar_call_unit/mod.rs",
        coordination_marker: "pub(super) fn replay_scalar_call_unit_function",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source/leaves/mod.rs",
        coordination_marker: "pub(super) fn derive_leaf",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source/structural/mod.rs",
        coordination_marker: "pub(super) fn derive_source_structural_unit_function",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay/mod.rs",
        coordination_marker: "pub(crate) fn replay_terminal_legalized_plan",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay/structural/mod.rs",
        coordination_marker: "pub(super) fn replay_structural_unit_function",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/replay/leaf/mod.rs",
        coordination_marker: "pub(super) fn replay_leaf",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/projected_structural_call_return/source/mod.rs",
        coordination_marker: "pub(in crate::legalization) fn derive",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/projected_structural_call_return/replay/mod.rs",
        coordination_marker: "pub(in crate::legalization) fn replay",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/mod.rs",
        coordination_marker: "pub fn select_instructions",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/mod.rs",
        coordination_marker: "pub(super) fn build_plan",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/scalar/mod.rs",
        coordination_marker: "let body = catalog::build(&context)?",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/scalar/blocks/entry/mod.rs",
        coordination_marker: "pub(in crate::selection::construction::scalar) fn condition",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/structural_unit/mod.rs",
        coordination_marker: "let call = call::build(function, source, plan, layout, keys, catalog)?",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/projected_structural_call_return/mod.rs",
        coordination_marker: "let call = constraints::call(&fragments, selection, physical, catalog)?",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/validation/projected_structural_call_return/mod.rs",
        coordination_marker: "target::replay(selected, constraints, physical, catalog)",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/validation/structural_unit/mod.rs",
        coordination_marker: "pub(super) fn reconstruct_structural_unit_contract",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/selected_lowering/literal_fold/mod.rs",
        coordination_marker: "compute::compute_terminal_literal_fold(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/selected_lowering/literal_fold/compute/mod.rs",
        coordination_marker: "derive_function_folds(selected, recovery, &rows)",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/selected_lowering/literal_fold/validate/mod.rs",
        coordination_marker: "reconstruct_literal_fold(selected, recovery, &rows)",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/allocation_recovery/pressure_rematerialization/mod.rs",
        coordination_marker: "pub fn rematerialize_selected_active_resident",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/validation/mod.rs",
        coordination_marker: "pub fn validate_selected_instructions",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/validation/blocks/mod.rs",
        coordination_marker: "pub(super) fn validate_selected_blocks",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/validation/blocks/entry_control/mod.rs",
        coordination_marker: "pub(super) fn validate",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/spill_choice/mod.rs",
        coordination_marker: "pub fn choose_spill_victims",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/logical_spill_operations/mod.rs",
        coordination_marker: "pub fn plan_logical_spill_operations",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/logical_spill_operations/compute/mod.rs",
        coordination_marker: "action::compute_action(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/logical_spill_operations/validate/mod.rs",
        coordination_marker: "replay::replay_action(",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/stack_slot_coloring/mod.rs",
        coordination_marker: "pub fn color_logical_spill_stack_slots",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/stack_slot_coloring/compute/mod.rs",
        coordination_marker: "color_intervals_first_fit(function, logical.machine, intervals)",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/stack_slot_coloring/validate/mod.rs",
        coordination_marker: "let expected = replay::replay(source)?;",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/abstract_spill_insertion/mod.rs",
        coordination_marker: "pub fn schedule_abstract_spill_insertion",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/reload_value_homes/mod.rs",
        coordination_marker: "pub fn assign_reload_value_homes",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/spill_recovery_choice/mod.rs",
        coordination_marker: "pub fn choose_spill_recovery_victims",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/spill_recovery_actions/mod.rs",
        coordination_marker: "pub fn plan_spill_recovery_actions",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/generalized_spill_insertion/mod.rs",
        coordination_marker: "pub fn schedule_generalized_spill_insertion",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/generalized_reload_value_homes/mod.rs",
        coordination_marker: "pub fn assign_generalized_reload_value_homes",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/generalized_spill_recovery_worklist/mod.rs",
        coordination_marker: "pub fn seed_generalized_spill_recovery_worklist",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/generalized_spill_recovery_choice/mod.rs",
        coordination_marker: "pub fn choose_generalized_spill_recovery_victims",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/generalized_spill_recovery_actions/mod.rs",
        coordination_marker: "pub fn plan_generalized_spill_recovery_actions",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/recursive_spill_insertion/mod.rs",
        coordination_marker: "pub fn schedule_recursive_spill_insertion",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/recursive_reload_value_homes/mod.rs",
        coordination_marker: "pub fn assign_recursive_reload_value_homes",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/spill_pseudo_instructions/mod.rs",
        coordination_marker: "pub fn lower_recursive_spill_pseudos",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/spill_pseudo_instructions/homed/mod.rs",
        coordination_marker: "pub fn lower_homed_recursive_spill_pseudos",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/abstract_spill_memory_effects/mod.rs",
        coordination_marker: "pub fn derive_abstract_spill_memory_effects",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/abstract_spill_access_constraints/mod.rs",
        coordination_marker: "pub fn constrain_abstract_spill_accesses",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/preservation/mod.rs",
        coordination_marker: "pub fn stage_allocated_callee_saved_requirements",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-frame-layout/src/save_storage/mod.rs",
        coordination_marker: "pub fn stage_non_authoritative_callee_save_storage",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-post-allocation-machine-to-frame-layout/src/spill_requirements/mod.rs",
        coordination_marker: "pub fn stage_non_authoritative_spill_frame_requirements",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/spill_recovery_worklist/mod.rs",
        coordination_marker: "pub fn seed_spill_recovery_worklist",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/assignment/synthetic_reload_values/mod.rs",
        coordination_marker: "pub fn bind_synthetic_reload_values",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/allocation_legality/mod.rs",
        coordination_marker: "pub fn analyze_allocation_legality",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/fixed_precolored_intervals/mod.rs",
        coordination_marker: "pub fn analyze_fixed_precolored_intervals",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/fixed_precolored_split_requirements/mod.rs",
        coordination_marker: "pub fn analyze_fixed_precolored_split_requirements",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/allocator_availability/mod.rs",
        coordination_marker: "pub fn materialize_allocator_availability",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/live_ranges/mod.rs",
        coordination_marker: "pub fn analyze_live_ranges",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/liveness/mod.rs",
        coordination_marker: "pub fn analyze_liveness",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/analyses/recovery_classification/mod.rs",
        coordination_marker: "pub fn classify_pressure_recovery",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/allocation_recovery/fixed_view_copy/mod.rs",
        coordination_marker: "pub fn materialize_fixed_view_copies",
    },
    RequiredCoordinationEntrance {
        path: "omega-rust/omega/pipeline/omega-selected-instructions-to-register-homes/src/rewrites/allocation_recovery/fixed_view_copy/validate/mod.rs",
        coordination_marker: "pub fn validate_fixed_view_copies",
    },
];

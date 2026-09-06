//! Canonical direct rel32-to-rel8 fixtures shared by the rule's test leaves.

use crate::tests::*;

pub(super) fn stage_with_budget(
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let homes = physical_homes();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let encoding = stage_optimized_layout_independent_selected_form_encoding(
        selected_stage.selected(),
        &machine,
        physical,
    )
    .unwrap();
    let baseline_layout = stage_optimized_resolved_selected_form_layout(
        selected_stage.selected(),
        &machine,
        physical,
        &encoding,
    )
    .unwrap();
    stage_optimized_x86_branch_relaxation(
        selected_stage.selected(),
        &machine,
        physical,
        &encoding,
        &baseline_layout,
        budget,
    )
}

pub(super) fn physical_homes() -> StagedOptimizedRegisterHomes {
    let (semantic, proof) = disconnected_conditional_artifact();
    let selections =
        OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
    let selected = stage_optimized_instruction_selection(target).unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    stage_optimized_register_homes(legality).unwrap()
}

pub(super) fn direct_realization() -> StagedFunctionRelativeLayoutOptimizationRealization {
    direct_realization_for(false)
}

pub(super) fn alternate_direct_realization() -> StagedFunctionRelativeLayoutOptimizationRealization
{
    direct_realization_for(true)
}

fn direct_realization_for(subtract: bool) -> StagedFunctionRelativeLayoutOptimizationRealization {
    let (semantic, proof) = conditional_exact_binary_artifact(subtract);
    let selections =
        OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_x64(),
        &[],
    )
    .unwrap();
    (staged)
        .into_function_relative_layout_for_test()
        .unwrap_or_else(|| {
            panic!("the exact rel8 selection must use the direct layout realization route")
        })
}

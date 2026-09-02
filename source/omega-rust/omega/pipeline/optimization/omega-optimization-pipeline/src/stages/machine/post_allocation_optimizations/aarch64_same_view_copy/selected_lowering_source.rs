//! Pipeline staging from selected-lowering register-home and machine-plan roots.

use omega_optimization_core::Optimization;

use crate::{
    OptimizedPostAllocationMachineOptimizationError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
};

use super::{
    StagedOptimizedAarch64SameViewCopyElision,
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt, stage_with_inputs,
    validate_with_inputs,
};

pub fn stage_optimized_aarch64_same_view_copy_elision_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
    OptimizedPostAllocationMachineOptimizationError,
> {
    stage(
        source,
        machine,
        Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
    )
}

pub fn stage_optimized_aarch64_same_view_copy_before_compare_zero_elision_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
    OptimizedPostAllocationMachineOptimizationError,
> {
    stage(
        source,
        machine,
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1,
    )
}

pub fn stage_optimized_aarch64_same_view_copy_before_compare_i64_left_operand_elision_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
    OptimizedPostAllocationMachineOptimizationError,
> {
    stage(
        source,
        machine,
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1,
    )
}

pub fn stage_optimized_aarch64_same_view_copy_before_compare_i64_right_operand_elision_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
    OptimizedPostAllocationMachineOptimizationError,
> {
    stage(
        source,
        machine,
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1,
    )
}

pub fn validate_optimized_aarch64_same_view_copy_elision_after_selected_lowering_custody(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedAarch64SameViewCopyElision,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        source, machine,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let run = source.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    match run.steps().last() {
        Some(step) => validate_with_inputs(
            step.fold(),
            step.liveness(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            staged,
        ),
        None => validate_with_inputs(
            selected_stage.selected(),
            run.source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .liveness(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            staged,
        ),
    }
}

fn stage(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    optimization: Optimization,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        source, machine,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let run = source.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    match run.steps().last() {
        Some(step) => stage_with_inputs(
            step.fold(),
            step.liveness(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            optimization,
        ),
        None => stage_with_inputs(
            selected_stage.selected(),
            run.source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .liveness(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            optimization,
        ),
    }
}

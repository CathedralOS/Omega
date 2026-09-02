//! Pipeline staging from baseline register-home and machine-plan roots.

use omega_optimization_core::Optimization;

use crate::{
    OptimizedPostAllocationMachineOptimizationError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, validate_optimized_post_allocation_machine_plan_custody,
};

use super::{
    StagedOptimizedAarch64SameViewCopyElision,
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt, stage_with_inputs,
    validate_with_inputs,
};

pub fn stage_optimized_aarch64_same_view_copy_elision(
    source: &StagedOptimizedRegisterHomes,
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

pub fn stage_optimized_aarch64_same_view_copy_before_compare_zero_elision(
    source: &StagedOptimizedRegisterHomes,
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

pub fn stage_optimized_aarch64_same_view_copy_before_compare_i64_left_operand_elision(
    source: &StagedOptimizedRegisterHomes,
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

pub fn stage_optimized_aarch64_same_view_copy_before_compare_i64_right_operand_elision(
    source: &StagedOptimizedRegisterHomes,
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

pub fn validate_optimized_aarch64_same_view_copy_elision_custody(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedAarch64SameViewCopyElision,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_custody(source, machine)
        .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let ranges = source.legality_stage().live_range_stage();
    let selected_stage = ranges.liveness_stage().selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    validate_with_inputs(
        selected_stage.selected(),
        ranges.liveness_stage().liveness(),
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
        staged,
    )
}

fn stage(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    optimization: Optimization,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_custody(source, machine)
        .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let ranges = source.legality_stage().live_range_stage();
    let selected_stage = ranges.liveness_stage().selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    stage_with_inputs(
        selected_stage.selected(),
        ranges.liveness_stage().liveness(),
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
        optimization,
    )
}

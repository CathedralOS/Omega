//! Pipeline staging from current allocation and machine-plan facts.

use omega_optimization_core::Optimization;

use crate::{
    OptimizedPostAllocationMachineOptimizationError, StagedOptimizedPostAllocationMachinePlan,
};

use super::{
    StagedOptimizedAarch64SameViewCopyElision,
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt, stage_with_inputs,
    validate_with_inputs,
};

pub fn stage_optimized_aarch64_same_view_copy_elision(
    source: &impl crate::AllocationSource,
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
    source: &impl crate::AllocationSource,
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
    source: &impl crate::AllocationSource,
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
    source: &impl crate::AllocationSource,
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
    source: &impl crate::AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedAarch64SameViewCopyElision,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let allocation = crate::replay_machine_source(source, machine)?;
    validate_with_inputs(
        allocation.selected(),
        allocation.liveness(),
        machine,
        allocation.register_environment().physical(),
        allocation.selections(),
        allocation.budget_per_pass(),
        staged,
    )
}

fn stage(
    source: &impl crate::AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    optimization: Optimization,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let allocation = crate::replay_machine_source(source, machine)?;
    stage_with_inputs(
        allocation.selected(),
        allocation.liveness(),
        machine,
        allocation.register_environment().physical(),
        allocation.selections(),
        allocation.budget_per_pass(),
        optimization,
    )
}

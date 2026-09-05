//! Exact core-rule dispatch and independent replay authentication.

use crate::{
    ValidatedAarch64SameViewCopyElision,
    optimize_aarch64_same_view_copy_i64_before_compare_i64_left_operand,
    optimize_aarch64_same_view_copy_i64_before_compare_i64_right_operand,
    optimize_aarch64_same_view_copy_i64_before_compare_zero,
    optimize_aarch64_same_view_copy_i64_before_return, require_post_allocation_machine_rule,
    validate_aarch64_same_view_copy_elision,
    validate_aarch64_same_view_copy_i64_before_compare_i64_left_operand,
    validate_aarch64_same_view_copy_i64_before_compare_i64_right_operand,
    validate_aarch64_same_view_copy_i64_before_compare_zero,
};
use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelections, OptimizationWorkBudget,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::{ValidatedLiveness, ValidatedSelectedAnalysis};

use crate::{
    OptimizedPostAllocationMachineOptimizationError, StagedOptimizedPostAllocationMachinePlan,
};

use super::{
    StagedOptimizedAarch64SameViewCopyElision,
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt,
};

pub(super) fn stage_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
    optimization: Optimization,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase_selections =
        selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let phase = require_post_allocation_machine_rule(
        &phase_selections,
        optimization,
        machine.machine().plan().target.architecture,
    )?;
    let elision = match optimization {
        Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1 => {
            optimize_aarch64_same_view_copy_i64_before_return(
                selected,
                liveness,
                machine.machine(),
                physical,
                budget,
            )
        }
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1 => {
            optimize_aarch64_same_view_copy_i64_before_compare_zero(
                selected,
                liveness,
                machine.machine(),
                physical,
                budget,
            )
        }
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1 => {
            optimize_aarch64_same_view_copy_i64_before_compare_i64_left_operand(
                selected,
                liveness,
                machine.machine(),
                physical,
                budget,
            )
        }
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1 => {
            optimize_aarch64_same_view_copy_i64_before_compare_i64_right_operand(
                selected,
                liveness,
                machine.machine(),
                physical,
                budget,
            )
        }
        _ => unreachable!("catalog dispatch supplies an exact same-view copy rule"),
    }
    .map_err(OptimizedPostAllocationMachineOptimizationError::SameViewCopyElision)?;
    let custody = custody_receipt(optimization, selections, &phase, &elision);
    Ok(StagedOptimizedAarch64SameViewCopyElision::new(
        elision, custody,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
    staged: &StagedOptimizedAarch64SameViewCopyElision,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let staged_custody = staged.custody();
    let phase_selections =
        selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let phase = require_post_allocation_machine_rule(
        &phase_selections,
        staged_custody.optimization(),
        machine.machine().plan().target.architecture,
    )?;
    if staged.elision().plan().budget != budget {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let replayed = match staged_custody.optimization() {
        Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1 => {
            validate_aarch64_same_view_copy_elision(
                selected,
                liveness,
                machine.machine(),
                physical,
                staged.elision().plan().clone(),
            )
        }
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1 => {
            validate_aarch64_same_view_copy_i64_before_compare_zero(
                selected,
                liveness,
                machine.machine(),
                physical,
                staged.elision().plan().clone(),
            )
        }
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1 => {
            validate_aarch64_same_view_copy_i64_before_compare_i64_left_operand(
                selected,
                liveness,
                machine.machine(),
                physical,
                staged.elision().plan().clone(),
            )
        }
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1 => {
            validate_aarch64_same_view_copy_i64_before_compare_i64_right_operand(
                selected,
                liveness,
                machine.machine(),
                physical,
                staged.elision().plan().clone(),
            )
        }
        _ => return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch),
    }
    .map_err(OptimizedPostAllocationMachineOptimizationError::SameViewCopyElision)?;
    if replayed.receipt() != staged.elision().receipt() {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let custody = custody_receipt(staged_custody.optimization(), selections, &phase, &replayed);
    if custody != staged_custody {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn custody_receipt(
    optimization: Optimization,
    selections: &OptimizationSelections,
    phase: &OptimizationSelections,
    elision: &ValidatedAarch64SameViewCopyElision,
) -> StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt {
    let receipt = elision.receipt();
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt::new(
        optimization,
        selections.identity(),
        phase.identity(),
        receipt.source(),
        receipt.identity(),
        receipt.action_count(),
    )
}

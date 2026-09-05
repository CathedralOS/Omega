use crate::{
    ValidatedX86MovR32Imm32Materialization, optimize_x86_materialize_i64_with_mov_r32_imm32,
    require_post_allocation_machine_rule, validate_x86_mov_r32_imm32_materialization,
};
use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelections, OptimizationWorkBudget,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::StagedOptimizedPostAllocationMachinePlan;

use super::super::OptimizedPostAllocationMachineOptimizationError;
use super::{
    StagedOptimizedX86MovR32Imm32Materialization,
    StagedOptimizedX86MovR32Imm32MaterializationCustodyReceipt,
};

pub fn stage_optimized_x86_mov_r32_imm32_materialization(
    source: &impl crate::AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedX86MovR32Imm32Materialization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let allocation = crate::replay_machine_source(source, machine)?;
    stage_with_inputs(
        allocation.selected(),
        machine,
        allocation.register_environment().physical(),
        allocation.selections(),
        allocation.budget_per_pass(),
    )
}

pub fn validate_optimized_x86_mov_r32_imm32_materialization_custody(
    source: &impl crate::AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedX86MovR32Imm32Materialization,
) -> Result<
    StagedOptimizedX86MovR32Imm32MaterializationCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let allocation = crate::replay_machine_source(source, machine)?;
    validate_with_inputs(
        allocation.selected(),
        machine,
        allocation.register_environment().physical(),
        allocation.selections(),
        allocation.budget_per_pass(),
        staged,
    )
}

fn stage_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<
    StagedOptimizedX86MovR32Imm32Materialization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase_selections =
        selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let phase = require_post_allocation_machine_rule(
        &phase_selections,
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        machine.machine().plan().target.architecture,
    )?;
    let materialization = optimize_x86_materialize_i64_with_mov_r32_imm32(
        selected,
        machine.machine(),
        physical,
        budget,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::X86MovR32Imm32Materialization)?;
    let custody = custody_receipt(selections, &phase, &materialization);
    Ok(StagedOptimizedX86MovR32Imm32Materialization {
        materialization,
        custody,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
    staged: &StagedOptimizedX86MovR32Imm32Materialization,
) -> Result<
    StagedOptimizedX86MovR32Imm32MaterializationCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase_selections =
        selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let phase = require_post_allocation_machine_rule(
        &phase_selections,
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        machine.machine().plan().target.architecture,
    )?;
    if staged.materialization.plan().budget != budget {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let replayed = validate_x86_mov_r32_imm32_materialization(
        selected,
        machine.machine(),
        physical,
        staged.materialization.plan().clone(),
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::X86MovR32Imm32Materialization)?;
    if replayed.receipt() != staged.materialization.receipt() {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let custody = custody_receipt(selections, &phase, &replayed);
    if custody != staged.custody {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn custody_receipt(
    selections: &OptimizationSelections,
    phase: &OptimizationSelections,
    materialization: &ValidatedX86MovR32Imm32Materialization,
) -> StagedOptimizedX86MovR32Imm32MaterializationCustodyReceipt {
    let receipt = materialization.receipt();
    StagedOptimizedX86MovR32Imm32MaterializationCustodyReceipt {
        selections: selections.identity(),
        post_allocation_machine_selections: phase.identity(),
        source: receipt.source(),
        materialization: receipt.identity(),
        action_count: receipt.action_count(),
        baseline_bytes: receipt.baseline_bytes(),
        selected_bytes: receipt.selected_bytes(),
    }
}

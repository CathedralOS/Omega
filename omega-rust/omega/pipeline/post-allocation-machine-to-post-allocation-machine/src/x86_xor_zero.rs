use crate::{
    ValidatedX86XorZeroMaterialization, optimize_x86_materialize_i64_zero_with_xor,
    require_post_allocation_machine_rule, validate_x86_xor_zero_materialization,
};
use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelections, OptimizationWorkBudget,
};
use physical_instructions::X86XorZeroMaterializationCustodyReceipt;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::{ValidatedLiveness, ValidatedSelectedAnalysis};

use crate::StagedOptimizedPostAllocationMachinePlan;

use super::OptimizedPostAllocationMachineOptimizationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedX86XorZeroMaterialization {
    materialization: ValidatedX86XorZeroMaterialization,
    custody: X86XorZeroMaterializationCustodyReceipt,
}

impl StagedOptimizedX86XorZeroMaterialization {
    pub const fn materialization(&self) -> &ValidatedX86XorZeroMaterialization {
        &self.materialization
    }

    pub const fn custody(&self) -> X86XorZeroMaterializationCustodyReceipt {
        self.custody
    }
}

pub fn stage_optimized_x86_xor_zero_materialization(
    source: &impl crate::AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<StagedOptimizedX86XorZeroMaterialization, OptimizedPostAllocationMachineOptimizationError>
{
    let allocation = crate::replay_machine_source(source, machine)?;
    stage_with_inputs(
        allocation.selected(),
        allocation.liveness(),
        machine,
        allocation.register_environment().physical(),
        allocation.selections(),
        allocation.budget_per_pass(),
    )
}

pub fn validate_optimized_x86_xor_zero_materialization_custody(
    source: &impl crate::AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedX86XorZeroMaterialization,
) -> Result<X86XorZeroMaterializationCustodyReceipt, OptimizedPostAllocationMachineOptimizationError>
{
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

fn stage_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86XorZeroMaterialization, OptimizedPostAllocationMachineOptimizationError>
{
    let phase_selections =
        selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let phase = require_post_allocation_machine_rule(
        &phase_selections,
        Optimization::X86SelectXorZeroI64MaterializationV1,
        machine.machine().plan().target.architecture,
    )?;
    let materialization = optimize_x86_materialize_i64_zero_with_xor(
        selected,
        liveness,
        machine.machine(),
        physical,
        budget,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::X86XorZeroMaterialization)?;
    let custody = custody_receipt(selections, &phase, &materialization);
    Ok(StagedOptimizedX86XorZeroMaterialization {
        materialization,
        custody,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
    staged: &StagedOptimizedX86XorZeroMaterialization,
) -> Result<X86XorZeroMaterializationCustodyReceipt, OptimizedPostAllocationMachineOptimizationError>
{
    let phase_selections =
        selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let phase = require_post_allocation_machine_rule(
        &phase_selections,
        Optimization::X86SelectXorZeroI64MaterializationV1,
        machine.machine().plan().target.architecture,
    )?;
    if staged.materialization.plan().budget != budget {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let replayed = validate_x86_xor_zero_materialization(
        selected,
        liveness,
        machine.machine(),
        physical,
        staged.materialization.plan().clone(),
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::X86XorZeroMaterialization)?;
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
    materialization: &ValidatedX86XorZeroMaterialization,
) -> X86XorZeroMaterializationCustodyReceipt {
    let receipt = materialization.receipt();
    X86XorZeroMaterializationCustodyReceipt::from_parts(
        selections.identity(),
        phase.identity(),
        receipt.source(),
        receipt.identity(),
        receipt.action_count(),
        receipt.baseline_bytes(),
        receipt.selected_bytes(),
    )
}

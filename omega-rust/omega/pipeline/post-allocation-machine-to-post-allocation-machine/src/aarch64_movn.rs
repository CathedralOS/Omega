use crate::{
    ValidatedAarch64MovnMaterialization, optimize_aarch64_materialize_i64_with_shortest_movn_seed,
    require_post_allocation_machine_rule, validate_aarch64_movn_materialization,
};
use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelections, OptimizationWorkBudget,
};
use physical_instructions::Aarch64MovnMaterializationCustodyReceipt;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::StagedOptimizedPostAllocationMachinePlan;

use super::OptimizedPostAllocationMachineOptimizationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedAarch64MovnMaterialization {
    materialization: ValidatedAarch64MovnMaterialization,
    custody: Aarch64MovnMaterializationCustodyReceipt,
}

impl StagedOptimizedAarch64MovnMaterialization {
    pub const fn materialization(&self) -> &ValidatedAarch64MovnMaterialization {
        &self.materialization
    }
    pub const fn custody(&self) -> Aarch64MovnMaterializationCustodyReceipt {
        self.custody
    }
}

pub fn stage_optimized_aarch64_movn_materialization(
    source: &impl crate::AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedAarch64MovnMaterialization,
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

pub fn validate_optimized_aarch64_movn_materialization_custody(
    source: &impl crate::AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedAarch64MovnMaterialization,
) -> Result<Aarch64MovnMaterializationCustodyReceipt, OptimizedPostAllocationMachineOptimizationError>
{
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
    StagedOptimizedAarch64MovnMaterialization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase_selections =
        selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let phase = require_post_allocation_machine_rule(
        &phase_selections,
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
        machine.machine().plan().target.architecture,
    )?;
    let materialization = optimize_aarch64_materialize_i64_with_shortest_movn_seed(
        selected,
        machine.machine(),
        physical,
        budget,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::MovnMaterialization)?;
    let custody = custody_receipt(selections, &phase, &materialization);
    Ok(StagedOptimizedAarch64MovnMaterialization {
        materialization,
        custody,
    })
}

fn validate_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
    staged: &StagedOptimizedAarch64MovnMaterialization,
) -> Result<Aarch64MovnMaterializationCustodyReceipt, OptimizedPostAllocationMachineOptimizationError>
{
    let phase_selections =
        selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let phase = require_post_allocation_machine_rule(
        &phase_selections,
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
        machine.machine().plan().target.architecture,
    )?;
    if staged.materialization.plan().budget != budget {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let replayed = validate_aarch64_movn_materialization(
        selected,
        machine.machine(),
        physical,
        staged.materialization.plan().clone(),
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::MovnMaterialization)?;
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
    materialization: &ValidatedAarch64MovnMaterialization,
) -> Aarch64MovnMaterializationCustodyReceipt {
    let receipt = materialization.receipt();
    Aarch64MovnMaterializationCustodyReceipt::from_parts(
        selections.identity(),
        phase.identity(),
        receipt.source(),
        receipt.identity(),
        receipt.action_count(),
        receipt.baseline_words(),
        receipt.selected_words(),
    )
}

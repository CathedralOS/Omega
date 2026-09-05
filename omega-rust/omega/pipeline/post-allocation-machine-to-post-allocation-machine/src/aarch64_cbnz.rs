use crate::{
    ValidatedAarch64CbnzFusion, optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz,
    require_post_allocation_machine_rule, validate_aarch64_cbnz_fusion,
};
use optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelections, OptimizationWorkBudget,
};
use physical_instructions::Aarch64CbnzFusionCustodyReceipt;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::{ValidatedLiveness, ValidatedSelectedAnalysis};

use crate::StagedOptimizedPostAllocationMachinePlan;

use super::OptimizedPostAllocationMachineOptimizationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedAarch64CbnzFusion {
    fusion: ValidatedAarch64CbnzFusion,
    custody: Aarch64CbnzFusionCustodyReceipt,
}

impl StagedOptimizedAarch64CbnzFusion {
    pub const fn fusion(&self) -> &ValidatedAarch64CbnzFusion {
        &self.fusion
    }

    pub const fn custody(&self) -> Aarch64CbnzFusionCustodyReceipt {
        self.custody
    }
}

pub fn stage_optimized_aarch64_cbnz_fusion(
    source: &impl crate::AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<StagedOptimizedAarch64CbnzFusion, OptimizedPostAllocationMachineOptimizationError> {
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

pub fn validate_optimized_aarch64_cbnz_fusion_custody(
    source: &impl crate::AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedAarch64CbnzFusion,
) -> Result<Aarch64CbnzFusionCustodyReceipt, OptimizedPostAllocationMachineOptimizationError> {
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
) -> Result<StagedOptimizedAarch64CbnzFusion, OptimizedPostAllocationMachineOptimizationError> {
    let phase_selections =
        selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let phase = require_post_allocation_machine_rule(
        &phase_selections,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        machine.machine().plan().target.architecture,
    )?;
    let fusion = optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz(
        selected,
        liveness,
        machine.machine(),
        physical,
        budget,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Fusion)?;
    let custody = custody_receipt(selections, &phase, &fusion);
    Ok(StagedOptimizedAarch64CbnzFusion { fusion, custody })
}

#[allow(clippy::too_many_arguments)]
fn validate_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
    staged: &StagedOptimizedAarch64CbnzFusion,
) -> Result<Aarch64CbnzFusionCustodyReceipt, OptimizedPostAllocationMachineOptimizationError> {
    let phase_selections =
        selections.project_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let phase = require_post_allocation_machine_rule(
        &phase_selections,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        machine.machine().plan().target.architecture,
    )?;
    if staged.fusion.plan().budget != budget {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let replayed = validate_aarch64_cbnz_fusion(
        selected,
        liveness,
        machine.machine(),
        physical,
        staged.fusion.plan().clone(),
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Fusion)?;
    if replayed.receipt() != staged.fusion.receipt() {
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
    fusion: &ValidatedAarch64CbnzFusion,
) -> Aarch64CbnzFusionCustodyReceipt {
    let receipt = fusion.receipt();
    Aarch64CbnzFusionCustodyReceipt::from_parts(
        selections.identity(),
        phase.identity(),
        receipt.source(),
        receipt.identity(),
        receipt.action_count(),
    )
}

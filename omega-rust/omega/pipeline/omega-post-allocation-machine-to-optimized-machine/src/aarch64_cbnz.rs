use omega_machine_optimizer::{
    Aarch64CbnzFusionIdentity, ValidatedAarch64CbnzFusion,
    optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz, require_post_allocation_machine_rule,
    validate_aarch64_cbnz_fusion,
};
use omega_optimization_core::{
    Optimization, OptimizationExecutionPhase, OptimizationSelectionIdentity,
    OptimizationSelections, OptimizationWorkBudget,
};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions_to_register_homes::{ValidatedLiveness, ValidatedSelectedAnalysis};

use crate::StagedOptimizedPostAllocationMachinePlan;

use super::OptimizedPostAllocationMachineOptimizationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedAarch64CbnzFusion {
    fusion: ValidatedAarch64CbnzFusion,
    custody: StagedOptimizedAarch64CbnzFusionCustodyReceipt,
}

impl StagedOptimizedAarch64CbnzFusion {
    pub const fn fusion(&self) -> &ValidatedAarch64CbnzFusion {
        &self.fusion
    }

    pub const fn custody(&self) -> StagedOptimizedAarch64CbnzFusionCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedAarch64CbnzFusionCustodyReceipt {
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: omega_machine_optimizer::PostAllocationMachineIdentity,
    fusion: Aarch64CbnzFusionIdentity,
    action_count: usize,
}

impl StagedOptimizedAarch64CbnzFusionCustodyReceipt {
    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        self.source
    }
    pub const fn fusion(self) -> Aarch64CbnzFusionIdentity {
        self.fusion
    }
    pub const fn action_count(self) -> usize {
        self.action_count
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
) -> Result<
    StagedOptimizedAarch64CbnzFusionCustodyReceipt,
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
) -> Result<
    StagedOptimizedAarch64CbnzFusionCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
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
) -> StagedOptimizedAarch64CbnzFusionCustodyReceipt {
    let receipt = fusion.receipt();
    StagedOptimizedAarch64CbnzFusionCustodyReceipt {
        selections: selections.identity(),
        post_allocation_machine_selections: phase.identity(),
        source: receipt.source(),
        fusion: receipt.identity(),
        action_count: receipt.action_count(),
    }
}

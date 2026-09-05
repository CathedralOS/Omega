use omega_machine_optimizer::{
    ValidatedX86XorZeroMaterialization, X86XorZeroMaterializationIdentity,
    optimize_x86_materialize_i64_zero_with_xor, require_post_allocation_machine_rule,
    validate_x86_xor_zero_materialization,
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
pub struct StagedOptimizedX86XorZeroMaterialization {
    materialization: ValidatedX86XorZeroMaterialization,
    custody: StagedOptimizedX86XorZeroMaterializationCustodyReceipt,
}

impl StagedOptimizedX86XorZeroMaterialization {
    pub const fn materialization(&self) -> &ValidatedX86XorZeroMaterialization {
        &self.materialization
    }

    pub const fn custody(&self) -> StagedOptimizedX86XorZeroMaterializationCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedX86XorZeroMaterializationCustodyReceipt {
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: omega_machine_optimizer::PostAllocationMachineIdentity,
    materialization: X86XorZeroMaterializationIdentity,
    action_count: usize,
    baseline_bytes: u64,
    selected_bytes: u64,
}

impl StagedOptimizedX86XorZeroMaterializationCustodyReceipt {
    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        self.source
    }
    pub const fn materialization(self) -> X86XorZeroMaterializationIdentity {
        self.materialization
    }
    pub const fn action_count(self) -> usize {
        self.action_count
    }
    pub const fn baseline_bytes(self) -> u64 {
        self.baseline_bytes
    }
    pub const fn selected_bytes(self) -> u64 {
        self.selected_bytes
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
) -> Result<
    StagedOptimizedX86XorZeroMaterializationCustodyReceipt,
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
) -> Result<
    StagedOptimizedX86XorZeroMaterializationCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
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
) -> StagedOptimizedX86XorZeroMaterializationCustodyReceipt {
    let receipt = materialization.receipt();
    StagedOptimizedX86XorZeroMaterializationCustodyReceipt {
        selections: selections.identity(),
        post_allocation_machine_selections: phase.identity(),
        source: receipt.source(),
        materialization: receipt.identity(),
        action_count: receipt.action_count(),
        baseline_bytes: receipt.baseline_bytes(),
        selected_bytes: receipt.selected_bytes(),
    }
}

use omega_machine_optimizer::{
    ValidatedX86XorZeroMaterialization, X86XorZeroMaterializationIdentity,
    optimize_x86_materialize_i64_zero_with_xor, require_post_allocation_machine_rule,
    validate_x86_xor_zero_materialization,
};
use omega_optimization_core::{
    Optimization, OptimizationSelectionIdentity, OptimizationSelections, OptimizationWorkBudget,
};
use omega_regalloc::{ValidatedLiveness, ValidatedSelectedAnalysis};
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterSelectedLowering,
    validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
};

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
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<StagedOptimizedX86XorZeroMaterialization, OptimizedPostAllocationMachineOptimizationError>
{
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
    )
}

pub fn validate_optimized_x86_xor_zero_materialization_custody(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedX86XorZeroMaterialization,
) -> Result<
    StagedOptimizedX86XorZeroMaterializationCustodyReceipt,
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

pub fn stage_optimized_x86_xor_zero_materialization_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<StagedOptimizedX86XorZeroMaterialization, OptimizedPostAllocationMachineOptimizationError>
{
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
        ),
    }
}

pub fn validate_optimized_x86_xor_zero_materialization_after_selected_lowering_custody(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedX86XorZeroMaterialization,
) -> Result<
    StagedOptimizedX86XorZeroMaterializationCustodyReceipt,
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

pub fn stage_optimized_x86_xor_zero_materialization_after_active_resident_rematerialization(
    source: &StagedOptimizedActiveResidentRematerialization,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<StagedOptimizedX86XorZeroMaterialization, OptimizedPostAllocationMachineOptimizationError>
{
    validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(
        source, machine,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let selected_stage = source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    stage_with_inputs(
        source.rematerialization(),
        source.liveness(),
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
    )
}

pub fn validate_optimized_x86_xor_zero_materialization_after_active_resident_rematerialization_custody(
    source: &StagedOptimizedActiveResidentRematerialization,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedX86XorZeroMaterialization,
) -> Result<
    StagedOptimizedX86XorZeroMaterializationCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(
        source, machine,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let selected_stage = source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    validate_with_inputs(
        source.rematerialization(),
        source.liveness(),
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
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
    let phase = require_post_allocation_machine_rule(
        selections,
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
    let phase = require_post_allocation_machine_rule(
        selections,
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

use omega_machine_optimizer::{
    ValidatedX86MovR64Imm32SignExtendedMaterialization,
    optimize_x86_materialize_i64_with_mov_r64_imm32_sign_extended,
    require_post_allocation_machine_rule, validate_x86_mov_r64_imm32_sign_extended_materialization,
};
use omega_optimization_core::{Optimization, OptimizationSelections, OptimizationWorkBudget};
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterSelectedLowering,
    validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
};

use super::super::OptimizedPostAllocationMachineOptimizationError;
use super::{
    StagedOptimizedX86MovR64Imm32SignExtendedMaterialization,
    StagedOptimizedX86MovR64Imm32SignExtendedMaterializationCustodyReceipt,
};

pub fn stage_optimized_x86_mov_r64_imm32_sign_extended_materialization(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedX86MovR64Imm32SignExtendedMaterialization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_custody(source, machine)
        .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let ranges = source.legality_stage().live_range_stage();
    let selected_stage = ranges.liveness_stage().selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    stage_with_inputs(
        selected_stage.selected(),
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
    )
}

pub fn validate_optimized_x86_mov_r64_imm32_sign_extended_materialization_custody(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedX86MovR64Imm32SignExtendedMaterialization,
) -> Result<
    StagedOptimizedX86MovR64Imm32SignExtendedMaterializationCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_custody(source, machine)
        .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let ranges = source.legality_stage().live_range_stage();
    let selected_stage = ranges.liveness_stage().selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    validate_with_inputs(
        selected_stage.selected(),
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
        staged,
    )
}

pub fn stage_optimized_x86_mov_r64_imm32_sign_extended_materialization_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedX86MovR64Imm32SignExtendedMaterialization,
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
        Some(step) => stage_with_inputs(
            step.fold(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
        ),
        None => stage_with_inputs(
            selected_stage.selected(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
        ),
    }
}

pub fn validate_optimized_x86_mov_r64_imm32_sign_extended_materialization_after_selected_lowering_custody(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedX86MovR64Imm32SignExtendedMaterialization,
) -> Result<
    StagedOptimizedX86MovR64Imm32SignExtendedMaterializationCustodyReceipt,
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
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            staged,
        ),
        None => validate_with_inputs(
            selected_stage.selected(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            staged,
        ),
    }
}

pub fn stage_optimized_x86_mov_r64_imm32_sign_extended_materialization_after_active_resident_rematerialization(
    source: &StagedOptimizedActiveResidentRematerialization,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedX86MovR64Imm32SignExtendedMaterialization,
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
    stage_with_inputs(
        source.rematerialization(),
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
    )
}

pub fn validate_optimized_x86_mov_r64_imm32_sign_extended_materialization_after_active_resident_rematerialization_custody(
    source: &StagedOptimizedActiveResidentRematerialization,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedX86MovR64Imm32SignExtendedMaterialization,
) -> Result<
    StagedOptimizedX86MovR64Imm32SignExtendedMaterializationCustodyReceipt,
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
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
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
    StagedOptimizedX86MovR64Imm32SignExtendedMaterialization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase = require_post_allocation_machine_rule(
        selections,
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
        machine.machine().plan().target.architecture,
    )?;
    let materialization = optimize_x86_materialize_i64_with_mov_r64_imm32_sign_extended(
        selected,
        machine.machine(),
        physical,
        budget,
    )
    .map_err(
        OptimizedPostAllocationMachineOptimizationError::X86MovR64Imm32SignExtendedMaterialization,
    )?;
    let custody = custody_receipt(selections, &phase, &materialization);
    Ok(StagedOptimizedX86MovR64Imm32SignExtendedMaterialization {
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
    staged: &StagedOptimizedX86MovR64Imm32SignExtendedMaterialization,
) -> Result<
    StagedOptimizedX86MovR64Imm32SignExtendedMaterializationCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase = require_post_allocation_machine_rule(
        selections,
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
        machine.machine().plan().target.architecture,
    )?;
    if staged.materialization.plan().budget != budget {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let replayed = validate_x86_mov_r64_imm32_sign_extended_materialization(
        selected,
        machine.machine(),
        physical,
        staged.materialization.plan().clone(),
    )
    .map_err(
        OptimizedPostAllocationMachineOptimizationError::X86MovR64Imm32SignExtendedMaterialization,
    )?;
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
    materialization: &ValidatedX86MovR64Imm32SignExtendedMaterialization,
) -> StagedOptimizedX86MovR64Imm32SignExtendedMaterializationCustodyReceipt {
    let receipt = materialization.receipt();
    StagedOptimizedX86MovR64Imm32SignExtendedMaterializationCustodyReceipt {
        selections: selections.identity(),
        post_allocation_machine_selections: phase.identity(),
        source: receipt.source(),
        materialization: receipt.identity(),
        action_count: receipt.action_count(),
        baseline_bytes: receipt.baseline_bytes(),
        selected_bytes: receipt.selected_bytes(),
    }
}

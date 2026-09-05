use super::super::{assembly::*, carriers::*, error::*, prelude::*};
use selected_instructions_to_register_homes::{AllocationSource, RetainedAllocation};

pub fn stage_selected_lowering_function_relative_realization(
    allocation: RetainedAllocation,
) -> Result<
    StagedSelectedLoweringFunctionRelativeRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    let current = allocation
        .replay_allocation()
        .map_err(FunctionRelativeOptimizationRealizationError::Allocation)?;
    let source = selected_lowering_source(&current)?;
    let machine = stage_optimized_post_allocation_machine_plan(&current)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    let (encoding, baseline_layout, relaxation, exit_contract, manifest) =
        build_realization(&current, &machine)?;
    let custody = custody_receipt(source, &machine, &exit_contract, &manifest);
    Ok(StagedSelectedLoweringFunctionRelativeRealization {
        allocation,
        machine,
        encoding,
        baseline_layout,
        relaxation,
        exit_contract,
        manifest,
        custody,
    })
}

pub fn validate_selected_lowering_function_relative_realization_custody(
    staged: &StagedSelectedLoweringFunctionRelativeRealization,
) -> Result<
    StagedSelectedLoweringFunctionRelativeRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let current = staged
        .allocation
        .replay_allocation()
        .map_err(FunctionRelativeOptimizationRealizationError::Allocation)?;
    let source = selected_lowering_source(&current)?;
    let replayed_machine =
        validate_optimized_post_allocation_machine_plan_custody(&current, &staged.machine)
            .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    if &replayed_machine != staged.machine.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    validate_realization_artifacts(
        current.selected(),
        &staged.machine,
        current.register_environment().physical(),
        &staged.encoding,
        &staged.baseline_layout,
        staged.relaxation.as_ref(),
        &staged.exit_contract,
        current.selections(),
    )?;
    let replayed = expected_manifest(
        &current,
        &staged.machine,
        &staged.encoding,
        &staged.baseline_layout,
        staged.relaxation.as_ref(),
        &staged.exit_contract,
    )?;
    if replayed.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = custody_receipt(source, &staged.machine, &staged.exit_contract, &replayed);
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

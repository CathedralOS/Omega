use super::super::{assembly::*, carriers::*, error::*, prelude::*};

pub fn stage_selected_lowering_function_relative_realization(
    homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
) -> Result<
    StagedSelectedLoweringFunctionRelativeRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    validate_optimized_register_home_after_selected_lowering_custody(&homes)
        .map_err(FunctionRelativeOptimizationRealizationError::Homes)?;
    let machine = stage_optimized_post_allocation_machine_plan_after_selected_lowering(&homes)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    let run = homes.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let optimized = selected_stage.optimized_target().optimized();
    let selections = optimized.selections();
    let budget = optimized.budget_per_pass();
    let (encoding, baseline_layout, relaxation, exit_contract, manifest) = match run.steps().last()
    {
        Some(step) => {
            build_realization(step.fold(), &homes, &machine, physical, selections, budget)?
        }
        None => build_realization(
            selected_stage.selected(),
            &homes,
            &machine,
            physical,
            selections,
            budget,
        )?,
    };
    let custody = custody_receipt(&homes, &machine, &exit_contract, &manifest);
    Ok(StagedSelectedLoweringFunctionRelativeRealization {
        homes,
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
    validate_optimized_register_home_after_selected_lowering_custody(&staged.homes)
        .map_err(FunctionRelativeOptimizationRealizationError::Homes)?;
    let replayed_machine =
        validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
            &staged.homes,
            &staged.machine,
        )
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    if &replayed_machine != staged.machine.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let run = staged.homes.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let optimized = selected_stage.optimized_target().optimized();
    let selections = optimized.selections();
    match run.steps().last() {
        Some(step) => {
            validate_realization_artifacts(
                step.fold(),
                &staged.machine,
                physical,
                &staged.encoding,
                &staged.baseline_layout,
                staged.relaxation.as_ref(),
                &staged.exit_contract,
                selections,
            )?;
        }
        None => {
            validate_realization_artifacts(
                selected_stage.selected(),
                &staged.machine,
                physical,
                &staged.encoding,
                &staged.baseline_layout,
                staged.relaxation.as_ref(),
                &staged.exit_contract,
                selections,
            )?;
        }
    }
    let replayed = expected_manifest(
        &staged.homes,
        &staged.machine,
        &staged.encoding,
        &staged.baseline_layout,
        staged.relaxation.as_ref(),
        &staged.exit_contract,
    )?;
    if replayed.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = custody_receipt(
        &staged.homes,
        &staged.machine,
        &staged.exit_contract,
        &replayed,
    );
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

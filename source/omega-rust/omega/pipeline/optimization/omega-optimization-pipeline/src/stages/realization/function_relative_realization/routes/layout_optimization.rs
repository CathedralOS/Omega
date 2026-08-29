use super::super::{assembly::*, carriers::*, error::*, prelude::*};

pub fn stage_function_relative_layout_optimization_realization(
    homes: StagedOptimizedRegisterHomes,
) -> Result<
    StagedFunctionRelativeLayoutOptimizationRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    validate_optimized_register_home_custody(
        homes.legality_stage(),
        homes.homes(),
        homes.post_allocation_manifest(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::DirectHomes)?;
    let machine = stage_optimized_post_allocation_machine_plan(&homes)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let optimized = selected_stage.optimized_target().optimized();
    let selections = optimized.selections();
    if !rel8_selected(selections)? {
        return Err(
            FunctionRelativeOptimizationRealizationError::MissingFunctionRelativeLayoutOptimization,
        );
    }
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let baseline_layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let relaxation = stage_optimized_x86_branch_relaxation(
        selected,
        &machine,
        physical,
        &encoding,
        &baseline_layout,
        optimized.budget_per_pass(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::X86BranchRelaxation)?;
    let exit_contract = stage_whole_function_exit_contract_after_x86_branch_relaxation(
        selected,
        &machine,
        physical,
        &encoding,
        &baseline_layout,
        &relaxation,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_direct_manifest(
        &homes,
        &machine,
        &encoding,
        &baseline_layout,
        &relaxation,
        &exit_contract,
    )?;
    let custody = direct_custody_receipt(&homes, &machine, &relaxation, &exit_contract, &manifest);
    Ok(StagedFunctionRelativeLayoutOptimizationRealization {
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

pub fn validate_function_relative_layout_optimization_realization_custody(
    staged: &StagedFunctionRelativeLayoutOptimizationRealization,
) -> Result<
    StagedFunctionRelativeLayoutOptimizationRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let source = validate_optimized_register_home_custody(
        staged.homes.legality_stage(),
        staged.homes.homes(),
        staged.homes.post_allocation_manifest(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::DirectHomes)?;
    if source != staged.homes.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&staged.homes, &staged.machine)
            .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    if &machine != staged.machine.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let selected_stage = staged
        .homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let selections = selected_stage.optimized_target().optimized().selections();
    if !rel8_selected(selections)? {
        return Err(
            FunctionRelativeOptimizationRealizationError::MissingFunctionRelativeLayoutOptimization,
        );
    }
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.baseline_layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_optimized_x86_branch_relaxation(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.relaxation,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::X86BranchRelaxation)?;
    validate_whole_function_exit_contract_after_x86_branch_relaxation(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.relaxation,
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_direct_manifest(
        &staged.homes,
        &staged.machine,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.relaxation,
        &staged.exit_contract,
    )?;
    if manifest.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = direct_custody_receipt(
        &staged.homes,
        &staged.machine,
        &staged.relaxation,
        &staged.exit_contract,
        &manifest,
    );
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

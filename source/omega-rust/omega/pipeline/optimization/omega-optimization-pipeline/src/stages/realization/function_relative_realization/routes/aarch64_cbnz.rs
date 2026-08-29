use super::super::{assembly::*, carriers::*, error::*, prelude::*};

pub fn stage_aarch64_cbnz_function_relative_realization(
    homes: StagedOptimizedRegisterHomes,
    machine: StagedOptimizedPostAllocationMachinePlan,
    fusion: StagedOptimizedAarch64CbnzFusion,
) -> Result<
    StagedAarch64CbnzFunctionRelativeRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    validate_optimized_register_home_custody(
        homes.legality_stage(),
        homes.homes(),
        homes.post_allocation_manifest(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::DirectHomes)?;
    validate_optimized_post_allocation_machine_plan_custody(&homes, &machine)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    validate_optimized_aarch64_cbnz_fusion_custody(&homes, &machine, &fusion)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)?;
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let (baseline_encoding, encoding, baseline_layout, layout, exit_contract) =
        build_cbnz_artifacts(selected, &machine, physical, &fusion)?;
    let manifest = expected_direct_cbnz_manifest(
        &homes,
        &machine,
        &fusion,
        &baseline_encoding,
        &encoding,
        &baseline_layout,
        &layout,
        &exit_contract,
    )?;
    let custody = StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
        source: homes.custody(),
        machine: machine.custody().clone(),
        fusion: fusion.custody(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    };
    Ok(StagedAarch64CbnzFunctionRelativeRealization {
        homes,
        machine,
        fusion,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
        manifest,
        custody,
    })
}

pub fn validate_aarch64_cbnz_function_relative_realization_custody(
    staged: &StagedAarch64CbnzFunctionRelativeRealization,
) -> Result<
    StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt,
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
    let fusion = validate_optimized_aarch64_cbnz_fusion_custody(
        &staged.homes,
        &staged.machine,
        &staged.fusion,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)?;
    if fusion != staged.fusion.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let selected_stage = staged
        .homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    validate_cbnz_artifacts(
        selected_stage.selected(),
        &staged.machine,
        selected_stage.register_environment().physical(),
        &staged.fusion,
        &staged.baseline_encoding,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout,
        &staged.exit_contract,
    )?;
    let manifest = expected_direct_cbnz_manifest(
        &staged.homes,
        &staged.machine,
        &staged.fusion,
        &staged.baseline_encoding,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout,
        &staged.exit_contract,
    )?;
    if manifest.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = StagedAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
        source,
        machine,
        fusion,
        exit_contract: staged.exit_contract.identity(),
        realization: manifest.record.identity,
    };
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

pub fn stage_selected_lowering_aarch64_cbnz_function_relative_realization(
    homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: StagedOptimizedPostAllocationMachinePlan,
    fusion: StagedOptimizedAarch64CbnzFusion,
) -> Result<
    StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    validate_optimized_register_home_after_selected_lowering_custody(&homes)
        .map_err(FunctionRelativeOptimizationRealizationError::Homes)?;
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        &homes, &machine,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody(
        &homes, &machine, &fusion,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)?;
    let run = homes.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let artifacts = match run.steps().last() {
        Some(step) => build_cbnz_artifacts(step.fold(), &machine, physical, &fusion)?,
        None => build_cbnz_artifacts(selected_stage.selected(), &machine, physical, &fusion)?,
    };
    let (baseline_encoding, encoding, baseline_layout, layout, exit_contract) = artifacts;
    let manifest = expected_selected_lowering_cbnz_manifest(
        &homes,
        &machine,
        &fusion,
        &baseline_encoding,
        &encoding,
        &baseline_layout,
        &layout,
        &exit_contract,
    )?;
    let custody = StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
        source: homes.custody().clone(),
        machine: machine.custody().clone(),
        fusion: fusion.custody(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    };
    Ok(
        StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization {
            homes,
            machine,
            fusion,
            baseline_encoding,
            encoding,
            baseline_layout,
            layout,
            exit_contract,
            manifest,
            custody,
        },
    )
}

pub fn validate_selected_lowering_aarch64_cbnz_function_relative_realization_custody(
    staged: &StagedSelectedLoweringAarch64CbnzFunctionRelativeRealization,
) -> Result<
    StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let source = validate_optimized_register_home_after_selected_lowering_custody(&staged.homes)
        .map_err(FunctionRelativeOptimizationRealizationError::Homes)?;
    if &source != staged.homes.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let machine = validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        &staged.homes,
        &staged.machine,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    if &machine != staged.machine.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let fusion = validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody(
        &staged.homes,
        &staged.machine,
        &staged.fusion,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)?;
    if fusion != staged.fusion.custody() {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    let run = staged.homes.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    match run.steps().last() {
        Some(step) => validate_cbnz_artifacts(
            step.fold(),
            &staged.machine,
            physical,
            &staged.fusion,
            &staged.baseline_encoding,
            &staged.encoding,
            &staged.baseline_layout,
            &staged.layout,
            &staged.exit_contract,
        )?,
        None => validate_cbnz_artifacts(
            selected_stage.selected(),
            &staged.machine,
            physical,
            &staged.fusion,
            &staged.baseline_encoding,
            &staged.encoding,
            &staged.baseline_layout,
            &staged.layout,
            &staged.exit_contract,
        )?,
    }
    let manifest = expected_selected_lowering_cbnz_manifest(
        &staged.homes,
        &staged.machine,
        &staged.fusion,
        &staged.baseline_encoding,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout,
        &staged.exit_contract,
    )?;
    if manifest.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = StagedSelectedLoweringAarch64CbnzFunctionRelativeRealizationCustodyReceipt {
        source,
        machine,
        fusion,
        exit_contract: staged.exit_contract.identity(),
        realization: manifest.record.identity,
    };
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn build_cbnz_artifacts<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    fusion: &StagedOptimizedAarch64CbnzFusion,
) -> Result<
    (
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedResolvedSelectedFormLayout,
        StagedOptimizedResolvedSelectedFormLayout,
        ValidatedWholeFunctionExitContract,
    ),
    FunctionRelativeOptimizationRealizationError,
> {
    let baseline_encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, machine, physical)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let baseline_layout = stage_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        &baseline_encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
            selected, machine, physical, fusion,
        )
        .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let layout = stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
        selected, machine, physical, &encoding, fusion,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let exit_contract = stage_whole_function_exit_contract_after_aarch64_cbnz_fusion(
        selected, machine, physical, &encoding, fusion, &layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    Ok((
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
    ))
}

#[allow(clippy::too_many_arguments)]
fn validate_cbnz_artifacts<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        machine,
        physical,
        baseline_encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        baseline_encoding,
        baseline_layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion(
        selected, machine, physical, fusion, encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
        selected, machine, physical, encoding, fusion, layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_whole_function_exit_contract_after_aarch64_cbnz_fusion(
        selected,
        machine,
        physical,
        encoding,
        fusion,
        layout,
        exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)
}

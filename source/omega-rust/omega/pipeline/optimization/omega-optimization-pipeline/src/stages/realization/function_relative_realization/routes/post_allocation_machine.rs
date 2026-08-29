//! One function-relative join for every registered post-allocation machine rule.

use omega_regalloc::ValidatedSelectedAnalysis;

use super::super::{assembly::*, carriers::*, error::*};
use crate::{
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterSelectedLowering,
    StagedOptimizedResolvedSelectedFormLayout, StagedOptimizedSelectedFormEncoding,
    ValidatedWholeFunctionExitContract,
    stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization,
    stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
    stage_whole_function_exit_contract_with_post_allocation_machine_optimization,
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization,
    validate_optimized_post_allocation_machine_optimization_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_optimization_custody,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
    validate_optimized_register_home_custody,
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
    validate_whole_function_exit_contract_with_post_allocation_machine_optimization,
};

pub fn stage_post_allocation_machine_function_relative_realization(
    homes: StagedOptimizedRegisterHomes,
    machine: StagedOptimizedPostAllocationMachinePlan,
    optimization: StagedOptimizedPostAllocationMachineOptimization,
) -> Result<
    StagedPostAllocationMachineFunctionRelativeRealization,
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
    validate_optimized_post_allocation_machine_optimization_custody(
        &homes,
        &machine,
        &optimization,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)?;
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let (baseline_encoding, encoding, baseline_layout, layout, exit_contract) = build_artifacts(
        selected_stage.selected(),
        &machine,
        selected_stage.register_environment().physical(),
        &optimization,
    )?;
    let manifest = expected_direct_post_allocation_machine_manifest(
        &homes,
        &machine,
        &optimization,
        &baseline_encoding,
        &encoding,
        &baseline_layout,
        &layout,
        &exit_contract,
    )?;
    let normalized = optimization
        .custody()
        .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)?;
    let custody = StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt {
        source: PostAllocationMachineFunctionRelativeSourceCustody::Direct(homes.custody()),
        machine: machine.custody().clone(),
        optimization: normalized,
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    };
    Ok(StagedPostAllocationMachineFunctionRelativeRealization {
        source: StagedPostAllocationMachineFunctionRelativeSource::Direct(homes),
        machine,
        optimization,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
        manifest,
        custody,
    })
}

pub fn stage_post_allocation_machine_function_relative_realization_after_selected_lowering(
    homes: StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: StagedOptimizedPostAllocationMachinePlan,
    optimization: StagedOptimizedPostAllocationMachineOptimization,
) -> Result<
    StagedPostAllocationMachineFunctionRelativeRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    validate_optimized_register_home_after_selected_lowering_custody(&homes)
        .map_err(FunctionRelativeOptimizationRealizationError::Homes)?;
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        &homes, &machine,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    validate_optimized_post_allocation_machine_optimization_after_selected_lowering_custody(
        &homes,
        &machine,
        &optimization,
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
        Some(step) => build_artifacts(step.fold(), &machine, physical, &optimization)?,
        None => build_artifacts(selected_stage.selected(), &machine, physical, &optimization)?,
    };
    let (baseline_encoding, encoding, baseline_layout, layout, exit_contract) = artifacts;
    let manifest = expected_selected_lowering_post_allocation_machine_manifest(
        &homes,
        &machine,
        &optimization,
        &baseline_encoding,
        &encoding,
        &baseline_layout,
        &layout,
        &exit_contract,
    )?;
    let normalized = optimization
        .custody()
        .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)?;
    let custody = StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt {
        source: PostAllocationMachineFunctionRelativeSourceCustody::AfterSelectedLowering(
            homes.custody().clone(),
        ),
        machine: machine.custody().clone(),
        optimization: normalized,
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    };
    Ok(StagedPostAllocationMachineFunctionRelativeRealization {
        source: StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes),
        machine,
        optimization,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
        manifest,
        custody,
    })
}

pub fn validate_post_allocation_machine_function_relative_realization_custody(
    staged: &StagedPostAllocationMachineFunctionRelativeRealization,
) -> Result<
    StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let normalized = staged
        .optimization
        .custody()
        .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)?;
    let (source, machine, manifest) = match &staged.source {
        StagedPostAllocationMachineFunctionRelativeSource::Direct(homes) => {
            let source = validate_optimized_register_home_custody(
                homes.legality_stage(),
                homes.homes(),
                homes.post_allocation_manifest(),
            )
            .map_err(FunctionRelativeOptimizationRealizationError::DirectHomes)?;
            if source != homes.custody() {
                return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
            }
            let machine =
                validate_optimized_post_allocation_machine_plan_custody(homes, &staged.machine)
                    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
            validate_optimized_post_allocation_machine_optimization_custody(
                homes,
                &staged.machine,
                &staged.optimization,
            )
            .map_err(
                FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization,
            )?;
            let selected_stage = homes
                .legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage();
            validate_artifacts(
                selected_stage.selected(),
                &staged.machine,
                selected_stage.register_environment().physical(),
                &staged.optimization,
                staged,
            )?;
            let manifest = expected_direct_post_allocation_machine_manifest(
                homes,
                &staged.machine,
                &staged.optimization,
                &staged.baseline_encoding,
                &staged.encoding,
                &staged.baseline_layout,
                &staged.layout,
                &staged.exit_contract,
            )?;
            (
                PostAllocationMachineFunctionRelativeSourceCustody::Direct(source),
                machine,
                manifest,
            )
        }
        StagedPostAllocationMachineFunctionRelativeSource::AfterSelectedLowering(homes) => {
            let source = validate_optimized_register_home_after_selected_lowering_custody(homes)
                .map_err(FunctionRelativeOptimizationRealizationError::Homes)?;
            if &source != homes.custody() {
                return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
            }
            let machine =
                validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
                    homes,
                    &staged.machine,
                )
                .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
            validate_optimized_post_allocation_machine_optimization_after_selected_lowering_custody(
                homes,
                &staged.machine,
                &staged.optimization,
            )
            .map_err(
                FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization,
            )?;
            let run = homes.selected_lowering_run();
            let selected_stage = run
                .source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .selected_stage();
            let physical = selected_stage.register_environment().physical();
            match run.steps().last() {
                Some(step) => validate_artifacts(
                    step.fold(),
                    &staged.machine,
                    physical,
                    &staged.optimization,
                    staged,
                )?,
                None => validate_artifacts(
                    selected_stage.selected(),
                    &staged.machine,
                    physical,
                    &staged.optimization,
                    staged,
                )?,
            }
            let manifest = expected_selected_lowering_post_allocation_machine_manifest(
                homes,
                &staged.machine,
                &staged.optimization,
                &staged.baseline_encoding,
                &staged.encoding,
                &staged.baseline_layout,
                &staged.layout,
                &staged.exit_contract,
            )?;
            (
                PostAllocationMachineFunctionRelativeSourceCustody::AfterSelectedLowering(source),
                machine,
                manifest,
            )
        }
    };
    if machine != staged.machine.custody().clone() || manifest.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let custody = StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt {
        source,
        machine,
        optimization: normalized,
        exit_contract: staged.exit_contract.identity(),
        realization: manifest.record.identity,
    };
    if custody != staged.custody {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn build_artifacts<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
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
        stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
            selected, machine, physical, None,
        )
        .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let baseline_layout =
        stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
            selected,
            machine,
            physical,
            &baseline_encoding,
            None,
        )
        .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
            selected,
            machine,
            physical,
            Some(optimization),
        )
        .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
            selected,
            machine,
            physical,
            &encoding,
            Some(optimization),
        )
        .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let exit_contract =
        stage_whole_function_exit_contract_with_post_allocation_machine_optimization(
            selected,
            machine,
            physical,
            &encoding,
            optimization,
            &layout,
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

fn validate_artifacts<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
    staged: &StagedPostAllocationMachineFunctionRelativeRealization,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        None,
        &staged.baseline_encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        &staged.baseline_encoding,
        None,
        &staged.baseline_layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        Some(optimization),
        &staged.encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        &staged.encoding,
        Some(optimization),
        &staged.layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_whole_function_exit_contract_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        &staged.encoding,
        optimization,
        &staged.layout,
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)
}

//! One function-relative join over current allocation facts, independent of rewrite history.

use omega_selected_instructions_to_register_homes::ValidatedSelectedAnalysis;
use omega_selected_instructions_to_register_homes::{
    AllocationReplayError, AllocationSource, RetainedAllocation,
};

use super::super::{assembly::*, carriers::*, error::*};
use crate::{
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedResolvedSelectedFormLayout, StagedOptimizedSelectedFormEncoding,
    ValidatedWholeFunctionExitContract,
    stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization,
    stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
    stage_whole_function_exit_contract_with_post_allocation_machine_optimization,
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization,
    validate_optimized_post_allocation_machine_optimization_custody,
    validate_optimized_post_allocation_machine_plan_custody,
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
    validate_whole_function_exit_contract_with_post_allocation_machine_optimization,
};

pub fn stage_post_allocation_machine_function_relative_realization<Source>(
    source: Source,
    machine: StagedOptimizedPostAllocationMachinePlan,
    optimization: StagedOptimizedPostAllocationMachineOptimization,
) -> Result<
    StagedPostAllocationMachineFunctionRelativeRealization,
    FunctionRelativeOptimizationRealizationError,
>
where
    Source: TryInto<RetainedAllocation>,
    AllocationReplayError: From<Source::Error>,
{
    // Conversion replays and admits the owned inputs before exposing current facts.
    let allocation = source
        .try_into()
        .map_err(|error| FunctionRelativeOptimizationRealizationError::Allocation(error.into()))?;
    let current = allocation.current();
    validate_optimized_post_allocation_machine_plan_custody(&current, &machine)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    validate_optimized_post_allocation_machine_optimization_custody(
        &current,
        &machine,
        &optimization,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)?;
    let (baseline_encoding, encoding, baseline_layout, layout, exit_contract) = build_artifacts(
        current.selected(),
        &machine,
        current.register_environment().physical(),
        &optimization,
    )?;
    let manifest = expected_allocated_post_allocation_machine_manifest(
        &current,
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
        .ok_or(FunctionRelativeOptimizationRealizationError::OptimizationCustodyUnavailable)?;
    let custody = StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt {
        source: current.evidence().clone(),
        machine: machine.custody().clone(),
        optimization: normalized,
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    };
    Ok(StagedPostAllocationMachineFunctionRelativeRealization {
        allocation,
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
    let current = staged
        .allocation
        .replay_allocation()
        .map_err(FunctionRelativeOptimizationRealizationError::Allocation)?;
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&current, &staged.machine)
            .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    validate_optimized_post_allocation_machine_optimization_custody(
        &current,
        &staged.machine,
        &staged.optimization,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)?;
    validate_artifacts(
        current.selected(),
        &staged.machine,
        current.register_environment().physical(),
        &staged.optimization,
        staged,
    )?;
    let manifest = expected_allocated_post_allocation_machine_manifest(
        &current,
        &staged.machine,
        &staged.optimization,
        &staged.baseline_encoding,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout,
        &staged.exit_contract,
    )?;
    if machine != *staged.machine.custody() || manifest.record != staged.manifest.record {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let normalized = staged
        .optimization
        .custody()
        .ok_or(FunctionRelativeOptimizationRealizationError::OptimizationCustodyUnavailable)?;
    let custody = StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt {
        source: current.evidence().clone(),
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

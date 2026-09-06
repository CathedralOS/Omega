//! One function-relative join over current allocation facts, independent of rewrite history.

use resolved_layout_to_resolved_layout::{
    ResolvedLayoutOptimization, execute_resolved_layout_optimization,
    validate_resolved_layout_optimization,
};

use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;
use selected_instructions_to_register_homes::{
    AllocationReplayError, AllocationSource, RetainedAllocation,
};

use super::super::{assembly::*, carriers::*, error::*};
use crate::{
    ValidatedWholeFunctionExitContract, stage_whole_function_exit_contract_for_layout,
    validate_whole_function_exit_contract_for_layout,
};
use post_allocation_machine_to_post_allocation_machine::{
    StagedOptimizedPostAllocationMachineOptimization,
    validate_optimized_post_allocation_machine_optimization_custody,
};
use post_allocation_machine_to_selected_form_encoding::{
    StagedOptimizedSelectedFormEncoding,
    stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization,
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization,
};
use register_homes_to_post_allocation_machine::{
    StagedOptimizedPostAllocationMachinePlan,
    validate_optimized_post_allocation_machine_plan_custody,
};
use selected_form_encoding_to_resolved_layout::{
    StagedOptimizedResolvedSelectedFormLayout,
    stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
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
    let frame = if super::super::frame::ordinary_frame_required(&current, &machine)? {
        Some(super::super::frame::stage_frame(
            &current,
            &machine,
            crate::frame_layout::TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            current.budget_per_pass(),
        )?)
    } else {
        None
    };
    let (baseline_encoding, encoding, baseline_layout, layout, layout_optimization, exit_contract) =
        build_artifacts(
            current.selected(),
            &machine,
            current.register_environment().physical(),
            &optimization,
            frame.as_ref(),
            &current,
        )?;
    let manifest = expected_allocated_post_allocation_machine_manifest(
        &current,
        &machine,
        &optimization,
        &baseline_encoding,
        &encoding,
        &baseline_layout,
        layout_optimization.layout(),
        &exit_contract,
        frame.as_ref(),
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
        layout_optimization,
        frame,
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
    if staged.frame.is_some()
        != super::super::frame::ordinary_frame_required(&current, &staged.machine)?
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    if let Some(frame) = &staged.frame {
        super::super::frame::validate_frame(
            &current,
            &staged.machine,
            frame,
            crate::frame_layout::TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
        )?;
    }
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
        staged.layout(),
        &staged.exit_contract,
        staged.frame.as_ref(),
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
    physical: &register_model::ValidatedPhysicalRegisterModel,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
    frame: Option<&super::super::FunctionRelativeFrame>,
    current: &selected_instructions_to_register_homes::AllocationOutput<'_>,
) -> Result<
    (
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedResolvedSelectedFormLayout,
        StagedOptimizedResolvedSelectedFormLayout,
        ResolvedLayoutOptimization,
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
    let layout_optimization = execute_resolved_layout_optimization(
        selected,
        machine,
        physical,
        &encoding,
        Some(optimization),
        &layout,
        &current
            .selections()
            .project_phase(optimization_core::OptimizationExecutionPhase::FunctionRelativeLayout),
        current.budget_per_pass(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::LayoutOptimization)?;
    let exit_contract = stage_whole_function_exit_contract_for_layout(
        selected,
        machine,
        physical,
        &encoding,
        Some(optimization),
        &layout,
        &layout_optimization,
        frame.map(|frame| (frame.layout(), frame.protocol())),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    Ok((
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        layout_optimization,
        exit_contract,
    ))
}

fn validate_artifacts<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &register_model::ValidatedPhysicalRegisterModel,
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
    validate_resolved_layout_optimization(
        selected,
        machine,
        physical,
        &staged.encoding,
        Some(optimization),
        &staged.layout,
        &staged
            .allocation
            .current()
            .selections()
            .project_phase(optimization_core::OptimizationExecutionPhase::FunctionRelativeLayout),
        &staged.layout_optimization,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::LayoutOptimization)?;
    validate_whole_function_exit_contract_for_layout(
        selected,
        machine,
        physical,
        &staged.encoding,
        Some(optimization),
        &staged.layout,
        &staged.layout_optimization,
        staged
            .frame
            .as_ref()
            .map(|frame| (frame.layout(), frame.protocol())),
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)
}

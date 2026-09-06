//! Direct ordinary function-relative realization with exact fixed-frame custody.

use super::super::{assembly::*, carriers::*, error::*, prelude::*};
use resolved_layout_to_resolved_layout::{
    execute_resolved_layout_optimization, validate_resolved_layout_optimization,
};
use selected_instructions_to_register_homes::{AllocationSource, RetainedAllocation};

pub fn stage_fixed_frame_function_relative_realization(
    allocation: RetainedAllocation,
    machine: StagedOptimizedPostAllocationMachinePlan,
    budget: OptimizationWorkBudget,
) -> Result<StagedFixedFrameFunctionRelativeRealization, FunctionRelativeOptimizationRealizationError>
{
    let current = allocation
        .replay_allocation()
        .map_err(FunctionRelativeOptimizationRealizationError::Allocation)?;
    let source = baseline_allocation_source(&current)?;
    validate_optimized_post_allocation_machine_plan_custody(&current, &machine)
        .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    let selected = current.selected();
    let environment = current.register_environment();
    let physical = environment.physical();
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let layout_optimization = execute_resolved_layout_optimization(
        selected,
        &machine,
        physical,
        &encoding,
        None,
        &layout,
        &current
            .selections()
            .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
        current.budget_per_pass(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::LayoutOptimization)?;
    let frame = super::super::frame::stage_frame(
        &current,
        &machine,
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
        budget,
    )?;
    let exit_contract = stage_whole_function_exit_contract_for_layout(
        selected,
        &machine,
        physical,
        &encoding,
        None,
        &layout,
        &layout_optimization,
        Some((frame.layout(), frame.protocol())),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_fixed_frame_manifest(
        &current,
        &machine,
        &encoding,
        layout_optimization.layout(),
        frame.layout(),
        frame.protocol(),
        &exit_contract,
    )?;
    let custody = fixed_frame_custody(
        source,
        &machine,
        frame.requirements(),
        frame.storage(),
        frame.layout(),
        frame.protocol(),
        &exit_contract,
        &manifest,
    );
    let staged = StagedFixedFrameFunctionRelativeRealization {
        allocation,
        machine,
        encoding,
        layout,
        layout_optimization,
        frame,
        exit_contract,
        manifest,
        custody,
    };
    validate_fixed_frame_function_relative_realization(&staged)?;
    Ok(staged)
}

pub fn validate_fixed_frame_function_relative_realization(
    staged: &StagedFixedFrameFunctionRelativeRealization,
) -> Result<
    StagedFixedFrameFunctionRelativeRealizationCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    let current = staged
        .allocation
        .replay_allocation()
        .map_err(FunctionRelativeOptimizationRealizationError::Allocation)?;
    let source = baseline_allocation_source(&current)?;
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&current, &staged.machine)
            .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    let selected = current.selected();
    let environment = current.register_environment();
    let physical = environment.physical();
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_resolved_layout_optimization(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        None,
        &staged.layout,
        &current
            .selections()
            .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
        &staged.layout_optimization,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::LayoutOptimization)?;
    super::super::frame::validate_frame(
        &current,
        &staged.machine,
        &staged.frame,
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
    )?;
    let frame = &staged.frame;
    validate_whole_function_exit_contract_for_layout(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        None,
        &staged.layout,
        &staged.layout_optimization,
        Some((frame.layout(), frame.protocol())),
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_fixed_frame_manifest(
        &current,
        &staged.machine,
        &staged.encoding,
        staged.layout(),
        frame.layout(),
        frame.protocol(),
        &staged.exit_contract,
    )?;
    let custody = fixed_frame_custody(
        source,
        &staged.machine,
        frame.requirements(),
        frame.storage(),
        frame.layout(),
        frame.protocol(),
        &staged.exit_contract,
        &manifest,
    );
    if source != staged.custody.source()
        || machine != staged.machine.custody().clone()
        || manifest.record != staged.manifest.record
        || custody != staged.custody
    {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

//! Direct ordinary function-relative realization with exact fixed-frame custody.

use super::super::{assembly::*, carriers::*, error::*, prelude::*};
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
    let requirements = stage_allocated_callee_saved_requirements(
        &current,
        AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
        budget,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::CalleeSavedRequirements)?;
    let storage = stage_non_authoritative_callee_save_storage(
        &requirements,
        environment,
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
        budget,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::CalleeSaveStorage)?;
    let frame = stage_target_frame_layout(
        &machine,
        &requirements,
        &storage,
        environment,
        TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::FrameLayout)?;
    let protocol = stage_target_frame_protocol_encoding(
        &frame,
        environment,
        TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::FrameProtocol)?;
    let exit_contract = stage_whole_function_exit_contract_with_frame(
        selected, &machine, physical, &encoding, &layout, &frame, &protocol,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_fixed_frame_manifest(
        &current,
        &machine,
        &encoding,
        &layout,
        &frame,
        &protocol,
        &exit_contract,
    )?;
    let custody = fixed_frame_custody(
        source,
        &machine,
        &requirements,
        &storage,
        &frame,
        &protocol,
        &exit_contract,
        &manifest,
    );
    let staged = StagedFixedFrameFunctionRelativeRealization {
        allocation,
        machine,
        encoding,
        layout,
        requirements,
        storage,
        frame,
        protocol,
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
    validate_optimized_resolved_selected_form_layout(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let requirements =
        validate_allocated_callee_saved_requirements(&current, staged.requirements.plan().clone())
            .map_err(FunctionRelativeOptimizationRealizationError::CalleeSavedRequirements)?;
    let storage = validate_non_authoritative_callee_save_storage(
        &requirements,
        environment,
        staged.storage.plan().clone(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::CalleeSaveStorage)?;
    let frame = validate_target_frame_layout(
        &staged.machine,
        &requirements,
        &storage,
        environment,
        staged.frame.plan().clone(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::FrameLayout)?;
    let protocol = validate_target_frame_protocol_encoding(
        &frame,
        environment,
        staged.protocol.plan().clone(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::FrameProtocol)?;
    validate_whole_function_exit_contract_with_frame(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
        &frame,
        &protocol,
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = expected_fixed_frame_manifest(
        &current,
        &staged.machine,
        &staged.encoding,
        &staged.layout,
        &frame,
        &protocol,
        &staged.exit_contract,
    )?;
    let custody = fixed_frame_custody(
        source,
        &staged.machine,
        &requirements,
        &storage,
        &frame,
        &protocol,
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

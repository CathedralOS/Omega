//! Direct ordinary function-relative realization with exact fixed-frame custody.

use super::super::{assembly::*, carriers::*, error::*, prelude::*};

pub fn stage_fixed_frame_function_relative_realization(
    homes: StagedOptimizedRegisterHomes,
    budget: OptimizationWorkBudget,
) -> Result<StagedFixedFrameFunctionRelativeRealization, FunctionRelativeOptimizationRealizationError>
{
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
    let environment = selected_stage.register_environment();
    let physical = environment.physical();
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let requirements = stage_allocated_callee_saved_requirements(
        &homes,
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
        &homes,
        &machine,
        &encoding,
        &layout,
        &frame,
        &protocol,
        &exit_contract,
    )?;
    let custody = fixed_frame_custody(
        &homes,
        &machine,
        &requirements,
        &storage,
        &frame,
        &protocol,
        &exit_contract,
        &manifest,
    );
    let staged = StagedFixedFrameFunctionRelativeRealization {
        homes,
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
    let source = validate_optimized_register_home_custody(
        staged.homes.legality_stage(),
        staged.homes.homes(),
        staged.homes.post_allocation_manifest(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::DirectHomes)?;
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&staged.homes, &staged.machine)
            .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)?;
    let selected_stage = staged
        .homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let environment = selected_stage.register_environment();
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
    let requirements = validate_allocated_callee_saved_requirements(
        &staged.homes,
        staged.requirements.plan().clone(),
    )
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
        &staged.homes,
        &staged.machine,
        &staged.encoding,
        &staged.layout,
        &frame,
        &protocol,
        &staged.exit_contract,
    )?;
    let custody = fixed_frame_custody(
        &staged.homes,
        &staged.machine,
        &requirements,
        &storage,
        &frame,
        &protocol,
        &staged.exit_contract,
        &manifest,
    );
    if source != staged.homes.custody()
        || machine != staged.machine.custody().clone()
        || manifest.record != staged.manifest.record
        || custody != staged.custody
    {
        return Err(FunctionRelativeOptimizationRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}

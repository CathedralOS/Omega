use selected_instructions_to_register_homes::{AllocationSource, RetainedAllocation};

use crate::{TargetFrameProtocolEncodingPolicy, stage_target_frame_protocol_encoding};
use crate::{stage_whole_function_exit_contract, stage_whole_function_exit_contract_with_frame};
use post_allocation_machine_to_frame_layout::{
    NonAuthoritativeCalleeSaveStoragePolicy, stage_non_authoritative_callee_save_storage,
};
use post_allocation_machine_to_frame_layout::{TargetFrameLayoutPolicy, stage_target_frame_layout};
use post_allocation_machine_to_selected_form_encoding::stage_optimized_layout_independent_selected_form_encoding;
use register_homes_to_post_allocation_machine::stage_optimized_post_allocation_machine_plan;
use selected_form_encoding_to_resolved_layout::stage_optimized_resolved_selected_form_layout;
use selected_instructions_to_register_homes::{
    AllocatedCalleeSavedRequirementPolicy, stage_allocated_callee_saved_requirements,
};
use target::Architecture;

use super::custody::unit_realization_receipt;
use super::manifest::expected_manifest;
use super::model::{
    OptimizedUnitFunctionRelativeRealizationError, StagedOptimizedUnitFunctionRelativeRealization,
    UnitSavedReturnAddressFrame,
};
use super::source::validate_source;

pub(super) fn construct_unit_function_relative_realization(
    allocation: RetainedAllocation,
) -> Result<
    StagedOptimizedUnitFunctionRelativeRealization,
    OptimizedUnitFunctionRelativeRealizationError,
> {
    let current = allocation
        .replay_allocation()
        .map_err(OptimizedUnitFunctionRelativeRealizationError::Allocation)?;
    let source = validate_source(&current)?;
    let selected = current.selected();
    let environment = current.register_environment();
    let physical = environment.physical();
    let budget = current.budget_per_pass();
    let machine = stage_optimized_post_allocation_machine_plan(&current)
        .map_err(OptimizedUnitFunctionRelativeRealizationError::Machine)?;
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Layout)?;
    let frame = match environment.target().architecture {
        Architecture::X86_64 => None,
        Architecture::Aarch64 => {
            let requirements = stage_allocated_callee_saved_requirements(
                &current,
                AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
                budget,
            )
            .map_err(OptimizedUnitFunctionRelativeRealizationError::CalleeSavedRequirements)?;
            let storage = stage_non_authoritative_callee_save_storage(
                &requirements,
                environment,
                NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
                budget,
            )
            .map_err(OptimizedUnitFunctionRelativeRealizationError::CalleeSaveStorage)?;
            let layout = stage_target_frame_layout(
                &machine,
                &requirements,
                &storage,
                environment,
                TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1,
            )
            .map_err(OptimizedUnitFunctionRelativeRealizationError::FrameLayout)?;
            let protocol = stage_target_frame_protocol_encoding(
                &layout,
                environment,
                TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1,
            )
            .map_err(OptimizedUnitFunctionRelativeRealizationError::FrameProtocol)?;
            Some(UnitSavedReturnAddressFrame {
                requirements,
                storage,
                layout,
                protocol,
            })
        }
    };
    let exit_contract = match &frame {
        Some(frame) => stage_whole_function_exit_contract_with_frame(
            selected,
            &machine,
            physical,
            &encoding,
            &layout,
            &frame.layout,
            &frame.protocol,
        ),
        None => {
            stage_whole_function_exit_contract(selected, &machine, physical, &encoding, &layout)
        }
    }
    .map_err(OptimizedUnitFunctionRelativeRealizationError::Exit)?;
    let manifest = expected_manifest(
        &current,
        &machine,
        &encoding,
        &layout,
        frame.as_ref(),
        &exit_contract,
    )?;
    let custody =
        unit_realization_receipt(source, &machine, frame.as_ref(), &exit_contract, &manifest);
    Ok(StagedOptimizedUnitFunctionRelativeRealization {
        allocation,
        machine,
        encoding,
        layout,
        frame,
        exit_contract,
        manifest,
        custody,
    })
}

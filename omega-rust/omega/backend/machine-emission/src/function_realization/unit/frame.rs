//! Return-address custody is a program/target requirement, not an optimization role.

use super::{UnitSavedReturnAddressFrame, validate_unit_shape};
use crate::function_realization::FunctionRelativeOptimizationRealizationError as Error;
use crate::function_realization::prelude::*;
use selected_instructions_to_register_homes::AllocationOutput;

fn required(current: &AllocationOutput<'_>) -> bool {
    current.register_environment().target().architecture == Architecture::Aarch64
        && validate_unit_shape(current.selected_plan()).is_ok()
}

pub(in crate::function_realization) fn stage_unit_frame(
    current: &AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<Option<UnitSavedReturnAddressFrame>, Error> {
    if !required(current) {
        return Ok(None);
    }
    let environment = current.register_environment();
    let requirements = stage_allocated_callee_saved_requirements(
        current,
        AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1,
        current.budget_per_pass(),
    )
    .map_err(Error::CalleeSavedRequirements)?;
    let storage = stage_non_authoritative_callee_save_storage(
        &requirements,
        environment,
        NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1,
        current.budget_per_pass(),
    )
    .map_err(Error::CalleeSaveStorage)?;
    let layout = stage_target_frame_layout(
        machine,
        &requirements,
        &storage,
        environment,
        TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1,
    )
    .map_err(Error::FrameLayout)?;
    let protocol = stage_target_frame_protocol_encoding(
        &layout,
        environment,
        TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1,
    )
    .map_err(Error::FrameProtocol)?;
    Ok(Some(UnitSavedReturnAddressFrame {
        requirements,
        storage,
        layout,
        protocol,
    }))
}

/// Replay the supplied frame, including required presence. A producer cannot
/// remove an AArch64 Unit frame and reseal a frameless exit to evade the join.
pub(in crate::function_realization) fn validate_unit_frame(
    current: &AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    frame: Option<&UnitSavedReturnAddressFrame>,
) -> Result<(), Error> {
    if frame.is_some() != required(current) {
        return Err(Error::RootMismatch);
    }
    let Some(frame) = frame else {
        return Ok(());
    };
    let environment = current.register_environment();
    if frame.layout().plan().policy != TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1 {
        return Err(Error::RootMismatch);
    }
    validate_allocated_callee_saved_requirements(current, frame.requirements().plan().clone())
        .map_err(Error::CalleeSavedRequirements)?;
    validate_non_authoritative_callee_save_storage(
        frame.requirements(),
        environment,
        frame.storage().plan().clone(),
    )
    .map_err(Error::CalleeSaveStorage)?;
    validate_target_frame_layout(
        machine,
        frame.requirements(),
        frame.storage(),
        environment,
        frame.layout().plan().clone(),
    )
    .map_err(Error::FrameLayout)?;
    validate_target_frame_protocol_encoding(
        frame.layout(),
        environment,
        frame.protocol().plan().clone(),
    )
    .map_err(Error::FrameProtocol)?;
    Ok(())
}

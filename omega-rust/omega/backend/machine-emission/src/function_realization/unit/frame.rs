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
    super::super::frame::stage_frame(
        current,
        machine,
        TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1,
        current.budget_per_pass(),
    )
    .map(Some)
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
    super::super::frame::validate_frame(
        current,
        machine,
        frame,
        TargetFrameLayoutPolicy::CanonicalSavedReturnAddressFrameV1,
    )
}

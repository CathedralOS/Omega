//! Physical ownership for the exact ranked `u32` countdown carrier.

use crate::assignment::placement::require_register_architecture;
use crate::assignment::shared::*;

pub(super) fn assign(
    countdown: &TargetRankedU32Countdown,
    target: NativeTarget,
) -> Result<AssignedOperation, AssignmentError> {
    let source = countdown.custody.graph.initial_value;
    let Some(placement) = countdown.call_plan.parameters.first() else {
        return Err(AssignmentError::RankedCountdownAbiMismatch(source));
    };
    let rank_home = match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 4,
            },
        ] => *register,
        [ValueLocation::Stack { .. }] => {
            return Err(AssignmentError::RankedCountdownRequiresRegister(source));
        }
        _ => return Err(AssignmentError::RankedCountdownAbiMismatch(source)),
    };
    require_register_architecture(source, rank_home, target.architecture)?;
    let expected_rank_home = if target == NativeTarget::linux_x64() {
        MachineRegister::X86Rdi
    } else if target == NativeTarget::linux_arm64() {
        MachineRegister::Aarch64X(0)
    } else {
        return Err(AssignmentError::RankedCountdownAbiMismatch(source));
    };
    if rank_home != expected_rank_home
        || placement.shape != ValueShape::integer(4, 4)
        || countdown.call_plan.parameters.len() != 1 + countdown.structural_parameters.len()
        || countdown
            .structural_parameters
            .iter()
            .zip(&countdown.call_plan.parameters[1..])
            .any(|(parameter, placement)| parameter.placement != *placement)
    {
        return Err(AssignmentError::RankedCountdownAbiMismatch(source));
    }

    Ok(AssignedOperation::RankedU32Countdown(
        AssignedRankedU32Countdown {
            custody: countdown.custody.clone(),
            call_plan: countdown.call_plan.clone(),
            rank_home,
            structural_types: countdown.structural_types.clone(),
            structural_parameters: countdown.structural_parameters.clone(),
            cleanup_actions: countdown.cleanup_actions.clone(),
        },
    ))
}

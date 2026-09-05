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
    let [structural_parameter] = countdown.structural_parameters.as_slice() else {
        return Err(AssignmentError::RankedCountdownAbiMismatch(source));
    };
    let [replay_machine] = countdown.custody.semantic_replay.machines.as_slice() else {
        return Err(AssignmentError::RankedCountdownAbiMismatch(source));
    };
    let [replay_parameter] = replay_machine.structural_parameters.as_slice() else {
        return Err(AssignmentError::RankedCountdownAbiMismatch(source));
    };
    let affine_owned = !replay_parameter.is_self
        && replay_parameter.multiplicity == terminal_psi::StructuralMultiplicity::Affine
        && replay_parameter.access == terminal_psi::StructuralAccess::Owned;
    let persistent_receiver = replay_parameter.is_self
        && replay_parameter.access == terminal_psi::StructuralAccess::MutableBorrow;
    let pointer_shape = ValueShape::integer(
        u16::try_from(target.pointer_size)
            .map_err(|_| AssignmentError::RankedCountdownAbiMismatch(source))?,
        u16::try_from(target.pointer_alignment)
            .map_err(|_| AssignmentError::RankedCountdownAbiMismatch(source))?,
    );
    if rank_home != expected_rank_home
        || placement.shape != ValueShape::integer(4, 4)
        || countdown.call_plan.parameters.len() != 1 + countdown.structural_parameters.len()
        || countdown
            .structural_parameters
            .iter()
            .zip(&countdown.call_plan.parameters[1..])
            .any(|(parameter, placement)| parameter.placement != *placement)
        || structural_parameter.place != replay_parameter.place
        || structural_parameter.structural_type != replay_parameter.structural_type
        || structural_parameter.multiplicity != replay_parameter.multiplicity
        || structural_parameter.access != replay_parameter.access
        || (!affine_owned && !persistent_receiver)
        || (persistent_receiver && structural_parameter.shape != pointer_shape)
        || (affine_owned
            && countdown.cleanup_actions.as_slice()
                != [terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                    structural_parameter.place,
                )])
        || (persistent_receiver && !countdown.cleanup_actions.is_empty())
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

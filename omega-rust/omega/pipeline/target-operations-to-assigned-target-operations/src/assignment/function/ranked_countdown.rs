//! Physical ownership for the exact ranked `u32` countdown carrier.

use super::unit::structural_scalar::{declaration_map, structural_value_shape};
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
    let invalid = || AssignmentError::RankedCountdownAbiMismatch(source);
    let expected_shape = if persistent_receiver {
        let declarations = declaration_map(&countdown.custody.semantic_replay.structural_types)
            .ok_or_else(invalid)?;
        let referent = structural_value_shape(replay_parameter.structural_type, &declarations)
            .ok_or_else(invalid)?;
        ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
    } else {
        structural_parameter.shape
    };
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::integer(4, 4), expected_shape],
            result: None,
        },
    )
    .map_err(|_| invalid())?;
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
        || structural_parameter.projected_qualifications
            != replay_parameter.projected_qualifications
        || (!affine_owned && !persistent_receiver)
        || structural_parameter.shape != expected_shape
        || countdown.call_plan != expected_call_plan
        || (persistent_receiver
            && countdown.structural_types != countdown.custody.semantic_replay.structural_types)
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

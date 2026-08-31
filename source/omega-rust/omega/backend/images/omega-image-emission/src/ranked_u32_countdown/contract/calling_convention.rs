//! Exact native argument placement and structural cleanup contract.

use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use omega_machine_code::RankedU32CountdownMachineCodeRecord;
use omega_target::NativeTarget;
use omega_target_operations::MachineRegister;
use psi_terminal::{StructuralAccess, StructuralMultiplicity, TerminalAffineCleanupAction};

pub(super) fn replay_calling_and_structural_contract(
    target: NativeTarget,
    record: &RankedU32CountdownMachineCodeRecord,
) -> Option<()> {
    let expected_rank_home = if target == NativeTarget::linux_x64() {
        MachineRegister::X86Rdi
    } else if target == NativeTarget::linux_arm64() {
        MachineRegister::Aarch64X(0)
    } else {
        return None;
    };
    let [rank, structural] = record.call_plan.parameters.as_slice() else {
        return None;
    };
    let [structural_parameter] = record.structural_parameters.as_slice() else {
        return None;
    };
    let replay = &record.custody.semantic_replay;
    let [replay_machine] = replay.machines.as_slice() else {
        return None;
    };
    let [replay_structural] = replay_machine.structural_parameters.as_slice() else {
        return None;
    };
    let referent_shape = crate::structural_condition_layout::replay_structural_value_shape(
        replay_structural.structural_type,
        &replay.structural_types,
    )?;
    let affine_owned = !replay_structural.is_self
        && replay_structural.multiplicity == StructuralMultiplicity::Affine
        && replay_structural.access == StructuralAccess::Owned;
    let persistent_receiver =
        replay_structural.is_self && replay_structural.access == StructuralAccess::MutableBorrow;
    let expected_structural_shape = if persistent_receiver {
        ValueShape::integer(
            u16::try_from(target.pointer_size).ok()?,
            u16::try_from(target.pointer_alignment).ok()?,
        )
    } else {
        referent_shape
    };
    let expected_rank = ValueShape::integer(4, 4);
    if rank.shape != expected_rank
        || rank.locations.as_slice()
            != [ValueLocation::Register {
                register: expected_rank_home,
                value_byte_offset: 0,
                byte_size: 4,
            }]
        || structural_parameter.place != replay_structural.place
        || structural_parameter.structural_type != replay_structural.structural_type
        || structural_parameter.multiplicity != replay_structural.multiplicity
        || structural_parameter.access != replay_structural.access
        || (!affine_owned && !persistent_receiver)
        || structural_parameter.shape != expected_structural_shape
        || structural != &structural_parameter.placement
        || record
            .structural_types
            .iter()
            .filter(|declaration| declaration.id == structural_parameter.structural_type)
            .count()
            != 1
        || (affine_owned
            && record.cleanup_actions.as_slice()
                != [TerminalAffineCleanupAction::DiscardRoot(
                    structural_parameter.place,
                )])
        || (persistent_receiver && !record.cleanup_actions.is_empty())
    {
        return None;
    }
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![expected_rank, structural_parameter.shape],
            result: None,
        },
    )
    .ok()?;
    (record.call_plan == expected).then_some(())
}

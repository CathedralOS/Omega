//! Resolve symbolic outgoing addresses against retained raw frame geometry.
//! These checks do not confer callee-save or frame-protocol authority.
use crate::OptimizedSelectedFormEncodingError as Error;
use machine_code::{ResolvedPhysicalAddress, TargetFrameLayoutPlan};
use physical_instructions::{
    PhysicalAddressOperation as Address, PostAllocationMachineFunction,
    PostAllocationMachineInstruction, PostAllocationMachinePlan,
};

pub(super) fn validate_frame_root(
    machine: &PostAllocationMachinePlan,
    frame: Option<&TargetFrameLayoutPlan>,
) -> Result<(), Error> {
    let Some(frame) = frame else {
        return if machine
            .functions
            .iter()
            .all(|function| function.outgoing_arguments.is_empty())
        {
            Ok(())
        } else {
            Err(Error::ArtifactMismatch)
        };
    };
    if frame.post_allocation_machine != machine.identity
        || frame.target != machine.target
        || frame.register_environment != machine.register_environment
        || frame.physical_register_model != machine.physical_register_model
        || frame.functions.len() != machine.functions.len()
        || frame
            .functions
            .iter()
            .zip(&machine.functions)
            .any(|(row, source)| row.machine != source.machine)
    {
        return Err(Error::ArtifactMismatch);
    }
    for row in &frame.functions {
        if row.outgoing_abi_area.byte_size > row.frame_size_bytes
            || u64::from(row.outgoing_abi_area.shadow_bytes) > row.outgoing_abi_area.byte_size
            || row.callee_save_slots.iter().any(|slot| {
                slot.frame_offset_bytes < row.outgoing_abi_area.byte_size
                    || slot
                        .frame_offset_bytes
                        .checked_add(slot.size_bytes)
                        .is_none_or(|end| end > row.frame_size_bytes)
            })
        {
            return Err(Error::ArtifactMismatch);
        }
    }
    Ok(())
}

pub(super) fn resolve(
    function: &PostAllocationMachineFunction,
    frame: Option<&TargetFrameLayoutPlan>,
    instruction: &PostAllocationMachineInstruction,
) -> Result<Option<ResolvedPhysicalAddress>, Error> {
    let Some(symbolic) = instruction.address else {
        return Ok(None);
    };
    let displacement = match symbolic {
        Address::Load64 { byte_offset, .. } => byte_offset,
        Address::Store64 { slot, byte_offset } | Address::FrameAddress { slot, byte_offset } => {
            let slot = function
                .outgoing_arguments
                .iter()
                .find(|row| row.id == slot)
                .ok_or(Error::ArtifactMismatch)?;
            let geometry = frame
                .and_then(|frame| {
                    frame
                        .functions
                        .iter()
                        .find(|row| row.machine == function.machine)
                })
                .ok_or(Error::ArtifactMismatch)?;
            let width = if matches!(symbolic, Address::Store64 { .. }) {
                8
            } else {
                0
            };
            if byte_offset >= slot.byte_size
                || byte_offset
                    .checked_add(width)
                    .is_none_or(|end| end > slot.byte_size)
            {
                return Err(Error::ArtifactMismatch);
            }
            let displacement = slot
                .abi_stack_byte_offset
                .checked_add(byte_offset)
                .ok_or(Error::ArtifactMismatch)?;
            if u64::from(displacement) + u64::from(width) > geometry.outgoing_abi_area.byte_size {
                return Err(Error::ArtifactMismatch);
            }
            displacement
        }
    };
    Ok(Some(ResolvedPhysicalAddress {
        symbolic,
        displacement,
    }))
}

/// Check the submitted address equation directly; never invoke address production.
pub(super) fn validate_address(
    function: &PostAllocationMachineFunction,
    frame: Option<&TargetFrameLayoutPlan>,
    instruction: &PostAllocationMachineInstruction,
    candidate: Option<ResolvedPhysicalAddress>,
) -> Result<(), Error> {
    let (Some(symbolic), Some(candidate)) = (instruction.address, candidate) else {
        return if instruction.address.is_none() && candidate.is_none() {
            Ok(())
        } else {
            Err(Error::ArtifactMismatch)
        };
    };
    if candidate.symbolic != symbolic {
        return Err(Error::ArtifactMismatch);
    }
    match symbolic {
        Address::Load64 {
            base_operand: 0,
            byte_offset,
        } if candidate.displacement == byte_offset => Ok(()),
        Address::Store64 { slot, byte_offset } | Address::FrameAddress { slot, byte_offset } => {
            let entries = function
                .outgoing_arguments
                .iter()
                .filter(|entry| entry.id == slot)
                .collect::<Vec<_>>();
            let [entry] = entries.as_slice() else {
                return Err(Error::ArtifactMismatch);
            };
            let frames = frame
                .ok_or(Error::ArtifactMismatch)?
                .functions
                .iter()
                .filter(|row| row.machine == function.machine)
                .collect::<Vec<_>>();
            let [geometry] = frames.as_slice() else {
                return Err(Error::ArtifactMismatch);
            };
            let width = if matches!(symbolic, Address::Store64 { .. }) {
                8_u64
            } else {
                0
            };
            let offset = u64::from(candidate.displacement);
            if offset != u64::from(entry.abi_stack_byte_offset) + u64::from(byte_offset)
                || byte_offset >= entry.byte_size
                || u64::from(byte_offset) + width > u64::from(entry.byte_size)
                || offset < u64::from(geometry.outgoing_abi_area.shadow_bytes)
                || offset + width > geometry.outgoing_abi_area.byte_size
                || offset > i32::MAX as u64
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        _ => Err(Error::ArtifactMismatch),
    }
}

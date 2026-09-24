//! Resolve symbolic frame addresses against retained raw frame geometry.
//! These checks do not confer callee-save or frame-protocol authority.
use crate::OptimizedSelectedFormEncodingError as Error;
use machine_code::{FunctionTargetFrameLayout, ResolvedPhysicalAddress, TargetFrameLayoutPlan};
use physical_instructions::{
    PhysicalAddressOperation as Address, PostAllocationMachineFunction,
    PostAllocationMachineInstruction, PostAllocationMachinePlan,
};
use selected_instructions::FrameStorageSlotId;

#[cfg(test)]
mod tests;

pub(super) fn validate_frame_root(
    machine: &PostAllocationMachinePlan,
    frame: Option<&TargetFrameLayoutPlan>,
) -> Result<(), Error> {
    let Some(frame) = frame else {
        return if machine.functions.iter().all(|function| {
            function.outgoing_arguments.is_empty() && function.local_storage_slots.is_empty()
        }) {
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
    {
        return Err(Error::ArtifactMismatch);
    }
    for (row, source) in frame.functions.iter().zip(&machine.functions) {
        if row.machine != source.machine
            || row.outgoing_abi_area.byte_size > row.frame_size_bytes
            || row.red_zone_resident_bytes > row.frame_size_bytes
            || u64::from(row.outgoing_abi_area.shadow_bytes) > row.outgoing_abi_area.byte_size
            || row.local_storage_slots.len() != source.local_storage_slots.len()
        {
            return Err(Error::ArtifactMismatch);
        }
        let mut local_end = row.outgoing_abi_area.byte_size;
        for (index, (placed, local)) in row
            .local_storage_slots
            .iter()
            .zip(&source.local_storage_slots)
            .enumerate()
        {
            if placed.id != local.id
                || placed.size_bytes != local.byte_size
                || placed.alignment_bytes != local.alignment
                || local.byte_size == 0
                || !local.alignment.is_power_of_two()
                || local.alignment > 8
                || !placed
                    .frame_offset_bytes
                    .is_multiple_of(u64::from(local.alignment))
                || placed
                    .frame_offset_bytes
                    .checked_sub(local_end)
                    .is_none_or(|padding| padding >= u64::from(local.alignment))
                || source.local_storage_slots[..index]
                    .iter()
                    .any(|earlier| earlier.id == local.id)
            {
                return Err(Error::ArtifactMismatch);
            }
            local_end = placed
                .frame_offset_bytes
                .checked_add(u64::from(placed.size_bytes))
                .filter(|end| *end <= row.frame_size_bytes)
                .ok_or(Error::ArtifactMismatch)?;
        }
        // The loan roster stays canonical and closed over the placed local
        // slots; exact equality with the materialized addresses is the
        // frame-layout replay's authority, checked upstream.
        if row
            .stable_address_loans
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || row
                .stable_address_loans
                .iter()
                .any(|loan| !row.local_storage_slots.iter().any(|slot| slot.id == *loan))
        {
            return Err(Error::ArtifactMismatch);
        }
        if row.callee_save_slots.iter().any(|slot| {
            slot.frame_offset_bytes < local_end
                || slot
                    .frame_offset_bytes
                    .checked_add(slot.size_bytes)
                    .is_none_or(|end| end > row.frame_size_bytes)
        }) {
            return Err(Error::ArtifactMismatch);
        }
    }
    Ok(())
}

/// Resolve only the retained slot roster; this does not derive address arithmetic.
fn slot_region(
    function: &PostAllocationMachineFunction,
    geometry: &FunctionTargetFrameLayout,
    slot: FrameStorageSlotId,
) -> Result<(u64, u32, u64, bool), Error> {
    match slot {
        FrameStorageSlotId::Incoming { .. } => Err(Error::ArtifactMismatch),
        FrameStorageSlotId::Outgoing(id) => {
            let mut entries = function
                .outgoing_arguments
                .iter()
                .filter(|entry| entry.id == id);
            let entry = entries.next().ok_or(Error::ArtifactMismatch)?;
            if entries.next().is_some() {
                return Err(Error::ArtifactMismatch);
            }
            Ok((
                u64::from(entry.abi_stack_byte_offset),
                entry.byte_size,
                geometry.outgoing_abi_area.byte_size,
                false,
            ))
        }
        FrameStorageSlotId::Local(id) => {
            let mut entries = function
                .local_storage_slots
                .iter()
                .filter(|entry| entry.id == id);
            let entry = entries.next().ok_or(Error::ArtifactMismatch)?;
            let mut placed = geometry
                .local_storage_slots
                .iter()
                .filter(|entry| entry.id == id);
            let position = placed.next().ok_or(Error::ArtifactMismatch)?;
            if entries.next().is_some()
                || placed.next().is_some()
                || entry.byte_size != position.size_bytes
                || entry.alignment != position.alignment_bytes
                || position.frame_offset_bytes < geometry.outgoing_abi_area.byte_size
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok((
                position.frame_offset_bytes,
                entry.byte_size,
                geometry.frame_size_bytes,
                true,
            ))
        }
    }
}

fn function_geometry<'frame>(
    function: &PostAllocationMachineFunction,
    frame: Option<&'frame TargetFrameLayoutPlan>,
) -> Result<&'frame FunctionTargetFrameLayout, Error> {
    let mut rows = frame
        .ok_or(Error::ArtifactMismatch)?
        .functions
        .iter()
        .filter(|row| row.machine == function.machine);
    let row = rows.next().ok_or(Error::ArtifactMismatch)?;
    if rows.next().is_some() {
        return Err(Error::ArtifactMismatch);
    }
    Ok(row)
}

/// The committed stack extent: the addressed extent minus the bytes resident
/// below the unadjusted entry stack pointer.
fn committed_extent(geometry: &FunctionTargetFrameLayout) -> Result<u64, Error> {
    geometry
        .frame_size_bytes
        .checked_sub(geometry.red_zone_resident_bytes)
        .ok_or(Error::ArtifactMismatch)
}

/// Signed post-prologue displacement of a frame-space offset: bytes resident
/// in the red zone sit below the unadjusted stack pointer and surface as
/// negative displacements.
fn frame_displacement(
    geometry: &FunctionTargetFrameLayout,
    frame_offset_bytes: u64,
) -> Result<i64, Error> {
    i64::try_from(frame_offset_bytes)
        .ok()
        .zip(i64::try_from(geometry.red_zone_resident_bytes).ok())
        .and_then(|(offset, resident)| offset.checked_sub(resident))
        .ok_or(Error::ArtifactMismatch)
}

/// A materialized activation-local address resolves only through the
/// validated stable-address-loan roster: a non-spill materialization is a
/// loan the layout must have recorded, while an allocator spill
/// materialization is a private reload window that must stay unrostered.
fn check_address_loan(
    geometry: &FunctionTargetFrameLayout,
    slot: selected_instructions::LocalStorageSlotId,
) -> Result<(), Error> {
    let spill = matches!(
        slot,
        selected_instructions::LocalStorageSlotId::Spill { .. }
    );
    if geometry.stable_address_loans.contains(&slot) == spill {
        return Err(Error::ArtifactMismatch);
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
        Address::FrameAddress {
            slot:
                FrameStorageSlotId::Incoming {
                    abi_stack_byte_offset,
                    ..
                },
            byte_offset: 0,
        } => {
            let geometry = function_geometry(function, frame)?;
            let committed = committed_extent(geometry)?;
            let return_address_bytes = match geometry.return_address {
                machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                    size_bytes,
                    ..
                } => u64::from(size_bytes),
                machine_code::ReturnAddressFrameCustody::SavedLinkRegister { .. }
                | machine_code::ReturnAddressFrameCustody::LiveLinkRegister { .. } => 0,
            };
            // Incoming storage starts above the committed extent plus the
            // caller-activation return address; resident bytes never shift it.
            let displacement = committed
                .checked_add(return_address_bytes)
                .and_then(|base| base.checked_add(u64::from(abi_stack_byte_offset)))
                .filter(|offset| *offset <= i32::MAX as u64)
                .ok_or(Error::ArtifactMismatch)?;
            i64::try_from(displacement).map_err(|_| Error::ArtifactMismatch)?
        }
        Address::SaveFloatingControl { slot } | Address::RestoreFloatingControl { slot } => {
            if !matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Boundary { .. }
            ) {
                return Err(Error::ArtifactMismatch);
            }
            let geometry = function_geometry(function, frame)?;
            let (start, size, limit, _) =
                slot_region(function, geometry, FrameStorageSlotId::Local(slot))?;
            let source = function
                .local_storage_slots
                .iter()
                .find(|entry| entry.id == slot)
                .ok_or(Error::ArtifactMismatch)?;
            let displacement = frame_displacement(geometry, start)?;
            if size != 8
                || source.alignment != 8
                || !start.is_multiple_of(8)
                || start.checked_add(8).is_none_or(|end| end > limit)
                || !(0..=i64::from(i32::MAX)).contains(&displacement)
            {
                return Err(Error::ArtifactMismatch);
            }
            displacement
        }
        Address::HostedWriteByteI32 { slot } => {
            if !matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Boundary { .. }
            ) {
                return Err(Error::ArtifactMismatch);
            }
            let geometry = function_geometry(function, frame)?;
            let (start, size, limit, _) =
                slot_region(function, geometry, FrameStorageSlotId::Local(slot))?;
            let source = function
                .local_storage_slots
                .iter()
                .find(|entry| entry.id == slot)
                .ok_or(Error::ArtifactMismatch)?;
            let displacement = frame_displacement(geometry, start)?;
            if size != 1
                || source.alignment != 1
                || start.checked_add(1).is_none_or(|end| end > limit)
                || !(0..=i64::from(i32::MAX)).contains(&displacement)
            {
                return Err(Error::ArtifactMismatch);
            }
            displacement
        }
        Address::HostedReadByte { slot } => {
            if !matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Structural { .. }
            ) {
                return Err(Error::ArtifactMismatch);
            }
            let geometry = function_geometry(function, frame)?;
            let (start, size, limit, _) =
                slot_region(function, geometry, FrameStorageSlotId::Local(slot))?;
            let source = function
                .local_storage_slots
                .iter()
                .find(|entry| entry.id == slot)
                .ok_or(Error::ArtifactMismatch)?;
            let displacement = frame_displacement(geometry, start)?;
            if size != 8
                || source.alignment != 4
                || !start.is_multiple_of(4)
                || start.checked_add(8).is_none_or(|end| end > limit)
                || !(0..=i64::from(i32::MAX)).contains(&displacement)
            {
                return Err(Error::ArtifactMismatch);
            }
            displacement
        }
        Address::Load8Indexed { .. } => 0,
        Address::LoadPacked { byte_offset, .. }
        | Address::StorePacked { byte_offset, .. }
        | Address::Load64 { byte_offset, .. }
        | Address::Load8 { byte_offset, .. }
        | Address::Load16 { byte_offset, .. }
        | Address::Load32 { byte_offset, .. }
        | Address::AddressOffset { byte_offset, .. } => i64::from(byte_offset),
        Address::Store {
            byte_offset,
            byte_size,
            ..
        } => {
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(Error::ArtifactMismatch);
            }
            i64::from(byte_offset)
        }
        Address::Store64 { slot, byte_offset } | Address::FrameAddress { slot, byte_offset } => {
            let geometry = function_geometry(function, frame)?;
            if let (Address::FrameAddress { .. }, FrameStorageSlotId::Local(id)) = (symbolic, slot)
            {
                check_address_loan(geometry, id)?;
            }
            let (start, size, limit, local) = slot_region(function, geometry, slot)?;
            let width = if matches!(symbolic, Address::Store64 { .. }) {
                8
            } else {
                0
            };
            if (!local && byte_offset >= size)
                || byte_offset.checked_add(width).is_none_or(|end| end > size)
            {
                return Err(Error::ArtifactMismatch);
            }
            let frame_offset = start
                .checked_add(u64::from(byte_offset))
                .ok_or(Error::ArtifactMismatch)?;
            if frame_offset
                .checked_add(u64::from(width))
                .is_none_or(|end| end > limit)
            {
                return Err(Error::ArtifactMismatch);
            }
            let displacement = frame_displacement(geometry, frame_offset)?;
            if i32::try_from(displacement).is_err() {
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
        Address::FrameAddress {
            slot:
                FrameStorageSlotId::Incoming {
                    abi_stack_byte_offset,
                    ..
                },
            byte_offset: 0,
        } => {
            let geometry = function_geometry(function, frame)?;
            let committed = committed_extent(geometry)?;
            let return_address_bytes = match geometry.return_address {
                machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                    size_bytes,
                    ..
                } => u64::from(size_bytes),
                machine_code::ReturnAddressFrameCustody::SavedLinkRegister { .. }
                | machine_code::ReturnAddressFrameCustody::LiveLinkRegister { .. } => 0,
            };
            let base = i64::try_from(
                committed
                    .checked_add(return_address_bytes)
                    .ok_or(Error::ArtifactMismatch)?,
            )
            .map_err(|_| Error::ArtifactMismatch)?;
            let incoming_offset = candidate.displacement.checked_sub(base);
            if incoming_offset != Some(i64::from(abi_stack_byte_offset))
                || !(0..=i64::from(i32::MAX)).contains(&candidate.displacement)
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        Address::SaveFloatingControl { slot } | Address::RestoreFloatingControl { slot } => {
            if !matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Boundary { .. }
            ) {
                return Err(Error::ArtifactMismatch);
            }
            let geometry = function_geometry(function, frame)?;
            let (start, size, limit, _) =
                slot_region(function, geometry, FrameStorageSlotId::Local(slot))?;
            let source = function
                .local_storage_slots
                .iter()
                .find(|entry| entry.id == slot)
                .ok_or(Error::ArtifactMismatch)?;
            if candidate.displacement != frame_displacement(geometry, start)?
                || size != 8
                || source.alignment != 8
                || !start.is_multiple_of(8)
                || start.checked_add(8).is_none_or(|end| end > limit)
                || !(0..=i64::from(i32::MAX)).contains(&candidate.displacement)
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        Address::HostedWriteByteI32 { slot } => {
            if !matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Boundary { .. }
            ) {
                return Err(Error::ArtifactMismatch);
            }
            let geometry = function_geometry(function, frame)?;
            let (start, size, limit, _) =
                slot_region(function, geometry, FrameStorageSlotId::Local(slot))?;
            let source = function
                .local_storage_slots
                .iter()
                .find(|entry| entry.id == slot)
                .ok_or(Error::ArtifactMismatch)?;
            if candidate.displacement != frame_displacement(geometry, start)?
                || size != 1
                || source.alignment != 1
                || start.checked_add(1).is_none_or(|end| end > limit)
                || !(0..=i64::from(i32::MAX)).contains(&candidate.displacement)
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        Address::HostedReadByte { slot } => {
            if !matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Structural { .. }
            ) {
                return Err(Error::ArtifactMismatch);
            }
            let geometry = function_geometry(function, frame)?;
            let (start, size, limit, _) =
                slot_region(function, geometry, FrameStorageSlotId::Local(slot))?;
            let source = function
                .local_storage_slots
                .iter()
                .find(|entry| entry.id == slot)
                .ok_or(Error::ArtifactMismatch)?;
            if candidate.displacement != frame_displacement(geometry, start)?
                || size != 8
                || source.alignment != 4
                || !start.is_multiple_of(4)
                || start.checked_add(8).is_none_or(|end| end > limit)
                || !(0..=i64::from(i32::MAX)).contains(&candidate.displacement)
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        Address::Store {
            base_operand: 0,
            byte_offset,
            byte_size,
        } if matches!(byte_size, 1 | 2 | 4 | 8)
            && candidate.displacement == i64::from(byte_offset) =>
        {
            Ok(())
        }
        Address::AddressOffset {
            base_operand: 0,
            byte_offset,
        } if candidate.displacement == i64::from(byte_offset) => Ok(()),
        Address::Load8Indexed {
            base_operand: 0,
            index_operand: 1,
        } if candidate.displacement == 0 => Ok(()),
        Address::LoadPacked {
            base_operand: 0,
            byte_offset,
            ..
        }
        | Address::StorePacked {
            base_operand: 0,
            byte_offset,
            ..
        }
        | Address::Load64 {
            base_operand: 0,
            byte_offset,
        }
        | Address::Load8 {
            base_operand: 0,
            byte_offset,
        }
        | Address::Load16 {
            base_operand: 0,
            byte_offset,
        }
        | Address::Load32 {
            base_operand: 0,
            byte_offset,
        } if candidate.displacement == i64::from(byte_offset) => Ok(()),
        Address::Store64 { slot, byte_offset } | Address::FrameAddress { slot, byte_offset } => {
            let geometry = function_geometry(function, frame)?;
            if let (Address::FrameAddress { .. }, FrameStorageSlotId::Local(id)) = (symbolic, slot)
            {
                check_address_loan(geometry, id)?;
            }
            let (start, size, limit, local) = slot_region(function, geometry, slot)?;
            let width = if matches!(symbolic, Address::Store64 { .. }) {
                8_u64
            } else {
                0
            };
            // Replay checks the frame-space equation: the signed displacement
            // plus the resident extent must land on the slot's byte position.
            let resident = i64::try_from(geometry.red_zone_resident_bytes)
                .map_err(|_| Error::ArtifactMismatch)?;
            let frame_offset = candidate
                .displacement
                .checked_add(resident)
                .and_then(|value| u64::try_from(value).ok());
            if frame_offset.and_then(|offset| offset.checked_sub(start))
                != Some(u64::from(byte_offset))
                || (!local && byte_offset >= size)
                || u64::from(byte_offset) + width > u64::from(size)
                || frame_offset.is_none_or(|offset| {
                    offset < u64::from(geometry.outgoing_abi_area.shadow_bytes)
                        || offset + width > limit
                })
                || i32::try_from(candidate.displacement).is_err()
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        _ => Err(Error::ArtifactMismatch),
    }
}

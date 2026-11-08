/*
We resolve symbolic frame and pointer addresses on post-allocation machine
instructions against the retained `TargetFrameLayoutPlan`, and we check
submitted `ResolvedPhysicalAddress` equations during independent replay
(`:FrameAddressEquations`).

`validate_frame_root` first verifies that a missing frame plan is permitted
only when no function declares outgoing arguments or local storage slots, and
that a present frame plan matches the post-allocation machine plan's roots,
outgoing ABI area bounds, aligned non-overlapping local storage placements,
sorted stable-address-loan roster, and callee-save slot bounds.

During production, `resolve` derives the signed post-prologue byte displacement
and required frame alignment for an instruction's `PhysicalAddressOperation`:
- Activation-local addresses (`FrameAddress` / `Store64`) resolve against the
  placed local or outgoing slot, subtracting `red_zone_resident_bytes` so slots
  in the red zone surface as negative offsets below the unadjusted stack
  pointer, and enforcing `check_address_loan` (materializing a non-spill local
  address requires a recorded stable-address loan, whereas a private spill
  reload must stay unrostered).
- Incoming stack parameters resolve above the committed frame extent plus any
  caller-activation return address.
- Hosted runtime windows (`SaveFloatingControl`, `RestoreFloatingControl`,
  `HostedWriteByteI32`, `HostedReadByte`) enforce exact slot class, width, and
  alignment contracts (`hosted_window_displacement`).

During replay, `validate_address` checks the frame-space equation directly
without invoking `resolve`.
*/

use crate::OptimizedSelectedFormEncodingError as Error;
use machine_code::{FunctionTargetFrameLayout, ResolvedPhysicalAddress, TargetFrameLayoutPlan};
use physical_instructions::{
    PhysicalAddressOperation as Address, PostAllocationMachineFunction,
    PostAllocationMachineInstruction, PostAllocationMachinePlan,
};
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot};

fn local_slot_region<'function, 'frame>(
    function: &'function PostAllocationMachineFunction,
    frame: Option<&'frame TargetFrameLayoutPlan>,
    slot: LocalStorageSlotId,
) -> Result<
    (
        &'frame FunctionTargetFrameLayout,
        u64,
        u32,
        u64,
        &'function SelectedLocalStorageSlot,
    ),
    Error,
> {
    let geometry = function_geometry(function, frame)?;
    let (start, size, limit, _) = slot_region(function, geometry, FrameStorageSlotId::Local(slot))?;
    let source = function
        .local_storage_slots
        .iter()
        .find(|entry| entry.id == slot)
        .ok_or(Error::ArtifactMismatch)?;
    Ok((geometry, start, size, limit, source))
}

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
                || !placed.frame_offset_bytes.is_multiple_of(u64::from(
                    local.alignment.min(row.abi_stack_alignment_bytes),
                ))
                || placed
                    .frame_offset_bytes
                    .checked_sub(local_end)
                    .is_none_or(|padding| {
                        padding >= u64::from(local.alignment.min(row.abi_stack_alignment_bytes))
                    })
                || source.local_storage_slots[..index]
                    .iter()
                    .any(|earlier| earlier.id == local.id)
            {
                return Err(Error::ArtifactMismatch);
            }
            local_end = placed
                .frame_offset_bytes
                .checked_add(placed.reserved_size_bytes(row.abi_stack_alignment_bytes))
                .filter(|end| *end <= row.frame_size_bytes)
                .ok_or(Error::ArtifactMismatch)?;
        }
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

fn unique<T>(entries: &[T], admits: impl Fn(&T) -> bool) -> Result<&T, Error> {
    let mut found = entries.iter().filter(|entry| admits(entry));
    let only = found.next().ok_or(Error::ArtifactMismatch)?;
    if found.next().is_some() {
        return Err(Error::ArtifactMismatch);
    }
    Ok(only)
}

fn slot_region(
    function: &PostAllocationMachineFunction,
    geometry: &FunctionTargetFrameLayout,
    slot: FrameStorageSlotId,
) -> Result<(u64, u32, u64, bool), Error> {
    match slot {
        FrameStorageSlotId::ActivationBase | FrameStorageSlotId::Incoming { .. } => {
            Err(Error::ArtifactMismatch)
        }
        FrameStorageSlotId::Outgoing(id) => {
            let entry = unique(&function.outgoing_arguments, |entry| entry.id == id)?;
            Ok((
                u64::from(entry.abi_stack_byte_offset),
                entry.byte_size,
                geometry.outgoing_abi_area.byte_size,
                false,
            ))
        }
        FrameStorageSlotId::Local(id) => {
            let entry = unique(&function.local_storage_slots, |entry| entry.id == id)?;
            let position = unique(&geometry.local_storage_slots, |entry| entry.id == id)?;
            if entry.byte_size != position.size_bytes
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
    let rows = &frame.ok_or(Error::ArtifactMismatch)?.functions;
    unique(rows, |row| row.machine == function.machine)
}

fn frame_address_alignment(
    function: &PostAllocationMachineFunction,
    frame: Option<&TargetFrameLayoutPlan>,
    symbolic: Address,
) -> Result<u16, Error> {
    let (Address::FrameAddress { slot, .. } | Address::Store64 { slot, .. }) = symbolic else {
        return Ok(1);
    };
    let geometry = function_geometry(function, frame)?;
    let (alignment, start) = match slot {
        FrameStorageSlotId::Local(slot) => {
            let placed = geometry
                .local_storage_slot(slot)
                .ok_or(Error::ArtifactMismatch)?;
            (placed.alignment_bytes, placed.frame_offset_bytes)
        }
        FrameStorageSlotId::Outgoing(slot)
            if slot.role == selected_instructions::OutgoingArgumentSlotRole::ValueCopy =>
        {
            let placed = function
                .outgoing_arguments
                .iter()
                .find(|entry| entry.id == slot)
                .ok_or(Error::ArtifactMismatch)?;
            (placed.alignment, u64::from(placed.abi_stack_byte_offset))
        }
        _ => return Ok(1),
    };
    if alignment <= geometry.abi_stack_alignment_bytes {
        return Ok(1);
    }
    if geometry.abi_stack_alignment_bytes != 16
        || geometry.red_zone_resident_bytes != 0
        || !alignment.is_power_of_two()
        || !start.is_multiple_of(16)
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok(alignment)
}

fn committed_extent(geometry: &FunctionTargetFrameLayout) -> Result<u64, Error> {
    geometry
        .frame_size_bytes
        .checked_sub(geometry.red_zone_resident_bytes)
        .ok_or(Error::ArtifactMismatch)
}

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

#[derive(Clone, Copy)]
enum HostedWindowClass {
    Boundary,   // Hosted runtime boundary storage (floating control and written bytes).
    Structural, // Read-only structural home.
}

impl HostedWindowClass {
    fn admits(self, slot: LocalStorageSlotId) -> bool {
        matches!(
            (self, slot),
            (Self::Boundary, LocalStorageSlotId::Boundary { .. })
                | (Self::Structural, LocalStorageSlotId::Structural { .. })
        )
    }
}

fn hosted_window_displacement(
    function: &PostAllocationMachineFunction,
    frame: Option<&TargetFrameLayoutPlan>,
    symbolic: Address,
) -> Result<i64, Error> {
    let (slot, class, width, alignment) = match symbolic {
        Address::SaveFloatingControl { slot } | Address::RestoreFloatingControl { slot } => {
            (slot, HostedWindowClass::Boundary, 8_u32, 8_u16)
        }
        Address::HostedWriteByteI32 { slot } => (slot, HostedWindowClass::Boundary, 1, 1),
        Address::HostedReadByte { slot } => (slot, HostedWindowClass::Structural, 8, 4),
        _ => return Err(Error::ArtifactMismatch),
    };
    if !class.admits(slot) {
        return Err(Error::ArtifactMismatch);
    }
    let (geometry, start, size, limit, source) = local_slot_region(function, frame, slot)?;
    let displacement = frame_displacement(geometry, start)?;
    if size != width
        || source.alignment != alignment
        || !start.is_multiple_of(u64::from(alignment))
        || start
            .checked_add(u64::from(width))
            .is_none_or(|end| end > limit)
        || !(0..=i64::from(i32::MAX)).contains(&displacement)
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok(displacement)
}

fn validate_hosted_window(
    candidate: &ResolvedPhysicalAddress,
    function: &PostAllocationMachineFunction,
    frame: Option<&TargetFrameLayoutPlan>,
    symbolic: Address,
) -> Result<(), Error> {
    if candidate.displacement != hosted_window_displacement(function, frame, symbolic)? {
        return Err(Error::ArtifactMismatch);
    }
    Ok(())
}

fn check_address_loan(
    geometry: &FunctionTargetFrameLayout,
    slot: LocalStorageSlotId,
) -> Result<(), Error> {
    let spill = matches!(slot, LocalStorageSlotId::Spill { .. });
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
    let frame_alignment_bytes = frame_address_alignment(function, frame, symbolic)?;
    let displacement = match symbolic {
        Address::FrameAddress {
            slot: FrameStorageSlotId::ActivationBase,
            byte_offset: 0,
        } => {
            function_geometry(function, frame)?;
            0
        }
        Address::FrameAddress {
            slot:
                FrameStorageSlotId::Incoming {
                    abi_stack_byte_offset,
                    ..
                },
            byte_offset: 0,
        } => {
            let base_bytes = incoming_frame_base_bytes(function, frame)?;
            let displacement = base_bytes
                .checked_add(u64::from(abi_stack_byte_offset))
                .filter(|offset| *offset <= i32::MAX as u64)
                .ok_or(Error::ArtifactMismatch)?;
            i64::try_from(displacement).map_err(|_| Error::ArtifactMismatch)?
        }
        Address::SaveFloatingControl { .. }
        | Address::RestoreFloatingControl { .. }
        | Address::HostedWriteByteI32 { .. }
        | Address::HostedReadByte { .. } => hosted_window_displacement(function, frame, symbolic)?,
        Address::Load8Indexed { .. } => 0,
        Address::AccessorScalarAccess { byte_offset, .. }
        | Address::LoadPacked { byte_offset, .. }
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
                .checked_add(if frame_alignment_bytes == 1 {
                    u64::from(byte_offset)
                } else {
                    0
                })
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
        frame_alignment_bytes,
    }))
}

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
    if candidate.symbolic != symbolic
        || candidate.frame_alignment_bytes != frame_address_alignment(function, frame, symbolic)?
    {
        return Err(Error::ArtifactMismatch);
    }
    match symbolic {
        Address::FrameAddress {
            slot: FrameStorageSlotId::ActivationBase,
            byte_offset: 0,
        } => {
            function_geometry(function, frame)?;
            if candidate.displacement != 0 || candidate.frame_alignment_bytes != 1 {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        Address::FrameAddress {
            slot:
                FrameStorageSlotId::Incoming {
                    abi_stack_byte_offset,
                    ..
                },
            byte_offset: 0,
        } => {
            let base_bytes = incoming_frame_base_bytes(function, frame)?;
            let base = i64::try_from(base_bytes).map_err(|_| Error::ArtifactMismatch)?;
            let incoming_offset = candidate.displacement.checked_sub(base);
            if incoming_offset != Some(i64::from(abi_stack_byte_offset))
                || !(0..=i64::from(i32::MAX)).contains(&candidate.displacement)
            {
                return Err(Error::ArtifactMismatch);
            }
            Ok(())
        }
        Address::SaveFloatingControl { .. }
        | Address::RestoreFloatingControl { .. }
        | Address::HostedWriteByteI32 { .. }
        | Address::HostedReadByte { .. } => {
            validate_hosted_window(&candidate, function, frame, symbolic)
        }
        Address::AccessorScalarAccess {
            base_operand: 0,
            byte_offset,
            form,
            ..
        } if form.ordinary_semantic().is_some()
            && candidate.displacement == i64::from(byte_offset) =>
        {
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
            let resident = i64::try_from(geometry.red_zone_resident_bytes)
                .map_err(|_| Error::ArtifactMismatch)?;
            let frame_offset = candidate
                .displacement
                .checked_add(resident)
                .and_then(|value| u64::try_from(value).ok());
            if frame_offset.and_then(|offset| offset.checked_sub(start))
                != Some(if candidate.frame_alignment_bytes == 1 {
                    u64::from(byte_offset)
                } else {
                    0
                })
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

fn incoming_frame_base_bytes(
    function: &PostAllocationMachineFunction,
    frame: Option<&TargetFrameLayoutPlan>,
) -> Result<u64, Error> {
    let geometry = function_geometry(function, frame)?;
    let committed = committed_extent(geometry)?;
    let return_address_bytes = match geometry.return_address {
        machine_code::ReturnAddressFrameCustody::CallerActivationStack { size_bytes, .. } => {
            u64::from(size_bytes)
        }
        machine_code::ReturnAddressFrameCustody::SavedLinkRegister { .. }
        | machine_code::ReturnAddressFrameCustody::LiveLinkRegister { .. } => 0,
    };
    committed
        .checked_add(return_address_bytes)
        .ok_or(Error::ArtifactMismatch)
}

#[cfg(test)]
mod tests {
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternative, MachineAlternativeApplicability,
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
        MachineLatencyKnowledge, MachineSizeKnowledge, SelectedInstructionId,
        SelectedLocalStorageSlot,
    };
    use semantic_vocabulary::{MachineId, OperationId, PlaceId};

    use super::{
        Address, FrameStorageSlotId, FunctionTargetFrameLayout, PostAllocationMachineFunction,
        PostAllocationMachineInstruction, ResolvedPhysicalAddress, TargetFrameLayoutPlan, resolve,
        validate_address,
    };

    fn fixture() -> (
        PostAllocationMachineFunction,
        TargetFrameLayoutPlan,
        PostAllocationMachineInstruction,
    ) {
        let id = LocalStorageSlotId::Structural {
            operation: OperationId::new(3).unwrap(),
            place: PlaceId::new(5).unwrap(),
        };
        let machine = MachineId::new(1).unwrap();
        let function = PostAllocationMachineFunction {
            machine,
            outgoing_arguments: Vec::new(),
            local_storage_slots: vec![SelectedLocalStorageSlot {
                id,
                byte_size: 16,
                alignment: 8,
            }],
            blocks: Vec::new(),
        };
        let frame = TargetFrameLayoutPlan {
            post_allocation_machine:
                physical_instructions::PostAllocationMachineIdentity::from_bytes([0; 32]),
            callee_saved_requirements:
                register_homes::AllocatedCalleeSavedRequirementIdentity::from_bytes([0; 32]),
            callee_save_storage:
                machine_code::NonAuthoritativeCalleeSaveStorageIdentity::from_bytes([0; 32]),
            register_environment: register_model::TargetRegisterEnvironmentIdentity::from_bytes(
                [0; 32],
            ),
            physical_register_model: register_model::PhysicalRegisterModelIdentity::from_bytes(
                [0; 32],
            ),
            target: target::NativeTarget::linux_x64(),
            abi: register_model::FrameAbiPreservationConvention::SystemVAMD64,
            policy: machine_code::TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
            functions: vec![FunctionTargetFrameLayout {
                machine,
                contains_call: false,
                stack_pointer: register_model::RegisterViewId(0),
                pre_call_stack_alignment: 16,
                frame_size_bytes: 16,
                red_zone_resident_bytes: 0,
                abi_stack_alignment_bytes: 16,
                outgoing_abi_area: machine_code::OutgoingAbiFrameArea {
                    byte_size: 0,
                    shadow_bytes: 0,
                },
                local_storage_slots: vec![machine_code::LocalStorageFrameSlot {
                    id,
                    frame_offset_bytes: 0,
                    size_bytes: 16,
                    alignment_bytes: 8,
                }],
                stable_address_loans: vec![id],
                callee_save_slots: Vec::new(),
                return_address: machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                    post_prologue_offset_bytes: 16,
                    size_bytes: 8,
                },
                stack_probe: machine_code::StackProbePlan {
                    interval_bytes: 4_096,
                    touches: 0,
                },
                unwind: machine_code::FrameUnwindPlan {
                    restores: Vec::new(),
                    released_bytes: 16,
                    return_address:
                        machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                            post_prologue_offset_bytes: 16,
                            size_bytes: 8,
                        },
                },
            }],
        };
        let instruction = PostAllocationMachineInstruction {
            instruction: SelectedInstructionId(1),
            alternative: MachineAlternative {
                key: MachineAlternativeKey {
                    family: MachineAlternativeFamily::FrameAddress,
                    variant: 0,
                },
                applicability: MachineAlternativeApplicability::Always,
                size: MachineSizeKnowledge::ExactBytes(8),
                latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                encoded: MachineEncodedEffects::fallthrough_v1(Vec::new(), vec![0]),
            },
            address: Some(Address::FrameAddress {
                slot: FrameStorageSlotId::Local(id),
                byte_offset: 16,
            }),
            operands: Vec::new(),
            implicit_unit_uses: Vec::new(),
            implicit_unit_defs: Vec::new(),
            implicit_unit_clobbers: Vec::new(),
            unit_uses: Vec::new(),
            unit_defs: Vec::new(),
            unit_clobbers: Vec::new(),
        };
        (function, frame, instruction)
    }

    fn place_local_slot(
        function: &mut PostAllocationMachineFunction,
        frame: &mut TargetFrameLayoutPlan,
        slot: LocalStorageSlotId,
        (offset, size, alignment): (u64, u32, u16),
    ) {
        function.local_storage_slots[0] = SelectedLocalStorageSlot {
            id: slot,
            byte_size: size,
            alignment,
        };
        frame.functions[0].local_storage_slots[0] = machine_code::LocalStorageFrameSlot {
            id: slot,
            frame_offset_bytes: offset,
            size_bytes: size,
            alignment_bytes: alignment,
        };
    }

    fn assert_rejected(
        function: &PostAllocationMachineFunction,
        frame: Option<&TargetFrameLayoutPlan>,
        instruction: &PostAllocationMachineInstruction,
        candidate: ResolvedPhysicalAddress,
    ) {
        assert!(validate_address(function, frame, instruction, Some(candidate)).is_err());
    }

    #[test]
    fn read_byte_result_requires_structural_identity_and_exact_eight_byte_home() {
        let (mut function, mut frame, mut instruction) = fixture();
        let slot = function.local_storage_slots[0].id;
        function.local_storage_slots[0].byte_size = 8;
        function.local_storage_slots[0].alignment = 4;
        frame.functions[0].local_storage_slots[0].size_bytes = 8;
        frame.functions[0].local_storage_slots[0].alignment_bytes = 4;
        instruction.address = Some(Address::HostedReadByte { slot });
        let resolved = resolve(&function, Some(&frame), &instruction)
            .unwrap()
            .unwrap();
        validate_address(&function, Some(&frame), &instruction, Some(resolved)).unwrap();
        for mutation in 0..8 {
            let mut changed_function = function.clone();
            let mut changed_frame = frame.clone();
            let mut changed_instruction = instruction.clone();
            let mut candidate = resolved;
            match mutation {
                0 => candidate.displacement = 4,
                1 => {
                    let wrong_slot = LocalStorageSlotId::Boundary {
                        operation: OperationId::new(3).unwrap(),
                    };
                    changed_function.local_storage_slots[0].id = wrong_slot;
                    changed_frame.functions[0].local_storage_slots[0].id = wrong_slot;
                    changed_instruction.address =
                        Some(Address::HostedReadByte { slot: wrong_slot });
                    candidate.symbolic = changed_instruction.address.unwrap();
                }
                2 => {
                    changed_function.local_storage_slots[0].byte_size = 4;
                    changed_frame.functions[0].local_storage_slots[0].size_bytes = 4;
                }
                3 => {
                    changed_function.local_storage_slots[0].alignment = 8;
                    changed_frame.functions[0].local_storage_slots[0].alignment_bytes = 8;
                }
                4 => changed_frame.functions[0].frame_size_bytes = 7,
                5 => changed_frame.functions[0].local_storage_slots[0].frame_offset_bytes = 1,
                6 => changed_function
                    .local_storage_slots
                    .push(function.local_storage_slots[0].clone()),
                _ => {
                    changed_frame.functions[0].local_storage_slots[0].id =
                        LocalStorageSlotId::Structural {
                            operation: OperationId::new(3).unwrap(),
                            place: PlaceId::new(7).unwrap(),
                        }
                }
            }
            if mutation != 0 {
                assert!(
                    resolve(
                        &changed_function,
                        Some(&changed_frame),
                        &changed_instruction
                    )
                    .is_err()
                );
            }
            assert_rejected(
                &changed_function,
                Some(&changed_frame),
                &changed_instruction,
                candidate,
            );
        }
    }

    #[test]
    fn incoming_pointer_slots_bind_entry_bias_frame_size_and_parameter_identity() {
        let (function, mut frame, mut instruction) = fixture();
        let slot = FrameStorageSlotId::Incoming {
            parameter_index: 8,
            abi_stack_byte_offset: 32,
        };
        instruction.address = Some(Address::FrameAddress {
            slot,
            byte_offset: 0,
        });
        for return_address in [
            machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 16,
                size_bytes: 8,
            },
            machine_code::ReturnAddressFrameCustody::LiveLinkRegister {
                view: register_model::RegisterViewId(30),
            },
            machine_code::ReturnAddressFrameCustody::SavedLinkRegister {
                view: register_model::RegisterViewId(30),
                frame_offset_bytes: 8,
                size_bytes: 8,
            },
        ] {
            frame.functions[0].return_address = return_address;
            let bias = if matches!(
                return_address,
                machine_code::ReturnAddressFrameCustody::CallerActivationStack { .. }
            ) {
                8
            } else {
                0
            };
            let resolved = resolve(&function, Some(&frame), &instruction)
                .unwrap()
                .unwrap();
            assert_eq!(resolved.displacement, 16 + bias + 32);
            validate_address(&function, Some(&frame), &instruction, Some(resolved)).unwrap();
            for displacement in [resolved.displacement - 8, resolved.displacement + 8] {
                assert_rejected(
                    &function,
                    Some(&frame),
                    &instruction,
                    ResolvedPhysicalAddress {
                        displacement,
                        ..resolved
                    },
                );
            }
            let changed = ResolvedPhysicalAddress {
                symbolic: Address::FrameAddress {
                    slot: FrameStorageSlotId::Incoming {
                        parameter_index: 9,
                        abi_stack_byte_offset: 32,
                    },
                    byte_offset: 0,
                },
                ..resolved
            };
            assert!(
                validate_address(&function, Some(&frame), &instruction, Some(changed)).is_err()
            );
        }
        assert!(resolve(&function, None, &instruction).is_err());
        for address in [
            Address::Store64 {
                slot,
                byte_offset: 0,
            },
            Address::FrameAddress {
                slot,
                byte_offset: 8,
            },
        ] {
            instruction.address = Some(address);
            assert!(resolve(&function, Some(&frame), &instruction).is_err());
            assert_rejected(
                &function,
                Some(&frame),
                &instruction,
                ResolvedPhysicalAddress {
                    symbolic: address,
                    frame_alignment_bytes: 1,
                    displacement: 48,
                },
            );
        }
    }

    #[test]
    fn floating_control_frame_custody_rejects_slot_geometry_and_address_substitution() {
        for restore in [false, true] {
            let (mut function, mut frame, mut instruction) = fixture();
            let slot = LocalStorageSlotId::Boundary {
                operation: OperationId::new(3).unwrap(),
            };
            place_local_slot(&mut function, &mut frame, slot, (8, 8, 8));
            frame.functions[0].outgoing_abi_area.byte_size = 8;
            frame.functions[0].stable_address_loans.clear();
            instruction.address = Some(if restore {
                Address::RestoreFloatingControl { slot }
            } else {
                Address::SaveFloatingControl { slot }
            });
            let resolved = resolve(&function, Some(&frame), &instruction)
                .unwrap()
                .unwrap();
            assert_eq!(resolved.displacement, 8);
            validate_address(&function, Some(&frame), &instruction, Some(resolved)).unwrap();
            for mutation in 0..9 {
                let mut changed_function = function.clone();
                let mut changed_frame = frame.clone();
                let mut changed_instruction = instruction.clone();
                let mut candidate = resolved;
                match mutation {
                    0 => candidate.displacement = 0,
                    1 => {
                        candidate.symbolic = if restore {
                            Address::SaveFloatingControl { slot }
                        } else {
                            Address::RestoreFloatingControl { slot }
                        }
                    }
                    2 => {
                        changed_function.local_storage_slots[0].byte_size = 4;
                        changed_frame.functions[0].local_storage_slots[0].size_bytes = 4;
                    }
                    3 => {
                        changed_function.local_storage_slots[0].alignment = 4;
                        changed_frame.functions[0].local_storage_slots[0].alignment_bytes = 4;
                    }
                    4 => changed_frame.functions[0].frame_size_bytes = 8,
                    5 => changed_frame.functions[0].local_storage_slots[0].frame_offset_bytes = 0,
                    6 => changed_frame.functions[0].local_storage_slots[0].frame_offset_bytes = 9,
                    7 => changed_function
                        .local_storage_slots
                        .push(changed_function.local_storage_slots[0].clone()),
                    _ => {
                        let wrong_slot = LocalStorageSlotId::Structural {
                            operation: OperationId::new(3).unwrap(),
                            place: PlaceId::new(5).unwrap(),
                        };
                        changed_function.local_storage_slots[0].id = wrong_slot;
                        changed_frame.functions[0].local_storage_slots[0].id = wrong_slot;
                        changed_instruction.address = Some(if restore {
                            Address::RestoreFloatingControl { slot: wrong_slot }
                        } else {
                            Address::SaveFloatingControl { slot: wrong_slot }
                        });
                        candidate.symbolic = changed_instruction.address.unwrap();
                    }
                }
                if mutation >= 2 {
                    assert!(
                        resolve(
                            &changed_function,
                            Some(&changed_frame),
                            &changed_instruction
                        )
                        .is_err(),
                        "mutation {mutation}"
                    );
                }
                assert!(
                    validate_address(
                        &changed_function,
                        Some(&changed_frame),
                        &changed_instruction,
                        Some(candidate)
                    )
                    .is_err(),
                    "mutation {mutation}"
                );
            }
            assert!(resolve(&function, None, &instruction).is_err());
        }
    }

    #[test]
    fn boundary_byte_scratch_requires_exact_origin_geometry_and_offset() {
        let (mut function, mut frame, mut instruction) = fixture();
        let slot = LocalStorageSlotId::Boundary {
            operation: OperationId::new(3).unwrap(),
        };
        place_local_slot(&mut function, &mut frame, slot, (0, 1, 1));
        instruction.address = Some(Address::HostedWriteByteI32 { slot });
        let resolved = resolve(&function, Some(&frame), &instruction).unwrap();
        validate_address(&function, Some(&frame), &instruction, resolved).unwrap();
        for mutation in 0..6 {
            let mut changed_function = function.clone();
            let mut changed_frame = frame.clone();
            let mut changed_instruction = instruction.clone();
            let mut candidate = resolved.unwrap();
            match mutation {
                0 => candidate.displacement = 1,
                1 => {
                    changed_instruction.address = Some(Address::HostedWriteByteI32 {
                        slot: LocalStorageSlotId::Structural {
                            operation: slot.operation().expect("source-backed local slot"),
                            place: PlaceId::new(5).unwrap(),
                        },
                    })
                }
                2 => {
                    changed_frame.functions[0].local_storage_slots[0].id =
                        LocalStorageSlotId::Boundary {
                            operation: OperationId::new(7).unwrap(),
                        }
                }
                3 => {
                    changed_function.local_storage_slots[0].byte_size = 2;
                    changed_frame.functions[0].local_storage_slots[0].size_bytes = 2;
                }
                4 => {
                    changed_function.local_storage_slots[0].alignment = 2;
                    changed_frame.functions[0].local_storage_slots[0].alignment_bytes = 2;
                }
                _ => changed_frame.functions[0].frame_size_bytes = 0,
            }
            if mutation != 0 {
                assert!(
                    resolve(
                        &changed_function,
                        Some(&changed_frame),
                        &changed_instruction
                    )
                    .is_err()
                );
            }
            assert_rejected(
                &changed_function,
                Some(&changed_frame),
                &changed_instruction,
                candidate,
            );
        }
    }

    #[test]
    fn empty_local_backing_allows_address_but_not_one_past_store() {
        let (function, frame, mut instruction) = fixture();
        let resolved = resolve(&function, Some(&frame), &instruction).unwrap();
        assert_eq!(resolved.unwrap().displacement, 16);
        validate_address(&function, Some(&frame), &instruction, resolved).unwrap();
        let Address::FrameAddress { slot, .. } = instruction.address.unwrap() else {
            unreachable!()
        };
        instruction.address = Some(Address::Store64 {
            slot,
            byte_offset: 16,
        });
        assert!(resolve(&function, Some(&frame), &instruction).is_err());
        assert_rejected(
            &function,
            Some(&frame),
            &instruction,
            ResolvedPhysicalAddress {
                symbolic: instruction.address.unwrap(),
                frame_alignment_bytes: 1,
                displacement: 16,
            },
        );
        instruction.address = Some(Address::Store64 {
            slot,
            byte_offset: 8,
        });
        let resolved = resolve(&function, Some(&frame), &instruction).unwrap();
        validate_address(&function, Some(&frame), &instruction, resolved).unwrap();
    }

    #[test]
    fn local_address_replay_rejects_displacement_source_and_extent_substitution() {
        let (function, frame, instruction) = fixture();
        let resolved = resolve(&function, Some(&frame), &instruction).unwrap();
        let mut shifted = resolved.unwrap();
        shifted.displacement -= 8;
        assert!(validate_address(&function, Some(&frame), &instruction, Some(shifted)).is_err());
        for mutation in 0..4 {
            let mut changed = frame.clone();
            let local = &mut changed.functions[0].local_storage_slots[0];
            match mutation {
                0 => {
                    local.id = LocalStorageSlotId::Structural {
                        operation: local.id.operation().expect("source-backed local slot"),
                        place: PlaceId::new(7).unwrap(),
                    }
                }
                1 => {
                    local.id = LocalStorageSlotId::Structural {
                        operation: OperationId::new(11).unwrap(),
                        place: local.id.structural_place().unwrap(),
                    }
                }
                2 => local.size_bytes += 8,
                _ => local.alignment_bytes = 4,
            }
            assert!(resolve(&function, Some(&changed), &instruction).is_err());
            assert!(validate_address(&function, Some(&changed), &instruction, resolved).is_err());
        }
        let mut duplicate = function.clone();
        duplicate
            .local_storage_slots
            .push(function.local_storage_slots[0].clone());
        assert!(resolve(&duplicate, Some(&frame), &instruction).is_err());
        assert!(validate_address(&duplicate, Some(&frame), &instruction, resolved).is_err());
    }

    #[test]
    fn pointer_address_replay_binds_base_offset_and_store_width_without_a_frame() {
        let (function, _, mut instruction) = fixture();
        for symbolic in [
            Address::Store {
                base_operand: 0,
                byte_offset: 2,
                byte_size: 2,
            },
            Address::AddressOffset {
                base_operand: 0,
                byte_offset: 2,
            },
        ] {
            instruction.address = Some(symbolic);
            let resolved = resolve(&function, None, &instruction).unwrap();
            validate_address(&function, None, &instruction, resolved).unwrap();
            let mut changed = resolved.unwrap();
            changed.displacement += 1;
            assert!(validate_address(&function, None, &instruction, Some(changed)).is_err());
            changed.displacement = 2;
            changed.symbolic = Address::Store {
                base_operand: 1,
                byte_offset: 2,
                byte_size: 2,
            };
            assert!(validate_address(&function, None, &instruction, Some(changed)).is_err());
        }
        instruction.address = Some(Address::Store {
            base_operand: 0,
            byte_offset: 2,
            byte_size: 3,
        });
        assert!(resolve(&function, None, &instruction).is_err());
        assert_rejected(
            &function,
            None,
            &instruction,
            ResolvedPhysicalAddress {
                symbolic: instruction.address.unwrap(),
                frame_alignment_bytes: 1,
                displacement: 2,
            },
        );
    }

    #[test]
    fn red_zone_resident_slots_resolve_below_the_unadjusted_stack_pointer() {
        let (mut function, mut frame, mut instruction) = fixture();
        let slot = LocalStorageSlotId::Spill {
            register: selected_instructions::VirtualRegisterId(7),
        };
        place_local_slot(&mut function, &mut frame, slot, (0, 8, 8));
        frame.functions[0].frame_size_bytes = 8;
        frame.functions[0].red_zone_resident_bytes = 8;
        frame.functions[0].return_address =
            machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes: 0,
                size_bytes: 8,
            };
        for (address, expected) in [
            (
                Address::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                -8,
            ),
            (
                Address::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 4,
                },
                -4,
            ),
            (
                Address::FrameAddress {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 8,
                },
                0,
            ),
        ] {
            instruction.address = Some(address);
            let resolved = resolve(&function, Some(&frame), &instruction)
                .unwrap()
                .unwrap();
            assert_eq!(resolved.displacement, expected);
            validate_address(&function, Some(&frame), &instruction, Some(resolved)).unwrap();
            for displacement in [expected - 8, expected + 8] {
                assert_rejected(
                    &function,
                    Some(&frame),
                    &instruction,
                    ResolvedPhysicalAddress {
                        displacement,
                        ..resolved
                    },
                );
            }
        }
        let incoming = Address::FrameAddress {
            slot: FrameStorageSlotId::Incoming {
                parameter_index: 8,
                abi_stack_byte_offset: 32,
            },
            byte_offset: 0,
        };
        instruction.address = Some(incoming);
        let resolved = resolve(&function, Some(&frame), &instruction)
            .unwrap()
            .unwrap();
        assert_eq!(resolved.displacement, 8 + 32);
        validate_address(&function, Some(&frame), &instruction, Some(resolved)).unwrap();
        let boundary = LocalStorageSlotId::Boundary {
            operation: OperationId::new(3).unwrap(),
        };
        place_local_slot(&mut function, &mut frame, boundary, (0, 1, 1));
        instruction.address = Some(Address::HostedWriteByteI32 { slot: boundary });
        assert!(resolve(&function, Some(&frame), &instruction).is_err());
        assert_rejected(
            &function,
            Some(&frame),
            &instruction,
            ResolvedPhysicalAddress {
                symbolic: instruction.address.unwrap(),
                frame_alignment_bytes: 1,
                displacement: -16,
            },
        );
        frame.functions[0].red_zone_resident_bytes = 24;
        assert!(resolve(&function, Some(&frame), &instruction).is_err());
    }

    #[test]
    fn compiler_spill_slot_addresses_bind_register_identity_and_exact_store_bounds() {
        let (mut function, mut frame, mut instruction) = fixture();
        let slot = LocalStorageSlotId::Spill {
            register: selected_instructions::VirtualRegisterId(7),
        };
        place_local_slot(&mut function, &mut frame, slot, (0, 8, 8));
        instruction.address = Some(Address::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        });
        assert!(resolve(&function, Some(&frame), &instruction).is_ok());
        instruction.address = Some(Address::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: 1,
        });
        assert!(resolve(&function, Some(&frame), &instruction).is_err());
        instruction.address = Some(Address::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        });
        let original_identity = machine_code::target_frame_layout_identity(&frame);
        frame.functions[0].local_storage_slots[0].id = LocalStorageSlotId::Spill {
            register: selected_instructions::VirtualRegisterId(8),
        };
        assert_ne!(
            machine_code::target_frame_layout_identity(&frame),
            original_identity
        );
        assert!(resolve(&function, Some(&frame), &instruction).is_err());
        frame.functions[0].local_storage_slots[0].id = slot;
        frame.functions[0].frame_size_bytes = 7;
        assert!(resolve(&function, Some(&frame), &instruction).is_err());
    }

    #[test]
    fn materialized_local_addresses_resolve_only_through_the_stable_address_loan_roster() {
        let (mut function, mut frame, mut instruction) = fixture();
        let slot = function.local_storage_slots[0].id;
        let resolved = resolve(&function, Some(&frame), &instruction).unwrap();
        validate_address(&function, Some(&frame), &instruction, resolved).unwrap();
        for mutation in 0..3 {
            let mut changed = frame.clone();
            match mutation {
                0 => changed.functions[0].stable_address_loans.clear(),
                1 => {
                    changed.functions[0].stable_address_loans = vec![LocalStorageSlotId::Boundary {
                        operation: OperationId::new(3).unwrap(),
                    }]
                }
                _ => {
                    changed.functions[0].stable_address_loans =
                        vec![LocalStorageSlotId::Structural {
                            operation: slot.operation().unwrap(),
                            place: PlaceId::new(7).unwrap(),
                        }]
                }
            }
            assert!(resolve(&function, Some(&changed), &instruction).is_err());
            assert!(validate_address(&function, Some(&changed), &instruction, resolved).is_err());
        }
        let spill = LocalStorageSlotId::Spill {
            register: selected_instructions::VirtualRegisterId(9),
        };
        function.local_storage_slots[0].id = spill;
        frame.functions[0].local_storage_slots[0].id = spill;
        instruction.address = Some(Address::FrameAddress {
            slot: FrameStorageSlotId::Local(spill),
            byte_offset: 0,
        });
        let resolved = resolve(&function, Some(&frame), &instruction).unwrap();
        validate_address(&function, Some(&frame), &instruction, resolved).unwrap();
        frame.functions[0].stable_address_loans = vec![spill];
        assert!(resolve(&function, Some(&frame), &instruction).is_err());
        assert!(validate_address(&function, Some(&frame), &instruction, resolved).is_err());
    }
}

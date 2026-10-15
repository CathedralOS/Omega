//! Rejoin a published descriptor residence to current selected and frame custody.

use super::{Error, source};
use calling_conventions::ValueShape;
use machine_code::StructuralSourceLocation;
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedFunction, SelectedInstructionKind,
    SelectedMemoryAccessRole, VirtualRegisterOrigin,
};
use semantic_vocabulary::{BlockId, OperationId, PlaceId};
use target_operations::TargetStructuralArgument;
use terminal_psi::{ByteSequenceCarrier, StructuralAccess, StructuralTypeShape};

/// This checks the publication relationship, not instruction realization or CFG
/// dominance. The enclosing admission validates the complete retained pipeline.
pub(super) fn location(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    selected: &SelectedFunction,
    argument: &TargetStructuralArgument,
    producer: OperationId,
) -> Result<StructuralSourceLocation, Error> {
    let invalid = || Error::Mismatch("established view differs from current descriptor custody");
    let (abstracted, _) = source::function(source, selected.machine)?;
    let mut producers = abstracted.operations.iter().filter(|operation| {
        matches!(operation,
        abstract_operations::element_view_or_byte_subslice_pattern!(psi_operation, ..)
        | abstract_operations::AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. }
        | abstract_operations::AbstractOperation::EstablishElementView { psi_operation, .. }
        if *psi_operation == producer)
    });
    match producers.next() {
        // A whole-view forward's argument type is the view type; an element
        // loan's root type is. `root_structural_type` covers both, and a
        // mutable view's affine result supplies the same descriptor residence.
        Some(abstract_operations::AbstractOperation::ByteSequenceSubslice { result, .. })
            if result.place == argument.place
                && result.structural_type == argument.root_structural_type
                && matches!(
                    result.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                )
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty() => {}
        Some(abstract_operations::AbstractOperation::ElementViewSubslice { result, .. })
            if result.place == argument.place
                && result.structural_type == argument.root_structural_type
                && matches!(
                    result.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                )
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty() => {}
        Some(abstract_operations::AbstractOperation::EstablishElementView { result, .. })
            if result.place == argument.place
                && result.structural_type == argument.root_structural_type
                && matches!(
                    result.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                )
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty() => {}
        // A literal's octets live in read-only image storage; its activation
        // holds only the two-word descriptor.
        Some(abstract_operations::AbstractOperation::EstablishByteSequenceLiteral {
            place,
            structural_type,
            ..
        }) if place.id == argument.place && structural_type.id == argument.structural_type => {}
        _ => return Err(invalid()),
    };
    if producers.next().is_some() {
        return Err(invalid());
    }
    local_location(
        source,
        selected,
        argument,
        LocalStorageSlotId::Structural {
            operation: producer,
            place: argument.place,
        },
        selected_instructions::SelectedMemoryAccessOrigin::Operation(producer),
    )
}

pub(super) fn block_location(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    selected: &SelectedFunction,
    argument: &TargetStructuralArgument,
    block: BlockId,
    place: PlaceId,
) -> Result<StructuralSourceLocation, Error> {
    let invalid = || Error::Mismatch("block view differs from current descriptor custody");
    let (abstracted, _) = source::function(source, selected.machine)?;
    let mut declarations = abstracted
        .block_entries
        .iter()
        .filter(|entry| entry.block == block)
        .flat_map(|entry| &entry.structural_parameters)
        .filter(|parameter| parameter.place == place);
    let parameter = declarations.next().ok_or_else(invalid)?;
    // The arrival is the argument's root: a whole forward or an element loan
    // (`local_location` checks which).
    if declarations.next().is_some()
        || place != argument.place
        || parameter.structural_type != argument.root_structural_type
        || !matches!(
            (parameter.access, argument.access),
            (
                StructuralAccess::SharedBorrow,
                StructuralAccess::SharedBorrow
            ) | (
                StructuralAccess::MutableBorrow,terminal_psi::reading_borrow_pair_pattern!())
        )
        || !selected.structural.as_ref().is_some_and(|signature| {
            terminal_semantics::view_descriptor_parameter(parameter, signature.structural_types.iter())
        })
        // A shared loan's qualifications describe the bytes it freezes and
        // add no carrier; an exclusive qualified loan keeps the refusal.
        || !(parameter.qualifications.is_empty()
            || parameter.access == StructuralAccess::SharedBorrow)
    {
        return Err(invalid());
    }
    local_location(
        source,
        selected,
        argument,
        LocalStorageSlotId::StructuralBlockParameter { block, place },
        selected_instructions::SelectedMemoryAccessOrigin::Block(block),
    )
}

fn local_location(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    selected: &SelectedFunction,
    argument: &TargetStructuralArgument,
    slot: LocalStorageSlotId,
    origin: selected_instructions::SelectedMemoryAccessOrigin,
) -> Result<StructuralSourceLocation, Error> {
    let invalid = || Error::Mismatch("local view differs from current descriptor custody");
    let declarations = &selected
        .structural
        .as_ref()
        .ok_or_else(invalid)?
        .structural_types;
    let declaration =
        terminal_psi::find_structural_type(declarations, argument.root_structural_type)
            .ok_or_else(invalid)?;
    // Residence follows the retained carrier, not its source spelling or loan
    // access. Dynamic descriptors store a length; fixed byte views retain one
    // backing pointer and obtain their extent from the declaration.
    let byte_size = match declaration.shape {
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        | StructuralTypeShape::ElementView { .. } => 16,
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView { .. }) => 8,
        _ => return Err(invalid()),
    };
    // A whole forward ships the descriptor as the argument; an element loan
    // ships the projected element's address, so its access, projection and
    // shape differ — the descriptor residence it checks is identical. Shared
    // and mutable whole views use the same declared home. The admitted pipeline
    // already checked the exact source authority; publication rejoins physical
    // residence without converting the borrowed descriptor into an owned value.
    let element_loan = !argument.path.is_empty();
    if element_loan {
        if argument.access == StructuralAccess::Owned
            || !argument.path.iter().all(|segment| {
                matches!(
                    segment,
                    terminal_psi::StructuralPathSegment::Field(_)
                        | terminal_psi::StructuralPathSegment::FixedIndex(_)
                        | terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
                )
            })
            || argument.shape.alignment == 0
            || argument.fixed_array_length.is_some()
            || argument.element_stride.is_some()
        {
            return Err(invalid());
        }
    } else if !matches!(
        argument.access,
        terminal_psi::reading_borrow_pair_pattern!()
    ) || !matches!(
        terminal_semantics::borrowed_referent_presentation(
            declarations.iter(),
            argument.root_structural_type,
            argument.structural_type,
        ),
        Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
    ) || argument.shape != ValueShape::borrowed_reference(byte_size, 8)
        || argument.source_byte_offset != 0
        || argument.fixed_array_length.is_some()
        || argument.element_stride.is_some()
    {
        return Err(invalid());
    }
    let mut selected_slots = selected
        .local_storage_slots
        .iter()
        .filter(|candidate| candidate.id == slot);
    let selected_slot = selected_slots.next().ok_or_else(invalid)?;
    if selected_slots.next().is_some()
        || selected_slot.byte_size != byte_size
        || selected_slot.alignment != 8
    {
        return Err(invalid());
    }
    let frame = source::frame(source, selected.machine)?.ok_or_else(invalid)?;
    let mut slots = frame
        .local_storage_slots
        .iter()
        .filter(|candidate| candidate.id == slot);
    let local = slots.next().ok_or_else(invalid)?;
    // The frame-space offset is published as a stack location: only a
    // committed frame can carry the structural storage this view describes.
    if frame.red_zone_resident_bytes != 0
        || slots.next().is_some()
        || local.size_bytes != byte_size
        || local.alignment_bytes != 8
        || local.frame_offset_bytes % 8 != 0
        || local
            .frame_offset_bytes
            .checked_add(u64::from(byte_size))
            .is_none_or(|end| end > frame.frame_size_bytes)
    {
        return Err(invalid());
    }
    let mut accesses = selected.memory_accesses.iter().filter(|access| {
        access.origin == origin
            && access.place == argument.place
            && access.role == (SelectedMemoryAccessRole::AddressLocal { slot })
            && access.byte_offset == 0
    });
    let access = accesses.next().ok_or_else(invalid)?;
    if accesses.next().is_some() || access.byte_offset != 0 || access.byte_count != byte_size {
        return Err(invalid());
    }
    let mut instructions = selected
        .instructions()
        .filter(|instruction| instruction.id == access.instruction);
    let instruction = instructions.next().ok_or_else(invalid)?;
    if instructions.next().is_some()
        || instruction.kind
            != (SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            })
        || match origin {
            selected_instructions::SelectedMemoryAccessOrigin::Operation(producer) => {
                instruction.provenance.operations.last() != Some(&producer)
            }
            selected_instructions::SelectedMemoryAccessOrigin::Block(owner) => {
                // Edge copies initialize this descriptor independently. Its pure
                // address is formed in the owning block, keeping the pointer
                // lifetime out of earlier calls rather than hoisting it to entry.
                instruction.provenance != Default::default()
                    || !selected.blocks.iter().any(|block| {
                        block.origin == selected_instructions::SelectedBlockOrigin::Source(owner)
                            && block.instruction(instruction.id).is_some()
                    })
            }
            selected_instructions::SelectedMemoryAccessOrigin::Edge(_) => true,
        }
        || instruction.operands.len() != 1
    {
        return Err(invalid());
    }
    let pointer = instruction.operands[0].virtual_register;
    let mut registers = selected
        .virtual_registers
        .iter()
        .filter(|register| register.id == pointer);
    let register = registers.next().ok_or_else(invalid)?;
    if registers.next().is_some()
        || register.origin
            != (VirtualRegisterOrigin::AbiTransport {
                instruction: instruction.id,
                place: argument.place,
                byte_offset: 0,
            })
    {
        return Err(invalid());
    }
    Ok(StructuralSourceLocation::Stack {
        byte_offset: u32::try_from(local.frame_offset_bytes).map_err(|_| Error::Overflow)?,
    })
}

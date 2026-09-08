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
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralTypeShape,
};

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
    let mut producers = abstracted.operations.iter().filter(|operation| matches!(operation,
        abstract_operations::AbstractOperation::ByteSequenceSubslice { psi_operation, .. } if *psi_operation == producer));
    let Some(abstract_operations::AbstractOperation::ByteSequenceSubslice { result, .. }) =
        producers.next()
    else {
        return Err(invalid());
    };
    if producers.next().is_some()
        || result.place != argument.place
        || result.structural_type != argument.structural_type
        || result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
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
    if declarations.next().is_some()
        || place != argument.place
        || parameter.structural_type != argument.structural_type
        || parameter.access != StructuralAccess::SharedBorrow
        || parameter.multiplicity != StructuralMultiplicity::Unrestricted
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
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
    if argument.access != StructuralAccess::SharedBorrow
        || !argument.path.is_empty()
        || argument.root_structural_type != argument.structural_type
        || argument.shape != ValueShape::borrowed_reference(16, 8)
        || argument.source_byte_offset != 0
        || argument.fixed_array_length.is_some()
        || argument.element_stride.is_some()
    {
        return Err(invalid());
    }
    let declarations = &selected
        .structural
        .as_ref()
        .ok_or_else(invalid)?
        .structural_types;
    if !declarations.iter().any(|declaration| {
        declaration.id == argument.structural_type
            && declaration.shape
                == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
    }) {
        return Err(invalid());
    }
    let mut selected_slots = selected
        .local_storage_slots
        .iter()
        .filter(|candidate| candidate.id == slot);
    let selected_slot = selected_slots.next().ok_or_else(invalid)?;
    if selected_slots.next().is_some()
        || selected_slot.byte_size != 16
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
    if slots.next().is_some()
        || local.size_bytes != 16
        || local.alignment_bytes != 8
        || local.frame_offset_bytes % 8 != 0
        || local
            .frame_offset_bytes
            .checked_add(16)
            .is_none_or(|end| end > frame.frame_size_bytes)
    {
        return Err(invalid());
    }
    let mut accesses = selected.memory_accesses.iter().filter(|access| {
        access.origin == origin
            && access.place == argument.place
            && access.role == (SelectedMemoryAccessRole::AddressLocal { slot })
    });
    let access = accesses.next().ok_or_else(invalid)?;
    if accesses.next().is_some() || access.byte_offset != 0 || access.byte_count != 16 {
        return Err(invalid());
    }
    let mut instructions = selected
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
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
                instruction.provenance.operations != [producer]
            }
            selected_instructions::SelectedMemoryAccessOrigin::Block(_) => {
                instruction.provenance != Default::default()
                    || !selected.blocks.iter().any(|block| {
                        block.id == selected.entry_block
                            && block
                                .instructions
                                .iter()
                                .any(|entry| entry.id == instruction.id)
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

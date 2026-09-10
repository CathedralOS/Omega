//! Transport an owned array through its exact ABI fragments.
use super::*;
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedMemoryAccessRole};

pub(super) fn argument(
    source: &LegalizedScalarFunction,
    operation: &legalized_operations::LegalizedScalarInstruction,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    builder: &mut Builder<'_>,
) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let place = semantic.place;
    let slot = match target.source {
        target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
            Some(LocalStorageSlotId::Structural {
                operation: psi_operation,
                place,
            })
        }
        target_operations::TargetStructuralArgumentSource::Placement(_) => None,
        _ => return Err(invalid()),
    };
    let block = source
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|row| row.operation == operation.operation)
        })
        .ok_or_else(invalid)?
        .id;
    let pointer = if let Some(slot) = slot {
        if builder
            .transport
            .local_slots
            .iter()
            .filter(|home| {
                home.id == slot
                    && home.byte_size == u32::from(target.shape.byte_size)
                    && home.alignment == target.shape.alignment
            })
            .count()
            != 1
        {
            return Err(invalid());
        }
        let pointer = super::structural_case::register(builder, place, 0, 64, false)?;
        super::structural_case::memory(
            builder,
            block,
            place,
            0,
            u32::from(target.shape.byte_size),
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            builder.constraints.keys.frame_address.ok_or_else(invalid)?,
            &[pointer],
            Default::default(),
        )?;
        Some(pointer)
    } else {
        None
    };
    let mut registers = Vec::new();
    for location in &target.destination.locations {
        let ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        } = location
        else {
            return Err(invalid());
        };
        let offset = u32::from(*value_byte_offset);
        let output = super::structural_case::register(builder, place, offset, 64, false)?;
        if let Some(pointer) = pointer {
            super::structural_case::memory(
                builder,
                block,
                place,
                offset,
                u32::from(*byte_size),
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            super::aggregate_memory::load(builder, pointer, output, offset, *byte_size)?;
        } else {
            let input = builder
                .transport
                .fragments
                .iter()
                .find(|(owner, position, _)| *owner == place && *position == offset)
                .map(|(_, _, register)| *register)
                .ok_or_else(invalid)?;
            builder.emit(
                SelectedInstructionKind::CopyI64,
                builder.constraints.keys.copy_i64,
                &[input, output],
                Default::default(),
            )?;
        }
        registers.push(output);
    }
    Ok(registers)
}

//! Establish the exact ordered array payload in one activation-local home.
use super::*;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let (established, shape, stores) =
        crate::selection::scalar_array_input::storage(source, row).ok_or_else(invalid)?;
    if shape.byte_size == 0 {
        // The semantic result remains in the exact source frontier. No address,
        // physical home or access exists; retain construction before the next
        // executable instruction, including the block's terminator.
        builder.pending_provenance.operations.push(row.operation);
        builder
            .pending_provenance
            .fuel
            .extend(row.fuel.iter().cloned());
        return Ok(());
    }
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: established.place,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: u32::from(shape.byte_size),
            alignment: shape.alignment,
        });
    // The address instruction charges construction once; payload stores carry
    // exact operand provenance without introducing additional semantic work.
    let pointer = local_storage::address(builder, row, slot, 0, u32::from(shape.byte_size), true)?;
    for (element, scalar, offset, width) in stores {
        let (_, value, _, actual) = builder.resolve(element).ok_or_else(invalid)?;
        if actual != scalar {
            return Err(invalid());
        }
        memory(
            builder,
            row,
            established.place,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit(
            SelectedInstructionKind::Store {
                byte_offset: offset,
                byte_size: width,
            },
            builder.constraints.keys.store.ok_or_else(invalid)?,
            &[pointer, value],
            SelectedInstructionProvenance {
                values: vec![element],
                ..provenance(row)
            },
        )?;
    }
    Ok(())
}

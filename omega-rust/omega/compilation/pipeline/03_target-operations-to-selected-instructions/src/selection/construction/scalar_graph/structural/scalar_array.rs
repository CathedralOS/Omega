//! Establish the exact ordered array payload in one activation-local home.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, SelectedInstructionProvenance,
    SelectedMemoryAccessRole, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::local_storage;
use crate::selection::construction::scalar_graph::structural::provenance;

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let (established, shape, stores) = crate::selection::scalar_array_input::storage(source, row)
        .ok_or_else(|| SelectedInstructionError::custody())?;
    // Empty arrays still lend an address, like empty records and zeroed values.
    // The anchor supplies physical custody without adding semantic elements;
    // the exact empty store sequence below performs no payload access.
    let slot = builder.transport.push_structural_local_slot(
        row.operation,
        established.place,
        shape.byte_size.max(1),
        shape.alignment,
    );
    // The address instruction charges construction once; payload stores carry
    // exact operand provenance without introducing additional semantic work.
    let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size.max(1), true)?;
    for (element, scalar, offset, width) in stores {
        let (_, value, _, actual) = builder
            .resolve(element)
            .ok_or_else(|| SelectedInstructionError::custody())?;
        if actual != scalar {
            return Err(SelectedInstructionError::custody());
        }
        let (element_pointer, displacement) = super::memory_pointer(
            builder,
            row,
            established.place,
            pointer,
            offset,
            u32::from(width),
        )?;
        memory(
            builder,
            row,
            established.place,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit_store(
            displacement,
            width,
            element_pointer,
            value,
            SelectedInstructionProvenance {
                values: vec![element],
                ..provenance(row)
            },
        )?;
    }
    // Publish the whole home to edge transport only after every payload store.
    builder
        .transport
        .pointers
        .push((established.place, pointer));
    Ok(())
}

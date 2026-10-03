//! Assemble completed array elements into one unpublished owned home.
use super::Builder;
use super::record;
use super::{LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind};
use crate::SelectedInstructionError;

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let (element_shape, offsets) = crate::selection::structural_array_input::elements(source, row)
        .ok_or_else(super::invalid)?;
    let LegalizedScalarInstructionKind::EstablishStructuralArray {
        result,
        elements,
        shape,
    } = &row.kind
    else {
        return Err(super::invalid());
    };
    // One charged allocation initializes inter-element padding before any copy.
    let pointer = record::initialize_storage(row, builder, result.place, *shape)?;
    for (argument, offset) in elements.iter().zip(offsets) {
        record::write_structural_operand(
            source,
            row,
            builder,
            result.place,
            pointer,
            argument,
            offset,
            element_shape,
        )?;
    }
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}

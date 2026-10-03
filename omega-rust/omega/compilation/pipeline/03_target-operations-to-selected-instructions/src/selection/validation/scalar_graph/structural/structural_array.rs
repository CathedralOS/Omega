//! Assemble completed array elements into one unpublished owned home.
use super::record;
use super::{LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let (element_shape, offsets) = crate::selection::structural_array_input::elements(source, row)
        .ok_or_else(SelectedInstructionError::custody)?;
    let LegalizedScalarInstructionKind::EstablishStructuralArray {
        result,
        elements,
        shape,
    } = &row.kind
    else {
        return Err(SelectedInstructionError::custody());
    };
    // One charged allocation initializes inter-element padding before any copy.
    let pointer = record::initialize_storage(row, replay, result.place, *shape)?;
    for (argument, offset) in elements.iter().zip(offsets) {
        record::write_structural_operand(
            source,
            row,
            replay,
            result.place,
            pointer,
            argument,
            offset,
            element_shape,
        )?;
    }
    replay.transport.pointers.push((result.place, pointer));
    Ok(())
}

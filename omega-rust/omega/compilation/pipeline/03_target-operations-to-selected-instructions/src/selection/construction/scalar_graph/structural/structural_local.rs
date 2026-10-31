//! Establish a memberwise structural local's home: the slot begins stale
//! (zeroed for a defined starting byte pattern) and the member stores that
//! complete it arrive as their own operations.
use super::{Builder, LegalizedScalarFunction, LegalizedScalarInstruction};
use crate::SelectedInstructionError;

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let (established, shape) = crate::selection::structural_local_input::storage(source, row)
        .ok_or(SelectedInstructionError::custody())?;
    // Own admission remains above. Complete constructors and memberwise
    // locals share only the unpublished home's allocation and byte fill.
    let pointer = super::record::initialize_storage(row, builder, established.place, shape)?;
    builder
        .transport
        .pointers
        .push((established.place, pointer));
    Ok(())
}

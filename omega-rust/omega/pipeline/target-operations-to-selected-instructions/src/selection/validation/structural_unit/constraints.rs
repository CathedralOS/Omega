//! Independent reconstruction of the structural-Unit call constraint row.

use crate::selection::constraints::row;
use crate::selection::shared::*;

pub(super) fn reconstruct_structural_call_row(
    function: usize,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<&RegisterInstructionConstraint, SelectedInstructionError> {
    let key = keys
        .structural_unit_call
        .ok_or(SelectedInstructionError::UnsupportedSourceShape { function })?;
    let reconstructed = row(catalog, key)?;
    if !reconstructed.operands.is_empty() {
        return Err(SelectedInstructionError::MissingConstraint(key));
    }
    Ok(reconstructed)
}

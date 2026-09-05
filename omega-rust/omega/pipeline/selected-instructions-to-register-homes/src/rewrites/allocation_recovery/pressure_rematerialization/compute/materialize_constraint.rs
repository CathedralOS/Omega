use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    ValidatedRegisterConstraintCatalog,
};

use crate::PressureRematerializationError;

pub(super) fn select(
    constraints: &ValidatedRegisterConstraintCatalog,
    keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<&RegisterInstructionConstraint, PressureRematerializationError> {
    let row = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == keys.materialize_i64)
        .ok_or(PressureRematerializationError::MaterializeConstraintMismatch)?;
    let [result] = row.operands.as_slice() else {
        return Err(PressureRematerializationError::MaterializeConstraintMismatch);
    };
    if result.operand != 0
        || result.access != RegisterOperandAccess::Def
        || result.fixed_view.is_some()
        || result.tied_to.is_some()
        || result.early_clobber
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(PressureRematerializationError::MaterializeConstraintMismatch);
    }
    Ok(row)
}

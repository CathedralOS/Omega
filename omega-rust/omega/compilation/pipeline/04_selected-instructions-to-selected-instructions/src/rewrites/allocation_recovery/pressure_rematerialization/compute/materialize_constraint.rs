use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    ValidatedRegisterConstraintCatalog,
};

use crate::PressureRematerializationError;

pub(super) fn select<'a>(
    constraints: &'a ValidatedRegisterConstraintCatalog,
    keys: &TargetRegisterEnvironmentConstraintKeys,
) -> Result<&'a RegisterInstructionConstraint, PressureRematerializationError> {
    let row = register_model::find_register_constraint(
        &constraints.catalog().constraints,
        keys.materialize_i64,
    )
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

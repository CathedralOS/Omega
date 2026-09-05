use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    ValidatedRegisterConstraintCatalog,
};

use crate::FixedViewCopyError;

pub(super) fn validated_copy_row(
    constraints: &ValidatedRegisterConstraintCatalog,
    keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<&RegisterInstructionConstraint, FixedViewCopyError> {
    let Some(row) = constraints
        .catalog()
        .constraints
        .iter()
        .find(|candidate| candidate.key == keys.copy_i64)
    else {
        return Err(FixedViewCopyError::CopyConstraintMismatch);
    };
    let operand_shape = row.operands.as_slice();
    let [source, result] = operand_shape else {
        return Err(FixedViewCopyError::CopyConstraintMismatch);
    };
    if source.operand != 0
        || source.access != RegisterOperandAccess::Use
        || result.operand != 1
        || result.access != RegisterOperandAccess::Def
        || source.class != result.class
        || [source, result].iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(FixedViewCopyError::CopyConstraintMismatch);
    }
    Ok(row)
}

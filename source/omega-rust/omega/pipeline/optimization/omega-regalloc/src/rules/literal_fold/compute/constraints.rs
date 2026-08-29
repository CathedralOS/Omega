//! Producer selection of admitted immediate-form constraints.

use omega_register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    ValidatedRegisterConstraintCatalog,
};

use crate::{LiteralFoldError, LiteralFoldPolicy};

pub(super) struct ImmediateRows<'a> {
    pub(super) add: Option<&'a RegisterInstructionConstraint>,
    pub(super) subtract: Option<&'a RegisterInstructionConstraint>,
}

pub(super) fn select_immediate_rows(
    constraints: &ValidatedRegisterConstraintCatalog,
    keys: TargetRegisterEnvironmentConstraintKeys,
    policy: LiteralFoldPolicy,
) -> Result<ImmediateRows<'_>, LiteralFoldError> {
    let find = |key| {
        constraints
            .catalog()
            .constraints
            .iter()
            .find(|row| row.key == key)
            .ok_or(LiteralFoldError::ImmediateConstraintMismatch)
    };
    let add = policy
        .enables_exact_add()
        .then(|| find(keys.add_i64_immediate))
        .transpose()?;
    let subtract = policy
        .enables_exact_subtract()
        .then(|| find(keys.subtract_i64_immediate))
        .transpose()?;
    for row in [add, subtract].into_iter().flatten() {
        validate_immediate_row(row)?;
    }
    Ok(ImmediateRows { add, subtract })
}

fn validate_immediate_row(row: &RegisterInstructionConstraint) -> Result<(), LiteralFoldError> {
    let [left, result] = row.operands.as_slice() else {
        return Err(LiteralFoldError::ImmediateConstraintMismatch);
    };
    if left.operand != 0
        || left.access != RegisterOperandAccess::Use
        || result.operand != 1
        || result.access != RegisterOperandAccess::Def
        || left.class != result.class
        || [left, result].iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
    {
        return Err(LiteralFoldError::ImmediateConstraintMismatch);
    }
    Ok(())
}

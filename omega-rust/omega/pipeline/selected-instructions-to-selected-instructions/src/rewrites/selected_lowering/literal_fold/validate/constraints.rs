use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, TargetRegisterEnvironmentConstraintKeys,
    ValidatedRegisterConstraintCatalog,
};

use crate::{LiteralFoldError, LiteralFoldPolicy};

pub(super) struct ValidationImmediateRows<'a> {
    pub(super) add: Option<&'a RegisterInstructionConstraint>,
    pub(super) subtract: Option<&'a RegisterInstructionConstraint>,
    pub(super) compare: Option<&'a RegisterInstructionConstraint>,
    /// The `MaterializeI64` row the unary extension folds rewrite into; bound
    /// only when the extension-elimination policy bit is selected.
    pub(super) materialize: Option<&'a RegisterInstructionConstraint>,
}

pub(super) fn reconstruct_immediate_rows<'a>(
    constraints: &'a ValidatedRegisterConstraintCatalog,
    keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: LiteralFoldPolicy,
) -> Result<ValidationImmediateRows<'a>, LiteralFoldError> {
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
    let compare = policy
        .enables_compare()
        .then(|| find(keys.compare_i64_immediate))
        .transpose()?;
    let materialize = policy
        .enables_extension()
        .then(|| find(keys.materialize_i64))
        .transpose()?;
    for row in [add, subtract, compare, materialize].into_iter().flatten() {
        validate_immediate_row(row)?;
    }
    Ok(ValidationImmediateRows {
        add,
        subtract,
        compare,
        materialize,
    })
}

fn validate_immediate_row(row: &RegisterInstructionConstraint) -> Result<(), LiteralFoldError> {
    match row.operands.as_slice() {
        [left, result] => {
            if left.operand != 0
                || left.access != RegisterOperandAccess::Use
                || result.operand != 1
                || result.access != RegisterOperandAccess::Def
                || left.class != result.class
                || [left, result].iter().any(|operand| {
                    operand.fixed_view.is_some()
                        || operand.tied_to.is_some()
                        || operand.early_clobber
                })
                || !row.implicit_uses.is_empty()
                || !row.implicit_defs.is_empty()
                || !row.clobbers.is_empty()
            {
                return Err(LiteralFoldError::ImmediateConstraintMismatch);
            }
        }
        // Flag-defining immediate rows admit one `Use` operand; the target
        // condition state must remain an explicit implicit definition.
        [left] if left.access == RegisterOperandAccess::Use => {
            if left.operand != 0
                || left.fixed_view.is_some()
                || left.tied_to.is_some()
                || left.early_clobber
                || !row.implicit_uses.is_empty()
                || row.implicit_defs.is_empty()
                || !row.clobbers.is_empty()
            {
                return Err(LiteralFoldError::ImmediateConstraintMismatch);
            }
        }
        // Constant-materialization rows carry one `Def` operand and no
        // register input; no implicit unit traffic may hide in the row.
        [result] if result.access == RegisterOperandAccess::Def => {
            if result.operand != 0
                || result.fixed_view.is_some()
                || result.tied_to.is_some()
                || result.early_clobber
                || !row.implicit_uses.is_empty()
                || !row.implicit_defs.is_empty()
                || !row.clobbers.is_empty()
            {
                return Err(LiteralFoldError::ImmediateConstraintMismatch);
            }
        }
        _ => return Err(LiteralFoldError::ImmediateConstraintMismatch),
    }
    Ok(())
}

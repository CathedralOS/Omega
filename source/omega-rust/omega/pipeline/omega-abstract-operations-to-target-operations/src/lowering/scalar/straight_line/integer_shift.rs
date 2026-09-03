//! Exact and wrapping shifts in straight-line scalar lowering.

use std::collections::BTreeMap;

use super::{
    AbstractOperation, KnownScalar, LoweringError, TerminalPsiProvenance, ValueId,
    WrappingShiftKind, insert_value, lower_exact_shift_left, lower_exact_shift_right,
    lower_wrapping_shift,
};

pub(super) fn lower(
    operation: &AbstractOperation,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let (psi_operation, result, value_type, shifted) = match operation {
        AbstractOperation::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        }
        | AbstractOperation::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => {
            let kind = if matches!(
                operation,
                AbstractOperation::WrappingIntegerShiftLeft { .. }
            ) {
                WrappingShiftKind::Left
            } else {
                WrappingShiftKind::Right
            };
            (
                *psi_operation,
                *result,
                *value_type,
                lower_wrapping_shift(
                    values,
                    *result,
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                    kind,
                    *psi_operation,
                )?,
            )
        }
        AbstractOperation::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            *psi_operation,
            *result,
            *value_type,
            lower_exact_shift_right(
                values,
                *result,
                *value_type,
                *count_type,
                *value,
                *count,
                *psi_operation,
                *obligation,
            )?,
        ),
        AbstractOperation::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            *psi_operation,
            *result,
            *value_type,
            lower_exact_shift_left(
                values,
                *result,
                *value_type,
                *count_type,
                *value,
                *count,
                *psi_operation,
                *obligation,
            )?,
        ),
        _ => unreachable!("integer-shift routing admits only its declared operations"),
    };
    insert_value(
        values,
        result,
        KnownScalar::Integer {
            scalar_type: value_type,
            value: shifted,
        },
    )?;
    provenance.operations.push(psi_operation);
    Ok(())
}

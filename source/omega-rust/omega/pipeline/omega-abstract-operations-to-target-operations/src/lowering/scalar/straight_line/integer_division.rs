//! Division and remainder operations in straight-line scalar lowering.

use std::collections::BTreeMap;

use super::{
    AbstractOperation, IntegerBinaryKind, KnownScalar, LoweringError, TerminalPsiProvenance,
    ValueId, insert_value, lower_conditional_integer_binary,
};

pub(super) fn lower(
    operation: &AbstractOperation,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let (psi_operation, result, scalar_type, left, right, kind) = match operation {
        AbstractOperation::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactDivide(*obligation),
        ),
        AbstractOperation::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::ExactRemainder(*obligation),
        ),
        AbstractOperation::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingDivide(*obligation),
        ),
        AbstractOperation::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::WrappingRemainder(*obligation),
        ),
        AbstractOperation::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingDivide(*obligation),
        ),
        AbstractOperation::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            IntegerBinaryKind::SaturatingRemainder(*obligation),
        ),
        _ => unreachable!("integer-division routing admits only its declared operations"),
    };
    let value = lower_conditional_integer_binary(
        values,
        result,
        scalar_type,
        left,
        right,
        kind,
        psi_operation,
    )?;
    insert_value(values, result, KnownScalar::Integer { scalar_type, value })?;
    provenance.operations.push(psi_operation);
    Ok(())
}

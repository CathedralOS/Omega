//! Binary bitwise operations in straight-line scalar lowering.

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
        AbstractOperation::IntegerBitwiseAnd {
            psi_operation,
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
            IntegerBinaryKind::BitwiseAnd,
        ),
        AbstractOperation::IntegerBitwiseOr {
            psi_operation,
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
            IntegerBinaryKind::BitwiseOr,
        ),
        AbstractOperation::IntegerBitwiseXor {
            psi_operation,
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
            IntegerBinaryKind::BitwiseXor,
        ),
        _ => unreachable!("integer-bitwise routing admits only its declared operations"),
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

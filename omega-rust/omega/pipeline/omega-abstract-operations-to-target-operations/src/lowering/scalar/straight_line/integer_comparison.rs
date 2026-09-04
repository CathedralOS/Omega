//! Integer equality and ordering operations in straight-line scalar lowering.

use std::collections::BTreeMap;

use super::{
    AbstractOperation, KnownScalar, LoweringError, TerminalPsiProvenance, ValueId, equal_integer,
    insert_value, order_integer,
};

pub(super) fn lower(
    operation: &AbstractOperation,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let (psi_operation, result, left, right) = match operation {
        AbstractOperation::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        }
        | AbstractOperation::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        }
        | AbstractOperation::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => (*psi_operation, *result, *left, *right),
        _ => unreachable!("integer-comparison routing admits only its declared operations"),
    };
    let left_value = values
        .get(&left)
        .cloned()
        .ok_or(LoweringError::UnknownValue(left))?;
    let right_value = values
        .get(&right)
        .cloned()
        .ok_or(LoweringError::UnknownValue(right))?;
    let value = match operation {
        AbstractOperation::IntegerEqual { .. } => {
            equal_integer(left, left_value, right, right_value, psi_operation, result)?
        }
        AbstractOperation::IntegerLessThan { .. }
        | AbstractOperation::IntegerLessOrEqual { .. } => order_integer(
            left,
            left_value,
            right,
            right_value,
            psi_operation,
            result,
            matches!(operation, AbstractOperation::IntegerLessOrEqual { .. }),
        )?,
        _ => unreachable!("integer-comparison routing admits only its declared operations"),
    };
    insert_value(values, result, value)?;
    provenance.operations.push(psi_operation);
    Ok(())
}

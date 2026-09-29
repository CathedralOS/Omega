//! Exact and wrapping integer-shift semantics.
use super::{KnownInteger, KnownScalar};
use crate::LoweringError;
use abstract_operations::{IntegerShiftDirection, IntegerShiftPolicy};
use semantic_vocabulary::{IntegerType, ValueId};
use std::collections::BTreeMap;
use target_operations::TargetIntegerExpression;

/// Lower one shift. A wrapping shift of two immediates folds; an exact shift
/// always stays a runtime expression so its in-range count obligation keeps
/// custody.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering::scalar) fn lower_shift(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value_id: ValueId,
    count_id: ValueId,
    policy: IntegerShiftPolicy,
    direction: IntegerShiftDirection,
    psi_operation: semantic_vocabulary::OperationId,
) -> Result<KnownInteger, LoweringError> {
    let mismatch = match policy {
        IntegerShiftPolicy::Wrapping => LoweringError::WrappingShiftOperandTypeMismatch(result),
        IntegerShiftPolicy::Exact { .. } => LoweringError::ExactShiftOperandTypeMismatch(result),
    };
    let operand = |id, expected_type| match values.get(&id).cloned() {
        Some(KnownScalar::Integer { scalar_type, value }) if scalar_type == expected_type => {
            Ok(value)
        }
        Some(_) => Err(mismatch.clone()),
        None => Err(LoweringError::UnknownValue(id)),
    };
    let value = operand(value_id, value_type)?;
    let count = operand(count_id, count_type)?;
    if policy == IntegerShiftPolicy::Wrapping
        && let (KnownInteger::Immediate(value_constant), KnownInteger::Immediate(count_constant)) =
            (&value, &count)
    {
        let shifted = policy
            .evaluate(
                direction,
                value_type,
                *value_constant,
                count_type,
                *count_constant,
            )
            .ok_or(mismatch)?;
        return Ok(KnownInteger::Immediate(shifted));
    }
    Ok(KnownInteger::Runtime(TargetIntegerExpression::Shift {
        psi_operation,
        policy,
        direction,
        count_type,
        value: Box::new(value.into_expression(value_id)),
        count: Box::new(count.into_expression(count_id)),
    }))
}

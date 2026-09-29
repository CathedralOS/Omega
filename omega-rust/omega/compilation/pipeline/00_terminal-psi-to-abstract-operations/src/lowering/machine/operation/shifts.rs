use std::collections::BTreeMap;

use abstract_operations::{AbstractOperation, IntegerShiftDirection, IntegerShiftPolicy};
use semantic_vocabulary::ScalarType;
use terminal_psi::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
) -> Result<AbstractOperation, LoweringError> {
    let (policy, direction, value, count) = match operation.kind.clone() {
        OperationKind::WrappingIntegerShiftLeft { value, count } => (
            IntegerShiftPolicy::Wrapping,
            IntegerShiftDirection::Left,
            value,
            count,
        ),
        OperationKind::WrappingIntegerShiftRight { value, count } => (
            IntegerShiftPolicy::Wrapping,
            IntegerShiftDirection::Right,
            value,
            count,
        ),
        OperationKind::ExactIntegerShiftLeft {
            value,
            count,
            obligation,
        } => (
            IntegerShiftPolicy::Exact { obligation },
            IntegerShiftDirection::Left,
            value,
            count,
        ),
        OperationKind::ExactIntegerShiftRight {
            value,
            count,
            obligation,
        } => (
            IntegerShiftPolicy::Exact { obligation },
            IntegerShiftDirection::Right,
            value,
            count,
        ),
        _ => unreachable!("shift router is exhaustive"),
    };
    let malformed = match policy {
        IntegerShiftPolicy::Wrapping => LoweringError::VerifiedWrappingShiftMalformed(operation.id),
        IntegerShiftPolicy::Exact { .. } => {
            LoweringError::VerifiedExactShiftMalformed(operation.id)
        }
    };
    let ScalarType::Integer(value_type) = operation.result.expect_scalar().scalar_type else {
        return Err(malformed);
    };
    let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied() else {
        return Err(malformed);
    };
    Ok(AbstractOperation::IntegerShift {
        psi_operation: operation.id,
        result: operation.result.expect_scalar().id,
        value_type,
        count_type,
        policy,
        direction,
        value,
        count,
    })
}

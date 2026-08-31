use std::collections::BTreeMap;

use omega_abstract_operations::AbstractOperation;
use psi_core::ScalarType;
use psi_terminal::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    value_types: &BTreeMap<psi_core::ValueId, ScalarType>,
) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::WrappingIntegerShiftLeft { value, count }
        | OperationKind::WrappingIntegerShiftRight { value, count } => {
            let ScalarType::Integer(value_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedWrappingShiftMalformed(operation.id));
            };
            let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied() else {
                return Err(LoweringError::VerifiedWrappingShiftMalformed(operation.id));
            };
            match operation.kind.clone() {
                OperationKind::WrappingIntegerShiftLeft { .. } => {
                    AbstractOperation::WrappingIntegerShiftLeft {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        value_type,
                        count_type,
                        value,
                        count,
                    }
                }
                OperationKind::WrappingIntegerShiftRight { .. } => {
                    AbstractOperation::WrappingIntegerShiftRight {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        value_type,
                        count_type,
                        value,
                        count,
                    }
                }
                _ => unreachable!(),
            }
        }
        OperationKind::ExactIntegerShiftRight {
            value,
            count,
            obligation,
        } => {
            let ScalarType::Integer(value_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
            };
            let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied() else {
                return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
            };
            AbstractOperation::ExactIntegerShiftRight {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                value_type,
                count_type,
                value,
                count,
            }
        }
        OperationKind::ExactIntegerShiftLeft {
            value,
            count,
            obligation,
        } => {
            let ScalarType::Integer(value_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
            };
            let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied() else {
                return Err(LoweringError::VerifiedExactShiftMalformed(operation.id));
            };
            AbstractOperation::ExactIntegerShiftLeft {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                value_type,
                count_type,
                value,
                count,
            }
        }
        _ => unreachable!("shift router is exhaustive"),
    })
}

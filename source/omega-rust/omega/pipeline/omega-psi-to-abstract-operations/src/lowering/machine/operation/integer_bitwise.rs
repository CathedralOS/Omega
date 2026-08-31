use omega_abstract_operations::AbstractOperation;
use psi_core::ScalarType;
use psi_terminal::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(operation: &Operation) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::IntegerBitwiseNot { operand } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedIntegerBitwiseMalformed(operation.id));
            };
            AbstractOperation::IntegerBitwiseNot {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                scalar_type,
                operand,
            }
        }
        OperationKind::IntegerBitwiseAnd { left, right }
        | OperationKind::IntegerBitwiseOr { left, right }
        | OperationKind::IntegerBitwiseXor { left, right } => {
            let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedIntegerBitwiseMalformed(operation.id));
            };
            match operation.kind.clone() {
                OperationKind::IntegerBitwiseAnd { .. } => AbstractOperation::IntegerBitwiseAnd {
                    psi_operation: operation.id,
                    result: operation.result.expect_scalar().id,
                    scalar_type,
                    left,
                    right,
                },
                OperationKind::IntegerBitwiseOr { .. } => AbstractOperation::IntegerBitwiseOr {
                    psi_operation: operation.id,
                    result: operation.result.expect_scalar().id,
                    scalar_type,
                    left,
                    right,
                },
                OperationKind::IntegerBitwiseXor { .. } => AbstractOperation::IntegerBitwiseXor {
                    psi_operation: operation.id,
                    result: operation.result.expect_scalar().id,
                    scalar_type,
                    left,
                    right,
                },
                _ => unreachable!(),
            }
        }
        _ => unreachable!("integer-bitwise router is exhaustive"),
    })
}

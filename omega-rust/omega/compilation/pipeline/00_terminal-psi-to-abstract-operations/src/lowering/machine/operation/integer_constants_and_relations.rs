use abstract_operations::AbstractOperation;
use terminal_psi::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(operation: &Operation) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::IntegerConstant { value } => AbstractOperation::IntegerConstant {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            scalar_type: operation.result.expect_scalar().scalar_type,
            value,
        },
        OperationKind::IntegerEqual { left, right } => AbstractOperation::IntegerEqual {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            left,
            right,
        },
        OperationKind::IntegerLessThan { left, right } => AbstractOperation::IntegerLessThan {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            left,
            right,
        },
        OperationKind::IntegerLessOrEqual { left, right } => {
            AbstractOperation::IntegerLessOrEqual {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                left,
                right,
            }
        }
        _ => unreachable!("integer constant-and-relation router is exhaustive"),
    })
}

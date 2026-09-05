use abstract_operations::AbstractOperation;
use terminal_psi::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(operation: &Operation) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::BooleanConstant { value } => AbstractOperation::BooleanConstant {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            value,
        },
        OperationKind::BooleanStructuralField { source, field } => {
            AbstractOperation::BooleanStructuralField {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                source,
                field,
            }
        }
        OperationKind::BooleanNot { operand } => AbstractOperation::BooleanNot {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            operand,
        },
        OperationKind::BooleanEqual { left, right } => AbstractOperation::BooleanEqual {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            left,
            right,
        },
        _ => unreachable!("boolean router is exhaustive"),
    })
}

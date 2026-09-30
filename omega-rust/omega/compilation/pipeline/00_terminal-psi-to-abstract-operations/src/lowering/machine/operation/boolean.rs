use abstract_operations::{AbstractOperation, AbstractResult};
use semantic_vocabulary::ScalarType;
use terminal_psi::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(operation: &Operation) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::BooleanConstant { value } => AbstractOperation::BooleanConstant {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            value,
        },
        OperationKind::BooleanStructuralField {
            source,
            path,
            field,
        } => AbstractOperation::StructuralScalarFieldRead {
            psi_operation: operation.id,
            result: AbstractResult {
                value: operation.result.expect_scalar().id,
                scalar_type: ScalarType::Boolean,
            },
            source,
            path,
            field,
        },
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

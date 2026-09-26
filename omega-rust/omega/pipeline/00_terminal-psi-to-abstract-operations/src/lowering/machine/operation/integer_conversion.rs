use std::collections::BTreeMap;

use abstract_operations::AbstractOperation;
use semantic_vocabulary::ScalarType;
use terminal_psi::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::IntegerWiden { operand } => {
            let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied() else {
                return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
            };
            let ScalarType::Integer(target_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
            };
            AbstractOperation::IntegerWiden {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                source_type,
                target_type,
                operand,
            }
        }
        OperationKind::IntegerExactCast {
            operand,
            obligation,
        } => {
            let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied() else {
                return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                    operation.id,
                ));
            };
            let ScalarType::Integer(target_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                    operation.id,
                ));
            };
            AbstractOperation::IntegerExactCast {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                source_type,
                target_type,
                operand,
            }
        }
        _ => unreachable!("integer-conversion router is exhaustive"),
    })
}

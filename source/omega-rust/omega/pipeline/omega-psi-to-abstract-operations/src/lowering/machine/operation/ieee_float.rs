//! Exact target-neutral IEEE scalar operations.

use omega_abstract_operations::AbstractOperation;
use psi_core::ScalarType;
use psi_terminal::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(operation: &Operation) -> Result<AbstractOperation, LoweringError> {
    let result = operation.result.expect_scalar();
    Ok(match operation.kind.clone() {
        OperationKind::IeeeFloatConstant { value } => {
            if result.scalar_type != ScalarType::IeeeFloat(value.format()) {
                return Err(LoweringError::VerifiedIeeeFloatMalformed(operation.id));
            }
            AbstractOperation::IeeeFloatConstant {
                psi_operation: operation.id,
                result: result.id,
                value,
            }
        }
        OperationKind::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
        } => {
            let ScalarType::IeeeFloat(format) = result.scalar_type else {
                return Err(LoweringError::VerifiedIeeeFloatMalformed(operation.id));
            };
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
                psi_operation: operation.id,
                result: result.id,
                format,
                left,
                right,
                addend,
            }
        }
        _ => unreachable!("IEEE float router is exhaustive"),
    })
}

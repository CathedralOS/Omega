//! Exact target-neutral IEEE scalar operations.

use abstract_operations::AbstractOperation;
use semantic_vocabulary::ScalarType;
use terminal_psi::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    value_types: &std::collections::BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = LoweringError::VerifiedIeeeFloatMalformed(operation.id);
    let result = operation.result.scalar().ok_or(invalid.clone())?;
    Ok(match operation.kind.clone() {
        OperationKind::IeeeFloatCompare {
            comparison,
            left,
            right,
        } => {
            let Some(ScalarType::IeeeFloat(format)) = value_types.get(&left).copied() else {
                return Err(invalid);
            };
            if result.scalar_type != ScalarType::Boolean
                || value_types.get(&right) != Some(&ScalarType::IeeeFloat(format))
            {
                return Err(invalid);
            }
            AbstractOperation::IeeeFloatCompare {
                psi_operation: operation.id,
                result: result.id,
                comparison,
                format,
                left,
                right,
            }
        }
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
        _ => return Err(invalid),
    })
}

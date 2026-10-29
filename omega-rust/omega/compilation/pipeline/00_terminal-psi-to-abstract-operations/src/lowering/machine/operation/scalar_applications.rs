//! Materialize completed target-bound scalar values at their original SSA
//! result and operation coordinates; canonical Terminal remains unchanged.

use abstract_operations::AbstractOperation;
use semantic_vocabulary::ScalarApplicationId;
use terminal_installation::{AdmittedScalarApplications, TerminalScalarValue};
use terminal_psi::Operation;

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    application: ScalarApplicationId,
    completed: Option<&AdmittedScalarApplications>,
) -> Result<AbstractOperation, LoweringError> {
    let value = completed
        .and_then(|completed| completed.values().value(application))
        .ok_or(LoweringError::UnclosedScalarApplication(application))?;
    let result = operation
        .result
        .scalar()
        .filter(|result| result.scalar_type == value.scalar_type())
        .ok_or(LoweringError::ScalarApplicationCarrierMismatch(
            operation.id,
        ))?;
    Ok(match value {
        TerminalScalarValue::Boolean(value) => AbstractOperation::BooleanConstant {
            psi_operation: operation.id,
            result: result.id,
            value,
        },
        TerminalScalarValue::Integer { value, .. } => AbstractOperation::IntegerConstant {
            psi_operation: operation.id,
            result: result.id,
            scalar_type: result.scalar_type,
            value,
        },
        TerminalScalarValue::IeeeFloat(value) => AbstractOperation::IeeeFloatConstant {
            psi_operation: operation.id,
            result: result.id,
            value,
        },
    })
}

//! Structural preflight for the attached Unit lowering lane.

use super::super::shared::*;

pub(super) fn validate_unit_function_shape(
    function: &AbstractFunction,
) -> Result<(), LoweringError> {
    if !function.parameters.is_empty() {
        return Err(LoweringError::UnitFunctionHasScalarParameters(
            function.machine,
        ));
    }
    if function.block_entries.len() != 1
        || function.block_entries[0].block != function.entry
        || !function.block_entries[0].parameters.is_empty()
    {
        return Err(LoweringError::UnitFunctionNotStraightLine(function.machine));
    }
    if let Some(AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. }) = function
        .operations
        .iter()
        .find(|operation| matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. }))
    {
        return Err(LoweringError::UnsupportedWriteOnlyPrimitiveStore {
            machine: function.machine,
            operation: *psi_operation,
        });
    }
    Ok(())
}

//! Structural preflight for the attached Unit lowering lane.

use super::super::shared::*;

pub(super) fn validate_unit_function_shape(
    function: &AbstractFunction,
) -> Result<(), LoweringError> {
    if !function.parameters.is_empty() && !has_bounded_scalar_parameter_shape(function) {
        return Err(LoweringError::UnitFunctionHasScalarParameters(
            function.machine,
        ));
    }
    if function.block_entries.len() != 1
        || function.block_entries[0].block != function.entry
        || function.block_entries[0].parameters != function.parameters
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

fn has_bounded_scalar_parameter_shape(function: &AbstractFunction) -> bool {
    matches!(
        function.parameters.as_slice(),
        [AbstractParameter {
            scalar_type: ScalarType::Integer(scalar_type),
            ..
        }] if scalar_type.carrier() == psi_core::IntegerCarrier::Fixed
            && scalar_type.sign() == psi_core::IntegerSign::Signed
            && scalar_type.bits() == 32
    )
}

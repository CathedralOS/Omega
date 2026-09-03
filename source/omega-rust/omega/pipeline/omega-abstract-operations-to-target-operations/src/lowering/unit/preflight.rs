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
    let canonical_entry_parameters = function.block_entries.first().is_some_and(|entry| {
        entry.parameters == function.parameters
            || (entry.parameters.is_empty()
                && (has_parameter_sourced_store_shape(function)
                    || has_parameter_sourced_unit_call_shape(function)))
    });
    if function.block_entries.len() != 1
        || function.block_entries[0].block != function.entry
        || !canonical_entry_parameters
    {
        return Err(LoweringError::UnitFunctionNotStraightLine(function.machine));
    }
    Ok(())
}

fn has_parameter_sourced_unit_call_shape(function: &AbstractFunction) -> bool {
    let [parameter] = function.parameters.as_slice() else {
        return false;
    };
    matches!(
        function.operations.as_slice(),
        [
            AbstractOperation::CallUnit {
                arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            },
            AbstractOperation::ReturnUnit { cleanup_actions, .. },
        ] if arguments.as_slice() == [parameter.value]
            && claim_transfers.is_empty()
            && requirement_obligations.is_empty()
            && crash_continuations.is_empty()
            && cleanup_actions.is_empty()
    )
}

fn has_parameter_sourced_store_shape(function: &AbstractFunction) -> bool {
    let [parameter] = function.parameters.as_slice() else {
        return false;
    };
    matches!(
        function.operations.as_slice(),
        [
            AbstractOperation::WriteOnlyPrimitiveStore { value, .. },
            AbstractOperation::ReturnUnit { .. },
        ] if value.value == parameter.value && value.scalar_type == parameter.scalar_type
    )
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

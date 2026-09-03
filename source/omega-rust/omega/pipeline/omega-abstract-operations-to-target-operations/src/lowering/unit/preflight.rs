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
    if function.parameters.is_empty() {
        return false;
    }
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
        ] if arguments.as_slice()
            == function
                .parameters
                .iter()
                .map(|parameter| parameter.value)
                .collect::<Vec<_>>()
            && claim_transfers.is_empty()
            && requirement_obligations.is_empty()
            && crash_continuations.is_empty()
            && cleanup_actions.is_empty()
    )
}

fn has_parameter_sourced_store_shape(function: &AbstractFunction) -> bool {
    matches!(
        function.operations.as_slice(),
        [
            AbstractOperation::WriteOnlyPrimitiveStore { value, .. },
            AbstractOperation::ReturnUnit { .. },
        ] if function.parameters.iter().any(|parameter| {
            value.value == parameter.value && value.scalar_type == parameter.scalar_type
        })
    )
}

fn has_bounded_scalar_parameter_shape(function: &AbstractFunction) -> bool {
    !function.parameters.is_empty()
        && function
            .parameters
            .iter()
            .all(|parameter| match parameter.scalar_type {
                ScalarType::Boolean => true,
                ScalarType::Integer(scalar_type) => {
                    scalar_type.carrier() == psi_core::IntegerCarrier::Fixed
                        && matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
                }
                ScalarType::IeeeFloat(_) => false,
            })
}

//! Structural preflight for the attached Unit lowering lane.

use super::super::shared::*;

pub(super) fn validate_unit_function_shape(
    function: &AbstractFunction,
) -> Result<(), LoweringError> {
    if super::continuation::has_shape(function) {
        return Ok(());
    }
    if !function.parameters.is_empty() && !has_bounded_scalar_parameter_shape(function) {
        return Err(LoweringError::UnitFunctionHasScalarParameters(
            function.machine,
        ));
    }
    let canonical_entry_parameters = function.block_entries.first().is_some_and(|entry| {
        entry.parameters.is_empty() || entry.parameters == function.parameters
    });
    if function.block_entries.len() != 1
        || function.block_entries[0].block != function.entry
        || !canonical_entry_parameters
    {
        return Err(LoweringError::UnitFunctionNotStraightLine(function.machine));
    }
    Ok(())
}

fn has_bounded_scalar_parameter_shape(function: &AbstractFunction) -> bool {
    !function.parameters.is_empty()
        && function
            .parameters
            .iter()
            .all(|parameter| match parameter.scalar_type {
                ScalarType::Boolean => true,
                ScalarType::Integer(scalar_type) => {
                    scalar_type.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                        && matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
                }
                ScalarType::IeeeFloat(_) => true,
            })
}

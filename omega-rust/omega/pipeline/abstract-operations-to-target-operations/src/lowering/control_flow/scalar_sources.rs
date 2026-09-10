//! Exact scalar sources retained by the dominating graph environment.
use super::LiveDefinitions;
use crate::lowering::shared::*;

pub(super) fn source(
    value: ValueId,
    function: &AbstractFunction,
    live: &LiveDefinitions,
) -> Result<TargetUnitScalarArgumentSource, LoweringError> {
    if let Some(known) = live.integers.get(&value) {
        return Ok(known.into_target_source(value));
    }
    if let Some(home) = live.scalar_homes.get(&value) {
        return Ok(TargetUnitScalarArgumentSource::Home(*home));
    }
    if let Some((defining_operation, immediate)) = live.booleans.get(&value) {
        return Ok(TargetUnitScalarArgumentSource::BooleanImmediate {
            defining_operation: *defining_operation,
            source_value: value,
            value: *immediate,
        });
    }
    if let Some((defining_operation, immediate)) = live.ieee_float_constants.get(&value) {
        return Ok(TargetUnitScalarArgumentSource::IeeeFloatImmediate {
            defining_operation: *defining_operation,
            source_value: value,
            value: *immediate,
        });
    }
    if let Some(parameter) = live.scalar_block_parameters.get(&value) {
        return Ok(TargetUnitScalarArgumentSource::BlockParameter(*parameter));
    }
    let (position, parameter) = function
        .parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.value == value)
        .ok_or(LoweringError::UnknownValue(value))?;
    Ok(TargetUnitScalarArgumentSource::Parameter {
        parameter_index: u32::try_from(position).map_err(|_| LoweringError::UnknownValue(value))?,
        source_value: value,
        scalar_type: parameter.scalar_type,
    })
}

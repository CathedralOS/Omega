//! Exact scalar sources retained by the dominating graph environment.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::unit::scalar_call::KnownUnitInteger;
use abstract_operations::AbstractFunction;
use semantic_vocabulary::{OperationId, ValueId};
use std::collections::BTreeMap;
use target_operations::{TargetUnitScalarArgumentSource, TargetUnitScalarHomeRequirement};

/// Borrowed view over the scalar maps a live graph carries, so sibling
/// lowering modules replay the same dominance precedence without naming the
/// private `LiveDefinitions` owner.
pub(crate) struct ScalarSources<'a> {
    pub integers: &'a BTreeMap<ValueId, KnownUnitInteger>,
    pub scalar_homes: &'a BTreeMap<ValueId, TargetUnitScalarHomeRequirement>,
    pub booleans: &'a BTreeMap<ValueId, (OperationId, bool)>,
    pub ieee_float_constants:
        &'a BTreeMap<ValueId, (OperationId, semantic_vocabulary::IeeeFloatValue)>,
    pub scalar_block_parameters: &'a BTreeMap<ValueId, target_operations::TargetScalarBlockValue>,
}

impl<'a> From<&'a LiveDefinitions> for ScalarSources<'a> {
    fn from(live: &'a LiveDefinitions) -> Self {
        Self {
            integers: &live.integers,
            scalar_homes: &live.scalar_homes,
            booleans: &live.booleans,
            ieee_float_constants: &live.ieee_float_constants,
            scalar_block_parameters: &live.scalar_block_parameters,
        }
    }
}

pub(super) fn source(
    value: ValueId,
    function: &AbstractFunction,
    live: &LiveDefinitions,
) -> Result<TargetUnitScalarArgumentSource, LoweringError> {
    resolved_source(value, function, &ScalarSources::from(live))
}

pub(crate) fn resolved_source(
    value: ValueId,
    function: &AbstractFunction,
    sources: &ScalarSources<'_>,
) -> Result<TargetUnitScalarArgumentSource, LoweringError> {
    if let Some(known) = sources.integers.get(&value) {
        return Ok(known.into_target_source(value));
    }
    if let Some(home) = sources.scalar_homes.get(&value) {
        return Ok(TargetUnitScalarArgumentSource::Home(*home));
    }
    if let Some((defining_operation, immediate)) = sources.booleans.get(&value) {
        return Ok(TargetUnitScalarArgumentSource::BooleanImmediate {
            defining_operation: *defining_operation,
            source_value: value,
            value: *immediate,
        });
    }
    if let Some((defining_operation, immediate)) = sources.ieee_float_constants.get(&value) {
        return Ok(TargetUnitScalarArgumentSource::IeeeFloatImmediate {
            defining_operation: *defining_operation,
            source_value: value,
            value: *immediate,
        });
    }
    if let Some(parameter) = sources.scalar_block_parameters.get(&value) {
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

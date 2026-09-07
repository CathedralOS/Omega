//! Independently join authored scalar bindings to canonical native call sources.

use abstract_operations::{AbstractBlockEntry, AbstractFunction, ValueBinding};
use semantic_vocabulary::{ScalarType, ValueId};
use std::collections::BTreeMap;
use target_operations::{
    TargetUnitBody, TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument,
};

pub(super) type Aliases = BTreeMap<ValueId, (ValueId, ScalarType)>;

pub(super) fn supported(scalar_type: ScalarType) -> bool {
    matches!(scalar_type, ScalarType::Integer(integer)
        if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
            && matches!(integer.bits(), 8 | 16 | 32 | 64))
}

pub(super) fn initial(source: &AbstractFunction, body: &TargetUnitBody) -> Option<Aliases> {
    if source.parameters.len() != body.scalar_parameters.len() {
        return None;
    }
    let mut aliases = Aliases::new();
    for (position, (parameter, actual)) in source
        .parameters
        .iter()
        .zip(&body.scalar_parameters)
        .enumerate()
    {
        if parameter.value != actual.value
            || parameter.scalar_type != actual.scalar_type
            || !supported(parameter.scalar_type)
            || body.call_plan.parameters.get(position) != Some(&actual.placement)
            || aliases
                .insert(parameter.value, (parameter.value, parameter.scalar_type))
                .is_some()
        {
            return None;
        }
    }
    Some(aliases)
}

pub(super) fn bind(
    next: &AbstractBlockEntry,
    bindings: &[ValueBinding],
    aliases: &mut Aliases,
) -> Option<()> {
    if bindings.len() != next.parameters.len() {
        return None;
    }
    let mut pending = Aliases::new();
    for (binding, parameter) in bindings.iter().zip(&next.parameters) {
        let source = aliases.get(&binding.argument).copied()?;
        if binding.parameter != parameter.value
            || binding.scalar_type != parameter.scalar_type
            || source.1 != binding.scalar_type
            || !supported(binding.scalar_type)
            || aliases.contains_key(&binding.parameter)
            || pending.insert(binding.parameter, source).is_some()
        {
            return None;
        }
    }
    aliases.extend(pending);
    Some(())
}

pub(super) fn arguments(
    source: &[ValueId],
    actual: &[TargetUnitScalarCallArgument],
    aliases: &Aliases,
    body: &TargetUnitBody,
) -> Option<()> {
    if source.len() != actual.len() {
        return None;
    }
    for (position, (source, actual)) in source.iter().zip(actual).enumerate() {
        let (canonical, scalar_type) = aliases.get(source).copied()?;
        let TargetUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value,
            scalar_type: actual_type,
        } = actual.source
        else {
            return None;
        };
        let parameter = body
            .scalar_parameters
            .get(usize::try_from(parameter_index).ok()?)?;
        if usize::try_from(actual.parameter_index).ok()? != position
            || canonical != source_value
            || scalar_type != actual_type
            || parameter.value != canonical
            || parameter.scalar_type != scalar_type
        {
            return None;
        }
    }
    Some(())
}

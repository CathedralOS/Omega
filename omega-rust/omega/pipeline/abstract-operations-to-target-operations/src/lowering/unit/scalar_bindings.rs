//! Simultaneous continuation bindings preserve the original scalar source identity.

use super::super::shared::*;

pub(super) fn initial(function: &AbstractFunction) -> BTreeMap<ValueId, ValueId> {
    function
        .parameters
        .iter()
        .map(|parameter| (parameter.value, parameter.value))
        .collect()
}

pub(super) fn supported(scalar_type: ScalarType) -> bool {
    matches!(scalar_type, ScalarType::Integer(integer)
        if super::super::scalar_abi::fixed_native_integer_shape(integer).is_some())
}

pub(super) fn bind(
    function: &AbstractFunction,
    target: semantic_vocabulary::BlockId,
    bindings: &[abstract_operations::ValueBinding],
    aliases: &mut BTreeMap<ValueId, ValueId>,
) -> Option<()> {
    let block = function
        .block_entries
        .iter()
        .find(|entry| entry.block == target)?;
    if block.parameters.len() != bindings.len() {
        return None;
    }
    let mut pending = BTreeMap::new();
    for (parameter, binding) in block.parameters.iter().zip(bindings) {
        let original = aliases.get(&binding.argument).copied()?;
        let source = function
            .parameters
            .iter()
            .find(|parameter| parameter.value == original)?;
        if parameter.value != binding.parameter
            || parameter.scalar_type != binding.scalar_type
            || source.scalar_type != binding.scalar_type
            || !supported(binding.scalar_type)
            || aliases.contains_key(&binding.parameter)
            || pending.insert(binding.parameter, original).is_some()
        {
            return None;
        }
    }
    aliases.extend(pending);
    Some(())
}

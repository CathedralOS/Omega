//! Parallel scalar binding replay for conditional CFG edges.

use super::super::*;

pub(super) fn bind_conditional_values(
    values: &mut BTreeMap<ValueId, KnownScalar>,
    bindings: &[omega_abstract_operations::ValueBinding],
    edge: EdgeId,
) -> Result<(), LoweringError> {
    let pending = bindings
        .iter()
        .map(|binding| {
            let value = values
                .get(&binding.argument)
                .cloned()
                .ok_or(LoweringError::UnknownValue(binding.argument))?;
            if binding.scalar_type != value.scalar_type() {
                return Err(LoweringError::ConditionalArmBindingTypeMismatch(edge));
            }
            Ok((
                binding.parameter,
                value.rebind_direct_parameter(binding.parameter),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (parameter, value) in pending {
        insert_value(values, parameter, value)?;
    }
    Ok(())
}

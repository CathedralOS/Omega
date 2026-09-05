//! Scalar binding composition across a removed empty block.

use std::collections::BTreeMap;

use abstract_operations::ValueBinding;
use optimization_unit::ValueDefinition;

pub(super) fn compose_linear_thread_bindings(
    parameters: &[ValueDefinition],
    incoming: &[ValueBinding],
    outgoing: &[ValueBinding],
) -> Option<Vec<ValueBinding>> {
    if parameters.len() != incoming.len() {
        return None;
    }
    let replacements = parameters
        .iter()
        .zip(incoming)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value && binding.scalar_type == parameter.scalar_type)
                .then_some((parameter.value, (binding.argument, binding.scalar_type)))
        })
        .collect::<Option<BTreeMap<_, _>>>()?;
    Some(
        outgoing
            .iter()
            .map(|binding| {
                replacements
                    .get(&binding.argument)
                    .map_or(*binding, |(argument, scalar_type)| ValueBinding {
                        parameter: binding.parameter,
                        argument: *argument,
                        scalar_type: *scalar_type,
                    })
            })
            .collect(),
    )
}

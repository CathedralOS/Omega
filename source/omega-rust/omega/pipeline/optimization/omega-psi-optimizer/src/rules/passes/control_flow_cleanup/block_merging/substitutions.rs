//! Exact block-parameter substitution reconstruction shared by both merge rules.

use super::super::*;

pub(super) fn merge_substitutions(
    machine: MachineId,
    target: &omega_optimization_unit::OptimizationBlock,
    bindings: &[omega_abstract_operations::ValueBinding],
    dominators: &[(BlockId, Vec<BlockId>)],
    use_definitions: &crate::UseDefinitionAnalysis,
) -> Option<Vec<ScalarSubstitution>> {
    let mut substitutions = target
        .parameters
        .iter()
        .zip(bindings)
        .map(|(parameter, binding)| {
            (binding.parameter == parameter.value
                && binding.scalar_type == parameter.scalar_type
                && replacement_dominates_parameter_uses(
                    machine,
                    binding.argument,
                    parameter.value,
                    dominators,
                    use_definitions,
                ))
            .then_some(ScalarSubstitution {
                from: parameter.value,
                to: binding.argument,
                scalar_type: parameter.scalar_type,
            })
        })
        .collect::<Option<Vec<_>>>()
        .filter(|_| target.parameters.len() == bindings.len())?;
    substitutions.sort();
    Some(substitutions)
}

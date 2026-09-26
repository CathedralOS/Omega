//! Exact block-parameter substitution reconstruction shared by both merge rules.

use semantic_vocabulary::{BlockId, MachineId};
use terminal_psi_to_abstract_operations::abstract_operations::ValueBinding;
use terminal_psi_to_abstract_operations::optimization_unit::{
    OptimizationBlock, ScalarSubstitution,
};

use crate::UseDefinitionAnalysis;
use crate::rules::support::replacement_dominates_parameter_uses;

pub(super) fn merge_substitutions(
    machine: MachineId,
    target: &OptimizationBlock,
    bindings: &[ValueBinding],
    dominators: &[(BlockId, Vec<BlockId>)],
    use_definitions: &UseDefinitionAnalysis,
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

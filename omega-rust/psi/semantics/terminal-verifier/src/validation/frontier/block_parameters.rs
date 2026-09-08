//! Whole owned successor bindings consume the old roots before establishing new ones.

use super::*;

pub(super) fn bind(
    frontier: &mut StructuralOwnershipFrontier,
    edge: EdgeId,
    target: &terminal_psi::Block,
    arguments: &[StructuralArgument],
) -> Result<(), ModuleError> {
    if arguments.len() != target.structural_parameters.len() {
        return Err(ModuleError::StructuralJumpArityMismatch {
            edge,
            expected: target.structural_parameters.len(),
            actual: arguments.len(),
        });
    }
    for (argument, parameter) in arguments.iter().zip(&target.structural_parameters) {
        if parameter.access != StructuralAccess::Owned {
            continue;
        }
        if !argument.path.is_empty()
            || argument.access != StructuralAccess::Owned
            || frontier.partial_custody_paths.contains_key(&argument.place)
            || frontier
                .claims
                .values()
                .any(|claim| claim.input == Some(argument.place))
            || (parameter.multiplicity == StructuralMultiplicity::Affine
                && frontier.owned_places.remove(&argument.place)
                    != Some(StructuralMultiplicity::Affine))
        {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: argument.place,
            });
        }
    }
    // Two-phase binding permits swaps and a self-loop without reviving a moved
    // source or overwriting an independently live target obligation.
    for parameter in &target.structural_parameters {
        if parameter.access == StructuralAccess::Owned
            && parameter.multiplicity == StructuralMultiplicity::Affine
            && frontier
                .owned_places
                .insert(parameter.place, parameter.multiplicity)
                .is_some()
        {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: parameter.place,
            });
        }
    }
    Ok(())
}

pub(super) fn disposal_order(machine: &TerminalMachine) -> Vec<&StructuralParameterDeclaration> {
    let mut parameters = machine.structural_parameters.iter().collect::<Vec<_>>();
    if machine
        .blocks
        .iter()
        .all(|block| block.structural_parameters.is_empty())
    {
        return parameters;
    }
    let dominators = crate::control_graph::dominators(machine);
    let mut blocks = machine
        .blocks
        .iter()
        .filter(|block| !block.structural_parameters.is_empty())
        .collect::<Vec<_>>();
    // Simultaneously live block roots have comparable dominating definitions.
    // Their establishment order follows dominance, never serialized block IDs.
    blocks.sort_by_key(|block| dominators.get(&block.id).map_or(0, BTreeSet::len));
    parameters.extend(
        blocks
            .into_iter()
            .flat_map(|block| &block.structural_parameters),
    );
    parameters
}

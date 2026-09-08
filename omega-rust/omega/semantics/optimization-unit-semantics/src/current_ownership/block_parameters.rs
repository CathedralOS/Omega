//! Current whole-owned edge transactions and parameter establishment order.

use super::*;
use abstract_operations::AbstractStructuralBinding;
use terminal_psi::StructuralParameterDeclaration;

pub(super) fn bind_owned_parameters(
    function: &PsiOptimizationFunction,
    block: BlockId,
    frontier: &mut CurrentOwnership,
    bindings: &[AbstractStructuralBinding],
    target: &OptimizationBlock,
) -> Result<(), OptimizationUnitValidationError> {
    let invalid = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
    // Signature validation already checked exact arity, types, modes and places.
    // Consume every old root before establishing any destination: swaps and
    // backedges must neither duplicate an owner nor overwrite another live root.
    for (binding, parameter) in bindings.iter().zip(&target.structural_parameters) {
        if parameter.access != StructuralAccess::Owned {
            continue;
        }
        let source = binding.argument.place;
        if frontier.partial_custody_paths.contains_key(&source)
            || frontier
                .claims
                .values()
                .any(|claim| claim.input == Some(source))
            || (parameter.multiplicity == StructuralMultiplicity::Affine
                && frontier.owned_places.remove(&source) != Some(StructuralMultiplicity::Affine))
        {
            return Err(invalid());
        }
    }
    for parameter in &target.structural_parameters {
        if parameter.access == StructuralAccess::Owned
            && parameter.multiplicity == StructuralMultiplicity::Affine
            && frontier
                .owned_places
                .insert(parameter.place, parameter.multiplicity)
                .is_some()
        {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(crate) fn parameter_establishment_order(
    function: &PsiOptimizationFunction,
) -> Vec<&StructuralParameterDeclaration> {
    let mut parameters = function.structural_parameters.iter().collect::<Vec<_>>();
    if function
        .blocks
        .iter()
        .all(|block| block.structural_parameters.is_empty())
    {
        return parameters;
    }
    let dominators = crate::independent_reachable_dominators(function);
    let mut blocks = function
        .blocks
        .iter()
        .filter(|block| !block.structural_parameters.is_empty())
        .collect::<Vec<_>>();
    // Simultaneously live block roots have comparable dominating definitions.
    // Serialized block/place IDs do not determine their establishment order.
    blocks.sort_by_key(|block| dominators.get(&block.id).map_or(0, BTreeSet::len));
    parameters.extend(
        blocks
            .into_iter()
            .flat_map(|block| &block.structural_parameters),
    );
    parameters
}

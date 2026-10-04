//! Current whole-owned edge transactions and parameter establishment order.
use crate::OptimizationUnitValidationError;
use crate::current_ownership::CurrentOwnership;
use abstract_operations::AbstractStructuralBinding;
use optimization_unit::{OptimizationBlock, PsiOptimizationFunction};
use semantic_vocabulary::BlockId;
use std::collections::BTreeSet;
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

pub(super) fn bind_owned_parameters(
    function: &PsiOptimizationFunction,
    types: &std::collections::BTreeMap<
        semantic_vocabulary::StructuralTypeId,
        &terminal_psi::StructuralTypeDeclaration,
    >,
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
    let mut transferred = Vec::new();
    for (binding, parameter) in bindings.iter().zip(&target.structural_parameters) {
        if parameter.access != StructuralAccess::Owned {
            continue;
        }
        let source = binding.argument.place;
        if parameter.multiplicity != StructuralMultiplicity::Unrestricted
            && !frontier.live_references.is_empty()
        {
            let mut remaining = frontier.live_references.clone();
            super::references::discard_owned(function, types, block, &mut remaining, source)?;
            if remaining.iter().any(|reference| {
                use crate::unit_validation::references::ReferenceOrigin;
                let root = match &reference.root {
                    ReferenceOrigin::Primitive(place)
                    | ReferenceOrigin::Alternatives { place, .. } => *place,
                    ReferenceOrigin::IngressLeaf(identity) => identity.place,
                };
                root == source || reference.carrier == source
            }) {
                return Err(invalid());
            }
        }
        if frontier.partial_custody_paths.contains_key(&source)
            || (parameter.multiplicity != StructuralMultiplicity::Unrestricted
                && frontier.owned_places.remove(&source) != Some(parameter.multiplicity))
        {
            return Err(invalid());
        }
        let claims = frontier
            .claims
            .iter()
            .filter_map(|(identity, claim)| (claim.input == Some(source)).then_some(*identity))
            .collect::<Vec<_>>();
        if (parameter.multiplicity == StructuralMultiplicity::Linear && claims.is_empty())
            || (parameter.multiplicity == StructuralMultiplicity::Unrestricted
                && !claims.is_empty())
        {
            return Err(invalid());
        }
        for identity in claims {
            let mut claim = frontier
                .claims
                .remove(&identity)
                .expect("captured live claim");
            claim.input = Some(parameter.place);
            transferred.push((identity, claim));
        }
    }
    for parameter in &target.structural_parameters {
        if parameter.access == StructuralAccess::Owned
            && parameter.multiplicity != StructuralMultiplicity::Unrestricted
            && frontier
                .owned_places
                .insert(parameter.place, parameter.multiplicity)
                .is_some()
        {
            return Err(invalid());
        }
    }
    for parameter in &target.structural_parameters {
        if parameter.access == StructuralAccess::Owned
            && frontier
                .claims
                .values()
                .any(|claim| claim.input == Some(parameter.place))
        {
            return Err(invalid());
        }
    }
    for (identity, claim) in transferred {
        if frontier.claims.insert(identity, claim).is_some() {
            return Err(invalid());
        }
    }
    // Simultaneous carrier rebinding preserves swaps; a destination must not
    // be mistaken for an old source by a later binding in the same edge.
    for reference in &mut frontier.live_references {
        if let Some((_, parameter)) =
            bindings
                .iter()
                .zip(&target.structural_parameters)
                .find(|(binding, parameter)| {
                    parameter.access == StructuralAccess::Owned
                        && binding.argument.access == StructuralAccess::Owned
                        && binding.argument.path.is_empty()
                        && binding.argument.place == reference.carrier
                })
        {
            reference.carrier = parameter.place;
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
    let dominators =
        crate::candidates::global_value_numbering::independent_reachable_dominators(function);
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

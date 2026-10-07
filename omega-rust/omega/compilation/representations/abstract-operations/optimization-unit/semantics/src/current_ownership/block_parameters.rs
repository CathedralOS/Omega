//! Current owned edge transactions and parameter establishment order.
//!
//! An edge closes in three steps, the order Terminal's ownership frontier
//! defines: every owned source leaves the frontier (`consume_edge_sources`),
//! the edge's residual and trivial cleanup runs against what remains, and only
//! then are the target's roots installed (`establish_edge_parameters`). A
//! projected owned argument moves one affine child out of a live root, so the
//! root keeps partial custody that the same edge's residual cleanup may retire.
use crate::OptimizationUnitValidationError;
use crate::current_ownership::structural::{
    partial_affine_root_type, projected_root_is_fully_consumed,
};
use crate::current_ownership::{CurrentOwnership, LiveClaim};
use abstract_operations::AbstractStructuralBinding;
use optimization_unit::{OptimizationBlock, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, ClaimId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration,
};

/// Claims carried by consumed sources, already renamed to their destination
/// parameters and installed only once every source has left the frontier.
pub(super) struct EdgeTransfer {
    claims: Vec<(ClaimId, LiveClaim)>,
}

/// Phase one: consume every owned source before the edge's cleanup runs and
/// before any destination is established, so swaps and backedges neither
/// duplicate an owner nor overwrite another live root. Signature validation
/// already checked exact arity, types, modes and places.
pub(super) fn consume_edge_sources(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &mut CurrentOwnership,
    bindings: &[AbstractStructuralBinding],
    target: &OptimizationBlock,
) -> Result<EdgeTransfer, OptimizationUnitValidationError> {
    let invalid = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
    let mut transferred = Vec::new();
    for (binding, parameter) in bindings.iter().zip(&target.structural_parameters) {
        if parameter.access != StructuralAccess::Owned {
            continue;
        }
        let source = binding.argument.place;
        if !binding.argument.path.is_empty() {
            consume_projected_source(function, types, block, frontier, binding, parameter)?;
            continue;
        }
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
    Ok(EdgeTransfer {
        claims: transferred,
    })
}

/// A projected move keeps its claim-free affine root live until the remaining
/// complement is transferred or disposed: the root records the exact moved
/// path as partial custody instead of leaving `owned_places`. The selected
/// carriers relocate with the child; no other live loan may suspend it.
fn consume_projected_source(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &mut CurrentOwnership,
    binding: &AbstractStructuralBinding,
    parameter: &StructuralParameterDeclaration,
) -> Result<(), OptimizationUnitValidationError> {
    let invalid = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
    let argument = &binding.argument;
    if argument.access != StructuralAccess::Owned
        || parameter.multiplicity != StructuralMultiplicity::Affine
        || frontier.owned_places.get(&argument.place) != Some(&StructuralMultiplicity::Affine)
        || frontier
            .claims
            .values()
            .any(|claim| claim.input == Some(argument.place))
        || !partial_affine_root_type(function, argument.place).is_some_and(|root_type| {
            crate::unit_validation::structural_catalog::resolve_structural_path(
                types,
                root_type,
                &argument.path,
            ) == Some(parameter.structural_type)
        })
    {
        return Err(invalid());
    }
    if !frontier.live_references.is_empty() {
        let mut remaining = frontier.live_references.clone();
        super::references::discard_subtree(
            function,
            types,
            block,
            &mut remaining,
            argument.place,
            &argument.path,
        )?;
        let terminator = function
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)
            .and_then(|candidate| u32::try_from(candidate.nodes.len().checked_sub(1)?).ok())
            .ok_or_else(invalid)?;
        super::references::check_region_access(
            function,
            block,
            terminator,
            &remaining,
            argument.place,
            &argument.path,
        )?;
    }
    let moved = frontier
        .partial_custody_paths
        .entry(argument.place)
        .or_default();
    if moved
        .iter()
        .any(|existing| existing.starts_with(&argument.path) || argument.path.starts_with(existing))
        || !moved.insert(argument.path.clone())
    {
        return Err(invalid());
    }
    // A fully transferred root has no residual complement left to discard;
    // it leaves the owned map at this edge, as at a projected call.
    if projected_root_is_fully_consumed(function, types, frontier, argument.place) {
        frontier.owned_places.remove(&argument.place);
    }
    Ok(())
}

/// Phase two: install the target roots after the edge's cleanup, then the
/// transferred claims, then relocate carried loans. Reading each old carrier
/// once preserves swaps; a projected move rebases the moved carriers onto the
/// destination parameter.
pub(super) fn establish_edge_parameters(
    function: &PsiOptimizationFunction,
    block: BlockId,
    frontier: &mut CurrentOwnership,
    bindings: &[AbstractStructuralBinding],
    target: &OptimizationBlock,
    transfer: EdgeTransfer,
) -> Result<(), OptimizationUnitValidationError> {
    let invalid = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
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
    for (identity, claim) in transfer.claims {
        if frontier.claims.insert(identity, claim).is_some() {
            return Err(invalid());
        }
    }
    for reference in &mut frontier.live_references {
        if let Some((binding, parameter)) =
            bindings
                .iter()
                .zip(&target.structural_parameters)
                .find(|(binding, parameter)| {
                    parameter.access == StructuralAccess::Owned
                        && binding.argument.access == StructuralAccess::Owned
                        && binding.argument.place == reference.carrier
                        && reference.carrier_path.starts_with(&binding.argument.path)
                })
        {
            reference.carrier = parameter.place;
            reference.carrier_path.drain(..binding.argument.path.len());
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

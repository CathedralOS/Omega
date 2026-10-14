//! Transport existing descriptor disposal through exact whole-owned arrivals.
//!
//! A descriptor type is not a disposal grant. The enclosing planner already
//! selected source births and their loan cleanup authority. Receiving homes
//! inherit that authority only over complete, typed, whole-owned edge rosters.
//! Seed reachability admits an anchored cycle; all-dependency pruning removes
//! unseeded cycles and a selected arrival with even one unauthorized source.
//! These equations establish provenance only. The ordinary custody solver
//! still checks actual consumption, formation and availability on every edge.

use super::{LoweringError, unsupported};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    Block, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, Terminator,
};

struct Arrival {
    place: PlaceId,
    sources: Vec<PlaceId>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn inherit(
    managed: &mut Vec<PlaceId>,
    parameters: &[StructuralParameterDeclaration],
    local_places: &[StructuralPlaceDeclaration],
    blocks: &[Block],
    types: &[StructuralTypeDeclaration],
    partial_transfers: &[PlaceId],
    scheduled: &[PlaceId],
) -> Result<(), LoweringError> {
    let mut arrivals = Vec::new();
    for block in blocks {
        for (position, parameter) in block.structural_parameters.iter().enumerate() {
            if managed.contains(&parameter.place)
                || !parameter.is_unqualified_owned_affine()
                || partial_transfers.contains(&parameter.place)
                || scheduled.contains(&parameter.place)
                || !descriptor_type(types, parameter.structural_type)
            {
                continue;
            }
            if block
                .structural_parameters
                .iter()
                .enumerate()
                .any(|(index, parameter)| usize::try_from(parameter.position).ok() != Some(index))
                || blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters)
                    .filter(|other| other.place == parameter.place)
                    .count()
                    != 1
            {
                return unsupported("descriptor arrival has no unique positional binding");
            }
            let mut sources = Vec::new();
            let mut complete = true;
            for predecessor in blocks {
                let edges = match &predecessor.terminator {
                    Terminator::Jump {
                        target,
                        structural_arguments,
                        cleanup_actions,
                        ..
                    } if *target == block.id => {
                        vec![(structural_arguments.as_slice(), cleanup_actions.as_slice())]
                    }
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => [when_true, when_false]
                        .into_iter()
                        .filter(|edge| edge.target == block.id)
                        .map(|edge| {
                            (
                                edge.structural_arguments.as_slice(),
                                edge.cleanup_actions.as_slice(),
                            )
                        })
                        .collect(),
                    Terminator::StructuralCase { cases, .. }
                        if cases.iter().any(|edge| edge.target == block.id) =>
                    {
                        complete = false;
                        Vec::new()
                    }
                    _ => Vec::new(),
                };
                for (arguments, cleanup) in edges {
                    if arguments.len() != block.structural_parameters.len() {
                        return unsupported("descriptor arrival has an incomplete edge roster");
                    }
                    let source = &arguments[position];
                    if source.access != StructuralAccess::Owned
                        || !source.path.is_empty()
                        || partial_transfers.contains(&source.place)
                        || cleanup.iter().any(|action| action.place() == source.place)
                        || owned_affine_type(parameters, local_places, blocks, source.place)
                            != Some(parameter.structural_type)
                    {
                        complete = false;
                    }
                    sources.push(source.place);
                }
            }
            if complete && !sources.is_empty() {
                arrivals.push(Arrival {
                    place: parameter.place,
                    sources,
                });
            }
        }
    }
    // Reachability is not authority: an external managed birth anchors an
    // SCC, but every incoming dependency must also close before publication.
    let mut inherited = Vec::new();
    loop {
        let previous = inherited.len();
        for arrival in &arrivals {
            if !inherited.contains(&arrival.place)
                && arrival
                    .sources
                    .iter()
                    .any(|source| managed.contains(source) || inherited.contains(source))
            {
                inherited.push(arrival.place);
            }
        }
        if previous == inherited.len() {
            break;
        }
    }
    loop {
        let previous = inherited.clone();
        inherited.retain(|place| {
            arrivals
                .iter()
                .find(|arrival| arrival.place == *place)
                .is_some_and(|arrival| {
                    arrival
                        .sources
                        .iter()
                        .all(|source| managed.contains(source) || previous.contains(source))
                })
        });
        if inherited == previous {
            break;
        }
    }
    managed.extend(inherited);
    Ok(())
}

fn descriptor_type(types: &[StructuralTypeDeclaration], identity: StructuralTypeId) -> bool {
    let mut matches = types
        .iter()
        .filter(|declaration| declaration.id == identity);
    let Some(declaration) = matches.next() else {
        return false;
    };
    matches.next().is_none()
        && (matches!(
            declaration.shape,
            StructuralTypeShape::Reference { .. } | StructuralTypeShape::ElementView { .. }
        ) || declaration.shape.is_borrowed_byte_view())
}

/// Identify owned affine storage, not permission to discard it. Qualification
/// changes neither multiplicity nor whether a whole move ends this home.
/// Inheritance separately requires anchored disposal authority; the enclosing
/// planner uses this query only to withdraw an already selected death after a
/// move, without extending its disposal capability roster.
pub(super) fn owned_affine_type(
    parameters: &[StructuralParameterDeclaration],
    local_places: &[StructuralPlaceDeclaration],
    blocks: &[Block],
    place: PlaceId,
) -> Option<StructuralTypeId> {
    let parameters = parameters
        .iter()
        .chain(blocks.iter().flat_map(|block| &block.structural_parameters))
        .filter(|parameter| parameter.place == place)
        .map(|parameter| {
            (parameter.access == StructuralAccess::Owned
                && parameter.multiplicity == StructuralMultiplicity::Affine)
                .then_some(parameter.structural_type)
        });
    let results = blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| operation.result.structural())
        .filter(|result| result.place == place)
        .map(|result| {
            (result.multiplicity == StructuralMultiplicity::Affine)
                .then_some(result.structural_type)
        });
    let locals = local_places
        .iter()
        .filter(|local| local.id == place)
        .filter_map(|local| match local.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                structural_type, ..
            } => Some(Some(structural_type)),
            _ => None,
        });
    let mut providers = parameters.chain(results).chain(locals);
    let structural_type = providers.next()??;
    providers.next().is_none().then_some(structural_type)
}

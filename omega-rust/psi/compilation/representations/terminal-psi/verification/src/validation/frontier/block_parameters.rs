//! Whole owned successor bindings consume the old roots before establishing new ones.

use super::super::borrowed_windows::{WindowAliases, check_edge_arguments};
use super::super::{StructuralArgument, StructuralPlaceKind, resolve_structural_path};
use super::{
    EdgeId, LiveClaim, ModuleError, StructuralAccess, StructuralMultiplicity,
    StructuralOwnershipFrontier, StructuralParameterDeclaration, TerminalMachine, TerminalModule,
    partial_affine_root_type, projected_root_is_fully_consumed,
};
/// Phase one consumes each owned source before the residual and trivial
/// cleanup for the same edge runs. A projected owned argument moves one
/// affine child out of a live root: the root records the exact moved path as
/// partial custody instead of leaving `owned_places`. The selected edge may
/// dispose the exact residual complement or carry it into the successor.
pub(super) fn consume(
    module: &TerminalModule,
    machine: &TerminalMachine,
    frontier: &mut StructuralOwnershipFrontier,
    edge: EdgeId,
    target: &terminal_psi::Block,
    arguments: &[StructuralArgument],
    window_aliases: &WindowAliases,
) -> Result<Vec<(super::ClaimId, LiveClaim)>, ModuleError> {
    if arguments.len() != target.structural_parameters.len() {
        return Err(ModuleError::StructuralJumpArityMismatch {
            edge,
            expected: target.structural_parameters.len(),
            actual: arguments.len(),
        });
    }
    // An open restoration window cannot cross an edge under any access:
    // the successor's binding could not name the hole this frame opened.
    check_edge_arguments(module, machine, frontier, window_aliases, edge, arguments)?;
    let mut transferred = Vec::new();
    for (argument, parameter) in arguments.iter().zip(&target.structural_parameters) {
        if parameter.access != StructuralAccess::Owned {
            continue;
        }
        if !argument.path.is_empty() {
            // A projected move keeps its root live until its remaining
            // complement is transferred or explicitly disposed.
            if argument.access != StructuralAccess::Owned
                || parameter.multiplicity != StructuralMultiplicity::Affine
                || frontier.owned_places.get(&argument.place)
                    != Some(&StructuralMultiplicity::Affine)
                || frontier
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(argument.place))
            {
                return Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place: argument.place,
                });
            }
            let path_is_exact =
                partial_affine_root_type(machine, argument.place).is_some_and(|root_type| {
                    resolve_structural_path(module, root_type, &argument.path)
                        == Some(parameter.structural_type)
                });
            let moved = frontier
                .partial_custody_paths
                .entry(argument.place)
                .or_default();
            if !path_is_exact
                || moved.iter().any(|existing| {
                    existing.starts_with(&argument.path) || argument.path.starts_with(existing)
                })
                || !moved.insert(argument.path.clone())
            {
                return Err(ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place: argument.place,
                });
            }
            // A fully transferred root has no residual complement left to
            // discard; it leaves both frontier maps at this edge.
            if projected_root_is_fully_consumed(module, machine, frontier, argument.place) {
                frontier.owned_places.remove(&argument.place);
            }
            continue;
        }
        if parameter.multiplicity != StructuralMultiplicity::Unrestricted
            && !frontier.references.is_empty()
        {
            // Check the existing leaf roster and external children without
            // ending any loan. The whole move keeps those leaves and backing.
            let mut remaining = frontier.references.clone();
            super::super::references::discard_owned(
                module,
                machine,
                &mut remaining,
                argument.place,
            )?;
            super::super::references::check_root_access(machine, &remaining, argument.place)?;
        }
        if argument.access != StructuralAccess::Owned
            || frontier.partial_custody_paths.contains_key(&argument.place)
            || (parameter.multiplicity != StructuralMultiplicity::Unrestricted
                && frontier.owned_places.remove(&argument.place) != Some(parameter.multiplicity))
        {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: argument.place,
            });
        }
        let claims = frontier
            .claims
            .iter()
            .filter_map(|(identity, claim)| {
                (claim.input == Some(argument.place)).then_some(*identity)
            })
            .collect::<Vec<_>>();
        if (parameter.multiplicity == StructuralMultiplicity::Linear && claims.is_empty())
            || (parameter.multiplicity == StructuralMultiplicity::Unrestricted
                && !claims.is_empty())
        {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: argument.place,
            });
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
    Ok(transferred)
}

/// Phase two installs the target roots after every consumed source has left
/// the frontier. Two-phase binding permits swaps and a self-loop without
/// reviving a moved source or overwriting an independently live target
/// obligation. A shared-borrow parameter is separately re-checked against
/// the post-transfer frontier: the loan never moves custody itself, but its
/// root must still be held — or be a place the frontier never tracks — and
/// no consumed or residual evidence on this same edge may have retired it.
pub(super) fn establish(
    module: &TerminalModule,
    machine: &TerminalMachine,
    frontier: &mut StructuralOwnershipFrontier,
    edge: EdgeId,
    target: &terminal_psi::Block,
    arguments: &[StructuralArgument],
    transferred: Vec<(super::ClaimId, LiveClaim)>,
) -> Result<(), ModuleError> {
    let aliases = super::super::borrowed_windows::window_aliases(module, machine);
    for (argument, parameter) in arguments.iter().zip(&target.structural_parameters) {
        if super::super::block_views::mutable_records::parameter_type(module, parameter) {
            let roots = aliases
                .get(&argument.place)
                .map(|roots| roots.iter().map(|(root, _)| *root).collect::<Vec<_>>())
                .unwrap_or_else(|| vec![argument.place]);
            for root in roots {
                if let Some(home) =
                    super::super::block_views::mutable_records::owned_home(module, machine, root)
                    && ((home.multiplicity == StructuralMultiplicity::Affine
                        && frontier.owned_places.get(&root)
                            != Some(&StructuralMultiplicity::Affine))
                        || frontier.partial_custody_paths.contains_key(&root)
                        || frontier
                            .claims
                            .values()
                            .any(|claim| claim.input == Some(root)))
                {
                    return Err(ModuleError::InvalidStructuralSuccessorArgument {
                        edge,
                        place: root,
                    });
                }
            }
        }
        if parameter.access == StructuralAccess::SharedBorrow
            && (frontier.partial_custody_paths.contains_key(&argument.place)
                || (carries_owned_frontier(machine, argument.place)
                    && !frontier.owned_places.contains_key(&argument.place)))
        {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: argument.place,
            });
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
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: parameter.place,
            });
        }
    }
    for parameter in &target.structural_parameters {
        if parameter.access == StructuralAccess::Owned
            && frontier
                .claims
                .values()
                .any(|claim| claim.input == Some(parameter.place))
        {
            return Err(ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: parameter.place,
            });
        }
    }
    for (identity, claim) in transferred {
        let place = claim.input.expect("transferred claim has a destination");
        if frontier.claims.insert(identity, claim).is_some() {
            return Err(ModuleError::InvalidStructuralSuccessorArgument { edge, place });
        }
    }
    // Read each old carrier once: sequential source replacement would rewrite
    // the first half of a swap again when processing its second binding.
    for reference in &mut frontier.references {
        if let Some((_, parameter)) =
            arguments
                .iter()
                .zip(&target.structural_parameters)
                .find(|(argument, parameter)| {
                    parameter.access == StructuralAccess::Owned
                        && argument.access == StructuralAccess::Owned
                        && argument.path.is_empty()
                        && argument.place == reference.carrier
                })
        {
            reference.carrier = parameter.place;
        }
    }
    Ok(())
}

/// Whether a live place sits in the frontier's owned map. Parameters and
/// trivial locals carry their declared custody; an operation result enters
/// the map exactly when its producer does not classify it as copyable or
/// borrowed. Untracked places stay live by their binding and need no row.
fn carries_owned_frontier(machine: &TerminalMachine, place: super::PlaceId) -> bool {
    if machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .any(|parameter| {
            parameter.place == place
                && parameter.access == StructuralAccess::Owned
                && parameter.multiplicity != StructuralMultiplicity::Unrestricted
        })
    {
        return true;
    }
    let Some(declaration) = machine
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)
    else {
        return false;
    };
    match declaration.kind {
        StructuralPlaceKind::TrivialAffineLocal { .. } => true,
        StructuralPlaceKind::OperationResult { producer, .. } => machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == producer)
            .and_then(|operation| operation.result.structural())
            .filter(|result| result.place == place)
            .is_none_or(super::result_requires_owned_frontier),
        _ => false,
    }
}

pub(super) fn disposal_order<'machine>(
    machine: &'machine TerminalMachine,
    dominators: &crate::control_graph::DominatorTree,
) -> Vec<&'machine StructuralParameterDeclaration> {
    let mut parameters = machine.structural_parameters.iter().collect::<Vec<_>>();
    if machine
        .blocks
        .iter()
        .all(|block| block.structural_parameters.is_empty())
    {
        return parameters;
    }
    let mut blocks = machine
        .blocks
        .iter()
        .filter(|block| !block.structural_parameters.is_empty())
        .collect::<Vec<_>>();
    // Simultaneously live block roots have comparable dominating definitions.
    // Their establishment order follows dominance, never serialized block IDs.
    blocks.sort_by_key(|block| dominators.depth(block.id).unwrap_or(0));
    parameters.extend(
        blocks
            .into_iter()
            .flat_map(|block| &block.structural_parameters),
    );
    parameters
}

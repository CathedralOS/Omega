//! Address joins retain original backing and pin their lending roots.
//!
//! An address join (`abstract_operations::control_flow::address_joins`) holds
//! only its referent's address, so the lending root must stay live, unmoved and
//! unwritten while the joined view can still be observed. The origins of one
//! join are the roots of every edge argument that binds it, transitively
//! through earlier joins. Its scope is every block its own block dominates:
//! the joined place is nameable nowhere else, so this covers the loan's whole
//! live range.
//!
//! Inside that scope no operation may move, release, write or exclusively
//! borrow a pinned root, and no edge may move or discard one. Shared reads
//! and further shared loans stay legal, and a return ends every loan with the
//! activation, so return cleanup may dispose the roots. Pinning the whole
//! dominated region rather than the view's last use is deliberately
//! conservative: a program that moves an origin after the view's final use
//! inside that region is rejected here even though it is sound.
//!
//! A mutable join over a local home additionally suspends observations and
//! copies of that original owner (including stale descriptor aliases). The
//! owner remains on the affine frontier across the loan and is disposed only
//! by the ordinary exact cleanup at true return, after the returned copy has
//! been evaluated. Transfers between mutable names retain the same backing.

use crate::OptimizationUnitValidationError;
use crate::current_ownership::CurrentOwnership;
use crate::unit_validation::operation_contracts::structural_source_contract;
use crate::unit_validation::references::ReferenceOrigin;
use abstract_operations::AbstractOperation as O;
use optimization_unit::{OptimizationEdge, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralTypeDeclaration};

/// Roots pinned in each block by the address joins whose scope includes it.
#[derive(Default)]
pub(super) struct AddressJoinPins {
    shared: BTreeMap<BlockId, BTreeSet<PlaceId>>,
    exclusive: BTreeMap<BlockId, BTreeSet<PlaceId>>,
    origins: BTreeMap<PlaceId, BTreeSet<PlaceId>>,
}

impl AddressJoinPins {
    pub(super) fn new(
        function: &PsiOptimizationFunction,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Self {
        let joins = function
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .structural_parameters
                    .iter()
                    .filter(|parameter| {
                        parameter.access == StructuralAccess::SharedBorrow
                            && abstract_operations::control_flow::address_joins::is_address_join(
                                parameter,
                                |identity| structural_types.get(&identity).copied(),
                            )
                    })
                    .map(move |parameter| (parameter.place, block.id))
            })
            .collect::<BTreeMap<PlaceId, BlockId>>();
        let mut origins = BTreeMap::<PlaceId, BTreeSet<PlaceId>>::new();
        for edge in function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.successors)
        {
            for binding in &edge.structural_bindings {
                if joins.contains_key(&binding.parameter) {
                    origins
                        .entry(binding.parameter)
                        .or_default()
                        .insert(binding.argument.place);
                }
            }
        }
        // Mutable arrivals transfer one exclusive permission; their exact
        // name availability is reconstructed by function_structure::mutable_views.
        // Shared reborrows must still freeze every intermediate mutable name,
        // not merely the entrance pointer it ultimately carries.
        let address_joins = function
            .blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .filter(|parameter| {
                abstract_operations::control_flow::address_joins::is_address_join(
                    parameter,
                    |identity| structural_types.get(&identity).copied(),
                )
            })
            .map(|parameter| parameter.place)
            .collect::<BTreeSet<_>>();
        if address_joins.is_empty() {
            return Self::default();
        }
        for edge in function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.successors)
        {
            for binding in &edge.structural_bindings {
                if address_joins.contains(&binding.parameter) {
                    origins
                        .entry(binding.parameter)
                        .or_default()
                        .insert(binding.argument.place);
                }
            }
        }
        // A join that reborrows an earlier join pins that join's roots too.
        loop {
            let mut expanded = origins.clone();
            for roots in expanded.values_mut() {
                let inherited = roots
                    .iter()
                    .filter_map(|root| origins.get(root))
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>();
                roots.extend(inherited);
            }
            if expanded == origins {
                break;
            }
            origins = expanded;
        }
        let dominators =
            crate::candidates::global_value_numbering::independent_reachable_dominators(function);
        let mut pins = BTreeMap::<BlockId, BTreeSet<PlaceId>>::new();
        for (join, owner) in &joins {
            let roots = origins
                .get(join)
                .into_iter()
                .flatten()
                .filter(|root| !joins.contains_key(root))
                .copied()
                .collect::<Vec<_>>();
            for (block, dominating) in &dominators {
                if block == owner || dominating.contains(owner) {
                    pins.entry(*block)
                        .or_default()
                        .extend(roots.iter().copied());
                }
            }
        }
        // A local owner keeps its affine disposal obligation while a joined
        // mutable name holds its address. Pin every possible local origin of
        // those joins; existing mutable-name availability handles forwarding.
        let mutable_joins = function
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .structural_parameters
                    .iter()
                    .filter(|parameter| {
                        parameter.access == StructuralAccess::MutableBorrow
                            && address_joins.contains(&parameter.place)
                    })
                    .map(move |parameter| (parameter.place, block.id))
            })
            .collect::<Vec<_>>();
        if mutable_joins.is_empty() {
            return Self {
                shared: pins,
                exclusive: BTreeMap::new(),
                origins,
            };
        }
        // Descriptor aliases are borrowed backing, not fresh owned copies.
        // Their reads must not bypass suspension merely by naming a view ID.
        let mut aliases = origins.clone();
        let mut descriptors = BTreeSet::new();
        for node in function.blocks.iter().flat_map(|block| &block.nodes) {
            let alias = match &node.operation {
                O::EstablishElementView { result, source, .. } => {
                    Some((result.place, source.place))
                }
                O::ElementViewSubslice { result, source, .. }
                | O::ByteSequenceSubslice { result, source, .. } => Some((result.place, *source)),
                _ => None,
            };
            if let Some((alias, source)) = alias {
                descriptors.insert(alias);
                aliases.entry(alias).or_default().insert(source);
            }
        }
        for block in &function.blocks {
            for parameter in &block.structural_parameters {
                if !address_joins.contains(&parameter.place)
                    && parameter.access != StructuralAccess::Owned
                {
                    descriptors.insert(parameter.place);
                }
            }
            for edge in block.nodes.iter().flat_map(|node| &node.successors) {
                for binding in &edge.structural_bindings {
                    if binding.argument.access != StructuralAccess::Owned {
                        aliases
                            .entry(binding.parameter)
                            .or_default()
                            .insert(binding.argument.place);
                    }
                }
            }
        }
        loop {
            let previous = aliases.clone();
            for roots in aliases.values_mut() {
                let inherited = roots
                    .iter()
                    .filter_map(|root| previous.get(root))
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>();
                roots.extend(inherited);
            }
            if aliases == previous {
                break;
            }
        }
        let mut exclusive = BTreeMap::<BlockId, BTreeSet<PlaceId>>::new();
        for (join, owner) in mutable_joins {
            let roots = origins
                .get(&join)
                .into_iter()
                .flatten()
                .filter(|root| {
                    function.structural_places.iter().any(|place| {
                        place.id == **root
                            && matches!(
                                place.kind,
                                semantic_vocabulary::StructuralPlaceKind::OperationResult { .. }
                            )
                    }) && structural_source_contract(function, **root, false)
                        .is_some_and(|source| source.access == StructuralAccess::Owned)
                })
                .copied()
                .collect::<BTreeSet<_>>();
            if roots.is_empty() {
                continue;
            }
            let mut suspended = roots.clone();
            suspended.extend(
                descriptors
                    .iter()
                    .filter(|descriptor| {
                        aliases.get(*descriptor).is_some_and(|origins| {
                            origins.iter().any(|origin| roots.contains(origin))
                        })
                    })
                    .copied(),
            );
            for (block, dominating) in &dominators {
                if *block == owner || dominating.contains(&owner) {
                    exclusive
                        .entry(*block)
                        .or_default()
                        .extend(suspended.iter().copied());
                }
            }
        }
        Self {
            shared: pins,
            exclusive,
            origins: aliases,
        }
    }

    /// Reject an operation in `block` that disturbs a pinned root. `moved`
    /// holds the whole and projected owned moves the replay itself derived
    /// for this operation; copies of an unrestricted root are not moves.
    pub(super) fn check_operation(
        &self,
        function: &PsiOptimizationFunction,
        block: BlockId,
        node: u32,
        operation: &O,
        moved: impl IntoIterator<Item = PlaceId>,
    ) -> Result<(), OptimizationUnitValidationError> {
        if let Some(pinned) = self.exclusive.get(&block) {
            // Normal return cleanup ends the activation loan after evaluating
            // the fresh returned value. Its exact disposal roster is replayed
            // separately; returning the borrowed owner's value is still a use.
            let inputs = match operation {
                O::ReturnStructural { source, .. } => vec![*source],
                O::Return { .. } | O::ReturnUnit { .. } => Vec::new(),
                _ => crate::unit_validation::function_structure::operation_place_inputs(operation),
            };
            if let Some(place) = inputs.into_iter().find(|place| pinned.contains(place)) {
                return Err(
                    OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed {
                        machine: function.machine,
                        block,
                        node,
                        place,
                    },
                );
            }
        }
        let Some(pinned) = self.shared.get(&block) else {
            return Ok(());
        };
        let disturbed = moved
            .into_iter()
            .chain(written_or_exclusive(operation))
            .find(|place| pinned.contains(place));
        match disturbed {
            Some(place) => Err(
                OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed {
                    machine: function.machine,
                    block,
                    node,
                    place,
                },
            ),
            None => Ok(()),
        }
    }

    /// Reject an edge leaving `block` that moves, exclusively lends or
    /// discards a pinned root. `destination` declares the edge's arrivals;
    /// binding an unrestricted owned arrival copies its root. Return cleanup
    /// is not an edge: it ends the joined view with the activation.
    pub(super) fn check_edge(
        &self,
        function: &PsiOptimizationFunction,
        block: BlockId,
        node: u32,
        edge: &OptimizationEdge,
        destination: &[terminal_psi::StructuralParameterDeclaration],
        frontier: &CurrentOwnership,
    ) -> Result<(), OptimizationUnitValidationError> {
        // A new mutable local loan borrows live storage; it does not move or
        // discharge the owner. Check the current frontier before edge cleanup
        // and reject aliases supplied or disposed in the same transaction.
        for (binding, parameter) in edge.structural_bindings.iter().zip(destination) {
            if parameter.access != StructuralAccess::MutableBorrow {
                continue;
            }
            let place = binding.argument.place;
            let Some(source) = structural_source_contract(function, place, false)
                .filter(|source| source.access == StructuralAccess::Owned)
            else {
                continue;
            };
            let owner_live = match source.multiplicity {
                terminal_psi::StructuralMultiplicity::Affine => {
                    frontier.owned_places.get(&place)
                        == Some(&terminal_psi::StructuralMultiplicity::Affine)
                }
                terminal_psi::StructuralMultiplicity::Unrestricted => true,
                terminal_psi::StructuralMultiplicity::Linear => false,
            };
            if !owner_live
                || frontier.partial_custody_paths.contains_key(&place)
                || frontier
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(place))
                || frontier.live_references.iter().any(|reference| {
                    reference.root == ReferenceOrigin::Primitive(place)
                        || reference.carrier == place
                })
                || edge
                    .structural_bindings
                    .iter()
                    .filter(|other| {
                        other.argument.place == place
                            || self
                                .origins
                                .get(&other.argument.place)
                                .is_some_and(|origins| origins.contains(&place))
                    })
                    .count()
                    != 1
                || edge.trivial_affine_discards.contains(&place)
                || edge
                    .residual_affine_discards
                    .iter()
                    .any(|discard| discard.place == place)
            {
                return Err(
                    OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed {
                        machine: function.machine,
                        block,
                        node,
                        place,
                    },
                );
            }
        }
        if let Some(pinned) = self.exclusive.get(&block)
            && let Some(place) = edge
                .structural_bindings
                .iter()
                .map(|binding| binding.argument.place)
                .chain(edge.trivial_affine_discards.iter().copied())
                .chain(
                    edge.residual_affine_discards
                        .iter()
                        .map(|discard| discard.place),
                )
                .find(|place| pinned.contains(place))
        {
            return Err(
                OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed {
                    machine: function.machine,
                    block,
                    node,
                    place,
                },
            );
        }
        let Some(pinned) = self.shared.get(&block) else {
            return Ok(());
        };
        let disturbed = edge
            .structural_bindings
            .iter()
            .zip(destination)
            .filter(|(binding, parameter)| match binding.argument.access {
                StructuralAccess::SharedBorrow => false,
                StructuralAccess::Owned => {
                    parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                }
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => true,
            })
            .map(|(binding, _)| binding.argument.place)
            .chain(edge.trivial_affine_discards.iter().copied())
            .chain(
                edge.residual_affine_discards
                    .iter()
                    .map(|discard| discard.place),
            )
            .find(|place| pinned.contains(place));
        match disturbed {
            Some(place) => Err(
                OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed {
                    machine: function.machine,
                    block,
                    node,
                    place,
                },
            ),
            None => Ok(()),
        }
    }
}

/// Every place an operation writes or lends exclusively. Moves are supplied
/// by the replay, which already knows each argument's consuming multiplicity.
fn written_or_exclusive(operation: &O) -> Vec<PlaceId> {
    let exclusive = |arguments: &[terminal_psi::StructuralArgument]| {
        arguments
            .iter()
            .filter(|argument| {
                matches!(
                    argument.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                )
            })
            .map(|argument| argument.place)
            .collect::<Vec<_>>()
    };
    match operation {
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallUnitWithDynamicArguments {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalarWithDynamicArguments {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => exclusive(structural_arguments),
        O::CallDynamicScalar {
            dynamic_dispatch, ..
        } => exclusive(std::slice::from_ref(&dynamic_dispatch.rebound.source)),
        O::PrimitiveLocalStore { destination, .. }
        | O::ByteSequenceWrite { destination, .. }
        | O::StructuralByteSequenceFieldByteStore { destination, .. }
        | O::StructuralByteSequenceFieldStore { destination, .. } => vec![*destination],
        O::WriteOnlyPrimitiveStore { destination, .. } => vec![destination.place],
        O::StoreStructuralField { destination, .. } => vec![destination.place()],
        O::StructuralScalarFieldStore { destination, .. } => vec![destination.place()],
        O::MoveStructuralField { source, .. } => vec![source.place()],
        _ => Vec::new(),
    }
}

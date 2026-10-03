//! Address joins retain original backing and pin their lending roots.
//!
//! An address join (`abstract_operations::control_flow::address_joins`) holds
//! only its referent's address, so the lending root must stay live, unmoved and
//! unwritten in the borrowed region while the joined view can still be observed. The origins of one
//! join are the roots of every edge argument that binds it, transitively
//! through earlier joins. Within the dominated scope, scalar observations and
//! exact edge forwarding delimit the live loan: a later state which retains
//! neither the join nor a forwarded descendant no longer pins that root.
//!
//! Inside that scope no operation may move, release, write or exclusively
//! borrow a pinned root, and no edge may move or discard one. Shared reads
//! and further shared loans stay legal, and a return ends every loan with the
//! activation, so return cleanup may dispose the roots. Shared pins retain exact
//! projections, allowing disjoint sibling writes. Their lifetimes cover whole
//! blocks, and backward reachability includes backedges, not just dominance.
//! Uses which may retain an untracked alias (including calls and descriptor
//! construction) keep the original conservative whole-scope pin. Releasing
//! those loans requires their producer's alias relation, not a last-use guess.
//!
//! A mutable join over a local home additionally suspends observations and
//! copies of that original owner (including stale descriptor aliases). The
//! owner remains on the affine frontier across the loan. It is disposed by the
//! ordinary exact cleanup at true return, after the returned copy has been
//! evaluated, or on an edge where a later state's loan chain ends: the shared
//! pin's rule admits that discard only when no block at or below the edge's
//! target still names the home or a join carrying it. Transfers between
//! mutable names retain the same backing.

pub(super) mod borrowed_regions;
mod descriptor_loans;
pub(crate) use descriptor_loans::validate_compatibility;
mod range_relations;

use crate::OptimizationUnitValidationError;
use crate::current_ownership::CurrentOwnership;
use crate::unit_validation::operation_contracts::structural_source_contract;
use crate::unit_validation::references::ReferenceOrigin;
use abstract_operations::AbstractOperation as O;
use borrowed_regions::{BorrowedRegions, Projection, Region};
use descriptor_loans::DescriptorLoans;
use optimization_unit::{OptimizationEdge, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, MachineId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralTypeDeclaration};

/// Borrowed regions pinned by shared arrivals and owners suspended by mutable arrivals.
#[derive(Default)]
pub(super) struct AddressJoinPins {
    borrowed_regions: BorrowedRegions,
    descriptor_loans: DescriptorLoans,
    shared: BTreeMap<BlockId, Vec<Region>>,
    exclusive: BTreeMap<BlockId, BTreeSet<PlaceId>>,
    origins: BTreeMap<PlaceId, BTreeSet<PlaceId>>,
    dominators: BTreeMap<BlockId, BTreeSet<BlockId>>,
    readers: BTreeMap<PlaceId, BTreeSet<BlockId>>,
}

impl AddressJoinPins {
    pub(super) fn descriptor_requirements(
        &self,
    ) -> Vec<terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement> {
        self.descriptor_loans.requirements()
    }

    pub(super) fn new(
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        call_accesses: &super::CallAccesses,
    ) -> Self {
        let borrowed_regions =
            BorrowedRegions::new(function, functions, structural_types, call_accesses);
        let descriptor_loans = DescriptorLoans::new(function, &borrowed_regions, structural_types);
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
            return Self {
                borrowed_regions,
                descriptor_loans,
                ..Self::default()
            };
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
        let mut predecessors = BTreeMap::<BlockId, BTreeSet<BlockId>>::new();
        for block in &function.blocks {
            for edge in block.nodes.iter().flat_map(|node| &node.successors) {
                predecessors
                    .entry(edge.target)
                    .or_default()
                    .insert(block.id);
            }
        }
        let mut pins = BTreeMap::<BlockId, Vec<Region>>::new();
        for (join, owner) in &joins {
            let live_blocks =
                shared_join_live_blocks(function, *join, *owner, &origins, &predecessors);
            let regions = borrowed_regions.resolve(*join, Projection::new(&[]));
            for (block, dominating) in &dominators {
                if (block == owner || dominating.contains(owner))
                    && live_blocks.as_ref().is_none_or(|live| live.contains(block))
                {
                    pins.entry(*block)
                        .or_default()
                        .extend(regions.iter().cloned());
                }
            }
        }
        // Blocks whose operations name each place, so an edge discard can
        // tell a loan that is still observed downstream from one whose
        // readers all ran already. Reads through a descriptor or join name
        // are resolved to their roots when an edge is checked.
        let mut readers = BTreeMap::<PlaceId, BTreeSet<BlockId>>::new();
        for block in &function.blocks {
            for node in &block.nodes {
                for place in crate::unit_validation::function_structure::operation_place_inputs(
                    &node.operation,
                ) {
                    readers.entry(place).or_default().insert(block.id);
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
                    .map(|parameter| parameter.place)
            })
            .collect::<Vec<_>>();
        if mutable_joins.is_empty() {
            return Self {
                borrowed_regions,
                descriptor_loans,
                shared: pins,
                exclusive: BTreeMap::new(),
                origins,
                dominators,
                readers,
            };
        }
        // Descriptor aliases are borrowed backing, not fresh owned copies.
        // Their reads must not bypass suspension merely by naming a view ID.
        let mut aliases = BTreeMap::<PlaceId, BTreeSet<PlaceId>>::new();
        let mut descriptors = BTreeSet::new();
        for node in function.blocks.iter().flat_map(|block| &block.nodes) {
            if let Some((_, result)) = node.operation.structural_result()
                && let Some(sources) = borrowed_regions.direct_aliases(result.place)
            {
                descriptors.insert(result.place);
                aliases
                    .entry(result.place)
                    .or_default()
                    .extend(sources.iter().map(|(source, _)| *source));
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
        // Keep exact parent edges beside possible transitive backing. A merged
        // descriptor belongs to this join's child cohort only when every source
        // path passes through the join; one fresh arm cannot cover a stale arm.
        let direct_aliases = aliases.clone();
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
        let available = crate::unit_validation::function_structure::current_mutable_borrows(
            function,
            structural_types,
        );
        let mut exclusive = BTreeMap::<BlockId, BTreeSet<PlaceId>>::new();
        for join in mutable_joins {
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
                        }) && !entirely_descends_from(
                            **descriptor,
                            join,
                            &direct_aliases,
                            &mut BTreeSet::new(),
                        )
                    })
                    .copied(),
            );
            for block in dominators.keys() {
                // A successor may already have transferred or attenuated
                // this exclusive spelling. Pin while any path still holds it;
                // every later mutable join establishes its own pin, including
                // stale descriptors of the same owned backing. Shared arrivals
                // retain the independent shared pin and never restore a retired
                // exclusive name merely because its address still exists.
                if available.may_be_live(*block, join) {
                    exclusive
                        .entry(*block)
                        .or_default()
                        .extend(suspended.iter().copied());
                }
            }
        }
        Self {
            borrowed_regions,
            descriptor_loans,
            shared: pins,
            exclusive,
            origins: aliases,
            dominators,
            readers,
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
        let moved = moved.into_iter().collect::<Vec<_>>();
        if let Some(place) = self.descriptor_loans.conflicted_operation(
            &self.borrowed_regions,
            block,
            node,
            operation,
            &moved,
        ) {
            return Err(
                OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed {
                    machine: function.machine,
                    block,
                    node,
                    place,
                },
            );
        }
        if let Some(pinned) = self.exclusive.get(&block) {
            // Normal return cleanup ends the activation loan after evaluating
            // the fresh returned value. Its exact disposal roster is replayed
            // separately; returning the borrowed owner's value is still a use.
            let inputs = match operation {
                O::ReturnStructural { source, .. } => vec![*source],
                O::Return { .. } | O::ReturnUnit { .. } => Vec::new(),
                // An edge's discards are judged by `check_edge`.
                O::Jump {
                    structural_bindings,
                    ..
                } => structural_bindings
                    .iter()
                    .map(|binding| binding.argument.place)
                    .collect(),
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
        let disturbed = self
            .borrowed_regions
            .conflicted_operation(pinned, (block, node), &moved);
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
        if let Some(place) = self.descriptor_loans.conflicted_edge(
            &self.borrowed_regions,
            block,
            node,
            edge,
            destination,
        ) {
            return Err(
                OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed {
                    machine: function.machine,
                    block,
                    node,
                    place,
                },
            );
        }
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
        // A later state's loan chain ends on an edge: discarding the
        // suspended home there is sound once nothing at or below the target
        // names the home or a join carrying it (the shared pin's rule).
        let exclusive_read_after = |place: &PlaceId| {
            let mut names = BTreeSet::from([*place]);
            for (name, roots) in &self.origins {
                if roots.contains(place) {
                    names.insert(*name);
                }
            }
            names.iter().any(|name| {
                self.readers.get(name).is_some_and(|blocks| {
                    blocks.iter().any(|reader| {
                        *reader == edge.target
                            || self
                                .dominators
                                .get(reader)
                                .is_some_and(|dominating| dominating.contains(&edge.target))
                    })
                })
            })
        };
        if let Some(pinned) = self.exclusive.get(&block)
            && let Some(place) = edge
                .structural_bindings
                .iter()
                .map(|binding| binding.argument.place)
                .chain(
                    edge.trivial_affine_discards
                        .iter()
                        .copied()
                        .filter(|place| exclusive_read_after(place)),
                )
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
        // A pin covers every block the join's owner dominates, but the loan
        // only needs the root alive while some operation can still read it.
        // A discard (or exclusive hand-off) on this edge disturbs the loan
        // only when a block at or below the target still names the place,
        // a join that carries it, or a descriptor aliased to it.
        let read_after = |place: &PlaceId| {
            let mut names = BTreeSet::from([*place]);
            for (name, roots) in &self.origins {
                if roots.contains(place) {
                    names.insert(*name);
                }
            }
            names.iter().any(|name| {
                self.readers.get(name).is_some_and(|blocks| {
                    blocks.iter().any(|reader| {
                        *reader == edge.target
                            || self
                                .dominators
                                .get(reader)
                                .is_some_and(|dominating| dominating.contains(&edge.target))
                    })
                })
            })
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
            .map(|(binding, _)| (binding.argument.place, binding.argument.path.as_slice()))
            .chain(
                edge.trivial_affine_discards
                    .iter()
                    .map(|place| (*place, &[][..])),
            )
            .chain(
                edge.residual_affine_discards
                    .iter()
                    .map(|discard| (discard.place, discard.path.as_slice())),
            )
            .find(|(place, path)| {
                self.borrowed_regions.overlaps(pinned, *place, path) && read_after(place)
            })
            .map(|(place, _)| place);
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

/// Possible roots detect interference; exact ancestry grants current-child
/// access. A cycle or a source outside this join cannot prove that every path
/// came from its current authority, even if another incoming arm did.
fn entirely_descends_from(
    place: PlaceId,
    ancestor: PlaceId,
    parents: &BTreeMap<PlaceId, BTreeSet<PlaceId>>,
    active: &mut BTreeSet<PlaceId>,
) -> bool {
    if place == ancestor {
        return true;
    }
    if !active.insert(place) {
        return false;
    }
    let descends = parents.get(&place).is_some_and(|sources| {
        !sources.is_empty()
            && sources
                .iter()
                .all(|source| entirely_descends_from(*source, ancestor, parents, active))
    });
    active.remove(&place);
    descends
}

/// Prove a shared join's block liveness only when every use either observes a
/// scalar or forwards the exact loan into another address join. Unknown escape
/// effects retain the old whole-scope pin. Root reads are deliberately absent:
/// after a loan ends, reading or writing its owner does not revive that loan.
fn shared_join_live_blocks(
    function: &PsiOptimizationFunction,
    join: PlaceId,
    owner: BlockId,
    origins: &BTreeMap<PlaceId, BTreeSet<PlaceId>>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Option<BTreeSet<BlockId>> {
    let mut names = BTreeSet::from([join]);
    names.extend(
        origins
            .iter()
            .filter_map(|(name, roots)| roots.contains(&join).then_some(*name)),
    );
    let mut live = BTreeSet::from([owner]);
    for block in &function.blocks {
        // Formation itself keeps the loan live in its receiving block, even
        // when an optimizer has removed every observation in that block.
        if block
            .structural_parameters
            .iter()
            .any(|parameter| names.contains(&parameter.place))
        {
            live.insert(block.id);
        }
        for node in &block.nodes {
            let inputs =
                crate::unit_validation::function_structure::operation_place_inputs(&node.operation);
            if !inputs.iter().any(|place| names.contains(place)) {
                continue;
            }
            let cannot_retain_alias = match &node.operation {
                O::PrimitiveScalarRead { .. }
                | O::StructuralScalarFieldRead { .. }
                | O::StructuralCaseMembership { .. }
                | O::ByteSequenceLength { .. }
                | O::ByteSequenceRead { .. }
                | O::StructuralByteSequenceFieldLength { .. }
                | O::StructuralByteSequenceFieldRead { .. } => true,
                O::Jump { .. } | O::Conditional { .. } => {
                    inputs
                        .iter()
                        .filter(|place| names.contains(place))
                        .all(|place| {
                            node.successors.iter().any(|edge| {
                                edge.structural_bindings.iter().any(|binding| {
                                    binding.argument.place == *place
                                        && binding.argument.access == StructuralAccess::SharedBorrow
                                        && names.contains(&binding.parameter)
                                })
                            })
                        })
                        && node
                            .successors
                            .iter()
                            .flat_map(|edge| &edge.structural_bindings)
                            .filter(|binding| names.contains(&binding.argument.place))
                            .all(|binding| {
                                binding.argument.access == StructuralAccess::SharedBorrow
                                    && names.contains(&binding.parameter)
                            })
                }
                _ => false,
            };
            if !cannot_retain_alias {
                return None;
            }
            live.insert(block.id);
        }
    }
    let mut pending = live.iter().copied().collect::<Vec<_>>();
    while let Some(block) = pending.pop() {
        for predecessor in predecessors.get(&block).into_iter().flatten() {
            if live.insert(*predecessor) {
                pending.push(*predecessor);
            }
        }
    }
    Some(live)
}

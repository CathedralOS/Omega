//! Shared address joins pin every root they may borrow for their whole scope.
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

use crate::OptimizationUnitValidationError;
use abstract_operations::AbstractOperation as O;
use optimization_unit::{OptimizationEdge, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralTypeDeclaration};

/// Roots pinned in each block by the address joins whose scope includes it.
#[derive(Default)]
pub(super) struct AddressJoinPins(BTreeMap<BlockId, BTreeSet<PlaceId>>);

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
                        abstract_operations::control_flow::address_joins::is_address_join(
                            parameter,
                            |identity| structural_types.get(&identity).copied(),
                        )
                    })
                    .map(move |parameter| (parameter.place, block.id))
            })
            .collect::<BTreeMap<PlaceId, BlockId>>();
        if joins.is_empty() {
            return Self::default();
        }
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
        Self(pins)
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
        let Some(pinned) = self.0.get(&block) else {
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
    ) -> Result<(), OptimizationUnitValidationError> {
        let Some(pinned) = self.0.get(&block) else {
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
        O::WriteOnlyPrimitiveStore { destination, .. }
        | O::WriteOnlyIndexedPrimitiveStore { destination, .. }
        | O::StructuralScalarFieldStore { destination, .. }
        | O::StoreStructuralField { destination, .. } => vec![destination.place],
        O::MoveStructuralField { source, .. } => vec![source.place],
        _ => Vec::new(),
    }
}

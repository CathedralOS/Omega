//! Structural producers and dominating availability of their executable uses.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId};

use crate::OptimizationUnitValidationError;
use crate::unit_validation::derived_metadata::dominators;

pub(crate) fn validate_structural_place_availability(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &optimization_unit::OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let mut producers = BTreeMap::<PlaceId, (BlockId, Option<u32>)>::new();
    for block in &function.blocks {
        for parameter in &block.structural_parameters {
            producers.insert(parameter.place, (block.id, None));
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let place =
                crate::unit_validation::operation_contracts::structural_operation_result_contract(
                    function,
                    &node.operation,
                )
                .and_then(|contract| contract.operation_result().map(|result| result.place))
                .or(match &node.operation {
                    // These establishments publish a declaration directly,
                    // rather than a StructuralOperationResult contract.
                    O::EstablishByteSequenceLiteral { place, .. }
                    | O::EstablishTrivialAffineLocal { place, .. } => Some(place.id),
                    _ => None,
                });
            if let Some(place) = place {
                producers.insert(
                    place,
                    (
                        block.id,
                        Some(u32::try_from(node_index).expect("unit node index fits u32")),
                    ),
                );
            }
        }
    }
    let dominators = dominators(function.entry, blocks.keys().copied(), predecessors);
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            for place in operation_place_inputs(&node.operation) {
                let Some((producer_block, producer_node)) = producers.get(&place) else {
                    continue;
                };
                let available = (*producer_block == block.id
                    && producer_node.is_none_or(|producer| producer < node_index))
                    || (*producer_block != block.id
                        && dominators
                            .get(&block.id)
                            .is_some_and(|set| set.contains(producer_block)));
                if !available {
                    return Err(
                        OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                            machine: function.machine,
                            block: block.id,
                            node: node_index,
                            place,
                        },
                    );
                }
            }
        }
    }
    Ok(())
}

/// Places the operation observes or lends, excluding disposal of owned custody.
/// A retired loan name cannot grant access merely because its descriptor still
/// has a cleanup obligation.
pub(crate) fn operation_place_accesses(operation: &O) -> Vec<PlaceId> {
    match operation {
        O::AtomicEvent { event, .. } => event.place().into_iter().collect(),
        O::EstablishByteSequenceConcatenation { left, right, .. } => {
            vec![left.source(), right.source()]
        }
        O::EstablishStructuralArray { elements, .. } => {
            elements.iter().map(|argument| argument.place).collect()
        }
        O::EstablishCase { fields, .. } | O::EstablishRecord { fields, .. } => fields
            .iter()
            .filter_map(|initializer| match &initializer.value {
                terminal_psi::RecordFieldValue::Structural(argument) => Some(argument.place),
                terminal_psi::RecordFieldValue::Scalar { .. } => None,
            })
            .collect(),
        O::Jump {
            structural_bindings,
            ..
        } => structural_bindings
            .iter()
            .map(|binding| binding.argument.place)
            .collect(),
        O::Conditional {
            when_true,
            when_false,
            ..
        } => when_true
            .structural_bindings
            .iter()
            .chain(&when_false.structural_bindings)
            .map(|binding| binding.argument.place)
            .collect(),
        O::PrimitiveLocalStore { destination, .. } => vec![*destination],
        O::WriteOnlyPrimitiveStore { destination, .. } => vec![destination.place()],
        O::StructuralScalarFieldStore { destination, .. } => vec![destination.place()],
        O::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments
            .iter()
            .map(|argument| argument.place)
            .collect(),
        // Capture reads the selected source now; a later stored dispatch uses
        // that retained descriptor, not a fresh read of this source name.
        O::StoreDynamicDescriptor { stored, .. } => vec![stored.selection.source.place],
        O::CallStructural {
            structural_arguments,
            dynamic_arguments,
            ..
        } => {
            let mut places = structural_arguments
                .iter()
                .map(|argument| argument.place)
                .collect::<Vec<_>>();
            for argument in dynamic_arguments {
                match &argument.source {
                    abstract_operations::AbstractDynamicDescriptorSource::Selection {
                        selection,
                        ..
                    } => places.push(selection.source.place),
                    abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                        initial,
                        rebound,
                        ..
                    } => {
                        places.push(initial.source.place);
                        places.push(rebound.source.place);
                    }
                    abstract_operations::AbstractDynamicDescriptorSource::Parameter(_) => {}
                }
            }
            places
        }
        // Only a rebound descriptor's two selection sources join here; a
        // stored descriptor's source is not required at the call.
        O::CallDynamic {
            dispatch: abstract_operations::AbstractDynamicDispatch::Rebound(dynamic_dispatch),
            ..
        } => vec![
            dynamic_dispatch.initial.source.place,
            dynamic_dispatch.rebound.source.place,
        ],
        O::ByteSequenceStore {
            destination,
            source,
            ..
        }
        | O::StructuralByteSequenceFieldStore {
            destination,
            source,
            ..
        }
        | O::StructuralByteViewFieldStore {
            destination,
            source,
            ..
        } => vec![*destination, *source],
        // Both sides of a window observe the borrowed root; the repair also
        // consumes its owned value place, which must dominate the store.
        O::MoveStructuralField { source, .. } => vec![source.place()],
        O::StoreStructuralField {
            destination, value, ..
        } => vec![destination.place(), value.place],
        // A leaf copy reads through its borrowed root; the root must
        // dominate the copy.
        O::StructuralLeafCopy { source, .. } => vec![*source],
        O::EstablishReference { source, .. } | O::EstablishElementView { source, .. } => {
            vec![source.place]
        }
        O::ReleaseReference { source, .. } => vec![*source],
        O::PrimitiveScalarRead { source, .. }
        | O::StructuralCaseMembership { source, .. }
        | O::ByteSequenceSubslice { source, .. }
        | O::StructuralCase { source, .. }
        | O::ByteSequenceRead { source, .. }
        | O::ByteSequenceWrite {
            destination: source,
            ..
        }
        | O::ElementViewWrite {
            destination: source,
            ..
        }
        | O::StructuralByteSequenceFieldByteStore {
            destination: source,
            ..
        }
        | O::ByteSequenceLength { source, .. }
        | O::ElementViewLength { source, .. }
        | O::ElementViewRead { source, .. }
        | O::ElementViewSubslice { source, .. }
        | O::StructuralByteSequenceFieldLength { source, .. }
        | O::StructuralByteSequenceFieldRead { source, .. }
        | O::StructuralScalarFieldRead { source, .. }
        | O::ReturnStructural { source, .. } => {
            vec![*source]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn operation_place_inputs(operation: &O) -> Vec<PlaceId> {
    let mut inputs = operation_place_accesses(operation);
    match operation {
        O::Jump {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => {
            inputs.extend(trivial_affine_discards.iter().copied());
            inputs.extend(residual_affine_discards.iter().map(|discard| discard.place));
        }
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => inputs.extend(cleanup_actions.iter().map(|cleanup| match cleanup {
            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => *place,
            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => discard.place,
            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => cleanup.place,
        })),
        O::ReturnStructural {
            trivial_affine_discards,
            ..
        } => inputs.extend(trivial_affine_discards.iter().copied()),
        _ => {}
    }
    inputs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::support::id;
    use abstract_operations::atomic::AbstractAtomicLocation;
    use abstract_operations::{AbstractAtomicEvent, AtomicModificationAfter};
    use semantic_vocabulary::{OperationId, StructuralFieldId, StructuralTypeId, ValueId};
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

    #[test]
    fn atomic_block_parameter_must_dominate_its_use() {
        let mut function = crate::tests::fixtures::scalar_units::unit()
            .functions
            .remove(0);
        let root = id(90, PlaceId::new);
        function.blocks[0].nodes[0].operation = O::AtomicEvent {
            psi_operation: id(91, OperationId::new),
            event: AbstractAtomicEvent::Store {
                location: AbstractAtomicLocation {
                    root,
                    path: Vec::new(),
                    field: id(92, StructuralFieldId::new),
                },
                ordering: language_core::atomic::MemoryOrdering::NoOrdering,
                value: id(3, ValueId::new),
            },
            reads_from: None,
            modification_after: Some(AtomicModificationAfter::InitialResidency),
        };
        let parameter = StructuralParameterDeclaration {
            place: root,
            position: 0,
            is_self: false,
            structural_type: id(93, StructuralTypeId::new),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        function.blocks[0].structural_parameters.push(parameter);
        let blocks = function
            .blocks
            .iter()
            .map(|block| (block.id, block))
            .collect();
        validate_structural_place_availability(&function, &blocks, &BTreeMap::new()).unwrap();
        let mut later = function.blocks[0].clone();
        later.id = id(94, BlockId::new);
        function.blocks[0].structural_parameters.clear();
        function.blocks.push(later);
        let blocks = function
            .blocks
            .iter()
            .map(|block| (block.id, block))
            .collect();
        let predecessors =
            BTreeMap::from([(function.blocks[1].id, BTreeSet::from([function.entry]))]);
        assert!(matches!(
            validate_structural_place_availability(&function, &blocks, &predecessors),
            Err(OptimizationUnitValidationError::StructuralPlaceNotAvailable { .. })
        ));
    }
}

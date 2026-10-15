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
                    abstract_operations::byte_literal_establish_pair_pattern!(place, ..) => {
                        Some(place.id)
                    }
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
        abstract_operations::establish_case_or_record_fields_pattern!(fields) => fields
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
        abstract_operations::boundary_or_structural_call_pair_pattern!(
            structural_arguments,
            ..
        ) => structural_arguments
            .iter()
            .map(|argument| argument.place)
            .collect(),
        abstract_operations::byte_sequence_store_pair_pattern!(destination, source, ..)
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
        O::EstablishReference { source, .. }
        | O::EstablishPlacedReference { source, .. }
        | O::EstablishElementView { source, .. }
        | O::ElementViewLength { source, .. }
        | O::ElementViewSubslice { source, .. } => {
            vec![source.place]
        }
        O::ElementViewWrite { destination, .. } => vec![destination.place],
        O::ReleaseReference { source, .. } => vec![*source],
        abstract_operations::scalar_read_or_membership_pattern!(source, ..)
        | O::StructuralCase { source, .. }
        | O::ByteSequenceRead { source, .. }
        | O::ByteSequenceWrite {
            destination: source,
            ..
        }
        | O::StructuralByteSequenceFieldByteStore {
            destination: source,
            ..
        }
        | O::ElementViewRead { source, .. }
        | abstract_operations::byte_len_or_field_read_pattern!(source, ..)
        | O::StructuralScalarFieldRead { source, .. }
        | O::ReturnStructural { source, .. } => {
            vec![*source]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn operation_place_inputs(operation: &O) -> Vec<PlaceId> {
    let mut inputs = operation_place_accesses(operation);
    for (_, actions) in operation.cleanup_rosters() {
        inputs.extend(actions.iter().map(|action| action.place()));
    }
    inputs
}

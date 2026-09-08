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
            let place = match &node.operation {
                O::ByteSequenceSubslice { result, .. }
                | O::EstablishPayloadlessCase { result, .. }
                | O::EstablishAffineScalarRecord { result, .. }
                | O::CallStructural { result, .. }
                | O::BoundaryCall {
                    result: abstract_operations::AbstractBoundaryResult::Structural(result),
                    ..
                } => Some(result.place),
                O::EstablishByteSequenceLiteral { place, .. }
                | O::EstablishTrivialAffineLocal { place, .. } => Some(place.id),
                _ => None,
            };
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

fn operation_place_inputs(operation: &O) -> Vec<PlaceId> {
    let mut inputs = match operation {
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
        O::WriteOnlyPrimitiveStore { destination, .. }
        | O::StructuralScalarFieldStore { destination, .. } => vec![destination.place],
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
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
        } => structural_arguments
            .iter()
            .map(|argument| argument.place)
            .collect(),
        O::CallStructuralScalarWithDynamicArguments {
            structural_arguments,
            dynamic_arguments,
            ..
        }
        | O::CallUnitWithDynamicArguments {
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
        O::CallDynamicScalar {
            dynamic_dispatch, ..
        }
        | O::CallDynamicUnit {
            dynamic_dispatch, ..
        } => vec![
            dynamic_dispatch.initial.source.place,
            dynamic_dispatch.rebound.source.place,
        ],
        O::ByteSequenceSubslice { source, .. }
        | O::ByteSequenceRead { source, .. }
        | O::ByteSequenceLength { source, .. }
        | O::BooleanStructuralField { source, .. }
        | O::ReturnStructural { source, .. } => {
            vec![*source]
        }
        O::IntegerStructuralField { source, .. } => vec![source.place],
        _ => Vec::new(),
    };
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

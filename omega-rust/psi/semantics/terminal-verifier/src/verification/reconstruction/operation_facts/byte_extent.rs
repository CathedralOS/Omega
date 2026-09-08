//! Join a length observation to its exact validated producer or incoming view.

use semantic_vocabulary::{Proposition, ScalarTerm, StructuralPlaceKind};
use terminal_psi::{
    Operation, OperationKind, StructuralAccess, TerminalMachine, TerminalModule, Terminator,
};
use terminal_semantics::{
    StructuralEffectObservation, literal_length_equation, structural_effect_leaf_observation,
    subslice_length_equation,
};

use crate::ModuleError;

pub(super) fn length_equation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    current: &Operation,
    observation: &StructuralEffectObservation,
) -> Result<Option<Proposition>, ModuleError> {
    let StructuralEffectObservation::ByteSequenceLengthRead { source, .. } = observation else {
        return Ok(None);
    };
    let Some(place_kind) = machine
        .structural_places
        .iter()
        .find(|place| place.id == *source)
        .map(|place| place.kind)
    else {
        return Ok(None);
    };
    if matches!(place_kind, StructuralPlaceKind::ByteSequenceLiteral { .. }) {
        let Some(producer) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishByteSequenceLiteral { destination, .. }
                        if destination == *source
                )
            })
        else {
            return Ok(None);
        };
        // Literal validation independently establishes uniqueness and that
        // this exact source was established before the current length read.
        return literal_length_equation(observation, producer)
            .map_err(ModuleError::OperationSemanticSchema);
    }
    if let StructuralPlaceKind::BlockParameter { block, position } = place_kind {
        return Ok(block_length_equation(
            module, machine, current, *source, block, position,
        ));
    }
    let StructuralPlaceKind::OperationResult { producer, .. } = place_kind else {
        return Ok(None);
    };
    // Module validation has checked this exact producer/result join, source
    // type and dominance. Introduce the equation only at the later read, not
    // at the producer's bounds goal or on another control-flow path.
    let Some(operation) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)
    else {
        return Ok(None);
    };
    let Some(producer_observation) = structural_effect_leaf_observation(operation)
        .map_err(ModuleError::OperationSemanticSchema)?
    else {
        return Ok(None);
    };
    subslice_length_equation(observation, &producer_observation)
        .map_err(ModuleError::OperationSemanticSchema)
}

/// One incoming binding establishes an observation equation, not a generic
/// parameter extent axiom. Multiple incoming edges establish no equation.
fn block_length_equation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    current: &Operation,
    source: semantic_vocabulary::PlaceId,
    block: semantic_vocabulary::BlockId,
    position: u32,
) -> Option<Proposition> {
    if !machine.blocks.iter().any(|candidate| {
        candidate.id == block
            && candidate
                .operations
                .iter()
                .any(|operation| operation.id == current.id)
    }) {
        return None;
    }
    let mut incoming = Vec::new();
    for predecessor in &machine.blocks {
        match &predecessor.terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                ..
            } if *target == block => {
                incoming.push((predecessor, structural_arguments.as_slice()));
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    if edge.target == block {
                        incoming.push((predecessor, edge.structural_arguments.as_slice()));
                    }
                }
            }
            Terminator::StructuralCase { cases, .. }
                if cases.iter().any(|edge| edge.target == block) =>
            {
                return None;
            }
            _ => {}
        }
    }
    let [(predecessor, arguments)] = incoming.as_slice() else {
        return None;
    };
    let argument = arguments.get(position as usize)?;
    let destination = machine
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?
        .structural_parameters
        .get(position as usize)?;
    if destination.place != source
        || destination.access != StructuralAccess::MutableBorrow
        || argument.access != StructuralAccess::MutableBorrow
        || !argument.path.is_empty()
    {
        return None;
    }
    let producer = predecessor.operations.iter().rev().find(|operation|
        matches!(operation.kind, OperationKind::ByteSequenceLength { source } if source == argument.place))?;
    for place in [argument.place, source] {
        if !crate::validation::view_length_is_current(
            module,
            machine,
            producer.id,
            current.id,
            place,
        ) {
            return None;
        }
    }
    let result = current.result.scalar()?;
    let previous = producer.result.scalar()?;
    (result.scalar_type == previous.scalar_type).then(|| {
        Proposition::Equal(
            ScalarTerm::value(result.id, result.scalar_type),
            ScalarTerm::value(previous.id, previous.scalar_type),
        )
    })
}

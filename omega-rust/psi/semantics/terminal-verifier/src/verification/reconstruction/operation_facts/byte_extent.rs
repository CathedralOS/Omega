//! Join a length observation to its exact validated producer or incoming view.

use std::collections::BTreeSet;

use semantic_vocabulary::{PlaceId, Proposition, ScalarTerm, StructuralPlaceKind};
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

/// Reconstruct the exact descriptor binding at every arrival. The equation
/// relates two real observations, never a generic parameter extent or invariant.
fn block_length_equation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    current: &Operation,
    source: semantic_vocabulary::PlaceId,
    block: semantic_vocabulary::BlockId,
    position: u32,
) -> Option<Proposition> {
    let destination = machine
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?
        .structural_parameters
        .get(position as usize)?;
    if destination.place != source || destination.access != StructuralAccess::MutableBorrow {
        return None;
    }
    let current_position = operation_position(machine, current.id)?;
    let dominators = crate::control_graph::dominators(machine);
    let current_dominators = dominators.get(&machine.blocks[current_position.0].id)?;
    let mut candidates = Vec::new();
    for (block_position, candidate_block) in machine.blocks.iter().enumerate() {
        if !current_dominators.contains(&candidate_block.id) {
            continue;
        }
        for (operation_position, producer) in candidate_block.operations.iter().enumerate() {
            if matches!(producer.kind, OperationKind::ByteSequenceLength { .. })
                && (block_position != current_position.0 || operation_position < current_position.1)
            {
                candidates.push((block_position, operation_position, producer));
            }
        }
    }
    // Dominating blocks form a chain. Prefer the nearest observation, retaining
    // stable identity order independently of the serialized block roster.
    candidates.sort_by_key(|(block_position, operation_position, _)| {
        let block = machine.blocks[*block_position].id;
        (
            std::cmp::Reverse(dominators[&block].len()),
            block,
            std::cmp::Reverse(*operation_position),
        )
    });
    for (block_position, operation_position, producer) in candidates {
        let OperationKind::ByteSequenceLength { source: measured } = producer.kind else {
            continue;
        };
        if producer.id == current.id {
            continue;
        }
        let Some(aliases) = descriptor_origins(
            machine,
            current_position,
            source,
            (block_position, operation_position),
            measured,
        ) else {
            continue;
        };
        // Every name encountered along the transfer chain is checked. A
        // mutation through an intermediate binding cannot hide behind the
        // destination's new place identity. Checking the union is conservative.
        if aliases.into_iter().any(|place| {
            !crate::validation::view_length_is_current(
                module,
                machine,
                producer.id,
                current.id,
                place,
            )
        }) {
            continue;
        }
        let result = current.result.scalar()?;
        let previous = producer.result.scalar()?;
        if result.scalar_type == previous.scalar_type {
            return Some(Proposition::Equal(
                ScalarTerm::value(result.id, result.scalar_type),
                ScalarTerm::value(previous.id, previous.scalar_type),
            ));
        }
    }
    None
}

fn operation_position(
    machine: &TerminalMachine,
    operation: semantic_vocabulary::OperationId,
) -> Option<(usize, usize)> {
    machine
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, declaration)| {
            declaration
                .operations
                .iter()
                .position(|candidate| candidate.id == operation)
                .map(|position| (block, position))
        })
}

fn descriptor_origins(
    machine: &TerminalMachine,
    current: (usize, usize),
    source: PlaceId,
    producer: (usize, usize),
    measured: PlaceId,
) -> Option<BTreeSet<PlaceId>> {
    let mut pending = vec![(current.0, current.1, source)];
    let mut visited = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    let mut reached_producer = false;
    while let Some((block_position, position, tracked)) = pending.pop() {
        if !visited.insert((block_position, position, tracked)) {
            continue;
        }
        aliases.insert(tracked);
        let block = &machine.blocks[block_position];
        if position != 0 {
            let previous = (block_position, position - 1);
            if previous == producer {
                if tracked != measured {
                    return None;
                }
                reached_producer = true;
                continue;
            }
            let operation = &block.operations[position - 1];
            if operation
                .result
                .structural()
                .is_some_and(|result| result.place == tracked)
                || matches!(operation.kind, OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == tracked)
            {
                return None;
            }
            pending.push((block_position, position - 1, tracked));
            continue;
        }
        if block.id == machine.entry {
            return None;
        }
        let parameter_position = block
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == tracked);
        let mut incoming_count = 0;
        let mut incoming = |predecessor_position: usize,
                            arguments: &[terminal_psi::StructuralArgument]|
         -> Option<()> {
            let previous = if let Some(position) = parameter_position {
                let argument = arguments.get(position)?;
                if argument.access != StructuralAccess::MutableBorrow || !argument.path.is_empty() {
                    return None;
                }
                argument.place
            } else {
                tracked
            };
            incoming_count += 1;
            pending.push((
                predecessor_position,
                machine.blocks[predecessor_position].operations.len(),
                previous,
            ));
            Some(())
        };
        for (predecessor_position, predecessor) in machine.blocks.iter().enumerate() {
            match &predecessor.terminator {
                Terminator::Jump {
                    target,
                    structural_arguments,
                    ..
                } if *target == block.id => incoming(predecessor_position, structural_arguments)?,
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for edge in [when_true, when_false] {
                        if edge.target == block.id {
                            incoming(predecessor_position, &edge.structural_arguments)?;
                        }
                    }
                }
                Terminator::StructuralCase { cases, .. } => {
                    for edge in cases {
                        if edge.target == block.id {
                            incoming(predecessor_position, &[])?;
                        }
                    }
                }
                _ => {}
            }
        }
        if incoming_count == 0 {
            return None;
        }
    }
    reached_producer.then_some(aliases)
}

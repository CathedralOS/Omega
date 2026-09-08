//! Join a length observation to its validated immutable descriptor producer.

use semantic_vocabulary::{Proposition, StructuralPlaceKind};
use terminal_psi::{OperationKind, TerminalMachine};
use terminal_semantics::{
    StructuralEffectObservation, literal_length_equation, structural_effect_leaf_observation,
    subslice_length_equation,
};

use crate::ModuleError;

pub(super) fn length_equation(
    machine: &TerminalMachine,
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

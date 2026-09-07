//! Join a length observation to its validated immutable descriptor producer.

use semantic_vocabulary::{Proposition, StructuralPlaceKind};
use terminal_psi::TerminalMachine;
use terminal_semantics::{
    StructuralEffectObservation, structural_effect_leaf_observation, subslice_length_equation,
};

use crate::ModuleError;

pub(super) fn length_equation(
    machine: &TerminalMachine,
    observation: &StructuralEffectObservation,
) -> Result<Option<Proposition>, ModuleError> {
    let StructuralEffectObservation::ByteSequenceLengthRead { source, .. } = observation else {
        return Ok(None);
    };
    let Some(StructuralPlaceKind::OperationResult { producer, .. }) = machine
        .structural_places
        .iter()
        .find(|place| place.id == *source)
        .map(|place| place.kind)
    else {
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

use semantic_vocabulary::Proposition;
use terminal_psi::{Operation, OperationKind};

use super::StructuralEffectObservation;
use crate::OperationSemanticError;

pub fn literal_length_equation(
    observation: &StructuralEffectObservation,
    producer: &Operation,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let StructuralEffectObservation::ByteSequenceLengthRead { source, result } =
        observation
    else {
        return Ok(None);
    };
    let OperationKind::EstablishByteSequenceLiteral {
        destination, bytes, ..
    } = &producer.kind
    else {
        return Ok(None);
    };
    if source.place != *destination || !source.path.is_empty() {
        return Ok(None);
    }
    super::structural_effect_leaf_observation(producer)?;
    super::u64_extent_proposition(*result, bytes.len() as u128)
}

pub fn subslice_length_equation(
    observation: &StructuralEffectObservation,
    producer: &StructuralEffectObservation,
) -> Result<Option<Proposition>, OperationSemanticError> {
    super::subslice_extent_equation(
        observation,
        producer,
        |observation| match observation {
            StructuralEffectObservation::ByteSequenceLengthRead { source, result } => {
                Some((source, *result))
            }
            _ => None,
        },
        |producer| match producer {
            StructuralEffectObservation::ByteSequenceSubslice {
                destination,
                start,
                end,
                ..
            } => Some((*destination, *start, *end)),
            _ => None,
        },
    )
}

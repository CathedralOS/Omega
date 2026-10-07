//! Exact extent observations of established immutable byte producers.

use semantic_vocabulary::Proposition;
use terminal_psi::{Operation, OperationKind};

use super::StructuralEffectObservation;
use crate::OperationSemanticError;

/// Relate a measured literal to its exact raw octet count. The caller must
/// validate the unique establishment and its dominance before using this fact.
/// Establishing a literal alone supplies no equation about a later SSA result.
pub fn literal_length_equation(
    observation: &StructuralEffectObservation,
    producer: &Operation,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let StructuralEffectObservation::ByteSequenceLengthRead { source, result } = observation else {
        return Ok(None);
    };
    let OperationKind::EstablishByteSequenceLiteral {
        destination, bytes, ..
    } = &producer.kind
    else {
        return Ok(None);
    };
    if source != destination {
        return Ok(None);
    }
    // Reuse the leaf schema's result-shape check rather than accepting bytes
    // attached to a malformed producer operation.
    super::structural_effect_leaf_observation(producer)?;
    super::u64_extent_proposition(*result, bytes.len() as u128)
}

/// Returns the subslice's direct extent denotation at a matching length read.
/// The caller must establish the exact defining producer and its dominance;
/// the producer's two bounds remain independent required proof obligations.
/// No equation is returned for other producers or a different measured view.
pub fn subslice_length_equation(
    observation: &StructuralEffectObservation,
    producer: &StructuralEffectObservation,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let StructuralEffectObservation::ByteSequenceLengthRead { source, result } = observation else {
        return Ok(None);
    };
    let StructuralEffectObservation::ByteSequenceSubslice {
        destination,
        start,
        end,
        ..
    } = producer
    else {
        return Ok(None);
    };
    if source != destination {
        return Ok(None);
    }
    super::u64_extent_difference_proposition(*result, *end, *start)
}

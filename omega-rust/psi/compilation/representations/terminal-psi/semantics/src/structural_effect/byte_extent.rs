//! Exact extent observations of established immutable byte producers.

use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};
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
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let count = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(bytes.len() as u128))
        .map_err(OperationSemanticError::InvalidProposition)?;
    Ok(Some(Proposition::Equal(
        ScalarTerm::value(*result, ScalarType::Integer(integer_type)),
        count,
    )))
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
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let value = |identity| ScalarTerm::value(identity, ScalarType::Integer(integer_type));
    let difference = ScalarTerm::exact_integer_subtract(integer_type, value(*end), value(*start))
        .map_err(OperationSemanticError::InvalidProposition)?;
    Ok(Some(Proposition::Equal(value(*result), difference)))
}

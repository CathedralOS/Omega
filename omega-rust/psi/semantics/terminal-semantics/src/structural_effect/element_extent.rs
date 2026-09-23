//! Exact extent observations of established immutable element-view producers.

use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};

use super::StructuralEffectObservation;
use crate::OperationSemanticError;

/// Returns the subslice's direct extent denotation at a matching length read:
/// the measured element count is `end - start`. The caller must establish the
/// exact defining producer and its dominance; the producer's two bounds remain
/// independent required proof obligations. No equation is returned for other
/// producers or a different measured view.
pub fn element_subslice_length_equation(
    observation: &StructuralEffectObservation,
    producer: &StructuralEffectObservation,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let StructuralEffectObservation::ElementViewLengthRead { source, result } = observation else {
        return Ok(None);
    };
    let StructuralEffectObservation::ElementViewSubslice {
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

/// Relate a measured element view to its whole collection's declared extent:
/// an `EstablishElementView` over a `FixedArray` names the array's static
/// element count. The caller must establish the exact producer/result join,
/// source type, and dominance before using this fact.
pub fn element_establishment_length_equation(
    observation: &StructuralEffectObservation,
    producer: &StructuralEffectObservation,
    collection_length: u64,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let StructuralEffectObservation::ElementViewLengthRead { source, result } = observation else {
        return Ok(None);
    };
    let StructuralEffectObservation::ElementViewEstablished { destination, .. } = producer else {
        return Ok(None);
    };
    if source != destination {
        return Ok(None);
    }
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let count = ScalarTerm::integer(
        integer_type,
        IntegerValue::Unsigned(u128::from(collection_length)),
    )
    .map_err(OperationSemanticError::InvalidProposition)?;
    Ok(Some(Proposition::Equal(
        ScalarTerm::value(*result, ScalarType::Integer(integer_type)),
        count,
    )))
}

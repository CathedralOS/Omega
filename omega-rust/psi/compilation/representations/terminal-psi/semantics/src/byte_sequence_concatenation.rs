//! A concatenation's live extent is the mathematical sum of its operand
//! observations. Its declared capacity supplies only an independent upper bound.
use crate::{OperationSemanticError, OperationSemanticTag, StructuralEffectObservation};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};
use terminal_psi::{Operation, OperationKind};

pub fn byte_sequence_concatenation_capacity_goal(
    operation: &Operation,
    capacity: u64,
) -> Result<Proposition, OperationSemanticError> {
    let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } = &operation.kind
    else {
        return Err(OperationSemanticError::OperandShapeMismatch(
            OperationSemanticTag::EstablishByteSequenceConcatenation,
        ));
    };
    let source_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let length =
        |operand: &terminal_psi::control_flow::ByteSequenceOperand| IntegerMathTerm::MathValue {
            source_type,
            value: operand.length(),
        };
    Ok(Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(Box::new(length(left)), Box::new(length(right))),
        IntegerMathTerm::literal(IntegerValue::Unsigned(u128::from(capacity))),
    ))
}

/// The caller establishes producer identity and dominance before publishing
/// this equation at a later observation of the completed immutable result.
pub fn byte_sequence_concatenation_length_equation(
    observation: &StructuralEffectObservation,
    producer: &Operation,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let StructuralEffectObservation::ByteSequenceLengthRead { source, result } = observation else {
        return Ok(None);
    };
    let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } = &producer.kind
    else {
        return Ok(None);
    };
    let Some(produced) = producer.result.structural() else {
        return Err(OperationSemanticError::ResultShapeMismatch(
            OperationSemanticTag::EstablishByteSequenceConcatenation,
        ));
    };
    if produced.place != source.place || !source.path.is_empty() {
        return Ok(None);
    }
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let value = |identity| ScalarTerm::value(identity, ScalarType::Integer(integer_type));
    let sum =
        ScalarTerm::exact_integer_add(integer_type, value(left.length()), value(right.length()))
            .map_err(OperationSemanticError::InvalidProposition)?;
    Ok(Some(Proposition::Equal(value(*result), sum)))
}

use super::StructuralEffectObservation;
use semantic_vocabulary::{
    ByteSequenceStructuralField, Proposition, ScalarTerm, ScalarType, StructuralTypeId,
};
use terminal_psi::{StructuralTypeDeclaration, control_flow::ByteSequenceOperand};

pub fn byte_equal_equation(
    observation: &StructuralEffectObservation,
    left_root_type: StructuralTypeId,
    right_root_type: StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
) -> Option<Proposition> {
    let StructuralEffectObservation::ByteSequenceEqualEvaluated {
        left,
        right,
        result,
    } = observation
    else {
        return None;
    };
    let left = operand_field(left, left_root_type, structural_types)?;
    let right = operand_field(right, right_root_type, structural_types)?;
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    Some(Proposition::Implication {
        premise: Box::new(Proposition::Equal(
            ScalarTerm::value(*result, ScalarType::Boolean),
            ScalarTerm::Boolean(true),
        )),
        conclusion: Box::new(Proposition::ByteSequenceEqual { left, right }),
    })
}

fn operand_field(
    operand: &ByteSequenceOperand,
    root_type: StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
) -> Option<ByteSequenceStructuralField> {
    let (place, path) =
        super::byte_sequence_field_path(operand, root_type, structural_types)?;
    ByteSequenceStructuralField::new(place, path).ok()
}

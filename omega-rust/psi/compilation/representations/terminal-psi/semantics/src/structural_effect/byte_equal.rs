//! Content observations name the exact descriptor, not its backing region.
//! Two views can alias while reading as distinct byte sequences; equality
//! observes only the live extent each operand's own length witness measures.

use super::StructuralEffectObservation;
use semantic_vocabulary::{
    ByteSequenceStructuralField, Proposition, ScalarTerm, ScalarType, StructuralTypeId,
};
use terminal_psi::{StructuralTypeDeclaration, control_flow::ByteSequenceOperand};

/// Reconstruct the conditional denotation after independently validating both
/// operands' access, establishment, and current same-source length witnesses.
/// A `result == true` premise concludes content equality of the two exact
/// descriptor paths; the proposition vocabulary names only field-resolved
/// operands, so an operand that stays a whole descriptor contributes no
/// axiom. Mutation invalidation belongs to the caller's current-storage fact
/// set.
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

/// Resolve one operand's exact byte-sequence projection. Field operands
/// append their field identity to the canonical source path; a whole
/// operand names no field and so cannot form the proposition.
fn operand_field(
    operand: &ByteSequenceOperand,
    root_type: StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
) -> Option<ByteSequenceStructuralField> {
    let (place, path) = super::byte_sequence_field_path(operand, root_type, structural_types)?;
    ByteSequenceStructuralField::new(place, path).ok()
}

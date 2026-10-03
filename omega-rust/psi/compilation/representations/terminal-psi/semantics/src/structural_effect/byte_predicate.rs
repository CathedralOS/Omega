//! Content observations name the exact descriptor, not its backing region.
//! A subslice and its parent can alias while having different UTF-8 verdicts.

use super::StructuralEffectObservation;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, Proposition, ScalarTerm, ScalarType, StructuralTypeId,
};
use terminal_psi::{
    StructuralTypeDeclaration, StructuralTypeShape, control_flow::ByteSequenceOperand,
};

/// Reconstruct the Boolean denotation after independently validating the
/// operand's access, establishment, and current same-source length witness.
/// Resolving field identities requires the current type catalog; operation
/// schema matching alone has no authority to turn field spellings into IDs.
/// Mutation invalidation belongs to the caller's current-storage fact set.
pub fn byte_predicate_equation(
    observation: &StructuralEffectObservation,
    root_type: StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
) -> Option<Proposition> {
    let StructuralEffectObservation::ByteSequencePredicateEvaluated {
        operand,
        predicate,
        result,
    } = observation
    else {
        return None;
    };
    let (root, path) = match operand {
        ByteSequenceOperand::Whole { source, .. } => {
            let declaration = structural_types.iter().find(|row| row.id == root_type)?;
            if !matches!(declaration.shape, StructuralTypeShape::ByteSequence(_)) {
                return None;
            }
            (*source, Vec::new())
        }
        ByteSequenceOperand::Field { source, field, .. } => {
            let (mut path, parent) = crate::canonical_static_projection(
                structural_types.iter(),
                root_type,
                &source.path,
            )?;
            let fields = match &parent.shape {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => return None,
            };
            let selected = fields
                .iter()
                .find(|row| row.id == *field && !row.relevance.is_erased())?;
            selected.field_type.byte_sequence_carrier()?;
            path.push(CanonicalStructuralPathSegment::Field(*field));
            (source.place, path)
        }
    };
    Some(Proposition::Equal(
        ScalarTerm::value(*result, ScalarType::Boolean),
        ScalarTerm::ByteSequencePredicate {
            predicate: *predicate,
            root,
            path,
        },
    ))
}

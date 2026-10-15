//! Content observations name the exact descriptor, not its backing region.
//! A subslice and its parent can alias while having different UTF-8 verdicts.

use super::StructuralEffectObservation;
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType, StructuralTypeId};
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
            let (path, declaration) = crate::canonical_static_projection(
                structural_types.iter(),
                root_type,
                &source.path,
            )?;
            if !matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
                && !(matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
                    && crate::fixed_byte_extent(structural_types.iter(), declaration.id).is_some())
            {
                return None;
            }
            (source.place, path)
        }
        ByteSequenceOperand::Field { .. } => {
            super::byte_sequence_field_path(operand, root_type, structural_types)?
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

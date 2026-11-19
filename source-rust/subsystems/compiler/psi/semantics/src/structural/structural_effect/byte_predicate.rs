use super::StructuralEffectObservation;
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType, StructuralTypeId};
use terminal_psi::{
    StructuralTypeDeclaration, StructuralTypeShape, control_flow::ByteSequenceOperand,
};

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
                    && crate::fixed_byte_extent(structural_types.iter(), declaration.id)
                        .is_some())
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
